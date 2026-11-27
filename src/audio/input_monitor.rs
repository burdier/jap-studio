//! Monitoreo de entrada en tiempo real (estilo Ableton `Off | Auto | In`):
//! captura la interfaz de entrada configurada, la pasa por un ring buffer lock-free (`rtrb`)
//! con control adaptativo de deriva, procesa el Rack FX vivo de la pista y la envía a la salida
//! con protección anti-clipping (`soft_ceiling`) y cero reservas en RT.
use super::{
    device::AudioConfig,
    effects::Rack,
    fx_gain::{FxGain, PreparedFxGain},
    output::{choose_input_config, choose_output_config, write_stereo},
    stereo_pan::StereoPan,
};
use cpal::{
    Sample, SampleFormat, SizedSample,
    traits::{DeviceTrait, StreamTrait},
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

const RING_CAPACITY_FRAMES: usize = 8192;
const RT_BLOCK_MAX: usize = 512;

/// Modo de monitoreo estilo Ableton Live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MonitorMode {
    /// Entrada silenciada.
    Off,
    /// Escucha siempre la entrada en vivo a través de los FX de la pista (incluso en Play/Stop).
    In,
    /// Escucha en vivo en Stop o durante grabación; se silencia durante reproducción normal.
    Auto,
}

impl MonitorMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::In,
            Self::In => Self::Auto,
            Self::Auto => Self::Off,
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            Self::Off => "MON",
            Self::In => "IN",
            Self::Auto => "AUTO",
        }
    }
}

/// Desglose de latencia de monitorización en vivo (buffer E/S + ring + PDC del rack FX).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct MonitorLatency {
    pub sample_rate: u32,
    pub io_buffer_frames: u32,
    pub ring_frames: u32,
    pub fx_pdc_frames: u32,
}

impl MonitorLatency {
    pub fn total_frames(self) -> u32 {
        self.io_buffer_frames
            .saturating_add(self.ring_frames)
            .saturating_add(self.fx_pdc_frames)
    }

    pub fn total_ms(self) -> f32 {
        self.frames_to_ms(self.total_frames())
    }

    pub fn io_ms(self) -> f32 {
        self.frames_to_ms(self.io_buffer_frames)
    }

    pub fn ring_ms(self) -> f32 {
        self.frames_to_ms(self.ring_frames)
    }

    pub fn fx_pdc_ms(self) -> f32 {
        self.frames_to_ms(self.fx_pdc_frames)
    }

    fn frames_to_ms(self, frames: u32) -> f32 {
        if self.sample_rate == 0 {
            0.0
        } else {
            frames as f32 * 1000.0 / self.sample_rate as f32
        }
    }
}

pub(crate) struct MonitorShared {
    active: AtomicBool,
    stereo: AtomicBool,
    gain: AtomicU32,
    pan: StereoPan,
    fx: FxGain,
    in_peak: AtomicU32,
    out_peak: AtomicU32,
    queued_frames: AtomicU32,
    fx_latency_frames: AtomicU32,
    failed: AtomicBool,
}

impl MonitorShared {
    pub fn new(stereo: bool) -> Self {
        Self {
            active: AtomicBool::new(true),
            stereo: AtomicBool::new(stereo),
            gain: AtomicU32::new(1.0_f32.to_bits()),
            pan: StereoPan::new(0.0),
            fx: FxGain::new(1.0),
            in_peak: AtomicU32::new(0),
            out_peak: AtomicU32::new(0),
            queued_frames: AtomicU32::new(0),
            fx_latency_frames: AtomicU32::new(0),
            failed: AtomicBool::new(false),
        }
    }
}

/// Estado RT de la salida del monitor, reutilizable en pruebas unitarias sin hardware.
pub(crate) struct MonitorProcessor {
    consumer: Consumer<[f32; 2]>,
    fx: PreparedFxGain,
    gate: f32,
    smooth_gain: f32,
    last_frame: [f32; 2],
    rate: u32,
}

