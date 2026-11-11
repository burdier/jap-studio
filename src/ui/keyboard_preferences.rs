//! Preferencia de navegación; separada del controlador MIDI y del tema.
use super::{DawApp, VimMode, profile};

pub(super) fn load_vim() -> bool {
    profile::load()
        .ok()
        .and_then(|text| {
            text.lines()
                .filter_map(|line| {
                    let (key, value) = line.split_once('=')?;
                    (key.trim() == "VIM_ENABLED").then(|| {
                        !matches!(
                            value.trim().to_ascii_lowercase().as_str(),
                            "false" | "off" | "0"
                        )
                    })
                })
                .last()
        })
        .unwrap_or(true)
}
impl DawApp {
    pub(super) fn toggle_vim(&mut self) {
        let enabled = !self.vim_enabled;
        if let Err(error) = profile::save_values(&[("VIM_ENABLED", enabled.to_string())]) {
            self.status_message = format!("No se pudo guardar la preferencia Vim: {error}");
            return;
        }
        self.vim_enabled = enabled;
        self.set_mode(VimMode::Normal);
        self.status_message = if enabled {
            "Vim activado · f: ampliar panel"
        } else {
            "Vim desactivado · ratón, flechas, Delete, MIDI y Espacio disponibles"
        }
        .into();
    }
    pub(super) fn mode_label(&self) -> &'static str {
        if !self.vim_enabled {
            return "VIM OFF";
        }
        match self.mode {
            VimMode::Normal => "NORMAL",
            VimMode::Insert => "INSERT",
            VimMode::Command => "COMMAND",
        }
    }
}
