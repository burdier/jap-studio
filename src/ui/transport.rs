//! Atajo compartido de transporte; reutiliza los controladores de Play y Stop existentes.
mod audio;
use super::DawApp;
use gpui::KeyDownEvent;

impl DawApp {
    /// Prioridad de interpretación en superficies musicales; los campos de texto
    /// conservan sus eventos porque sus focos no llaman esta operación.
    pub(super) fn handle_performance_key(&mut self, event: &KeyDownEvent) -> bool {
        if self.handle_transport_key(event) {
            return true;
        }
        if self.command_open
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
        if modifiers.control || modifiers.alt || modifiers.platform {
            return false;
        }
        // Estos contextos usan Espacio para editar texto o activar sus propios controles.
        if self.command_open
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
            self.last_key = "space".into();
            self.toggle_transport(modifiers.shift);
        }
        true
    }

    pub(in crate::ui) fn toggle_playback(&mut self) {
        self.toggle_transport(false);
    }

    fn toggle_transport(&mut self, resume: bool) {
        if self.player.is_some()
            || self.file_playback.is_some()
            || self.is_audio_recording()
            || self.instrument.clip_playing()
            || self.sampler.clip_playing()
            || self.vst_any_clip_playing()
        {
            self.stop_transport();
        } else if resume && self.center_view == super::CenterView::Arrangement {
            self.play_audio_from(
                self.timeline
                    .last_stop_beat
                    .unwrap_or(self.timeline.edit_beat()),
            );
        } else {
            self.play_context();
        }
    }
}
