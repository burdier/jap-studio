//! Búsquedas independientes y asignación compartida por ratón y teclado.
use super::ToolbarAction;
use crate::ui::{DawApp, icons::ToolbarBindings};
use gpui::KeyDownEvent;

impl ToolbarBindings {
    pub fn can_bind_selected(&self) -> bool {
        self.icons.get(self.selected_icon).is_some()
            && if self.editing_custom_command {
                let command = self.custom_command.trim().trim_start_matches(':').trim();
                !command.is_empty() && command.len() <= 256 && !command.contains(['\n', '\r', '\0'])
            } else {
                ToolbarAction::available(self.picker_scope, &self.action_query)
                    .contains(&self.selected_action)
            }
    }

    pub fn edit_search(&mut self, key: &str) {
        let query = if self.searching_actions {
            &mut self.action_query
        } else {
            &mut self.search_query
        };
        if key == "backspace" {
            query.pop();
        } else {
            query.push_str(key);
        }
        if self.searching_actions {
            let actions = ToolbarAction::available(self.picker_scope, &self.action_query);
            if !actions.contains(&self.selected_action) {
                if let Some(action) = actions.first() {
                    self.selected_action = *action;
                }
            }
        }
    }

    pub fn edit_custom_command(&mut self, text: &str) {
        self.editing_custom_command = true;
        if text == "backspace" {
            self.custom_command.pop();
        } else {
            self.custom_command.push_str(text);
        }
    }
}

impl DawApp {
    pub(in crate::ui) fn assign_toolbar_icon(&mut self) {
        if !self.toolbar_icons.can_bind_selected() {
            return;
        }
        if self.toolbar_icons.editing_custom_command {
            let Some((scope, icon, command)) = self.toolbar_icons.bind_custom_command() else {
                self.status_message = "Escribe un comando (máximo 256 caracteres)".into();
                return;
            };
            self.toolbar_icons.picker_open = false;
            self.status_message = format!(
                "Icono «{icon}» vinculado al comando «{command}» en {}",
                scope.label()
            );
            if let Err(error) = self.toolbar_icons.save_bindings() {
                self.status_message
                    .push_str(&format!(" · no se pudo guardar: {error}"));
            }
            return;
        }
        let (scope, icon, action) = self.toolbar_icons.bind_selected();
        self.toolbar_icons.picker_open = false;
        self.status_message = format!("Icono «{icon}» vinculado a «{action}» en {}", scope.label());
        if let Err(error) = self.toolbar_icons.save_bindings() {
            self.status_message
                .push_str(&format!(" · no se pudo guardar: {error}"));
        }
    }

    pub(in crate::ui) fn handle_icon_picker_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut gpui::Context<Self>,
    ) {
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(&event.keystroke.key);
        let modifiers = &event.keystroke.modifiers;
        if self.toolbar_icons.editing_custom_command {
            if modifiers.control && key.eq_ignore_ascii_case("v") {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.toolbar_icons
                        .edit_custom_command(&text.replace(['\n', '\r'], " "));
                }
                return;
            }
            match key {
                "escape" => self.toolbar_icons.picker_open = false,
                "enter" => self.assign_toolbar_icon(),
                "backspace" => self.toolbar_icons.edit_custom_command("backspace"),
                _ if key.chars().count() == 1
                    && !modifiers.control
                    && !modifiers.alt
                    && !modifiers.platform =>
                {
                    self.toolbar_icons.edit_custom_command(key);
                }
                _ => {}
            }
            return;
        }
        match key {
            "escape" => self.toolbar_icons.picker_open = false,
            "tab" => self.toolbar_icons.searching_actions = !self.toolbar_icons.searching_actions,
            "enter" => self.assign_toolbar_icon(),
            "backspace" => self.toolbar_icons.edit_search(key),
            _ if key.chars().count() == 1
                && !modifiers.control
                && !modifiers.alt
                && !modifiers.platform =>
            {
                self.toolbar_icons.edit_search(key);
            }
            _ => {}
        }
    }
}