impl MonitorProcessor {
    pub fn new(consumer: Consumer<[f32; 2]>, shared: &MonitorShared, rate: u32) -> Self {
        Self {
            consumer,
            fx: shared.fx.prepare(),
            gate: 0.0,
            smooth_gain: f32::from_bits(shared.gain.load(Ordering::Relaxed)),
            last_frame: [0.0; 2],
            rate: rate.max(8_000),
        }
    }

    /// Drena el ring buffer aplicando control de deriva, procesa FX + volumen + pan y entrega
    /// `frames` listos para escribir en la salida (sin asignaciones ni bloqueos).
    pub fn render(&mut self, shared: &MonitorShared, frames: &mut [[f32; 2]]) {
        let len = frames.len();
        if len == 0 {
            return;
        }
        // Control adaptativo de deriva de reloj entre entrada y salida:
        // evita que la latencia crezca por encima de ~4 bloques (~15-20 ms @ 48 kHz).
        let max_queued = (len * 4).max(2048);
        let target_queued = (len * 2).max(512);
        let available = self.consumer.slots();
        if available > max_queued {
            let drop_count = available - target_queued;
            for _ in 0..drop_count {
                if self.consumer.pop().is_err() {
                    break;
                }
            }
        }
        let queued = self.consumer.slots().min(u32::MAX as usize) as u32;
        shared.queued_frames.store(queued, Ordering::Relaxed);

        for frame in frames.iter_mut() {
            if let Ok(sample) = self.consumer.pop() {
                self.last_frame = sample;
                *frame = sample;
            } else {
                // Underrun suave hacia silencio (evita chasquidos digitales).
                self.last_frame[0] *= 0.92;
                self.last_frame[1] *= 0.92;
                *frame = self.last_frame;
            }
        }

        // Procesar toda la cadena FX en vivo de la pista monitoreada.
        self.fx
            .process_timed_automated(&shared.fx, frames, None, None);

        let target_active = if shared.active.load(Ordering::Acquire) {
            1.0_f32
        } else {
            0.0_f32
        };
        let target_gain = f32::from_bits(shared.gain.load(Ordering::Relaxed)).clamp(0.0, 2.0);
        let ramp_step = 1.0 / (self.rate as f32 * 0.004).max(1.0); // ~4 ms anti-click

        for frame in frames.iter_mut() {
            self.gate += (target_active - self.gate).clamp(-ramp_step, ramp_step);
            self.smooth_gain += (target_gain - self.smooth_gain).clamp(-ramp_step, ramp_step);
            let scale = self.gate * self.smooth_gain;
            frame[0] *= scale;
            frame[1] *= scale;
        }
        shared.pan.apply(frames);

        let mut block_peak = 0.0_f32;
        for frame in frames.iter() {
            block_peak = block_peak.max(frame[0].abs().max(frame[1].abs()));
        }
        let prev = f32::from_bits(shared.out_peak.load(Ordering::Relaxed));
        let smoothed = block_peak.max(prev * 0.78);
        shared.out_peak.store(smoothed.to_bits(), Ordering::Relaxed);
    }
}

pub(crate) struct InputMonitor {
    _input_stream: cpal::Stream,
    _output_stream: cpal::Stream,
    shared: Arc<MonitorShared>,
    pub output_rate: u32,
    pub io_buffer_frames: u32,
    pub input_device_name: String,
}

