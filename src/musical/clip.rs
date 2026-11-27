//! Componente `MidiClip` y grabadora/búfer de captura MIDI (`MidiCapture`).
//! Sin GPUI, CPAL ni motores de audio: mapea lo que se toca desde el controlador
//! a `NoteEvent` en ticks (`Pattern`) para insertarlo en un carril y reproducirlo.
use super::{
    drums::{DrumClipTarget, DrumSession, piece_pattern},
    midi::MidiMessage,
    model::{InstrumentId, NoteEvent, Pattern},
    time::{Meter, PPQ, Tick},
    track::TrackId,
};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

/// Identidad de instancia durante la sesión; clonar para dibujar conserva el ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClipId(u64);
static NEXT_CLIP_ID: AtomicU64 = AtomicU64::new(1);

impl ClipId {
    pub fn fresh() -> Self {
        Self(NEXT_CLIP_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn value(self) -> u64 {
        self.0
    }

    pub fn next_id() -> u64 {
        NEXT_CLIP_ID.load(Ordering::Relaxed)
    }

    pub fn reserve_next_id(next: u64) -> Result<(), String> {
        if next == 0 || next > i64::MAX as u64 {
            return Err("Siguiente ID de clip inválido".into());
        }
        NEXT_CLIP_ID.fetch_max(next, Ordering::Relaxed);
        Ok(())
    }

    /// Recupera una identidad persistida sin permitir que futuras inserciones la reutilicen.
    pub fn restore(value: u64) -> Result<Self, String> {
        if value == 0 || value >= i64::MAX as u64 {
            return Err("ID de clip inválido".into());
        }
        NEXT_CLIP_ID.fetch_max(value + 1, Ordering::Relaxed);
        Ok(Self(value))
    }
}

/// Propiedades individuales por clip (estilo Media Item de REAPER).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipProps {
    pub muted: bool,
    pub locked: bool,
    pub looped: bool,
    pub gain_db: f32,
    pub pitch_semitones: i8,
    pub color_index: Option<u8>,
}

impl Default for ClipProps {
    fn default() -> Self {
        Self {
            muted: false,
            locked: false,
            looped: false,
            gain_db: 0.0,
            pitch_semitones: 0,
            color_index: None,
        }
    }
}

impl ClipProps {
    pub fn gain_linear(self) -> f32 {
        if self.muted {
            0.0
        } else {
            10.0_f32.powf(self.gain_db / 20.0)
        }
    }
}

/// Componente clip MIDI ubicado en un carril del Arrangement.
/// Las funciones avanzadas de edición (piano roll, split, loop interno) llegarán en
/// etapas posteriores; esta primera etapa almacena el `Pattern` mapeado y su posición.
#[derive(Debug, Clone, PartialEq)]
pub struct MidiClip {
    pub id: ClipId,
    pub name: String,
    pub track: usize,
    /// Destino musical explícito; el carril visual sigue siendo `track`.
    /// `None` usa el instrumento del carril o el Hilo de su propia cabecera.
    pub hilo_target: Option<TrackId>,
    /// La posición y los recortes son de esta ocurrencia, identificada por `id`.
    pub drum_target: Option<DrumClipTarget>,
    pub start_beat: f64,
    pub pattern: Pattern,
    /// Canal de salida del clip, numerado 1–16.
    pub channel: u8,
    pub props: ClipProps,
    pub muted_notes: HashSet<super::model::NoteId>,
    /// Recorte no destructivo: beats del patrón omitidos al inicio.
    pub trim_start: f64,
    /// Recorte no destructivo: beats visibles; `None` = hasta el final del patrón.
    pub trim_len: Option<f64>,
}

impl MidiClip {
    pub fn new(name: impl Into<String>, track: usize, start_beat: f64, pattern: Pattern) -> Self {
        Self {
            id: ClipId::fresh(),
            name: name.into(),
            track,
            hilo_target: None,
            drum_target: None,
            start_beat: start_beat.max(0.0),
            pattern,
            channel: 1,
            props: ClipProps::default(),
            muted_notes: HashSet::new(),
            trim_start: 0.0,
            trim_len: None,
        }
    }

    /// Copia editable con identidad nueva; no confundir con un snapshot de render.
    pub fn duplicate(&self) -> Self {
        let mut copy = self.clone();
        copy.id = ClipId::fresh();
        copy
    }

    /// Un clip de la cabecera usa ese Hilo por defecto; el destino explícito
    /// permite enviar clips desde cualquier otro carril sin moverlos.
    pub fn targets_hilo(&self, group_index: usize, group: TrackId) -> bool {
        self.drum_target.is_none()
            && (self.hilo_target == Some(group)
                || (self.hilo_target.is_none() && self.track == group_index))
    }

    pub fn refresh_drum_link(&mut self, session: &DrumSession) -> Result<(), String> {
        if let Some(target) = self.drum_target
            && let Some(source) = target.resolve(session)?
        {
            let swung = source.swung_notes()?;
            self.pattern = piece_pattern(&swung, target.piece)?;
        }
        Ok(())
    }

