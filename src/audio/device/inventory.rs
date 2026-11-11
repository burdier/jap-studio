//! Descubrimiento y capacidades: ejecutar en worker, nunca durante render/RT.
use super::AudioConfig;
use cpal::{
    DeviceDirection, SupportedBufferSize, SupportedStreamConfigRange,
    traits::{DeviceTrait, HostTrait},
};

#[derive(Clone, Debug)]
pub(crate) struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub address: Option<String>,
    pub direction: DeviceDirection,
    pub input_error: Option<String>,
    pub output_error: Option<String>,
    pub inputs: Vec<SupportedStreamConfigRange>,
    pub outputs: Vec<SupportedStreamConfigRange>,
}
impl DeviceInfo {
    pub fn matches_id(&self, id: &str) -> bool {
        self.id == id || self.aliases.iter().any(|alias| alias == id)
    }

    pub fn shared(&self) -> bool {
        self.id.starts_with("pipewire:")
    }

    pub fn same_output_group(&self, other: &Self) -> bool {
        self.shared() && other.shared() && self.address.is_some() && self.address == other.address
    }

    pub fn output_label(&self, first: u16) -> String {
        if self.shared() && self.channels(false) == 2 {
            self.name.clone()
        } else {
            format!(
                "Salida {} · canales {} / {}",
                first / 2 + 1,
                first + 1,
                first + 2
            )
        }
    }

    pub fn output_pairs(&self) -> impl Iterator<Item = u16> {
        (0..self.channels(false).min(256).saturating_sub(1)).step_by(2)
    }

    /// Enumeration metadata remains usable when another stream owns the hardware.
    pub fn supports(&self, input: bool) -> bool {
        self.channels(input) > 0
            || match self.direction {
                DeviceDirection::Input => input,
                DeviceDirection::Output => !input,
                _ => true,
            }
    }
    pub fn error(&self, input: bool) -> Option<&str> {
        if input {
            self.input_error.as_deref()
        } else {
            self.output_error.as_deref()
        }
    }
    pub fn status(&self, input: bool) -> String {
        if let Some(error) = self.error(input) {
            if error.to_lowercase().contains("busy") {
                "Ocupada · vuelve a actualizar cuando esté libre".into()
            } else {
                format!("No se pudieron consultar capacidades: {error}")
            }
        } else {
            format!("{} canales", self.channels(input))
        }
    }

