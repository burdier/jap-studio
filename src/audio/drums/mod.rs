//! Audición del patrón con cuatro percusiones sintetizadas.
//! Adaptador provisional: comparte el patrón musical, aún no el motor multipista.
mod stream;
mod voice;

use super::output::choose_output_config;
use cpal::{
    SampleFormat,
    traits::{HostTrait, StreamTrait},
};
use elevate::devices::utility_slot::UtilitySlot;
use elevate::musical::{
    model::Pattern,
    scheduler::PreparedPattern,
    time::{TimeBase, TimeError},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

pub(super) struct DrumState {
    gain: AtomicU32,
    utility: UtilitySlot,
    failed: AtomicBool,
    frame: AtomicU64,
}

pub(crate) struct DrumPlayer {
    // El stream debe caer antes que el estado que comparte con el callback.
    _stream: cpal::Stream,
    state: Arc<DrumState>,
    time_base: TimeBase,
    loop_frames: u64,
    events: usize,
    pub sample_rate: u32,
}

impl DrumPlayer {
    /// Compila el patrón fuera del callback y abre la salida al tempo indicado.
    /// El plan es inmutable: para escuchar cambios hay que detener y volver a reproducir.
    pub fn start(
        pattern: &Pattern,
        bpm: f64,
        gain: f32,
        utility: UtilitySlot,
    ) -> Result<Self, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No hay dispositivo de salida predeterminado")?;
        let config = choose_output_config(&device, None)?;
        let sample_rate = config.sample_rate();
        let time_base = TimeBase::new(sample_rate, bpm).map_err(time_error)?;
        let plan = Arc::new(PreparedPattern::compile(pattern, time_base).map_err(time_error)?);
        let loop_frames = plan.length_frames();
        let events = plan.event_count();
        let state = Arc::new(DrumState {
            gain: AtomicU32::new(gain.clamp(0.0, 2.0).to_bits()),
            utility,
            failed: AtomicBool::new(false),
            frame: AtomicU64::new(0),
        });
        let stream = match config.sample_format() {
            SampleFormat::F32 => {
                stream::build_stream::<f32>(&device, config.into(), plan, state.clone())
            }
            SampleFormat::I16 => {
                stream::build_stream::<i16>(&device, config.into(), plan, state.clone())
            }
            SampleFormat::I32 => {
                stream::build_stream::<i32>(&device, config.into(), plan, state.clone())
            }
            SampleFormat::U16 => {
                stream::build_stream::<u16>(&device, config.into(), plan, state.clone())
            }
            format => return Err(format!("Formato de salida no compatible: {format:?}")),
        }?;
        stream.play().map_err(|error| error.to_string())?;
        Ok(Self {
            _stream: stream,
            state,
            time_base,
            loop_frames,
            events,
            sample_rate,
        })
    }

    /// Posición absoluta desde que se abrió el stream, en negras.
    pub fn beat(&self) -> f64 {
        self.state.frame.load(Ordering::Relaxed) as f64 / f64::from(self.sample_rate)
            * self.time_base.bpm()
            / 60.0
    }

    pub fn loop_beats(&self) -> f64 {
        self.loop_frames as f64 / f64::from(self.sample_rate) * self.time_base.bpm() / 60.0
    }

    pub fn event_count(&self) -> usize {
        self.events
    }

    pub fn set_gain(&self, gain: f32) {
        self.state
            .gain
            .store(gain.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }

    pub fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Relaxed)
    }
}

fn time_error(error: TimeError) -> String {
    error.to_string()
}
