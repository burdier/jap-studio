//! Componente `MidiClip` y grabadora/búfer de captura MIDI (`MidiCapture`).
//! Sin GPUI, CPAL ni motores de audio: mapea lo que se toca desde el controlador
//! a `NoteEvent` en ticks (`Pattern`) para insertarlo en un carril y reproducirlo.
use super::{
    midi::MidiMessage,
    model::{InstrumentId, NoteEvent, Pattern},
    time::{PPQ, Tick},
};
use std::collections::HashMap;

/// Componente clip MIDI ubicado en un carril del Arrangement.
/// Las funciones avanzadas de edición (piano roll, split, loop interno) llegarán en
/// etapas posteriores; esta primera etapa almacena el `Pattern` mapeado y su posición.
#[derive(Debug, Clone)]
pub struct MidiClip {
    pub name: String,
    pub track: usize,
    pub start_beat: f64,
    pub pattern: Pattern,
}

impl MidiClip {
    pub fn new(name: impl Into<String>, track: usize, start_beat: f64, pattern: Pattern) -> Self {
        Self {
            name: name.into(),
            track,
            start_beat: start_beat.max(0.0),
            pattern,
        }
    }

    pub fn duration_beats(&self) -> f64 {
        self.pattern.length().0 as f64 / PPQ as f64
    }

    pub fn end_beat(&self) -> f64 {
        self.start_beat + self.duration_beats()
    }

