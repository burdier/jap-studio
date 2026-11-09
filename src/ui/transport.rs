//! Atajo compartido de transporte; reutiliza los controladores de Play y Stop existentes.
use super::{DawApp, VimMode};
use gpui::KeyDownEvent;

impl DawApp {
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
            || self.instrument.clip_playing()
            || self.sampler.clip_playing()
            || self.pattern_player.is_some()
        {
            self.stop_all();
            self.status_message = "Detenido".into();
        } else {
            self.play_context();
        }
    }
}