impl InputMonitor {
    /// Abre streams CPAL de entrada y salida conectados mediante ring buffer lock-free.
    pub fn start(
        config: &AudioConfig,
        stereo: bool,
        initial_rack: Option<Arc<Rack>>,
        initial_fx_gain: f32,
        initial_gain: f32,
        initial_pan: f32,
    ) -> Result<Self, String> {
        let in_device = config.device(true)?;
        let out_device = config.device(false)?;
        let input_device_name = in_device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Entrada predeterminada".into());

        let in_setup = choose_input_config(&in_device, config)?;
        let out_setup = choose_output_config(&out_device, config)?;
        let output_rate = out_setup.config.sample_rate;
        let fallback_buf = config.buffer_frames.unwrap_or(256);
        let in_buf = match in_setup.config.buffer_size {
            cpal::BufferSize::Fixed(n) => n,
            cpal::BufferSize::Default => fallback_buf,
        };
        let out_buf = match out_setup.config.buffer_size {
            cpal::BufferSize::Fixed(n) => n,
            cpal::BufferSize::Default => fallback_buf,
        };
        let io_buffer_frames = in_buf.saturating_add(out_buf);

        let shared = Arc::new(MonitorShared::new(stereo));
        let fx_pdc = initial_rack
            .as_ref()
            .map_or(0, |r| r.latency_samples().min(u64::from(u32::MAX)) as u32);
        shared.fx_latency_frames.store(fx_pdc, Ordering::Relaxed);
        shared.fx.set(initial_fx_gain);
        shared.fx.set_rack(initial_rack, output_rate);
        shared
            .gain
            .store(initial_gain.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
        shared.pan.set(initial_pan);

        let (producer, consumer) = RingBuffer::<[f32; 2]>::new(RING_CAPACITY_FRAMES);
        let in_channels = usize::from(in_setup.config.channels);
        let in_first = usize::from(in_setup.first_channel).min(in_channels.saturating_sub(1));
        let out_channels = usize::from(out_setup.config.channels);
        let out_first = usize::from(out_setup.first_channel);

        let input_stream = match in_setup.format {
            SampleFormat::F32 => build_in_stream::<f32>(
                &in_device,
                in_setup.config,
                in_channels,
                in_first,
                producer,
                shared.clone(),
            ),
            SampleFormat::I16 => build_in_stream::<i16>(
                &in_device,
                in_setup.config,
                in_channels,
                in_first,
                producer,
                shared.clone(),
            ),
            SampleFormat::I32 => build_in_stream::<i32>(
                &in_device,
                in_setup.config,
                in_channels,
                in_first,
                producer,
                shared.clone(),
            ),
            SampleFormat::U16 => build_in_stream::<u16>(
                &in_device,
                in_setup.config,
                in_channels,
                in_first,
                producer,
                shared.clone(),
            ),
            _ => Err("Formato de entrada no compatible para monitoreo".into()),
        }?;

        let processor = MonitorProcessor::new(consumer, &shared, output_rate);
        let output_stream = match out_setup.format {
            SampleFormat::F32 => build_out_stream::<f32>(
                &out_device,
                out_setup.config,
                out_channels,
                out_first,
                processor,
                shared.clone(),
            ),
            SampleFormat::I16 => build_out_stream::<i16>(
                &out_device,
                out_setup.config,
                out_channels,
                out_first,
                processor,
                shared.clone(),
            ),
            SampleFormat::I32 => build_out_stream::<i32>(
                &out_device,
                out_setup.config,
                out_channels,
                out_first,
                processor,
                shared.clone(),
            ),
            SampleFormat::U16 => build_out_stream::<u16>(
                &out_device,
                out_setup.config,
                out_channels,
                out_first,
                processor,
                shared.clone(),
            ),
            _ => Err("Formato de salida no compatible para monitoreo".into()),
        }?;

        input_stream
            .play()
            .map_err(|e| format!("No se pudo iniciar la entrada de monitoreo: {e}"))?;
        output_stream
            .play()
            .map_err(|e| format!("No se pudo iniciar la salida de monitoreo: {e}"))?;

        Ok(Self {
            _input_stream: input_stream,
            _output_stream: output_stream,
            shared,
            output_rate,
            io_buffer_frames,
            input_device_name,
        })
    }

    pub fn set_active(&self, active: bool) {
        self.shared.active.store(active, Ordering::Release);
    }

    pub fn set_stereo(&self, stereo: bool) {
        self.shared.stereo.store(stereo, Ordering::Release);
    }

    pub fn set_mix(&self, gain: f32, pan: f32) {
        self.shared
            .gain
            .store(gain.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
        self.shared.pan.set(pan);
    }

    pub fn set_fx(&self, rack: Option<Arc<Rack>>, fx_gain: f32) {
        let fx_pdc = rack
            .as_ref()
            .map_or(0, |r| r.latency_samples().min(u64::from(u32::MAX)) as u32);
        self.shared
            .fx_latency_frames
            .store(fx_pdc, Ordering::Relaxed);
        self.shared.fx.set(fx_gain);
        self.shared.fx.set_rack(rack, self.output_rate);
    }

    pub fn latency(&self) -> MonitorLatency {
        MonitorLatency {
            sample_rate: self.output_rate,
            io_buffer_frames: self.io_buffer_frames,
            ring_frames: self.shared.queued_frames.load(Ordering::Relaxed),
            fx_pdc_frames: self.shared.fx_latency_frames.load(Ordering::Relaxed),
        }
    }

    pub fn input_peak(&self) -> f32 {
        f32::from_bits(self.shared.in_peak.load(Ordering::Relaxed))
    }

    pub fn output_peak(&self) -> f32 {
        f32::from_bits(self.shared.out_peak.load(Ordering::Relaxed))
    }

    pub fn failed(&self) -> bool {
        self.shared.failed.load(Ordering::Relaxed)
    }
}

fn build_in_stream<T: SizedSample + Sample>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    first: usize,
    mut producer: Producer<[f32; 2]>,
    shared: Arc<MonitorShared>,
) -> Result<cpal::Stream, String>
where
    f32: cpal::FromSample<T>,
{
    let err_shared = shared.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let stereo = shared.stereo.load(Ordering::Acquire) && first + 1 < channels;
                let mut block_peak = 0.0_f32;
                for frame in data.chunks_exact(channels) {
                    let raw_l: f32 = frame[first].to_sample();
                    let raw_r: f32 = if stereo {
                        frame[first + 1].to_sample()
                    } else {
                        raw_l
                    };
                    let l = if raw_l.is_finite() { raw_l } else { 0.0 };
                    let r = if raw_r.is_finite() { raw_r } else { 0.0 };
                    block_peak = block_peak.max(l.abs().max(r.abs()));
                    let _ = producer.push([l, r]);
                }
                let prev = f32::from_bits(shared.in_peak.load(Ordering::Relaxed));
                let smoothed = block_peak.max(prev * 0.72);
                shared.in_peak.store(smoothed.to_bits(), Ordering::Relaxed);
            },
            move |err| {
                eprintln!("Error en entrada de monitoreo: {err}");
                err_shared.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| format!("No se pudo abrir la entrada para monitoreo: {e}"))
}

