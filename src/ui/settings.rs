//! Estado de los overlays de configuración y ayuda de teclado.
use super::{
    DawApp,
    cursor_fx::CursorFxMode,
    design::ThemePreset,
    views::tracks::{TcpControlStyle, TcpLayoutPreset, TcpWidget},
};

#[derive(Default)]
pub(super) struct SettingsUi {
    settings_open: bool,
    shortcuts_open: bool,
}

impl SettingsUi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_visible(&self) -> bool {
        self.settings_open || self.shortcuts_open
    }

    pub fn shortcuts_open(&self) -> bool {
        self.shortcuts_open
    }

    fn open(&mut self) {
        self.settings_open = true;
        self.shortcuts_open = false;
    }

    fn show_shortcuts(&mut self) {
        self.settings_open = false;
        self.shortcuts_open = true;
    }

    fn close(&mut self) {
        self.settings_open = false;
        self.shortcuts_open = false;
    }
}

impl DawApp {
    pub(super) fn open_settings(&mut self) {
        self.settings.open();
    }

    pub(super) fn show_shortcuts(&mut self) {
        self.settings.show_shortcuts();
    }

    pub(super) fn close_settings_overlay(&mut self) {
        self.settings.close();
    }

    pub(super) fn handle_settings_key(&mut self, key: &str) {
        if key == "escape" {
            self.close_settings_overlay();
        }
    }

    pub(super) fn set_theme(&mut self, theme: ThemePreset) {
        self.ds.apply_theme(theme);
        self.status_message = format!("Tema: {} ({})", theme.label(), theme.token());
    }

    pub(super) fn cycle_theme(&mut self) {
        let all = ThemePreset::ALL;
        let idx = all.iter().position(|t| *t == self.ds.theme).unwrap_or(0);
        let next = all[(idx + 1) % all.len()];
        self.set_theme(next);
    }

    pub(super) fn set_cursor_fx(&mut self, mode: CursorFxMode) {
        self.ds.cursor_fx = mode;
        self.cursor_fx.set_mode(mode);
        self.status_message = format!("Puntero GPU: {} ({})", mode.label(), mode.token());
    }

    pub(super) fn set_tcp_preset(&mut self, preset: TcpLayoutPreset) {
        self.ds.track_header.set_preset_preserving_style(preset);
        self.status_message = format!("TCP Header: {} ({})", preset.label(), preset.token());
    }

    pub(super) fn set_tcp_control_style(&mut self, style: TcpControlStyle) {
        self.ds.track_header.set_control_style(style);
        self.status_message = format!("Controles TCP: {} ({})", style.label(), style.token());
    }

    pub(super) fn toggle_tcp_widget(&mut self, widget: TcpWidget) {
        self.ds.track_header.toggle_widget(widget);
        let state = if self.ds.track_header.is_visible(widget) {
            "visible"
        } else {
            "oculto"
        };
        self.status_message = format!("TCP {}: {state}", widget.label());
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsUi;

    #[test]
    fn settings_opens_then_shortcut_help_replaces_it_and_escape_closes_both() {
        let mut settings = SettingsUi::new();
        assert!(!settings.is_visible());
        settings.open();
        assert!(settings.is_visible());
        assert!(!settings.shortcuts_open());
        settings.show_shortcuts();
        assert!(settings.is_visible());
        assert!(settings.shortcuts_open());
        settings.close();
        assert!(!settings.is_visible());
    }
}
