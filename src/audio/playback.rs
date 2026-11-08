//! Reproductor de UN clip. Adaptador provisional: no es el motor multipista.
use super::{AudioClip, BEATS_PER_SECOND, output::choose_output_config};
use cpal::{
    FromSample, SampleFormat, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use elevate::{Primitive, ProcessSpec, primitives::amplitude::Volume};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

struct PlaybackState {
    gain: AtomicU32,
    bypass: AtomicBool,
    position: AtomicU64,
    finished: AtomicBool,
    failed: AtomicBool,
}

pub(crate) struct Player {
    _stream: cpal::Stream,
    state: Arc<PlaybackState>,
    pub sample_rate: u32,
}

impl Player {
    pub fn start(
        clip: &AudioClip,
        start_beat: f64,
        gain: f32,
        bypass: bool,
    ) -> Result<Self, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No hay dispositivo de salida predeterminado")?;
        let rate = clip.file.sample_rate;
        let config = choose_output_config(&device, Some(rate))?;
        let state = Arc::new(PlaybackState {
            gain: AtomicU32::new(gain.to_bits()),
            bypass: AtomicBool::new(bypass),
            position: AtomicU64::new(
                (start_beat / BEATS_PER_SECOND * f64::from(rate)).round() as u64
            ),
            finished: AtomicBool::new(false),
            failed: AtomicBool::new(false),
        });
        let stream = match config.sample_format() {
            SampleFormat::F32 => Self::stream::<f32>(&device, config.into(), clip, state.clone()),
            SampleFormat::I16 => Self::stream::<i16>(&device, config.into(), clip, state.clone()),
            SampleFormat::I32 => Self::stream::<i32>(&device, config.into(), clip, state.clone()),
            SampleFormat::U16 => Self::stream::<u16>(&device, config.into(), clip, state.clone()),
            _ => return Err("Formato de salida no compatible".into()),
        }?;
        stream.play().map_err(|error| error.to_string())?;
        Ok(Self {
            _stream: stream,
            state,
            sample_rate: rate,
        })
    }

    fn stream<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        clip: &AudioClip,
        state: Arc<PlaybackState>,
    ) -> Result<cpal::Stream, String> {
        let mut volume = Volume::with_gain(
            ProcessSpec::new(f64::from(config.sample_rate)).map_err(|error| error.to_string())?,
            f32::from_bits(state.gain.load(Ordering::Relaxed)),
        )
        .map_err(|error| error.to_string())?;
        volume.set_bypassed(state.bypass.load(Ordering::Relaxed));
        let file = clip.file.clone();
        let clip_start =
            (clip.start_beat / BEATS_PER_SECOND * f64::from(file.sample_rate)).round() as u64;
        let clip_end = clip_start.saturating_add(file.frames.len() as u64);
        let mut position = state.position.load(Ordering::Relaxed);
        let error_state = state.clone();
        device
            .build_output_stream(
                config,
                move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                    let gain = f32::from_bits(state.gain.load(Ordering::Relaxed));
                    if volume.gain() != gain && volume.set_gain(gain).is_err() {
                        state.failed.store(true, Ordering::Relaxed);
                    }
                    volume.set_bypassed(state.bypass.load(Ordering::Relaxed));
                    let mut block = [[0.0_f32; 2]; 256];
                    for output_block in output.chunks_mut(512) {
                        let count = output_block.len() / 2;
                        for frame in block.iter_mut().take(count) {
                            *frame = position
                                .checked_sub(clip_start)
                                .and_then(|index| file.frames.get(index as usize))
                                .copied()
                                .unwrap_or([0.0; 2]);
                            position = position.saturating_add(1).min(clip_end);
                        }
                        volume.process(&mut block[..count]);
                        for (destination, frame) in
                            output_block.chunks_exact_mut(2).zip(block.iter())
                        {
                            destination[0] = T::from_sample(frame[0].clamp(-1.0, 1.0));
                            destination[1] = T::from_sample(frame[1].clamp(-1.0, 1.0));
                        }
                    }
                    state.position.store(position, Ordering::Relaxed);
                    if position >= clip_end {
                        state.finished.store(true, Ordering::Relaxed);
                    }
                },
                move |error| {
                    eprintln!("Error de salida de audio: {error}");
                    error_state.failed.store(true, Ordering::Relaxed);
                },
                None,
            )
            .map_err(|error| error.to_string())
    }

    pub fn set_gain(&self, gain: f32) {
        self.state
            .gain
            .store(gain.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }
    pub fn set_bypass(&self, bypass: bool) {
        self.state.bypass.store(bypass, Ordering::Relaxed);
    }
    pub fn beat(&self) -> f64 {
        self.state.position.load(Ordering::Relaxed) as f64 / f64::from(self.sample_rate)
            * BEATS_PER_SECOND
    }
    pub fn finished(&self) -> bool {
        self.state.finished.load(Ordering::Relaxed)
    }
    pub fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Relaxed)
    }
}
