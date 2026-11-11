//! Alias pedidos por el usuario y acciones del catálogo; misma operación que el panel.
use super::{AudioChange, DawApp};
use gpui::Context;
impl DawApp {
    pub(in crate::ui) fn execute_audio_command(
        &mut self,
        command: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Ok(parts) = elevate::actions::tokenize(command) else {
            return false;
        };
        let Some(name) = parts.first() else {
            return false;
        };
        let result: Result<(), String> = match (name.as_str(), &parts[1..]) {
            ("list", [kind]) if kind == "device" => { self.open_audio_settings(cx); Ok(()) }
            ("use", [kind]) if kind == "device" => { self.open_audio_settings(cx); self.settings.audio.focus_request = Some(super::AudioField::Output); Ok(()) }
            ("use", [kind, value]) if kind == "device" => { self.use_audio_device(false, value, cx); Ok(()) }
            ("use", [kind, direction, value]) if kind == "device" && matches!(direction.as_str(), "input" | "output") => { self.use_audio_device(direction == "input", value, cx); Ok(()) }
            ("audio.settings" | "audio.devices", []) => { self.open_audio_settings(cx); Ok(()) }
            ("audio.apply", []) => { self.apply_audio_settings(cx); Ok(()) }
            ("audio.revert", []) => { self.restore_audio_settings(); self.refresh_audio_devices(cx); Ok(()) }
            ("audio.output" | "audio.input", [value]) => { self.use_audio_device(name == "audio.input", value, cx); Ok(()) }
            ("audio.driver", [value]) => { self.change_audio_setting(AudioChange::Driver(if value == "default" { None } else { Some(value.clone()) }), cx); Ok(()) }
            ("audio.rate" | "audio.buffer" | "audio.output.channel" | "audio.input.channel", [value]) => {
                match value.parse::<u32>() {
                    Ok(n) if name == "audio.rate" && (n == 0 || (8000..=384000).contains(&n)) => { self.change_audio_setting(AudioChange::Rate(if n == 0 { None } else { Some(n) }), cx); self.apply_audio_settings(cx); Ok(()) }
                    Ok(n) if name == "audio.buffer" && n <= 8192 => { self.change_audio_setting(AudioChange::Buffer(if n == 0 { None } else { Some(n) }), cx); self.apply_audio_settings(cx); Ok(()) }
                    Ok(n) if name.ends_with(".channel") && (1..=255).contains(&n) => { self.change_audio_setting(if name == "audio.output.channel" { AudioChange::OutputPair(n as u16 - 1) } else { AudioChange::InputChannel(n as u16 - 1) }, cx); self.apply_audio_settings(cx); Ok(()) }
                    _ => Err("Valor de audio fuera de rango; 0 = Auto para frecuencia/buffer; canales desde 1".into()),
                }
            }
            ("audio.test.output", []) => { self.open_settings(); self.settings.category = super::super::settings::SettingsCategory::Audio; self.toggle_audio_probe(false, cx); Ok(()) }
            ("audio.test.input", []) => { self.open_settings(); self.settings.category = super::super::settings::SettingsCategory::Audio; self.toggle_audio_probe(true, cx); Ok(()) }
            _ if name.starts_with("audio.") || matches!(name.as_str(), "list" | "use") => Err("Uso: list device · use device 'nombre, ID o número' · use device input 'nombre'".into()),
            _ => return false,
        };
        if let Err(error) = result {
            self.audio_message(false, error);
        }
        cx.notify();
        true
    }
}
