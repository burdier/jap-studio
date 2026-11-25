//! Audición de archivos de biblioteca por frames, sin reloj ni pistas del proyecto.
use super::{
    AudioFile,
    device::AudioConfig,
    export::resample,
    output::{StreamSetup, choose_output_config, write_stereo},
};
use cpal::{
    FromSample, SampleFormat, SizedSample,
    traits::{DeviceTrait, StreamTrait},
};
use elevate::StereoFrame;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering::Relaxed},
    },
    time::Duration,
};

pub(crate) struct PreparedSamplePreview {
    device: cpal::Device,
    setup: StreamSetup,
    frames: Vec<StereoFrame>,
    looping: bool,
}

pub(crate) struct SamplePreview {
    // Cerrar el stream antes de liberar la última referencia al PCM.
    _stream: cpal::Stream,
    _frames: Arc<Vec<StereoFrame>>,
    state: Arc<PreviewState>,
}

struct PreviewState {
    looping: AtomicBool,
    paused: AtomicBool,
    playback: AtomicU8,
    position: AtomicUsize,
    level: AtomicU8,
    failed: AtomicBool,
}

const PLAYING: u8 = 0;
const REWIND: u8 = 1;
const FINISHED: u8 = 2;

impl PreparedSamplePreview {
    pub fn load(path: &Path, audio: &AudioConfig) -> Result<(Self, Option<(u8, i32)>), String> {
        let file = AudioFile::decode(path)?;
        let root = super::fundamental::stable_root(&file.frames, file.sample_rate);
        Ok((
            Self::from_frames(file.frames, file.sample_rate, audio, false)?,
            root,
        ))
    }

    pub fn phrase(
        path: &Path,
        mut clip: elevate::musical::clip::MidiClip,
        mut track: elevate::musical::track::Track,
        bpm: f64,
        root: u8,
        cents: i32,
        audio: &AudioConfig,
    ) -> Result<Self, String> {
        use super::{
            piano::{PreparedArrangementVoice, VoiceSource},
            sampler::{SampleAsset, SamplerSnapshot},
        };
        use elevate::{
            devices::sampler::{ROOT, SamplerParams, TUNE},
            musical::{arrangement_midi::PreparedMidiArrangement, model::InstrumentId},
        };
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, audio)?;
        let rate = setup.config.sample_rate;
        let id = InstrumentId(1);
        track.instrument = Some(id);
        clip.start_beat = 0.0;
        clip.hilo_target = None;
        clip.drum_target = None;
        clip.props.muted = false;
        if clip.playback_pattern().notes().is_empty() {
            return Err("El tramo del clip MIDI está vacío".into());
        }
        let plan =
            PreparedMidiArrangement::compile_track(&[clip.clone()], &track, clip.track, rate, bpm)?;
        let mut params = SamplerParams::default();
        params.set(ROOT, f32::from(root));
        params.set(TUNE, -(cents as f32) / 100.0);
        let asset = SampleAsset::load(path)?;
        let mut voice = PreparedArrangementVoice::prepare(
            id,
            VoiceSource::Sampler(SamplerSnapshot { asset, params }),
            rate,
        )?;
        let total = (plan.end_frame() + u64::from(rate) / 4).min(u64::from(rate) * 30);
        let mut frames = vec![[0.0; 2]; total as usize];
        for (i, block) in frames.chunks_mut(256).enumerate() {
            voice.render_timed(&plan, i as u64 * 256, block, None);
        }
        // Evita un corte brusco si una frase larga alcanza el límite de preescucha.
        for (i, frame) in frames
            .iter_mut()
            .rev()
            .take((rate / 100) as usize)
            .enumerate()
        {
            let gain = i as f32 / (rate / 100) as f32;
            frame[0] *= gain;
            frame[1] *= gain;
        }
        Ok(Self {
            device,
            setup,
            frames,
            looping: false,
        })
    }
    pub fn from_frames(
        frames: Vec<StereoFrame>,
        source_rate: u32,
        audio: &AudioConfig,
        looping: bool,
    ) -> Result<Self, String> {
        if frames.is_empty() {
            return Err("Selección vacía".into());
        }
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, audio)?;
        let rate = setup.config.sample_rate;
        let frames = if rate == source_rate {
            frames
        } else {
            resample(&frames, source_rate, rate)?
        };
        Ok(Self {
            device,
            setup,
            frames,
            looping,
        })
    }

    pub fn start(self, level: u8) -> Result<SamplePreview, String> {
        let frames = Arc::new(self.frames);
        let state = Arc::new(PreviewState {
            looping: AtomicBool::new(self.looping),
            paused: AtomicBool::new(false),
            playback: AtomicU8::new(PLAYING),
            position: AtomicUsize::new(0),
            level: AtomicU8::new(level.min(100)),
            failed: AtomicBool::new(false),
        });
        let stream = match self.setup.format {
            SampleFormat::F32 => {
                build_stream::<f32>(&self.device, &self.setup, frames.clone(), state.clone())
            }
            SampleFormat::I16 => {
                build_stream::<i16>(&self.device, &self.setup, frames.clone(), state.clone())
            }
            SampleFormat::I32 => {
                build_stream::<i32>(&self.device, &self.setup, frames.clone(), state.clone())
            }
            SampleFormat::U16 => {
                build_stream::<u16>(&self.device, &self.setup, frames.clone(), state.clone())
            }
            _ => return Err("Formato de salida no compatible".into()),
        }?;
        stream.play().map_err(|error| error.to_string())?;
        Ok(SamplePreview {
            _stream: stream,
            _frames: frames,
            state,
        })
    }
}

