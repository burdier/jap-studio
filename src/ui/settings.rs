//! Estado de los overlays de configuración y ayuda de teclado.
use super::{
    DawApp,
    cursor_fx::CursorFxMode,
    design::ThemePreset,
    profile,
    views::tracks::{TcpControlStyle, TcpLayoutPreset, TcpWidget},
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum SettingsCategory {
    #[default]
    Appearance,
    Tracks,
    Export,
    Shortcuts,
}
impl SettingsCategory {
    pub const ALL: [Self; 4] = [
        Self::Appearance,
        Self::Tracks,
        Self::Export,
        Self::Shortcuts,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Apariencia",
            Self::Tracks => "Pistas",
            Self::Export => "Exportación",
            Self::Shortcuts => "Atajos",
        }
    }
}

#[derive(Default)]
pub(super) struct SettingsUi {
    settings_open: bool,
    pub category: SettingsCategory,
    pub export: super::export_preferences::ExportPreferences,
    appearance_save_error: Option<String>,
}

impl SettingsUi {
    pub fn new() -> Self {
        Self {
            export: super::export_preferences::ExportPreferences::load(),
            ..Self::default()
        }
    }

    pub fn is_visible(&self) -> bool {
        self.settings_open
    }

    #[cfg(test)]
    pub fn shortcuts_open(&self) -> bool {
        self.settings_open && self.category == SettingsCategory::Shortcuts
    }

    pub fn appearance_save_error(&self) -> Option<&str> {
        self.appearance_save_error.as_deref()
    }

    fn open(&mut self) {
        self.settings_open = true;
        self.export.editing = None;
    }

    fn show_shortcuts(&mut self) {
        self.settings_open = true;
        self.category = SettingsCategory::Shortcuts;
        self.export.editing = None;
    }

    fn close(&mut self) {
        self.settings_open = false;
        self.export.editing = None;
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

    pub(super) fn handle_settings_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut gpui::Context<Self>,
    ) {
        if event.keystroke.key == "escape" {
            if self.settings.export.editing.take().is_none() {
                self.close_settings_overlay();
            }
        } else {
            self.edit_export_settings_key(event, cx);
        }
    }

    pub(super) fn set_theme(&mut self, theme: ThemePreset) {
        self.ds.apply_theme(theme);
        self.save_appearance(format!("Tema: {}", theme.label()));
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
        self.save_appearance(format!("Cursor: {}", mode.label()));
    }

    pub(in crate::ui) fn set_selection_contrast(&mut self, value: f32) {
        self.ds.selection_contrast = value.clamp(0.0, 0.4);
        self.save_appearance(format!("Contraste de selección: {:.0}%", value * 100.0));
    }

    fn save_appearance(&mut self, description: String) {
        match profile::save_appearance(&self.ds) {
            Ok(()) => {
                self.settings.appearance_save_error = None;
                self.status_message = format!("{description} · guardado");
            }
            Err(error) => {
                let message = format!("No se pudo guardar la apariencia: {error}");
                self.settings.appearance_save_error = Some(message.clone());
                self.status_message = format!("{description} · solo esta sesión. {message}");
            }
        }
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
        let mut settings = SettingsUi::default();
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