    /// Confirma un cambio de notas con los mismos IDs y desvincula solo este clip.
    pub fn replace_pattern(&mut self, mut pattern: Pattern) {
        if let Some(target) = self.drum_target {
            for note in &mut pattern.notes {
                note.instrument = target.piece;
            }
        }
        if self.pattern != pattern {
            self.detach_drum_link();
        }
        self.pattern = pattern;
    }

    /// El enlace requiere comparar el resultado para conservarlo cuando no hubo cambios.
    pub fn edit_pattern<R>(&mut self, edit: impl FnOnce(&mut Pattern) -> R) -> R {
        if self.drum_target.is_some() {
            let mut pattern = self.pattern.clone();
            let result = edit(&mut pattern);
            self.replace_pattern(pattern);
            result
        } else {
            edit(&mut self.pattern)
        }
    }

    pub fn detach_drum_link(&mut self) {
        if let Some(target) = &mut self.drum_target {
            target.linked = false;
        }
    }

    /// Longitud completa del patrón, sin recorte.
    pub fn pattern_beats(&self) -> f64 {
        self.pattern.length().0 as f64 / PPQ as f64
    }

    /// Longitud de un ciclo sin bucle (patrón menos recorte inicial).
    pub fn cycle_beats(&self) -> f64 {
        (self.pattern_beats() - self.trim_start).max(0.0)
    }

    /// Longitud visible en el Arrangement (patrón recortado o repetido en bucle).
    pub fn duration_beats(&self) -> f64 {
        let available = self.cycle_beats();
        if self.props.looped {
            self.trim_len.unwrap_or(available).max(0.0)
        } else {
            self.trim_len.map_or(available, |len| len.min(available))
        }
    }

    /// Patrón a reproducir: ventana visible (y repetida en ciclos si `props.looped`), sin modificar el original.
    pub fn playback_pattern(&self) -> Pattern {
        let needs_loop = self.props.looped && self.duration_beats() > self.cycle_beats() + 1e-6;
        if !needs_loop
            && self.trim_start == 0.0
            && self.trim_len.is_none()
            && self.muted_notes.is_empty()
        {
            return self.pattern.clone();
        }
        let window_start = (self.trim_start * PPQ as f64).round() as i64;
        let window_len = (self.duration_beats() * PPQ as f64).round().max(1.0) as i64;
        let cycle_len = (self.cycle_beats() * PPQ as f64).round().max(1.0) as i64;
        let Ok(mut trimmed) = Pattern::new(Tick(window_len)) else {
            return self.pattern.clone();
        };
        let cycles = if self.props.looped {
            (window_len + cycle_len - 1) / cycle_len
        } else {
            1
        };
        for cycle in 0..cycles {
            let cycle_offset = cycle * cycle_len;
            let cycle_end_in_window = (cycle_offset + cycle_len).min(window_len);
            let cycle_span = cycle_end_in_window - cycle_offset;
            if cycle_span <= 0 {
                break;
            }
            for note in self.pattern.notes() {
                if self.muted_notes.contains(&note.id) {
                    continue;
                }
                let start = note.start.0.max(window_start);
                let end = (note.start.0 + note.duration.0).min(window_start + cycle_span);
                if end > start {
                    let mut note = note.clone();
                    note.pitch_curve =
                        note.pitch_curve
                            .window(start - note.start.0, end - start, note.duration.0);
                    let _ = trimmed.add_note_copy(
                        &note,
                        Tick(cycle_offset + (start - window_start)),
                        Tick(end - start),
                        note.instrument,
                    );
                }
            }
        }
        trimmed.automation = self
            .pattern
            .controls()
            .window(Tick(window_start), Tick(window_len.min(cycle_len)));
        trimmed
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
    pub release_velocity: u8,
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
    capture_bpm: Option<f64>,
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
        self.flush_active(now_ns, self.capture_bpm.unwrap_or(bpm));
        self.recording = false;
    }

