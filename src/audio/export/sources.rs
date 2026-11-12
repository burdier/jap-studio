//! Adaptadores de fuentes del Arrangement. Los clips solapados conservan voces independientes.
use super::{Snapshot, TrackMix, render::frame_at, soxr};
use crate::audio::{
    AudioClip,
    piano::{OfflinePiano, PianoSnapshot},
    stereo_pan::StereoPan,
};
use elevate::{
    Primitive, ProcessSpec, StereoFrame,
    musical::clip::{ClipProps, MidiClip},
    primitives::amplitude::Volume,
};
use std::{
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
};

struct Mix {
    volume: Volume,
    pan: StereoPan,
    clip_gain: f32,
}

impl Mix {
    fn new(track: &TrackMix, props: ClipProps, rate: u32) -> Result<Self, String> {
        if props.looped || props.pitch_semitones != 0 {
            return Err(
                "Export: loop interno/transposición de clips todavía no tienen procesador".into(),
            );
        }
        let gain = props.gain_linear();
        if !gain.is_finite() {
            return Err("Ganancia de clip no finita".into());
        }
        let spec = ProcessSpec::new(rate as f64).map_err(|e| e.to_string())?;
        Ok(Self {
            volume: Volume::with_gain(spec, track.gain).map_err(|e| e.to_string())?,
            pan: StereoPan::new(track.pan),
            clip_gain: gain * track.fx_gain,
        })
    }
    fn add(&mut self, source: &mut [StereoFrame], output: &mut [StereoFrame]) {
        for sample in source.iter_mut().flatten() {
            *sample *= self.clip_gain;
        }
        self.volume.process(source);
        self.pan.apply(source);
        for (out, src) in output.iter_mut().zip(source) {
            out[0] += src[0];
            out[1] += src[1];
        }
    }
}

pub(super) struct AudioSource {
    file: Arc<crate::audio::AudioFile>,
    converted: Option<Vec<StereoFrame>>,
    trim: usize,
    start: u64,
    length: u64,
    mix: Mix,
}

impl AudioSource {
    fn new(clip: &AudioClip, track: &TrackMix, rate: u32, bpm: f64) -> Result<Self, String> {
        let mix = Mix::new(track, clip.props, rate)?;
        let converted = if clip.file.sample_rate != rate {
            Some(soxr::resample(
                &clip.file.frames,
                clip.file.sample_rate,
                rate,
            )?)
        } else {
            None
        };
        let trim = usize::try_from(frame_at(clip.trim_start, rate)?)
            .map_err(|_| "Recorte fuera de rango")?;
        let available = converted
            .as_ref()
            .map_or(clip.file.frames.len(), Vec::len)
            .saturating_sub(trim);
        let length = frame_at(clip.length_seconds(), rate)?.min(available as u64);
        Ok(Self {
            file: clip.file.clone(),
            converted,
            trim,
            length,
            start: frame_at(clip.start_beat * 60.0 / bpm, rate)?,
            mix,
        })
    }
    fn render(&mut self, position: u64, output: &mut [StereoFrame]) {
        let begin = position.max(self.start);
        let end = (position + output.len() as u64).min(self.start.saturating_add(self.length));
        if end <= begin {
            return;
        }
        let offset = (begin - position) as usize;
        let count = (end - begin) as usize;
        let index = self.trim + (begin - self.start) as usize;
        let frames = self.converted.as_deref().unwrap_or(&self.file.frames);
        let mut scratch = [[0.0; 2]; 512];
        scratch[..count].copy_from_slice(&frames[index..index + count]);
        self.mix
            .add(&mut scratch[..count], &mut output[offset..offset + count]);
    }
}

pub(super) struct MidiSource {
    clip: MidiClip,
    start: u64,
    synth: Option<OfflinePiano>,
    sampler: bool,
    mix: Mix,
}

