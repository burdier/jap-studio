//! Secuencias de acordes en ticks: plan editable fuera del callback, reutilizable por el secuenciador.
use super::{
    midi::{MidiInputEvent, MidiMessage, MidiTarget, RoutedMidi},
    time::Tick,
};

const MINOR_SCALE_STEPS: [u8; 7] = [0, 2, 3, 5, 7, 8, 10];
const MAJOR_SCALE_STEPS: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];

/// Mapea una nota MIDI entrante al grado (`0..=6`) de la escala activa y al desplazamiento
/// de octava respecto a la octava central (`C4 = 60`). Si la nota no pertenece a la escala,
/// se ajusta al grado inmediatamente inferior (*snap to scale*).
pub fn scale_degree_for_note(note: u8, scale_root: u8, scale_minor: bool) -> (u8, i8) {
    let root_pc = scale_root % 12;
    let steps = if scale_minor {
        &MINOR_SCALE_STEPS
    } else {
        &MAJOR_SCALE_STEPS
    };
    let rel = (note + 12 - root_pc) % 12;
    let mut degree = 0u8;
    for (idx, &step) in steps.iter().enumerate() {
        if step <= rel {
            degree = idx as u8;
        }
    }
    let degree_pc = (root_pc + steps[usize::from(degree)]) % 12;
    let base_pitch = 60 + i16::from(degree_pc);
    let snapped_pitch = i16::from(note) - i16::from(rel - steps[usize::from(degree)]);
    let octave_shift = ((snapped_pitch - base_pitch) as f32 / 12.0)
        .round()
        .clamp(-4.0, 4.0) as i8;
    (degree, octave_shift)
}

/// Expande notas individuales MIDI entrantes a acordes completos con conteo de referencias
/// para soportar *legato* sin cortar notas compartidas ni dejar voces colgadas.
#[derive(Default)]
pub struct MidiChordExpander {
    active: Vec<(u8, u8, Vec<u8>)>,
}

impl MidiChordExpander {
    pub fn expand_note_on(
        &mut self,
        channel: u8,
        trigger_note: u8,
        velocity: u8,
        chord_pitches: &[u8],
    ) -> Vec<MidiInputEvent> {
        let mut out = self.expand_note_off(channel, trigger_note);
        let mut unique = Vec::with_capacity(chord_pitches.len());
        for &pitch in chord_pitches {
            if pitch <= 127 && !unique.contains(&pitch) {
                unique.push(pitch);
                out.push(MidiInputEvent {
                    channel,
                    message: MidiMessage::NoteOn {
                        note: pitch,
                        velocity,
                    },
                });
            }
        }
        if !unique.is_empty() {
            self.active.push((channel, trigger_note, unique));
        }
        out
    }

    pub fn expand_note_off(&mut self, channel: u8, trigger_note: u8) -> Vec<MidiInputEvent> {
        let Some(pos) = self
            .active
            .iter()
            .position(|&(ch, note, _)| ch == channel && note == trigger_note)
        else {
            return Vec::new();
        };
        let (_, _, released) = self.active.swap_remove(pos);
        let mut out = Vec::new();
        for note in released {
            let still_held = self
                .active
                .iter()
                .any(|(ch, _, notes)| *ch == channel && notes.contains(&note));
            if !still_held {
                out.push(MidiInputEvent {
                    channel,
                    message: MidiMessage::NoteOff { note },
                });
            }
        }
        out
    }

    pub fn release_all(&mut self) -> Vec<MidiInputEvent> {
        let mut out = Vec::new();
        for (channel, _, notes) in self.active.drain(..) {
            for note in notes {
                if !out.iter().any(|ev: &MidiInputEvent| {
                    ev.channel == channel && ev.message == MidiMessage::NoteOff { note }
                }) {
                    out.push(MidiInputEvent {
                        channel,
                        message: MidiMessage::NoteOff { note },
                    });
                }
            }
        }
        out
    }

