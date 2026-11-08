//! Selección compartida de formato CPAL. Abrir/configurar fuera del callback.
use cpal::{SampleFormat, traits::DeviceTrait};

fn supported_format(format: SampleFormat) -> bool {
    matches!(
        format,
        SampleFormat::F32 | SampleFormat::I16 | SampleFormat::I32 | SampleFormat::U16
    )
}

/// `None` sigue la frecuencia del dispositivo; `Some` exige la del archivo.
pub(super) fn choose_output_config(
    device: &cpal::Device,
    requested_rate: Option<u32>,
) -> Result<cpal::SupportedStreamConfig, String> {
    let rate = if let Some(rate) = requested_rate {
        rate
    } else {
        let default = device
            .default_output_config()
            .map_err(|error| error.to_string())?;
        if default.channels() == 2 && supported_format(default.sample_format()) {
            return Ok(default);
        }
        default.sample_rate()
    };
    device.supported_output_configs().map_err(|error| error.to_string())?
        .filter(|config| config.channels() == 2 && supported_format(config.sample_format()))
        .find_map(|config| config.try_with_sample_rate(rate))
        .ok_or_else(|| format!("La salida no admite estéreo a {rate} Hz; se necesita remuestreo o elegir otro dispositivo"))
}