impl SamplePreview {
    pub fn paused(&self) -> bool {
        self.state.paused.load(Relaxed)
    }
    pub fn set_paused(&self, value: bool) {
        self.state.paused.store(value, Relaxed)
    }
    pub fn set_looping(&self, value: bool) {
        self.state.looping.store(value, Relaxed)
    }
    pub fn rewind(&self) {
        self.state.playback.store(REWIND, Relaxed);
    }
    pub fn position(&self) -> f64 {
        self.state.position.load(Relaxed) as f64 / self._frames.len().max(1) as f64
    }
    pub fn set_level(&self, level: u8) {
        self.state.level.store(level.min(100), Relaxed);
    }

    pub fn finished(&self) -> Result<bool, String> {
        if self.state.failed.load(Relaxed) {
            return Err("Falló la salida de preescucha".into());
        }
        Ok(self.state.playback.load(Relaxed) == FINISHED)
    }
}

fn build_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    setup: &StreamSetup,
    frames: Arc<Vec<StereoFrame>>,
    state: Arc<PreviewState>,
) -> Result<cpal::Stream, String> {
    let channels = usize::from(setup.config.channels);
    let first = usize::from(setup.first_channel);
    let rate = f64::from(setup.config.sample_rate);
    let failed = state.clone();
    let mut at = 0;
    let mut playback_end = None;
    let mut block = [[0.0; 2]; 256];
    device
        .build_output_stream(
            setup.config.clone(),
            move |output: &mut [T], info: &cpal::OutputCallbackInfo| {
                let timing = info.timestamp();
                if state
                    .playback
                    .compare_exchange(REWIND, PLAYING, Relaxed, Relaxed)
                    .is_ok()
                {
                    at = 0;
                    playback_end = None;
                    state.position.store(0, Relaxed);
                }
                let looping = state.looping.load(Relaxed);
                if state.paused.load(Relaxed) {
                    output.fill(T::from_sample(0.));
                    return;
                }
                if looping && at == frames.len() && state.playback.load(Relaxed) != FINISHED {
                    at = 0;
                    playback_end = None;
                }
                let remaining = frames.len() - at;
                if !looping && playback_end.is_none() && remaining <= output.len() / channels {
                    let last_block = timing.playback.max(timing.callback);
                    let duration = Duration::from_secs_f64(remaining as f64 / rate);
                    playback_end = Some(last_block.checked_add(duration).unwrap_or(last_block));
                }
                let level = f32::from(state.level.load(Relaxed)) / 100.0;
                for output in output.chunks_mut(block.len() * channels) {
                    let count = output.len() / channels;
                    for frame in &mut block[..count] {
                        *frame = if let Some(source) = frames.get(at) {
                            at += 1;
                            if looping && at == frames.len() {
                                at = 0
                            }
                            [source[0] * level, source[1] * level]
                        } else {
                            [0.; 2]
                        };
                    }
                    write_stereo(output, &block[..count], channels, first);
                }
                state.position.store(at, Relaxed);
                if playback_end.is_some_and(|end| timing.callback >= end) {
                    // El final de este bloque no invalida un reinicio pedido mientras se renderiza.
                    let _ = state
                        .playback
                        .compare_exchange(PLAYING, FINISHED, Relaxed, Relaxed);
                }
            },
            move |error| {
                eprintln!("Error de salida de preescucha: {error}");
                failed.failed.store(true, Relaxed);
            },
            None,
        )
        .map_err(|error| error.to_string())
}
