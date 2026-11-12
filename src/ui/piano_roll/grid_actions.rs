//! Rejilla editable y gestos discretos para la misma acción de subdivisión Lua.
use crate::ui::{DawApp, VimMode};
use elevate::musical::grid::NoteGrid;
use gpui::{KeyDownEvent, TouchPhase};

#[derive(Default)]
pub(super) struct SplitGesture {
    wheel: f32,
    pinch: f32,
    pinch_fired: bool,
}

impl SplitGesture {
    pub fn wheel(&mut self, delta: f32, phase: TouchPhase) -> bool {
        if matches!(
            phase,
            TouchPhase::Started | TouchPhase::Ended | TouchPhase::Cancelled
        ) {
            self.wheel = 0.0;
        }
        if !delta.is_finite() || matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            return false;
        }
        self.wheel += delta;
        if self.wheel.abs() < 24.0 {
            return false;
        }
        self.wheel = 0.0;
        true
    }

    pub fn pinch(&mut self, delta: f32, phase: TouchPhase) -> bool {
        if matches!(
            phase,
            TouchPhase::Started | TouchPhase::Ended | TouchPhase::Cancelled
        ) {
            self.pinch = 0.0;
            self.pinch_fired = false;
        }
        if !delta.is_finite() || matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            return false;
        }
        self.pinch += delta;
        if self.pinch_fired || self.pinch.abs() < 0.08 {
            return false;
        }
        self.pinch_fired = true;
        true
    }
}

impl DawApp {
    pub(in crate::ui) fn change_roll_grid(&mut self, finer: bool) {
        let current = self.piano_roll.resolution;
        self.piano_roll.resolution = if finer {
            current.finer()
        } else {
            current.coarser()
        };
        self.piano_roll.hint = format!(
            "Rejilla 1/{} · Alt+rueda o Shift+pellizco: subdividir selección",
            self.piano_roll.resolution.denominator()
        );
        self.status_message = self.piano_roll.hint.clone();
    }

    pub(in crate::ui) fn handle_roll_grid_key(&mut self, event: &KeyDownEvent) -> bool {
        let modifiers = event.keystroke.modifiers;
        if self.mode == VimMode::Command
            || self.action_form.is_some()
            || !modifiers.alt
            || modifiers.control
            || modifiers.platform
            || modifiers.shift
        {
            return false;
        }
        match event.keystroke.key.as_str() {
            "1" | "2" => {
                if !event.is_held {
                    self.change_roll_grid(event.keystroke.key == "1");
                }
                true
            }
            _ => false,
        }
    }

    pub(in crate::ui) fn execute_roll_grid_command(&mut self, command: &str) -> bool {
        let mut parts = command.split_whitespace();
        match parts
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "midi.grid.finer" if parts.next().is_none() => self.change_roll_grid(true),
            "midi.grid.coarser" if parts.next().is_none() => self.change_roll_grid(false),
            "midi.grid" => {
                let value = parts.next().and_then(|value| value.parse().ok());
                let result = value
                    .ok_or_else(|| "Uso: midi.grid 16".to_string())
                    .and_then(NoteGrid::new);
                match result {
                    Ok(grid) if parts.next().is_none() => {
                        self.piano_roll.resolution = grid;
                        self.piano_roll.hint = format!("Rejilla 1/{}", grid.denominator());
                        self.status_message = self.piano_roll.hint.clone();
                    }
                    Err(error) => self.status_message = error,
                    _ => self.status_message = "Uso: midi.grid 16".into(),
                }
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gestures_accumulate_fractions_and_pinch_fires_once_until_end() {
        let mut gesture = SplitGesture::default();
        assert!(!gesture.wheel(6.0, TouchPhase::Started));
        assert!(!gesture.wheel(12.0, TouchPhase::Moved));
        assert!(gesture.wheel(6.0, TouchPhase::Moved));
        assert!(!gesture.wheel(24.0, TouchPhase::Cancelled));
        assert!(!gesture.pinch(0.04, TouchPhase::Started));
        assert!(gesture.pinch(0.05, TouchPhase::Moved));
        assert!(!gesture.pinch(0.5, TouchPhase::Moved));
        assert!(!gesture.pinch(0.5, TouchPhase::Ended));
        assert!(gesture.pinch(-0.1, TouchPhase::Started));
        assert!(!gesture.pinch(f32::NAN, TouchPhase::Moved));
    }
}