    pub fn channels(&self, input: bool) -> u16 {
        self.ranges(input)
            .iter()
            .map(|r| r.channels())
            .max()
            .unwrap_or(0)
    }
    pub fn ranges(&self, input: bool) -> &[SupportedStreamConfigRange] {
        if input { &self.inputs } else { &self.outputs }
    }
    pub fn rates(&self, input: bool) -> Vec<u32> {
        let mut rates: Vec<_> = [22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000]
            .into_iter()
            .filter(|rate| {
                self.ranges(input)
                    .iter()
                    .any(|r| *rate >= r.min_sample_rate() && *rate <= r.max_sample_rate())
            })
            .collect();
        for range in self.ranges(input) {
            rates.extend([range.min_sample_rate(), range.max_sample_rate()]);
        }
        rates.retain(|rate| (8000..=384000).contains(rate));
        rates.sort_unstable();
        rates.dedup();
        rates
    }
    pub fn buffers(&self, rate: Option<u32>, pair: u16) -> Vec<u32> {
        [32, 64, 128, 256, 512, 1024, 2048]
            .into_iter()
            .filter(|size| {
                self.outputs.iter().any(|r| {
                    r.channels() >= pair.saturating_add(2)
                        && rate.is_none_or(|rate| {
                            rate >= r.min_sample_rate() && rate <= r.max_sample_rate()
                        })
                        && match r.buffer_size() {
                            SupportedBufferSize::Range { min, max } => size >= min && size <= max,
                            SupportedBufferSize::Unknown => false,
                        }
                })
            })
            .collect()
    }
}
#[derive(Default)]
pub(crate) struct DeviceInventory {
    pub hosts: Vec<String>,
    pub host: String,
    pub devices: Vec<DeviceInfo>,
    pub default_output: Option<String>,
    pub default_input: Option<String>,
}
impl DeviceInventory {
    pub fn selected(&self, config: &AudioConfig, input: bool) -> Option<&DeviceInfo> {
        let id = if input {
            config.input.as_ref().or(self.default_input.as_ref())
        } else {
            config.output.as_ref().or(self.default_output.as_ref())
        }?;
        self.devices.iter().find(|d| d.matches_id(id))
    }
    pub fn resolve(&self, value: &str, input: bool) -> Result<Option<String>, String> {
        if value.eq_ignore_ascii_case("default") {
            return Ok(None);
        }
        let by_index = value
            .parse::<usize>()
            .ok()
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| self.devices.get(index));
        let matches: Vec<_> = self
            .devices
            .iter()
            .filter(|d| d.matches_id(value) || d.name.eq_ignore_ascii_case(value))
            .collect();
        let device = if let Some(d) = by_index {
            d
        } else {
            match matches.as_slice() {
                [device] => device,
                [] => return Err("Interfaz no encontrada; usa :list device".into()),
                _ => return Err("Nombre ambiguo: usa el número o ID de :list device".into()),
            }
        };
        if !device.supports(input) {
            return Err(if input {
                "Esta interfaz no ofrece entrada"
            } else {
                "Esta interfaz no ofrece salida"
            }
            .into());
        }
        Ok(Some(device.id.clone()))
    }
}
pub(crate) fn scan(config: &AudioConfig) -> Result<DeviceInventory, String> {
    let host = config.host()?;
    let mut devices = Vec::new();
    for device in host.devices().map_err(|e| e.to_string())? {
        let Ok(id) = device.id() else {
            continue;
        };
        let description = device.description().ok();
        let name = description
            .as_ref()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|| id.to_string());
        let direction = description
            .as_ref()
            .map(|d| d.direction())
            .unwrap_or(DeviceDirection::Unknown);
        let inputs = device
            .supported_input_configs()
            .map(|v| v.collect::<Vec<_>>());
        let outputs = device
            .supported_output_configs()
            .map(|v| v.collect::<Vec<_>>());
        devices.push(DeviceInfo {
            id: id.to_string(),
            aliases: Vec::new(),
            address: description
                .as_ref()
                .and_then(|d| d.address())
                .map(str::to_owned),
            name,
            direction,
            input_error: inputs.as_ref().err().map(ToString::to_string),
            output_error: outputs.as_ref().err().map(ToString::to_string),
            inputs: inputs.unwrap_or_default(),
            outputs: outputs.unwrap_or_default(),
        });
    }
    let mut devices = super::aliases::consolidate(devices);
    // CPAL también enumera los nodos internos de un split ALSA de PipeWire.
    // Conservar las salidas nombradas; un nodo elegido explícitamente sigue visible.
    let split_parents: Vec<_> = devices
        .iter()
        .filter(|d| {
            (d.id.starts_with("pipewire:alsa_output.hw_")
                || d.id.starts_with("pipewire:alsa_input.hw_"))
                && config.output.as_ref() != Some(&d.id)
                && config.input.as_ref() != Some(&d.id)
                && d.address.is_some()
                && devices.iter().any(|child| {
                    child.shared()
                        && child.address == d.address
                        && child.id != d.id
                        && !child.id.starts_with("pipewire:alsa_output.hw_")
                        && !child.id.starts_with("pipewire:alsa_input.hw_")
                        && child.supports(false) == d.supports(false)
                })
        })
        .map(|d| d.id.clone())
        .collect();
    devices.retain(|d| !split_parents.contains(&d.id));
    devices.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    devices.dedup_by(|a, b| a.id == b.id);
    Ok(DeviceInventory {
        hosts: cpal::available_hosts()
            .iter()
            .map(|id| id.name().to_string())
            .collect(),
        host: host.id().name().into(),
        devices,
        default_output: host
            .default_output_device()
            .and_then(|d| d.id().ok())
            .map(|id| id.to_string()),
        default_input: host
            .default_input_device()
            .and_then(|d| d.id().ok())
            .map(|id| id.to_string()),
    })
}