fn build_out_stream<T: SizedSample + Sample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    first: usize,
    mut processor: MonitorProcessor,
    shared: Arc<MonitorShared>,
) -> Result<cpal::Stream, String> {
    let err_shared = shared.clone();
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                let mut scratch = [[0.0_f32; 2]; RT_BLOCK_MAX];
                let total_frames = output.len() / channels;
                let mut offset = 0;
                while offset < total_frames {
                    let count = (total_frames - offset).min(RT_BLOCK_MAX);
                    let slice = &mut scratch[..count];
                    processor.render(&shared, slice);
                    let out_start = offset * channels;
                    let out_end = (offset + count) * channels;
                    write_stereo(&mut output[out_start..out_end], slice, channels, first);
                    offset += count;
                }
            },
            move |err| {
                eprintln!("Error en salida de monitoreo: {err}");
                err_shared.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| format!("No se pudo abrir la salida para monitoreo: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::effects::{ComponentEffect, Plan, Rack};
    use elevate::musical::fx::FxId;

    #[test]
    fn monitor_processor_routes_live_input_through_track_fx_and_applies_active_gate() {
        let (mut tx, rx) = RingBuffer::<[f32; 2]>::new(4096);
        let shared = MonitorShared::new(false);

        // Conectar Hay Echo (10 ms, 100% wet) al rack en vivo del monitor.
        let desc = Arc::new(
            elevate::scripting::audio_components::parse(include_str!(
                "../../assets/scripts/components/audio/echo.lua"
            ))
            .unwrap(),
        );
        let echo = ComponentEffect::prepare(desc, 48_000).unwrap();
        echo.set(1, 10.0).unwrap();
        echo.set(2, 10.0).unwrap();
        echo.set(7, 0.0).unwrap(); // sin wobble
        echo.set(8, 100.0).unwrap(); // 100% wet
        let rack = Rack::new(Plan::Component(FxId(501), echo, None), 0);
        shared.fx.set_rack(Some(rack), 48_000);
        shared.gain.store(0.5_f32.to_bits(), Ordering::Relaxed);

        let mut proc = MonitorProcessor::new(rx, &shared, 48_000);

        // Estabilizar la rampa del tiempo de retardo y la compuerta anti-click (~120 ms).
        let mut warmup = [[0.0_f32; 2]; 256];
        for _ in 0..24 {
            for _ in 0..256 {
                let _ = tx.push([0.0, 0.0]);
            }
            proc.render(&shared, &mut warmup);
        }

        // Inyectar impulso + silencio en el ring buffer
        tx.push([1.0, 1.0]).unwrap();
        for _ in 1..1024 {
            tx.push([0.0, 0.0]).unwrap();
        }
        let mut out = [[0.0_f32; 2]; 1024];
        for chunk in out.chunks_mut(256) {
            proc.render(&shared, chunk);
        }

        // Debe aparecer el eco procesado en ~480 muestras, atenuado por el fader (0.5).
        let echo_peak = out[475..490]
            .iter()
            .map(|f| f[0].abs())
            .fold(0.0_f32, f32::max);
        assert!(
            echo_peak > 0.12 && echo_peak < 0.65,
            "esperado eco live a través del rack del monitor, obt {echo_peak}"
        );

        // Al desactivar active (ej. pista muteada o modo Auto en Play), se silencia suavemente.
        shared.active.store(false, Ordering::Release);
        for _ in 0..512 {
            tx.push([0.8, 0.8]).unwrap();
        }
        let mut muted_out = [[0.0_f32; 2]; 512];
        proc.render(&shared, &mut muted_out[..256]);
        proc.render(&shared, &mut muted_out[256..]);
        assert!(
            muted_out[511][0].abs() < 1e-3,
            "monitor desactivado debía silenciarse, obt {:?}",
            muted_out[511]
        );
    }

    #[test]
    fn monitor_latency_reports_buffer_ring_and_fx_pdc_in_ms() {
        let lat = MonitorLatency {
            sample_rate: 48_000,
            io_buffer_frames: 256,
            ring_frames: 128,
            fx_pdc_frames: 96,
        };
        assert_eq!(lat.total_frames(), 480);
        assert!((lat.total_ms() - 10.0).abs() < 1e-4);
        assert!((lat.fx_pdc_ms() - 2.0).abs() < 1e-4);

        let (mut tx, rx) = RingBuffer::<[f32; 2]>::new(1024);
        let shared = MonitorShared::new(false);
        for _ in 0..320 {
            tx.push([0.1, 0.1]).unwrap();
        }
        let mut proc = MonitorProcessor::new(rx, &shared, 48_000);
        let mut out = [[0.0_f32; 2]; 128];
        proc.render(&shared, &mut out);
        assert_eq!(shared.queued_frames.load(Ordering::Relaxed), 320);
    }
}