    pub fn notes(&self) -> &[NoteEvent] {
        self.pattern.notes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapturedNote {
    pub start: Tick,
    pub duration: Tick,
    pub pitch: u8,
    pub velocity: u8,
}

/// Grabadora y búfer retrospectivo de salida MIDI.
/// - Siempre conserva las últimas notas tocadas (para «Recuperar MIDI» estilo Capture MIDI
///   aunque no hubieras pulsado Record antes de tocar).
/// - También admite modo `recording` explícito (`R` / botón `●` en transporte) que limpia
///   la toma al armar y construye el `MidiClip` al detener.
#[derive(Debug, Default)]
pub struct MidiCapture {
    recording: bool,
    origin_ns: Option<u64>,
    last_event_ns: Option<u64>,
    active: HashMap<u8, (Tick, u8)>,
    notes: Vec<CapturedNote>,
}

/// Si el músico deja de tocar más de 6 segundos cuando no está grabando explícitamente,
/// la siguiente frase inicia una captura retrospectiva limpia.
const RETROSPECTIVE_GAP_NS: u64 = 6_000_000_000;
const MIN_DURATION_TICKS: i64 = 60;

impl MidiCapture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn note_count(&self) -> usize {
        self.notes.len() + self.active.len()
    }

    pub fn has_notes(&self) -> bool {
        !self.notes.is_empty() || !self.active.is_empty()
    }

    pub fn start_recording(&mut self) {
        self.clear();
        self.recording = true;
    }

    pub fn stop_recording(&mut self, now_ns: u64, bpm: f64) {
        self.flush_active(now_ns, bpm);
        self.recording = false;
    }

    pub fn clear(&mut self) {
        self.origin_ns = None;
        self.last_event_ns = None;
        self.active.clear();
        self.notes.clear();
    }

    /// Registra un mensaje MIDI con marca de tiempo en nanosegundos monotónicos.
    pub fn record(&mut self, message: MidiMessage, now_ns: u64, bpm: f64) {
        match message {
            MidiMessage::NoteOn { note, velocity } => {
                if !self.recording
                    && let Some(last) = self.last_event_ns
                    && self.active.is_empty()
                    && now_ns.saturating_sub(last) > RETROSPECTIVE_GAP_NS
                {
                    self.clear();
                }
                let origin = *self.origin_ns.get_or_insert(now_ns);
                let tick = ns_to_tick(now_ns.saturating_sub(origin), bpm);
                self.active.insert(note, (tick, velocity.max(1)));
                self.last_event_ns = Some(now_ns);
            }
            MidiMessage::NoteOff { note } => {
                if let Some((start, velocity)) = self.active.remove(&note) {
                    let origin = self.origin_ns.unwrap_or(now_ns);
                    let end = ns_to_tick(now_ns.saturating_sub(origin), bpm);
                    let duration = Tick((end.0 - start.0).max(MIN_DURATION_TICKS));
                    self.notes.push(CapturedNote {
                        start,
                        duration,
                        pitch: note,
                        velocity,
                    });
                    self.last_event_ns = Some(now_ns);
                }
            }
            MidiMessage::AllNotesOff => {
                self.flush_active(now_ns, bpm);
            }
            MidiMessage::ControlChange { .. }
            | MidiMessage::PitchBend { .. }
            | MidiMessage::ProgramChange { .. } => {}
        }
    }

    fn flush_active(&mut self, now_ns: u64, bpm: f64) {
        let origin = self.origin_ns.unwrap_or(now_ns);
        let end = ns_to_tick(now_ns.saturating_sub(origin), bpm);
        let mut active: Vec<(u8, (Tick, u8))> = self.active.drain().collect();
        active.sort_by_key(|&(pitch, _)| pitch);
        for (pitch, (start, velocity)) in active {
            let duration = Tick((end.0 - start.0).max(MIN_DURATION_TICKS));
            self.notes.push(CapturedNote {
                start,
                duration,
                pitch,
                velocity,
            });
        }
    }

    /// Convierte lo tocado en un `MidiClip` listo para insertarse en `track` en `start_beat`.
    /// Normaliza el inicio a la primera nota y redondea la longitud al compás (múltiplo de 4 negras).
    pub fn to_clip(
        &mut self,
        name: impl Into<String>,
        track: usize,
        start_beat: f64,
        instrument: InstrumentId,
        now_ns: u64,
        bpm: f64,
    ) -> Option<MidiClip> {
        self.flush_active(now_ns, bpm);
        if self.notes.is_empty() {
            return None;
        }
        let min_start = self.notes.iter().map(|n| n.start.0).min().unwrap_or(0);
        let max_end = self
            .notes
            .iter()
            .map(|n| n.start.0 - min_start + n.duration.0)
            .max()
            .unwrap_or(PPQ);
        let bar_ticks = PPQ * 4;
        let length_ticks = ((max_end + bar_ticks - 1) / bar_ticks).max(1) * bar_ticks;
        let mut pattern = Pattern::new(Tick(length_ticks)).ok()?;
        let mut sorted = self.notes.clone();
        sorted.sort_by_key(|n| (n.start.0, n.pitch));
        for note in sorted {
            let start = Tick((note.start.0 - min_start).max(0));
            let duration = Tick(note.duration.0.min(length_ticks - start.0).max(1));
            let _ = pattern.add_note(instrument, start, duration, note.pitch, note.velocity);
        }
        Some(MidiClip::new(name, track, start_beat, pattern))
    }
}

fn ns_to_tick(elapsed_ns: u64, bpm: f64) -> Tick {
    let seconds = elapsed_ns as f64 / 1_000_000_000.0;
    let beats = seconds * (bpm / 60.0);
    Tick((beats * PPQ as f64).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musical::time::DEFAULT_BPM;

    #[test]
    fn captured_notes_build_normalized_midi_clip_with_bar_aligned_length() {
        let mut capture = MidiCapture::new();
        let inst = InstrumentId(1);
        // 120 BPM -> 1 negra = 0.5 s = 500_000_000 ns = 960 ticks
        capture.record(
            MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
            1_000_000_000,
            DEFAULT_BPM,
        );
        capture.record(
            MidiMessage::NoteOff { note: 60 },
            1_500_000_000,
            DEFAULT_BPM,
        );
        capture.record(
            MidiMessage::NoteOn {
                note: 64,
                velocity: 110,
            },
            1_500_000_000,
            DEFAULT_BPM,
        );
        capture.record(
            MidiMessage::NoteOff { note: 64 },
            2_000_000_000,
            DEFAULT_BPM,
        );

        let clip = capture
            .to_clip("Toma MIDI", 2, 4.0, inst, 2_000_000_000, DEFAULT_BPM)
            .expect("debe crear el clip");
        assert_eq!(clip.track, 2);
        assert_eq!(clip.start_beat, 4.0);
        assert_eq!(clip.duration_beats(), 4.0);
        assert_eq!(clip.notes().len(), 2);
        assert_eq!(clip.notes()[0].pitch, 60);
        assert_eq!(clip.notes()[0].start, Tick(0));
        assert_eq!(clip.notes()[0].duration, Tick(PPQ));
        assert_eq!(clip.notes()[1].pitch, 64);
        assert_eq!(clip.notes()[1].start, Tick(PPQ));
    }

    #[test]
    fn retrospective_capture_resets_after_long_silence_when_not_recording() {
        let mut capture = MidiCapture::new();
        capture.record(
            MidiMessage::NoteOn {
                note: 60,
                velocity: 90,
            },
            0,
            DEFAULT_BPM,
        );
        capture.record(MidiMessage::NoteOff { note: 60 }, 200_000_000, DEFAULT_BPM);
        assert_eq!(capture.note_count(), 1);

        // Más de 6 segundos después sin estar en modo grabación explícito:
        capture.record(
            MidiMessage::NoteOn {
                note: 72,
                velocity: 95,
            },
            10_000_000_000,
            DEFAULT_BPM,
        );
        capture.record(
            MidiMessage::NoteOff { note: 72 },
            10_300_000_000,
            DEFAULT_BPM,
        );
        let clip = capture
            .to_clip(
                "Nueva frase",
                0,
                0.0,
                InstrumentId(1),
                10_300_000_000,
                DEFAULT_BPM,
            )
            .unwrap();
        assert_eq!(clip.notes().len(), 1);
        assert_eq!(clip.notes()[0].pitch, 72);
    }
}
