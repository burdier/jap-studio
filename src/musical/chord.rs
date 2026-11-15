//! Secuencias de acordes en ticks: plan editable fuera del callback, reutilizable por el secuenciador.
use super::{
    midi::{MidiMessage, MidiTarget, RoutedMidi},
    time::Tick,
};

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
}
