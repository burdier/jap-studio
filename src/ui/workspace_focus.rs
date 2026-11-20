//! Ampliación visual reversible. No cambia tamaños guardados, editores ni transporte.
use super::{CenterView, DawApp, VimMode};
use gpui::{Context, KeyDownEvent, Window};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Panel {
    #[default]
    Arrangement,
    PianoRoll,
    Instrument,
    Fx,
    Explorer,
    Center,
}
#[derive(Default)]
pub(super) struct WorkspaceFocus {
    pub expanded: Option<Panel>,
    pub active: Panel,
}
impl DawApp {
    pub(super) fn arrangement_visible(&self) -> bool {
        match self.workspace_focus.expanded {
            Some(Panel::Arrangement | Panel::PianoRoll | Panel::Instrument) => true,
            Some(_) => false,
            None => matches!(
                self.center_view,
                CenterView::Arrangement | CenterView::Session
            ),
        }
    }

    fn detached_window(&self, window: &Window) -> bool {
        let id = window.window_handle().window_id();
        self.instrument_window.is_some_and(|w| w.window_id() == id)
            || self.piano_roll_window.is_some_and(|w| w.window_id() == id)
    }
    pub(super) fn workspace_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.action_form.is_some()
            || self.palette.is_open()
            || self.settings.is_visible()
            || self.toolbar_icons.picker_open
            || self.clip_menu.is_some()
            || self.track_menu.is_some()
            || self.track_color_menu.is_some()
            || self.fx.menu.is_some()
            || self.piano_roll.choices.is_some()
            || self.piano_roll.lane_menu.is_some()
            || self.mode == VimMode::Command
        {
            return false;
        }
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.shift {
            return false;
        }
        let key = event.keystroke.key.as_str();
        if key == "escape" && self.piano_roll.drag.is_some() {
            self.apply_roll_action(super::piano_roll::RollAction::ClearSelection, window, cx);
            return true;
        }
        if key == "f11" || (self.vim_enabled && self.mode == VimMode::Normal && key == "f") {
            if !event.is_held {
                self.toggle_workspace_focus(window, cx);
            }
            return true;
        }
        if key == "escape"
            && !self.detached_window(window)
            && self.workspace_focus.expanded.is_some()
        {
            self.workspace_focus.expanded = None;
            self.status_message = "Distribución restaurada".into();
            return true;
        }
        false
    }
    pub(super) fn toggle_workspace_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.track_delete_pending = false;
        self.split_drag = None;
        if self.detached_window(window) {
            window.zoom_window();
            return;
        }
        if self.workspace_focus.expanded.take().is_some() {
            self.status_message = "Distribución restaurada".into();
            cx.notify();
            return;
        }
        let panel = if self.fx.view.read(cx).focus.is_focused(window) {
            Panel::Fx
        } else if self.explorer.view.read(cx).focus.is_focused(window) {
            Panel::Explorer
        } else if self.piano_roll_focus.is_focused(window) {
            Panel::PianoRoll
        } else if self.center_view == CenterView::Mixer {
            Panel::Center
        } else {
            self.workspace_focus.active
        };
        self.workspace_focus.expanded = Some(panel);
        self.validate_workspace_focus();
        if self.workspace_focus.expanded.is_some() {
            let focus = match panel {
                Panel::PianoRoll => self.piano_roll_focus.clone(),
                Panel::Fx => self.fx.view.read(cx).focus.clone(),
                Panel::Explorer => self.explorer.view.read(cx).focus.clone(),
                _ => self.focus_handle.clone(),
            };
            window.focus(&focus, cx);
            self.status_message = "Panel ampliado · f / F11 / Esc: restaurar".into();
        }
        cx.notify();
    }
    pub(super) fn validate_workspace_focus(&mut self) {
        let valid = match self.workspace_focus.expanded {
            Some(Panel::PianoRoll) => {
                self.piano_roll.open
                    && self.timeline.midi_clip.is_some()
                    && self.piano_roll_window.is_none()
            }
            Some(Panel::Instrument) => {
                self.instrument_window.is_none()
                    && self.fx.editor_track.is_some_and(|id| {
                        self.tracks
                            .get(self.cursor_row)
                            .is_some_and(|t| t.id == id && t.instrument.is_some())
                    })
            }
            Some(Panel::Fx) => self.fx.width > 0.,
            Some(Panel::Explorer) => self.explorer.width > 0.,
            _ => true,
        };
        if !valid {
            self.workspace_focus.expanded = None;
            self.workspace_focus.active = Panel::Arrangement;
        }
    }
}
