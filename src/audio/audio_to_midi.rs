//! Transcripción polifónica Audio → MIDI offline con `basic-pitch-rs` (Spotify Basic Pitch).
//! Sin dependencias de GPUI ni E/S en el hilo de audio: ejecutar siempre en segundo plano.
use basic_pitch::{BasicPitch, NoteParams, constants::AUDIO_SAMPLE_RATE};
use elevate::{
    StereoFrame,
    musical::{
        model::{InstrumentId, Pattern},
        time::{PPQ, Tick},
    },
};
use std::{num::NonZeroUsize, sync::OnceLock};

static TRANSCRIBER: OnceLock<Result<BasicPitch, String>> = OnceLock::new();

pub(crate) struct TranscribedNote {
    pub start: Tick,
    pub duration: Tick,
    pub pitch: u8,
    pub velocity: u8,
}

pub(crate) struct AudioToMidiReport {
    pub length: Tick,
    pub notes: Vec<TranscribedNote>,
    pub min_pitch: u8,
    pub max_pitch: u8,
}

impl AudioToMidiReport {
    pub(crate) fn note_count(&self) -> usize {
        self.notes.len()
    }

    pub(crate) fn into_pattern(self, instrument: InstrumentId) -> Result<Pattern, String> {
        let mut pattern = Pattern::new(self.length)
            .map_err(|err| format!("No se pudo construir el patrón MIDI: {err}"))?;
        for note in self.notes {
            let clamped_dur = note.duration.0.min(self.length.0 - note.start.0).max(1);
            let _ = pattern.add_note(
                instrument,
                note.start,
                Tick(clamped_dur),
                note.pitch,
                note.velocity,
            );
        }
        Ok(pattern)
    }
}

fn transcriber() -> Result<&'static BasicPitch, String> {
    let model = TRANSCRIBER.get_or_init(|| {
        let cores = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
        BasicPitch::new()
            .map(|bp| bp.with_threads(cores))
            .map_err(|err| format!("No se pudo inicializar el modelo Basic Pitch: {err}"))
    });
    match model {
        Ok(bp) => Ok(bp),
        Err(err) => Err(err.clone()),
    }
}

/// Convierte frames estéreo a mono 22 050 Hz (formato requerido por Basic Pitch).
/// Usa `libsoxr` VHQ cuando está disponible y cae a interpolación lineal como respaldo.
fn prepare_mono_22k(frames: &[StereoFrame], sample_rate: u32) -> Result<Vec<f32>, String> {
    if frames.is_empty() {
        return Err("El fragmento de audio está vacío".into());
    }
    if sample_rate == 0 {
        return Err("Frecuencia de muestreo inválida".into());
    }
    let resampled_stereo;
    let source = if sample_rate == AUDIO_SAMPLE_RATE {
        frames
    } else if let Ok(soxr_out) =
        crate::audio::export::resample(frames, sample_rate, AUDIO_SAMPLE_RATE)
    {
        resampled_stereo = soxr_out;
        &resampled_stereo
    } else {
        return Ok(linear_resample_mono(frames, sample_rate, AUDIO_SAMPLE_RATE));
    };

    let mut mono = Vec::with_capacity(source.len());
    for &[l, r] in source {
        let sample = 0.5 * (l + r);
        mono.push(if sample.is_finite() { sample } else { 0.0 });
    }
    Ok(mono)
}

fn linear_resample_mono(frames: &[StereoFrame], from: u32, to: u32) -> Vec<f32> {
    let ratio = f64::from(from) / f64::from(to);
    let out_len = ((frames.len() as f64) / ratio).round().max(1.0) as usize;
    let last = frames.len().saturating_sub(1);
    let mut mono = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = (pos.floor() as usize).min(last);
        let next = (idx + 1).min(last);
        let frac = (pos - idx as f64) as f32;
        let a = 0.5 * (frames[idx][0] + frames[idx][1]);
        let b = 0.5 * (frames[next][0] + frames[next][1]);
        let s = a + (b - a) * frac;
        mono.push(if s.is_finite() { s } else { 0.0 });
    }
    mono
}

/// Transcribe un buffer estéreo a notas polifónicas alineadas al `bpm` indicado.
pub(crate) fn transcribe_audio_to_midi(
    frames: &[StereoFrame],
    sample_rate: u32,
    bpm: f64,
) -> Result<AudioToMidiReport, String> {
    if bpm <= 0.0 || !bpm.is_finite() {
        return Err("Tempo inválido para transcripción Audio → MIDI".into());
    }
    let mono = prepare_mono_22k(frames, sample_rate)?;
    let model = transcriber()?;
    let transcription = model
        .transcribe_with(&mono, &NoteParams::default())
        .map_err(|err| format!("Error al transcribir audio: {err}"))?;

    let sorted = transcription.sorted_notes();
    let ticks_per_sec = (bpm / 60.0) * (PPQ as f64);
    let min_note_ticks = (PPQ / 16).max(1);
    let clip_duration_s = frames.len() as f64 / f64::from(sample_rate);
    let clip_ticks = (clip_duration_s * ticks_per_sec).round().max(PPQ as f64) as i64;

    let mut notes = Vec::with_capacity(sorted.len());
    let mut max_end_tick = clip_ticks;
    let mut min_pitch = u8::MAX;
    let mut max_pitch = 0u8;

    for note in sorted {
        let start = (note.start_s.max(0.0) * ticks_per_sec).round() as i64;
        let end = (note.end_s.max(note.start_s) * ticks_per_sec).round() as i64;
        let duration = (end - start).max(min_note_ticks);
        let velocity = note.velocity().clamp(1, 127);
        let pitch = note.pitch.min(127);
        max_end_tick = max_end_tick.max(start + duration);
        min_pitch = min_pitch.min(pitch);
        max_pitch = max_pitch.max(pitch);
        notes.push(TranscribedNote {
            start: Tick(start),
            duration: Tick(duration),
            pitch,
            velocity,
        });
    }

    // Redondear ligeramente a un múltiplo de negra para que el clip encaje en la rejilla.
    let pattern_len = ((max_end_tick + PPQ - 1) / PPQ).max(1) * PPQ;
    if notes.is_empty() {
        min_pitch = 60;
        max_pitch = 60;
    }

    Ok(AudioToMidiReport {
        length: Tick(pattern_len),
        notes,
        min_pitch,
        max_pitch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcribes_a4_sine_wave_to_midi_note_69() {
        let sr = 44_100u32;
        let frames: Vec<StereoFrame> = (0..(sr as usize * 2))
            .map(|i| {
                let s = 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin();
                [s, s]
            })
            .collect();
        let report = transcribe_audio_to_midi(&frames, sr, 120.0)
            .expect("la transcripción de A4 debe completarse");
        let pattern = report
            .into_pattern(InstrumentId(1))
            .expect("debe construir el patrón");
        assert!(
            pattern.notes().iter().any(|n| n.pitch == 69),
            "debe detectar A4 (MIDI 69), notas={:?}",
            pattern.notes()
        );
    }
}
