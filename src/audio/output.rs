//! Negociación y ruteo compartidos. Consultar dispositivos siempre fuera del callback.
use super::device::AudioConfig;
use cpal::{BufferSize, SampleFormat, SupportedBufferSize, traits::DeviceTrait};

pub(super) fn supported_format(format: SampleFormat) -> bool {
    matches!(
        format,
        SampleFormat::F32 | SampleFormat::I16 | SampleFormat::I32 | SampleFormat::U16
    )
}
pub(crate) struct StreamSetup {
    pub config: cpal::StreamConfig,
    pub format: SampleFormat,
    pub first_channel: u16,
}

pub(super) fn choose_output_config(
    device: &cpal::Device,
    options: &AudioConfig,
) -> Result<StreamSetup, String> {
    choose_config(device, options, false)
}
pub(super) fn choose_input_config(
    device: &cpal::Device,
    options: &AudioConfig,
) -> Result<StreamSetup, String> {
    choose_config(device, options, true)
}
fn choose_config(
    device: &cpal::Device,
    options: &AudioConfig,
    input: bool,
) -> Result<StreamSetup, String> {
    let rate = match options.sample_rate {
        Some(rate) => rate,
        None => if input {
            device.default_input_config()
        } else {
            device.default_output_config()
        }
        .map_err(|e| e.to_string())?
        .sample_rate(),
    };
    let first = if input {
        options.input_channel
    } else {
        options.output_pair
    };
    let needed = first
        .checked_add(if input { 1 } else { 2 })
        .ok_or("Canales fuera de rango")?;
    let ranges: Vec<cpal::SupportedStreamConfigRange> = if input {
        device
            .supported_input_configs()
            .map_err(|e| e.to_string())?
            .collect()
    } else {
        device
            .supported_output_configs()
            .map_err(|e| e.to_string())?
            .collect()
    };
    let mut choices: Vec<_> = ranges
        .into_iter()
        .filter(|r| {
            r.channels() >= needed
                && r.channels() <= 256
                && supported_format(r.sample_format())
                && rate >= r.min_sample_rate()
                && rate <= r.max_sample_rate()
                && options.buffer_frames.is_none_or(|size| {
                    size > 0
                        && match r.buffer_size() {
                            SupportedBufferSize::Range { min, max } => size >= *min && size <= *max,
                            SupportedBufferSize::Unknown => false,
                        }
                })
        })
        .collect();
    choices.sort_by_key(|r| (r.channels(), r.sample_format() != SampleFormat::F32));
    let range = choices.first().ok_or_else(|| format!("La {} no admite {} Hz, canal {} y el buffer elegido. Prueba Auto o revisa sus capacidades.", if input { "entrada" } else { "salida" }, rate, needed))?;
    let supported = range.clone().with_sample_rate(rate);
    let format = supported.sample_format();
    let mut config = supported.config();
    config.buffer_size = options
        .buffer_frames
        .map_or(BufferSize::Default, BufferSize::Fixed);
    Ok(StreamSetup {
        config,
        format,
        first_channel: first,
    })
}

/// Techo suave transparente por encima de -0.5 dBFS: evita recorte digital duro
/// sin alterar la señal por debajo de ±0.94.
#[inline]
fn soft_ceiling(sample: f32) -> f32 {
    if !sample.is_finite() {
        return 0.0;
    }
    const KNEE: f32 = 0.94;
    let abs = sample.abs();
    if abs <= KNEE {
        sample
    } else {
        let excess = (abs - KNEE) / (1.0 - KNEE);
        sample.signum() * (KNEE + (1.0 - KNEE) * excess.tanh())
    }
}

/// Distribuye el master estéreo a la pareja elegida; silencia las demás salidas.
/// Sin reservas, conversión y ruteo acotados por el bloque recibido.
pub(super) fn write_stereo<T: cpal::SizedSample + cpal::FromSample<f32>>(
    output: &mut [T],
    frames: &[[f32; 2]],
    channels: usize,
    first: usize,
) {
    output.fill(T::from_sample(0.0));
    if channels < 2 || first + 1 >= channels {
        return;
    }
    for (destination, frame) in output.chunks_exact_mut(channels).zip(frames) {
        destination[first] = T::from_sample(soft_ceiling(frame[0]));
        destination[first + 1] = T::from_sample(soft_ceiling(frame[1]));
    }
}
