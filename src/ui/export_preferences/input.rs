//! Entrada de los campos de preferencias; no llega a los atajos musicales.
use super::ExportField;
use crate::ui::{DawApp, settings::SettingsCategory};
use gpui::{Context, KeyDownEvent};

impl DawApp {
    pub(in crate::ui) fn edit_export_settings_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) {
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(&event.keystroke.key);
        let modifiers = event.keystroke.modifiers;
        if self.settings.category != SettingsCategory::Export {
            return;
        }
        if modifiers.control && key.eq_ignore_ascii_case("s") {
            self.save_export_preferences();
            return;
        }
        let draft = &mut self.settings.export;
        if key == "tab" {
            let index = draft
                .editing
                .map_or(if modifiers.shift { 0 } else { 4 }, |field| field as usize);
            draft.editing =
                Some(ExportField::ALL[(index + if modifiers.shift { 4 } else { 1 }) % 5]);
            draft.select_all = true;
            return;
        }
        if draft.editing.is_none() {
            return;
        }
        if modifiers.control && key.eq_ignore_ascii_case("a") {
            draft.select_all = true;
            return;
        }
        if modifiers.control && key.eq_ignore_ascii_case("v") {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                draft.edit(&text.replace(['\n', '\r', '\0'], " "));
            }
            return;
        }
        match key {
            "enter" => draft.editing = None,
            "backspace" => draft.edit("backspace"),
            "space" => draft.edit(" "),
            _ if key.chars().count() == 1
                && !modifiers.control
                && !modifiers.alt
                && !modifiers.platform =>
            {
                draft.edit(key)
            }
            _ => {}
        }
    }
}