    pub fn is_active(&self) -> bool {
        !self.active.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordStep {
    pub pitches: Vec<u8>,
    pub strum_ticks: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlannedNote {
    pub start: Tick,
    pub duration: Tick,
    pub pitch: u8,
}

/// Los huecos son silencios. `unit` es un compás o un tiempo del compás actual.
pub fn plan_sequence(
    slots: &[Option<ChordStep>],
    origin: Tick,
    unit: Tick,
    clip_end: Tick,
) -> Result<Vec<PlannedNote>, String> {
    if origin.0 < 0 || unit.0 <= 0 || clip_end.0 <= 0 {
        return Err("Posición o unidad musical inválida".into());
    }
    let mut notes = Vec::new();
    for (index, slot) in slots.iter().enumerate() {
        let Some(slot) = slot else { continue };
        if slot.pitches.is_empty() || slot.pitches.iter().any(|&pitch| pitch > 127) {
            return Err("El acorde no contiene voces MIDI válidas".into());
        }
        let offset = (index as i64)
            .checked_mul(unit.0)
            .ok_or("La secuencia es demasiado larga")?;
        let start = origin
            .0
            .checked_add(offset)
            .ok_or("Posición MIDI agotada")?;
        let end = start.checked_add(unit.0).ok_or("Posición MIDI agotada")?;
        if end > clip_end.0 {
            return Err("La secuencia no cabe en el clip MIDI desde el cursor".into());
        }
        let mut pitches = slot.pitches.clone();
        pitches.sort_unstable();
        pitches.dedup();
        for (voice, pitch) in pitches.into_iter().enumerate() {
            let delay = slot
                .strum_ticks
                .max(0)
                .saturating_mul(voice as i64)
                .min(unit.0 - 1);
            notes.push(PlannedNote {
                start: Tick(start + delay),
                duration: Tick(unit.0 - delay),
                pitch,
            });
        }
    }
    Ok(notes)
}

/// Puente de audición: el mismo par NoteOn/NoteOff puede enviarse desde un pad
/// o por el secuenciador cuando su transporte alcance los ticks del plan.
#[derive(Default)]
pub struct ChordAudition {
    held: Vec<(MidiTarget, u8)>,
}

impl ChordAudition {
    pub fn press(&mut self, target: MidiTarget, pitches: &[u8]) -> Vec<RoutedMidi> {
        let mut events = self.release();
        for &pitch in pitches {
            if pitch <= 127 && !self.held.contains(&(target, pitch)) {
                self.held.push((target, pitch));
                events.push(RoutedMidi {
                    target,
                    message: MidiMessage::NoteOn {
                        note: pitch,
                        velocity: 100,
                    },
                });
            }
        }
        events
    }

    pub fn release(&mut self) -> Vec<RoutedMidi> {
        self.held
            .drain(..)
            .map(|(target, note)| RoutedMidi {
                target,
                message: MidiMessage::NoteOff { note },
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_respects_silent_slots_strum_and_clip_boundary() {
        let slots = [
            Some(ChordStep {
                pitches: vec![67, 60, 64],
                strum_ticks: 12,
            }),
            None,
            Some(ChordStep {
                pitches: vec![71],
                strum_ticks: 0,
            }),
        ];
        let notes = plan_sequence(&slots, Tick(960), Tick(960), Tick(3840)).unwrap();
        assert_eq!(
            notes
                .iter()
                .map(|n| (n.start.0, n.duration.0, n.pitch))
                .collect::<Vec<_>>(),
            vec![
                (960, 960, 60),
                (972, 948, 64),
                (984, 936, 67),
                (2880, 960, 71)
            ]
        );
        assert!(plan_sequence(&slots, Tick(960), Tick(960), Tick(3839)).is_err());
    }

    #[test]
    fn bar_and_beat_spacing_follow_the_meter_instead_of_four_four() {
        let meter = super::super::time::Meter::new(6, 8).unwrap();
        let chord = Some(ChordStep {
            pitches: vec![60],
            strum_ticks: 0,
        });
        let slots = [chord.clone(), chord];
        let bars = plan_sequence(&slots, Tick(0), Tick(meter.ticks_per_bar()), Tick(5760)).unwrap();
        let beats =
            plan_sequence(&slots, Tick(0), Tick(meter.ticks_per_beat()), Tick(5760)).unwrap();
        assert_eq!([bars[0].start.0, bars[1].start.0], [0, 2880]);
        assert_eq!([beats[0].start.0, beats[1].start.0], [0, 480]);
    }

    #[test]
    fn audition_releases_the_original_target_before_replacing_it() {
        let a = MidiTarget::Instrument(super::super::model::InstrumentId(1));
        let b = MidiTarget::Instrument(super::super::model::InstrumentId(2));
        let mut audition = ChordAudition::default();
        assert_eq!(audition.press(a, &[60, 64]).len(), 2);
        let events = audition.press(b, &[67]);
        assert_eq!(events[0].target, a);
        assert_eq!(events[1].target, a);
        assert_eq!(events[2].target, b);
        assert_eq!(audition.release().len(), 1);
        assert!(audition.release().is_empty());
    }

    #[test]
    fn scale_degree_mapping_and_legato_expander_preserve_shared_notes() {
        // En Do mayor: C4 -> grado 0 (oct 0), F4 -> grado 3 (oct 0), C#4 -> snap a grado 0.
        assert_eq!(scale_degree_for_note(60, 0, false), (0, 0));
        assert_eq!(scale_degree_for_note(65, 0, false), (3, 0));
        assert_eq!(scale_degree_for_note(61, 0, false), (0, 0));
        // En La menor (root = 9): A4 (69) -> grado 0 (i), C5 (72) -> grado 2 (III).
        assert_eq!(scale_degree_for_note(69, 9, true), (0, 0));
        assert_eq!(scale_degree_for_note(72, 9, true).0, 2);

        let mut exp = MidiChordExpander::default();
        // Pulsa Cmaj (60, 64, 67) y luego Am (57, 60, 64) en legato: comparten 60 y 64.
        let on1 = exp.expand_note_on(0, 60, 100, &[60, 64, 67]);
        assert_eq!(on1.len(), 3);
        let on2 = exp.expand_note_on(0, 57, 100, &[57, 60, 64]);
        assert_eq!(on2.len(), 3);
        // Al soltar 60, solo debe apagar 67 (60 y 64 siguen sostenidas por 57).
        let off1 = exp.expand_note_off(0, 60);
        assert_eq!(
            off1,
            vec![MidiInputEvent {
                channel: 0,
                message: MidiMessage::NoteOff { note: 67 }
            }]
        );
        // Al soltar 57, apaga las 3 restantes (57, 60, 64).
        let off2 = exp.expand_note_off(0, 57);
        assert_eq!(off2.len(), 3);
        assert!(!exp.is_active());
    }
}