pub(super) struct Sources {
    audio: Vec<AudioSource>,
    midi: Vec<MidiSource>,
    piano: Option<PianoSnapshot>,
    sampler: Option<crate::audio::sampler::SamplerSnapshot>,
    rate: u32,
    bpm: f64,
}

impl Sources {
    pub fn prepare(
        snapshot: &Snapshot,
        selected: &[usize],
        rate: u32,
        cancel: &AtomicBool,
    ) -> Result<Self, String> {
        let mut audio = Vec::new();
        let mut midi = Vec::new();
        for clip in &snapshot.audio {
            if cancel.load(Ordering::Relaxed) {
                return Err("Exportación cancelada".into());
            }
            let track = snapshot
                .tracks
                .get(clip.track)
                .ok_or("Clip de audio en pista inexistente")?;
            if selected.contains(&clip.track) && track.gain != 0.0 && !clip.props.muted {
                audio.push(AudioSource::new(clip, track, rate, snapshot.bpm)?);
            }
        }
        for clip in &snapshot.midi {
            let track = snapshot
                .tracks
                .get(clip.track)
                .ok_or("Clip MIDI en pista inexistente")?;
            if selected.contains(&clip.track) && track.gain != 0.0 && !clip.props.muted {
                let sampler = snapshot
                    .sampler
                    .as_ref()
                    .is_some_and(|(tracks, _)| tracks.contains(&clip.track));
                if !sampler && snapshot.piano.is_none() {
                    return Err("Falta el instrumento para exportar MIDI".into());
                }
                midi.push(MidiSource {
                    clip: clip.clone(),
                    start: frame_at(clip.start_beat * 60.0 / snapshot.bpm, rate)?,
                    synth: None,
                    sampler,
                    // El loop MIDI es de audición; exportar conserva la duración finita del clip.
                    mix: Mix::new(
                        track,
                        ClipProps {
                            looped: false,
                            ..clip.props
                        },
                        rate,
                    )?,
                });
            }
        }
        Ok(Self {
            audio,
            midi,
            piano: snapshot.piano.clone(),
            sampler: snapshot.sampler.as_ref().map(|(_, s)| s.clone()),
            rate,
            bpm: snapshot.bpm,
        })
    }

    pub fn finish_input(&mut self) {
        for source in &mut self.midi {
            if let Some(synth) = &mut source.synth {
                synth.finish_input();
            }
        }
    }

    pub fn render(
        &mut self,
        position: u64,
        output: &mut [StereoFrame],
        tail: bool,
    ) -> Result<(), String> {
        output.fill([0.0; 2]);
        if !tail {
            for source in &mut self.audio {
                source.render(position, output);
            }
        }
        for source in &mut self.midi {
            if tail && source.synth.is_none() {
                continue;
            }
            if source.start >= position + output.len() as u64 {
                continue;
            }
            let offset = source.start.saturating_sub(position) as usize;
            if source.synth.is_none() {
                source.synth = Some(if source.sampler {
                    let snapshot = self
                        .sampler
                        .as_ref()
                        .ok_or("Falta el sample para exportar")?
                        .clone();
                    let engine = crate::audio::sampler::SfizzEngine::prepare(snapshot, self.rate)?;
                    OfflinePiano::from_backend(
                        crate::audio::piano::SynthBackend::Sampler(engine),
                        &source.clip.playback_pattern(),
                        self.rate,
                        self.bpm,
                        source.clip.channel.saturating_sub(1).min(15),
                    )?
                } else {
                    OfflinePiano::new(
                        self.piano.as_ref().ok_or("Falta SoundFont")?,
                        &source.clip.playback_pattern(),
                        self.rate,
                        self.bpm,
                        source.clip.channel.saturating_sub(1).min(15),
                    )?
                });
            }
            let count = output.len() - offset;
            let mut scratch = [[0.0; 2]; 512];
            source.synth.as_mut().unwrap().render(&mut scratch[..count]);
            source.mix.add(&mut scratch[..count], &mut output[offset..]);
        }
        Ok(())
    }
}
