//! Entidades Select por ventana; cambios de valor pasan por el controlador común.
use super::{AudioChange, AudioField};
use crate::ui::{
    DawApp,
    components::select::{SelectControl, SelectItems, SelectOption},
    settings::SettingsCategory,
};
use gpui::{Context, Subscription, Window, component::select::SelectEvent};

pub(in crate::ui) struct AudioSelector {
    pub field: AudioField,
    pub label: &'static str,
    pub placeholder: String,
    pub control: SelectControl<AudioChange>,
    _subscription: Subscription,
}
#[derive(Default)]
pub(in crate::ui) struct AudioSelectors {
    pub entries: Vec<AudioSelector>,
}
impl DawApp {
    pub(in crate::ui) fn sync_audio_selectors(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.settings.is_visible() || self.settings.category != SettingsCategory::Audio {
            return;
        }
        let id = window.window_handle().window_id();
        let mut controls = self
            .settings
            .audio
            .selectors
            .remove(&id)
            .unwrap_or_default();
        for (field, label, placeholder, choices) in self.audio_selector_models() {
            let selected = choices
                .iter()
                .find(|(_, _, checked)| *checked)
                .map(|(_, change, _)| change.clone());
            let options = choices
                .into_iter()
                .map(|(label, value, _)| {
                    let display_label = match &value {
                        AudioChange::Input(Some(id)) | AudioChange::Output(Some(id)) => self
                            .settings
                            .audio
                            .inventory
                            .devices
                            .iter()
                            .find(|d| &d.id == id)
                            .map(|d| d.name.clone()),
                        _ => None,
                    };
                    SelectOption {
                        label,
                        value,
                        display_label,
                    }
                })
                .collect();
            let index = controls
                .entries
                .iter()
                .position(|e| e.field == field)
                .unwrap_or_else(|| {
                    let control = SelectControl::new(window, cx);
                    let subscription = cx.subscribe_in(
                        &control.state,
                        window,
                        |this, _, event: &SelectEvent<SelectItems<AudioChange>>, _, cx| {
                            if let SelectEvent::Confirm(Some(value)) = event {
                                this.change_audio_setting(value.clone(), cx);
                            }
                        },
                    );
                    controls.entries.push(AudioSelector {
                        field,
                        label,
                        placeholder: placeholder.clone(),
                        control,
                        _subscription: subscription,
                    });
                    controls.entries.len() - 1
                });
            let entry = &mut controls.entries[index];
            entry.placeholder = placeholder;
            entry.control.sync(options, selected.as_ref(), window, cx);
            if self.settings.audio.focus_request == Some(field) {
                entry.control.focus(window, cx);
                self.settings.audio.focus_request = None;
            }
        }
        self.settings.audio.selectors.insert(id, controls);
    }
    pub(in crate::ui) fn audio_select_popup_focused(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> bool {
        self.settings
            .audio
            .selectors
            .get(&window.window_handle().window_id())
            .is_some_and(|controls| {
                controls
                    .entries
                    .iter()
                    .any(|e| e.control.popup_focused(window, cx))
            })
    }
    pub(in crate::ui) fn tab_audio_select(
        &self,
        backwards: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.settings.is_visible() || self.settings.category != SettingsCategory::Audio {
            return false;
        }
        if let Some(controls) = self
            .settings
            .audio
            .selectors
            .get(&window.window_handle().window_id())
        {
            let len = controls.entries.len();
            if len == 0 {
                return false;
            }
            let current = controls
                .entries
                .iter()
                .position(|e| e.control.focused(window, cx));
            let index = match (current, backwards) {
                (Some(i), true) => (i + len - 1) % len,
                (Some(i), false) => (i + 1) % len,
                (None, true) => len - 1,
                (None, false) => 0,
            };
            controls.entries[index].control.focus(window, cx);
            return true;
        }
        false
    }
}
