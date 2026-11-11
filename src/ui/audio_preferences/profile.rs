//! Persistencia en el perfil existente, sin tocar las claves de apariencia/exportación.
use super::{AudioConfig, AudioPreferences};
use crate::ui::profile;
impl AudioPreferences {
    pub fn load() -> Self {
        let result = (|| {
            let text = profile::load().unwrap_or_default();
            let mut config = AudioConfig::default();
            for (key, value) in text.lines().filter_map(|line| line.split_once('=')) {
                let value = value.trim();
                let optional = || {
                    if value.is_empty() || value == "default" {
                        None
                    } else {
                        Some(value.to_string())
                    }
                };
                let number = || {
                    value
                        .parse::<u32>()
                        .map(|v| if v == 0 { None } else { Some(v) })
                        .map_err(|_| format!("Valor inválido para {key}"))
                };
                match key.trim() {
                    "AUDIO_HOST" => config.host = optional(),
                    "AUDIO_OUTPUT" => config.output = optional(),
                    "AUDIO_INPUT" => config.input = optional(),
                    "AUDIO_RATE" => config.sample_rate = number()?,
                    "AUDIO_BUFFER" => config.buffer_frames = number()?,
                    "AUDIO_OUTPUT_FIRST" => {
                        config.output_pair = value
                            .parse::<u16>()
                            .ok()
                            .filter(|v| *v > 0 && *v < 256)
                            .ok_or("Canal de salida inválido")?
                            - 1
                    }
                    "AUDIO_INPUT_CHANNEL" => {
                        config.input_channel = value
                            .parse::<u16>()
                            .ok()
                            .filter(|v| *v > 0 && *v <= 256)
                            .ok_or("Canal de entrada inválido")?
                            - 1
                    }
                    _ => {}
                }
            }
            Ok::<_, String>(config)
        })();
        match result {
            Ok(config) => Self {
                saved: config.clone(),
                draft: config,
                ..Self::default()
            },
            Err(error) => Self {
                message: Some((false, error)),
                ..Self::default()
            },
        }
    }
}
pub(super) fn save(config: &AudioConfig) -> Result<(), String> {
    let values = [
        ("AUDIO_HOST", config.host.clone().unwrap_or_default()),
        ("AUDIO_OUTPUT", config.output.clone().unwrap_or_default()),
        ("AUDIO_INPUT", config.input.clone().unwrap_or_default()),
        ("AUDIO_RATE", config.sample_rate.unwrap_or(0).to_string()),
        (
            "AUDIO_BUFFER",
            config.buffer_frames.unwrap_or(0).to_string(),
        ),
        ("AUDIO_OUTPUT_FIRST", (config.output_pair + 1).to_string()),
        (
            "AUDIO_INPUT_CHANNEL",
            (config.input_channel + 1).to_string(),
        ),
    ];
    if values.iter().any(|(_, value)| value.contains(['\n', '\r'])) {
        return Err("Identificador de dispositivo inválido".into());
    }
    profile::save_values(&values).map_err(|e| e.to_string())
}
