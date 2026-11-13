//! Atajo compartido de transporte; reutiliza los controladores de Play y Stop existentes.
mod audio;
use super::{DawApp, VimMode};
use gpui::KeyDownEvent;

impl DawApp {
    /// Prioridad de interpretación en superficies musicales; los campos de texto
    /// conservan sus eventos porque sus focos no llaman esta operación.
    pub(super) fn handle_performance_key(&mut self, event: &KeyDownEvent) -> bool {
        if self.handle_transport_key(event) {
            return true;
        }
        if self.mode == VimMode::Command
            || self.action_form.is_some()
            || self.palette.is_open()
            || self.settings.is_visible()
            || self.toolbar_icons.picker_open
            || self.clip_menu.is_some()
            || self.track_menu.is_some()
            || self.fx.menu.is_some()
        {
            return false;
        }
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return false;
        }
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(&event.keystroke.key);
        if !self.controller.enabled() && key != elevate::musical::controller::TOGGLE_KEY {
            return false;
        }
        self.controller_key_down(key, event.is_held)
    }

    /// Devuelve true si Espacio pertenece al transporte, incluidas las repeticiones ignoradas.
    pub(super) fn handle_transport_key(&mut self, event: &KeyDownEvent) -> bool {
        let key = &event.keystroke;
        if !matches!(key.key.as_str(), " " | "space") && key.key_char.as_deref() != Some(" ") {
            return false;
        }
        let modifiers = key.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.shift {
            return false;
        }
        // Estos contextos usan Espacio para editar texto o activar sus propios controles.
        if self.mode == VimMode::Command
            || self.action_form.is_some()
            || self.palette.is_open()
            || self.settings.is_visible()
            || self.toolbar_icons.picker_open
            || self.clip_menu.is_some()
            || self.track_menu.is_some()
            || self.fx.menu.is_some()
        {
            return false;
        }
        if !event.is_held {
            self.track_delete_pending = false;
            self.last_key = "space".into();
            self.toggle_playback();
        }
        true
    }

    fn toggle_playback(&mut self) {
        if self.player.is_some()
            || self.file_playback.is_some()
            || self.instrument.clip_playing()
            || self.sampler.clip_playing()
            || self.vst.clip_playing()
            || self.pattern_player.is_some()
        {
            self.stop_all();
            self.status_message = "Detenido".into();
        } else {
            self.play_context();
        }
    }
}
