//! Estado de los overlays de configuración y ayuda de teclado.
use super::{
    DawApp,
    cursor_fx::CursorFxMode,
    design::ThemePreset,
    i18n::{Language, set_active_language, tr, tr_fmt},
    profile,
    shortcuts::ShortcutPreset,
    views::tracks::{TcpControlStyle, TcpLayoutPreset, TcpWidget},
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum SettingsCategory {
    #[default]
    Appearance,
    Audio,
    Paths,
    Tracks,
    Export,
    MidiMap,
    Shortcuts,
}
impl SettingsCategory {
    pub const ALL: [Self; 7] = [
        Self::Appearance,
        Self::Audio,
        Self::Paths,
        Self::Tracks,
        Self::Export,
        Self::MidiMap,
        Self::Shortcuts,
    ];
    pub fn label_in(self, lang: Language) -> &'static str {
        match self {
            Self::Audio => tr(lang, "settings.cat.audio"),
            Self::Appearance => tr(lang, "settings.cat.appearance"),
            Self::Paths => tr(lang, "settings.cat.paths"),
            Self::Tracks => tr(lang, "settings.cat.tracks"),
            Self::Export => tr(lang, "settings.cat.export"),
            Self::MidiMap => tr(lang, "settings.cat.midi_map"),
            Self::Shortcuts => tr(lang, "settings.cat.shortcuts"),
        }
    }
}

#[derive(Default)]
pub(super) struct SettingsUi {
    settings_open: bool,
    pub welcome_open: bool,
    pub category: SettingsCategory,
    pub language: Language,
    pub shortcut_preset: ShortcutPreset,
    pub audio: super::audio_preferences::AudioPreferences,
    pub export: super::export_preferences::ExportPreferences,
    pub paths: elevate::paths::AppPaths,
    pub paths_picker: Option<gpui::Task<()>>,
    pub paths_save_error: Option<String>,
    appearance_save_error: Option<String>,
}

impl SettingsUi {
    pub fn new() -> Self {
        let saved_lang = Language::load_saved();
        let language = saved_lang.unwrap_or_default();
        set_active_language(language);
        Self {
            welcome_open: saved_lang.is_none(),
            language,
            shortcut_preset: ShortcutPreset::load(),
            export: super::export_preferences::ExportPreferences::load(),
            audio: super::audio_preferences::AudioPreferences::load(),
            paths: elevate::paths::AppPaths::load(),
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
        self.audio.probe = None;
        self.audio.focus_request = None;
        self.audio.selectors.clear();
        self.settings_open = false;
        self.export.editing = None;
    }
}

impl DawApp {
    pub(in crate::ui) fn tr(&self, key: &'static str) -> &'static str {
        tr(self.settings.language, key)
    }

    pub(in crate::ui) fn tr_fmt(&self, key: &'static str, args: &[(&str, &str)]) -> String {
        tr_fmt(self.settings.language, key, args)
    }

    pub(in crate::ui) fn set_language(&mut self, language: Language) {
        self.settings.language = language;
        set_active_language(language);
        match language.save() {
            Ok(()) => {
                self.status_message = format!("Idioma / Language: {} · guardado", language.label());
            }
            Err(error) => {
                self.status_message =
                    format!("Idioma: {} · solo esta sesión ({error})", language.label());
            }
        }
    }

    pub(in crate::ui) fn set_shortcut_preset(&mut self, preset: ShortcutPreset) {
        self.settings.shortcut_preset = preset;
        match preset.save() {
            Ok(()) => {
                self.status_message = format!("Atajos / Keymap: {} · guardado", preset.label());
            }
            Err(error) => {
                self.status_message =
                    format!("Atajos: {} · solo esta sesión ({error})", preset.label());
            }
        }
    }

    pub(in crate::ui) fn complete_welcome(&mut self) {
        let _ = self.settings.language.save();
        let _ = self.settings.shortcut_preset.save();
        self.settings.welcome_open = false;
        self.status_message = format!(
            "{} · {}",
            self.settings.language.label(),
            self.settings.shortcut_preset.label()
        );
    }

    pub(super) fn open_settings(&mut self) {
        self.settings.open();
    }

    pub(super) fn show_shortcuts(&mut self) {
        self.settings.show_shortcuts();
    }

    pub(super) fn open_midi_map_settings(&mut self) {
        self.settings.open();
        self.settings.category = SettingsCategory::MidiMap;
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
        self.timeline.set_scale(self.ds.timeline_pixels_per_beat);
        self.refresh_track_layout();
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

    pub(in crate::ui) fn set_font_size(&mut self, size: f32) {
        let clamped = (size * 2.0).round() * 0.5;
        let clamped = clamped.clamp(10.0, 20.0);
        self.ds.font_size_xs = clamped;
        self.save_appearance(format!(
            "Fuente: {:.1} px ({}%)",
            clamped,
            self.ds.font_scale_percent()
        ));
    }

    pub(in crate::ui) fn step_font_size(&mut self, delta: f32) {
        self.set_font_size(self.ds.font_size_xs + delta);
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

    pub(in crate::ui) fn choose_config_path_folder(
        &mut self,
        group: elevate::paths::PathGroup,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.settings.paths_picker.is_some() {
            return;
        }
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(format!("Añadir carpeta para {}", group.label()).into()),
        });
        self.settings.paths_picker = Some(cx.spawn(async move |this, cx| {
            let result = prompt.await;
            let _ = this.update(cx, |this, cx| {
                this.settings.paths_picker = None;
                if let Ok(Ok(Some(paths))) = result
                    && let Some(dir) = paths.into_iter().next()
                {
                    if this.settings.paths.add_path(group, dir) {
                        this.persist_config_paths(group);
                    } else {
                        this.status_message = "Esa carpeta ya está configurada".into();
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(in crate::ui) fn remove_config_path(
        &mut self,
        group: elevate::paths::PathGroup,
        index: usize,
    ) {
        if self.settings.paths.remove_path(group, index) {
            self.persist_config_paths(group);
        }
    }

    pub(in crate::ui) fn reset_config_path_group(&mut self, group: elevate::paths::PathGroup) {
        self.settings.paths.reset_group(group);
        self.persist_config_paths(group);
    }

    pub(in crate::ui) fn reset_all_config_paths(&mut self) {
        self.settings.paths.reset_all();
        self.persist_config_paths(elevate::paths::PathGroup::Vst3);
        self.synth_ui.vital_library_loaded = false;
    }

    fn persist_config_paths(&mut self, group: elevate::paths::PathGroup) {
        match self.settings.paths.save() {
            Ok(()) => {
                self.settings.paths_save_error = None;
                if group == elevate::paths::PathGroup::SynthPresets {
                    self.synth_ui.vital_library =
                        elevate::devices::vital_import::discover_vital_presets();
                    self.synth_ui.vital_library_loaded = true;
                }
                self.status_message = format!("Rutas · {} guardado", group.label());
            }
            Err(err) => {
                self.settings.paths_save_error = Some(err.clone());
                self.status_message = format!("No se pudo guardar paths.profile: {err}");
            }
        }
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
