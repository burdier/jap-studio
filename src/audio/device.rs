//! Configuración de dispositivos fuera de RT; IDs CPAL estables, sin GPUI.
#[path = "device/aliases.rs"]
mod aliases;
#[path = "device/inventory.rs"]
mod inventory;
use cpal::traits::{DeviceTrait, HostTrait};
pub(crate) use inventory::{DeviceInventory, scan};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AudioConfig {
    /// None sigue el backend del sistema. Los IDs incluyen el backend.
    pub host: Option<String>,
    pub output: Option<String>,
    pub input: Option<String>,
    pub sample_rate: Option<u32>,
    pub buffer_frames: Option<u32>,
    /// Primer canal físico (base cero) de la pareja estéreo.
    pub output_pair: u16,
    pub input_channel: u16,
}

impl AudioConfig {
    pub fn host(&self) -> Result<cpal::Host, String> {
        match &self.host {
            None => {
                // Conserva perfiles antiguos que ya seleccionaron un dispositivo ALSA.
                if let Some(selected) = self.output.as_ref().or(self.input.as_ref()) {
                    let id: cpal::DeviceId =
                        selected.parse().map_err(|e: cpal::Error| e.to_string())?;
                    return cpal::host_from_id(id.host()).map_err(|e| e.to_string());
                }
                #[cfg(target_os = "linux")]
                if cpal::available_hosts().contains(&cpal::HostId::PipeWire) {
                    return cpal::host_from_id(cpal::HostId::PipeWire).map_err(|e| e.to_string());
                }
                Ok(cpal::default_host())
            }
            Some(name) => cpal::available_hosts()
                .into_iter()
                .find(|id| id.name().eq_ignore_ascii_case(name))
                .ok_or_else(|| format!("Backend de audio no disponible: {name}"))
                .and_then(|id| cpal::host_from_id(id).map_err(|e| e.to_string())),
        }
    }
    pub fn device(&self, input: bool) -> Result<cpal::Device, String> {
        let host = self.host()?;
        let selected = if input { &self.input } else { &self.output };
        if let Some(selected) = selected {
            let id: cpal::DeviceId = selected.parse().map_err(|e: cpal::Error| e.to_string())?;
            if id.host() != host.id() {
                return Err("La interfaz pertenece a otro backend".into());
            }
            host.device_by_id(&id)
                .ok_or_else(|| format!("Interfaz desconectada o no disponible: {selected}"))
        } else if input {
            host.default_input_device()
                .ok_or("No hay entrada predeterminada".into())
        } else {
            host.default_output_device()
                .ok_or("No hay salida predeterminada".into())
        }
    }
    pub fn validate(&self) -> Result<String, String> {
        let device = self.device(false)?;
        let output = super::output::choose_output_config(&device, self)?;
        if self.input.is_some() {
            super::output::choose_input_config(&self.device(true)?, self)?;
        }
        let name = device
            .description()
            .map_err(|e| e.to_string())?
            .name()
            .to_string();
        Ok(format!(
            "{name} · {} Hz · salida {}/{}",
            output.config.sample_rate,
            self.output_pair + 1,
            self.output_pair + 2
        ))
    }
}
