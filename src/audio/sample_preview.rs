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
        atomic::{AtomicBool, AtomicU8, Ordering::Relaxed},
    },
    time::Duration,
};

pub(crate) struct PreparedSamplePreview {
    device: cpal::Device,
    setup: StreamSetup,
    frames: Vec<StereoFrame>,
}

pub(crate) struct SamplePreview {
    // Cerrar el stream antes de liberar la última referencia al PCM.
    _stream: cpal::Stream,
    _frames: Arc<Vec<StereoFrame>>,
    state: Arc<PreviewState>,
}

struct PreviewState {
    level: AtomicU8,
    finished: AtomicBool,
    failed: AtomicBool,
}

impl PreparedSamplePreview {
    pub fn load(path: &Path, audio: &AudioConfig) -> Result<Self, String> {
        let file = AudioFile::decode(path)?;
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, audio)?;
        let rate = setup.config.sample_rate;
        let frames = if rate == file.sample_rate {
            file.frames
        } else {
            resample(&file.frames, file.sample_rate, rate)?
        };
        Ok(Self {
            device,
            setup,
            frames,
        })
    }

    pub fn start(self, level: u8) -> Result<SamplePreview, String> {
        let frames = Arc::new(self.frames);
        let state = Arc::new(PreviewState {
            level: AtomicU8::new(level.min(100)),
            finished: AtomicBool::new(false),
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
    pub fn set_level(&self, level: u8) {
        self.state.level.store(level.min(100), Relaxed);
    }

    pub fn finished(&self) -> Result<bool, String> {
        if self.state.failed.load(Relaxed) {
            return Err("Falló la salida de preescucha".into());
        }
        Ok(self.state.finished.load(Relaxed))
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
                let remaining = frames.len() - at;
                if playback_end.is_none() && remaining <= output.len() / channels {
                    let last_block = timing.playback.max(timing.callback);
                    let duration = Duration::from_secs_f64(remaining as f64 / rate);
                    playback_end = Some(last_block.checked_add(duration).unwrap_or(last_block));
                }
                let level = f32::from(state.level.load(Relaxed)) / 100.0;
                for output in output.chunks_mut(block.len() * channels) {
                    let count = (output.len() / channels).min(frames.len() - at);
                    for (frame, source) in block[..count].iter_mut().zip(&frames[at..at + count]) {
                        *frame = [source[0] * level, source[1] * level];
                    }
                    write_stereo(output, &block[..count], channels, first);
                    at += count;
                }
                if playback_end.is_some_and(|end| timing.callback >= end) {
                    state.finished.store(true, Relaxed);
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
