//! Prueba de salida/entrada: stream independiente y temporal, sin archivos ni monitorización.
use super::{
    device::AudioConfig,
    output::{StreamSetup, choose_input_config, choose_output_config},
};
use cpal::{
    FromSample, Sample, SampleFormat, SizedSample,
    traits::{DeviceTrait, StreamTrait},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};
#[derive(Default)]
struct ProbeState {
    peak: AtomicU32,
    frames: AtomicU32,
    failed: AtomicBool,
    finished: AtomicBool,
}
pub(crate) struct DeviceProbe {
    _stream: cpal::Stream,
    state: Arc<ProbeState>,
    pub input: bool,
    pub rate: u32,
}
impl DeviceProbe {
    pub fn start(config: &AudioConfig, input: bool) -> Result<Self, String> {
        let device = config.device(input)?;
        let setup = if input {
            choose_input_config(&device, config)?
        } else {
            choose_output_config(&device, config)?
        };
        let state = Arc::new(ProbeState::default());
        let rate = setup.config.sample_rate;
        let stream = match setup.format {
            SampleFormat::F32 => build::<f32>(&device, setup, input, state.clone()),
            SampleFormat::I16 => build::<i16>(&device, setup, input, state.clone()),
            SampleFormat::I32 => build::<i32>(&device, setup, input, state.clone()),
            SampleFormat::U16 => build::<u16>(&device, setup, input, state.clone()),
            _ => Err("Formato no compatible para prueba".into()),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            _stream: stream,
            state,
            input,
            rate,
        })
    }
    pub fn level(&self) -> f32 {
        f32::from_bits(self.state.peak.load(Ordering::Relaxed)).clamp(0., 1.)
    }
    pub fn frames(&self) -> u32 {
        self.state.frames.load(Ordering::Relaxed)
    }
    pub fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Relaxed)
    }
    pub fn finished(&self) -> bool {
        self.state.finished.load(Ordering::Relaxed)
    }
}
fn build<T>(
    device: &cpal::Device,
    setup: StreamSetup,
    input: bool,
    state: Arc<ProbeState>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
    f32: FromSample<T>,
{
    let channels = usize::from(setup.config.channels);
    let first = usize::from(setup.first_channel);
    let error_state = state.clone();
    if input {
        device.build_input_stream(
            setup.config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let mut peak = 0.0_f32;
                for frame in data.chunks_exact(channels) {
                    peak = peak.max(f32::from_sample(frame[first]).abs());
                }
                state.peak.store(peak.to_bits(), Ordering::Relaxed);
                state
                    .frames
                    .store((data.len() / channels) as u32, Ordering::Relaxed);
            },
            move |_| {
                error_state.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
    } else {
        let rate = f64::from(setup.config.sample_rate);
        let mut position = 0_u64;
        let length = (rate * 2.) as u64;
        device.build_output_stream(
            setup.config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                data.fill(T::from_sample(0.));
                for frame in data.chunks_exact_mut(channels) {
                    if position >= length {
                        break;
                    }
                    let ramp = (position.min(length - position) as f64 / 128.).min(1.);
                    let value = ((position as f64 * 440. * std::f64::consts::TAU / rate).sin()
                        * 0.063
                        * ramp) as f32;
                    frame[first] = T::from_sample(value);
                    frame[first + 1] = T::from_sample(value);
                    position += 1;
                }
                state
                    .frames
                    .store((data.len() / channels) as u32, Ordering::Relaxed);
                state.finished.store(position >= length, Ordering::Relaxed);
            },
            move |_| {
                error_state.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
    }
    .map_err(|e| e.to_string())
}
