//! Navegación por teclado de las acciones y menús del piano roll.
use super::{RollAction, RollTool};
use crate::ui::DawApp;
use gpui::{Context, KeyDownEvent, Window};

impl DawApp {
    pub(in crate::ui) fn handle_roll_selection_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.command_open
            || self.piano_roll.choices.is_some()
            || self.piano_roll.lane_menu.is_some()
        {
            return false;
        }
        let modifiers = event.keystroke.modifiers;
        if modifiers.alt || !(modifiers.control || modifiers.platform) {
            return false;
        }
        let key = event.keystroke.key.to_ascii_lowercase();
        match key.as_str() {
            "d" if !modifiers.shift => {
                if !event.is_held {
                    self.duplicate_roll_notes();
                }
                return true;
            }
            "z" | "y" => {
                if !event.is_held && self.piano_roll.drag.is_none() {
                    self.arrangement_midi_undo(key == "y" || modifiers.shift);
                }
                return true;
            }
            _ => {}
        }
        let action = match event.keystroke.key.to_lowercase().as_str() {
            "a" if modifiers.shift => RollAction::ClearSelection,
            "a" => RollAction::SelectAll,
            "c" if !modifiers.shift => RollAction::Copy,
            "x" if !modifiers.shift => RollAction::Cut,
            "v" if !modifiers.shift => RollAction::Paste,
            _ => return false,
        };
        self.apply_roll_action(action, window, cx);
        true
    }

    pub(in crate::ui) fn handle_roll_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = &mut self.piano_roll.choices {
            let total = menu.rows.len();
            match key {
                "escape" => self.piano_roll.choices = None,
                "up" if total > 0 => menu.selected = (menu.selected + total - 1) % total,
                "down" if total > 0 => menu.selected = (menu.selected + 1) % total,
                "enter" | "space" if total > 0 => {
                    let command = menu.rows[menu.selected].1.clone();
                    self.piano_roll.choices = None;
                    self.execute_command_with_context(&command, window, cx);
                }
                _ => {}
            }
            return;
        }
        if let Some(menu) = &mut self.piano_roll.lane_menu {
            let total = super::lane::LANE_MENU_ITEMS.len();
            match key {
                "escape" => {
                    self.piano_roll.lane_menu = None;
                    return;
                }
                "up" => {
                    menu.selected = (menu.selected + total - 1) % total;
                    return;
                }
                "down" => {
                    menu.selected = (menu.selected + 1) % total;
                    return;
                }
                "enter" | "space" => {
                    let selected_lane = super::lane::LANE_MENU_ITEMS
                        .get(menu.selected)
                        .map(|item| item.lane)
                        .unwrap_or(super::RollLane::Velocity);
                    self.apply_roll_action(RollAction::SetLane(selected_lane), window, cx);
                    return;
                }
                _ => {}
            }
        }
        let action = match key {
            "0" => Some(RollAction::ResetView),
            "escape" if self.piano_roll.drag.is_some() || self.piano_roll.pitch_drag.is_some() => {
                Some(RollAction::ClearSelection)
            }
            "escape" => Some(RollAction::Close),
            "delete" | "backspace" => Some(RollAction::DeleteSelection),
            "v" => Some(RollAction::Tool(RollTool::Select)),
            "b" => Some(RollAction::Tool(RollTool::Draw)),
            "e" | "E" => Some(RollAction::Tool(RollTool::Erase)),
            "p" => Some(RollAction::Tool(RollTool::Paint)),
            "m" => Some(RollAction::Tool(RollTool::Mute)),
            "q" => Some(RollAction::Quantize),
            "H" => Some(RollAction::Humanize),
            "left" | "right" => {
                let direction = if key == "left" { -1 } else { 1 };
                if let Some(clip) = &self.timeline.midi_clip {
                    self.piano_roll.cursor.0 = (self.piano_roll.cursor.0
                        + direction * self.piano_roll.resolution.ticks().0)
                        .clamp(0, clip.pattern.length().0 - 1);
                }
                None
            }
            "down" | "up" => {
                if let Some(clip) = &self.timeline.midi_clip {
                    let notes = clip.notes();
                    if !notes.is_empty() {
                        let current = notes
                            .iter()
                            .position(|n| Some(n.id) == self.piano_roll.selected);
                        let next = match (current, key == "down") {
                            (Some(i), true) => (i + 1) % notes.len(),
                            (Some(i), false) => (i + notes.len() - 1) % notes.len(),
                            (None, _) => 0,
                        };
                        self.piano_roll.selected = Some(notes[next].id);
                        self.piano_roll.selected_notes = vec![notes[next].id];
                    }
                }
                None
            }
            _ => None,
        };
        if let Some(action) = action {
            self.apply_roll_action(action, window, cx);
        }
    }
}