    pub fn clear(&mut self) {
        self.origin_ns = None;
        self.capture_bpm = None;
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
                let capture_bpm = *self.capture_bpm.get_or_insert(bpm);
                let tick = ns_to_tick(now_ns.saturating_sub(origin), capture_bpm);
                self.active.insert(note, (tick, velocity.max(1)));
                self.last_event_ns = Some(now_ns);
            }
            MidiMessage::NoteOff { note } => {
                if let Some((start, velocity)) = self.active.remove(&note) {
                    let origin = self.origin_ns.unwrap_or(now_ns);
                    let end = ns_to_tick(
                        now_ns.saturating_sub(origin),
                        self.capture_bpm.unwrap_or(bpm),
                    );
                    let duration = Tick((end.0 - start.0).max(MIN_DURATION_TICKS));
                    self.notes.push(CapturedNote {
                        start,
                        duration,
                        pitch: note,
                        velocity,
                        release_velocity: 64,
                    });
                    self.last_event_ns = Some(now_ns);
                }
            }
            MidiMessage::AllNotesOff => {
                self.flush_active(now_ns, self.capture_bpm.unwrap_or(bpm));
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
                release_velocity: 64,
            });
        }
    }

    /// Convierte lo tocado en un `MidiClip` listo para insertarse en `track` en `start_beat`.
    /// Compatibilidad para consumidores que todavía usan el compás inicial 4/4.
    pub fn to_clip(
        &mut self,
        name: impl Into<String>,
        track: usize,
        start_beat: f64,
        instrument: InstrumentId,
        now_ns: u64,
        bpm: f64,
    ) -> Option<MidiClip> {
        self.to_clip_with_meter(
            name,
            track,
            start_beat,
            instrument,
            now_ns,
            bpm,
            Meter::default(),
        )
    }

    /// Devuelve las notas capturadas con tiempos normalizados desde `Tick(0)`,
    /// preservando el ritmo relativo, duraciones y velocidades de acordes o melodías.
    pub fn normalized_notes(&mut self, now_ns: u64, bpm: f64) -> Vec<CapturedNote> {
        self.flush_active(now_ns, self.capture_bpm.unwrap_or(bpm));
        if self.notes.is_empty() {
            return Vec::new();
        }
        let min_start = self.notes.iter().map(|n| n.start.0).min().unwrap_or(0);
        let mut sorted = self.notes.clone();
        sorted.sort_by_key(|n| (n.start.0, n.pitch));
        for note in &mut sorted {
            note.start = Tick((note.start.0 - min_start).max(0));
            note.duration = Tick(note.duration.0.max(1));
        }
        sorted
    }

    /// Normaliza el inicio a la primera nota y redondea la longitud al compás vigente.
    pub fn to_clip_with_meter(
        &mut self,
        name: impl Into<String>,
        track: usize,
        start_beat: f64,
        instrument: InstrumentId,
        now_ns: u64,
        bpm: f64,
        meter: Meter,
    ) -> Option<MidiClip> {
        self.flush_active(now_ns, self.capture_bpm.unwrap_or(bpm));
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
        let bar_ticks = meter.ticks_per_bar();
        let length_ticks = ((max_end + bar_ticks - 1) / bar_ticks).max(1) * bar_ticks;
        let mut pattern = Pattern::new(Tick(length_ticks)).ok()?;
        let mut sorted = self.notes.clone();
        sorted.sort_by_key(|n| (n.start.0, n.pitch));
        for note in sorted {
            let start = Tick((note.start.0 - min_start).max(0));
            let duration = Tick(note.duration.0.min(length_ticks - start.0).max(1));
            if let Ok(id) = pattern.add_note(instrument, start, duration, note.pitch, note.velocity)
            {
                let _ = pattern.set_note_property(
                    id,
                    super::note_expression::NoteProperty::ReleaseVelocity,
                    i16::from(note.release_velocity),
                );
            }
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
    fn normalized_notes_preserves_chords_and_relative_timing() {
        let mut capture = MidiCapture::new();
        let bpm = DEFAULT_BPM;
        let beat_ns = 500_000_000_u64; // 120 BPM => 0.5s por negra
        // Acorde 1: C4-E4-G4 en t = 1s..1.5s
        for p in [60, 64, 67] {
            capture.record(
                MidiMessage::NoteOn {
                    note: p,
                    velocity: 96,
                },
                2 * beat_ns,
                bpm,
            );
        }
        for p in [60, 64, 67] {
            capture.record(MidiMessage::NoteOff { note: p }, 3 * beat_ns, bpm);
        }
        // Acorde 2: F4-A4-C5 en t = 2s..2.5s (1 negra después del fin del primero)
        for p in [65, 69, 72] {
            capture.record(
                MidiMessage::NoteOn {
                    note: p,
                    velocity: 104,
                },
                4 * beat_ns,
                bpm,
            );
        }
        for p in [65, 69, 72] {
            capture.record(MidiMessage::NoteOff { note: p }, 5 * beat_ns, bpm);
        }
        let notes = capture.normalized_notes(6 * beat_ns, bpm);
        assert_eq!(notes.len(), 6);
        assert_eq!(notes[0].start, Tick(0));
        assert_eq!(notes[0].pitch, 60);
        assert_eq!(notes[0].duration, Tick(PPQ));
        assert_eq!(notes[3].start, Tick(2 * PPQ));
        assert_eq!(notes[3].pitch, 65);
        assert_eq!(notes[3].velocity, 104);
    }

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

    #[test]
    fn looped_midi_clip_tiles_notes_across_stretched_length() {
        let mut pattern = Pattern::new(Tick(PPQ * 4)).unwrap();
        pattern
            .add_note(InstrumentId(1), Tick(0), Tick(PPQ), 60, 100)
            .unwrap();
        let mut clip = MidiClip::new("Loop", 0, 0.0, pattern);
        clip.props.looped = true;
        clip.trim_len = Some(12.0); // 3 compases (3 ciclos de 4 beats)
        assert_eq!(clip.duration_beats(), 12.0);
        let tiled = clip.playback_pattern();
        let starts: Vec<i64> = tiled.notes().iter().map(|n| n.start.0).collect();
        assert_eq!(starts, vec![0, PPQ * 4, PPQ * 8]);
    }
}
