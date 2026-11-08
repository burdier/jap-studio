//! Teclas y comandos de texto. Ratón y teclado comparten acciones de edición.
use super::{CenterView, DawApp, VimMode, pattern_editor::PatternAction};
use gpui::{KeyDownEvent, KeyUpEvent};

impl DawApp {
    pub(super) fn handle_key(&mut self, event: &KeyDownEvent) {
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(&event.keystroke.key);
        self.last_key = if event.keystroke.modifiers.control {
            format!("C-{key}")
        } else {
            key.to_string()
        };
        if self.palette.is_open() {
            let text = !event.keystroke.modifiers.control && !event.keystroke.modifiers.alt;
            self.handle_palette_key(key, text);
            return;
        }
        if self.mode == VimMode::Normal
            && self.center_view == CenterView::Instrument
            && let Some(note) = self.instrument.keyboard_note(key)
        {
            if !event.is_held {
                self.note_on(note);
            }
            return;
        }
        match self.mode {
            VimMode::Normal => self.handle_normal_key(key),
            VimMode::Insert => {
                if matches!(key, "escape" | "enter") {
                    self.set_mode(VimMode::Normal);
                }
            }
            VimMode::Command => self.handle_command_key(key),
        }
    }
    pub(super) fn handle_key_up(&mut self, event: &KeyUpEvent) {
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(&event.keystroke.key);
        if let Some(note) = self.instrument.keyboard_note(key) {
            self.note_off(note);
        }
    }
    fn handle_normal_key(&mut self, key: &str) {
        match key {
            " " | "space" => {
                if self.center_view == CenterView::Instrument {
                    self.all_notes_off();
                } else if self.center_view == CenterView::Session {
                    if self.pattern_player.is_some() {
                        self.stop_pattern();
                        self.status_message = "Patrón detenido".into();
                    } else {
                        self.play_pattern();
                    }
                } else if self.player.is_some() {
                    self.stop_audio();
                    self.status_message = "Detenido".into();
                } else {
                    self.play_audio();
                }
            }
            "enter" if self.center_view == CenterView::Session => {
                self.apply_pattern_action(PatternAction::Toggle {
                    lane: self.cursor_row,
                    step: self.cursor_col,
                })
            }
            "[" => self.change_gain(-0.1),
            "]" => self.change_gain(0.1),
            "a" | "insert" => self.open_palette(),
            "b" => self.toggle_bypass(),
            "escape" => {
                self.all_notes_off();
                self.status_message = "Normal".into();
            }
            "i" => self.set_mode(VimMode::Insert),
            ":" => self.set_mode(VimMode::Command),
            "tab" => self.set_center_view(match self.center_view {
                CenterView::Arrangement => CenterView::Instrument,
                CenterView::Instrument => CenterView::Session,
                CenterView::Session => CenterView::Mixer,
                CenterView::Mixer => CenterView::Arrangement,
            }),
            "h" | "left" if self.center_view == CenterView::Arrangement => {
                self.timeline.move_cursor(-1.0)
            }
            "l" | "right" if self.center_view == CenterView::Arrangement => {
                self.timeline.move_cursor(1.0)
            }
            "home" if self.center_view == CenterView::Arrangement => {
                self.stop_audio();
                self.timeline.go_to_start();
            }
            "+" | "=" if self.center_view == CenterView::Arrangement => {
                self.timeline.zoom_at_cursor(2.0)
            }
            "-" if self.center_view == CenterView::Arrangement => self.timeline.zoom_at_cursor(0.5),
            "h" | "left" => self.cursor_col = self.cursor_col.saturating_sub(1),
            "l" | "right" => {
                self.cursor_col = (self.cursor_col + 1).min(self.pattern_editor.step_count() - 1)
            }
            "k" | "up" => {
                self.cursor_row = self.cursor_row.saturating_sub(1);
                self.reveal_track();
            }
            "j" | "down" => {
                let last = if self.center_view == CenterView::Session {
                    self.pattern_editor.lanes.len() - 1
                } else {
                    7
                };
                self.cursor_row = (self.cursor_row + 1).min(last);
                self.reveal_track();
            }
            "1" => self.set_center_view(CenterView::Arrangement),
            "2" => self.set_center_view(CenterView::Session),
            "3" => self.set_center_view(CenterView::Mixer),
            "4" => self.set_center_view(CenterView::Instrument),
            "R" => {
                self.reload();
                self.status_message = "Design reloaded".into();
            }
            _ => {}
        }
    }
    fn handle_command_key(&mut self, key: &str) {
        match key {
            "escape" => self.set_mode(VimMode::Normal),
            "enter" => {
                let cmd = self.command_buffer.clone();
                self.execute_command(&cmd);
                self.set_mode(VimMode::Normal);
            }
            "backspace" => {
                self.command_buffer.pop();
            }
            _ if key.len() == 1 => self.command_buffer.push_str(key),
            _ => {}
        }
    }
    pub(super) fn execute_command(&mut self, cmd: &str) {
        match cmd.to_lowercase().as_str() {
            "add" | "browse" | "insert" => self.open_palette(),
            "play" => self.play_context(),
            "stop" => {
                self.stop_all();
                self.status_message = "Detenido".into();
            }
            "record" => self.status_message = "Grabación todavía no implementada".into(),
            "arrangement" | "arr" => self.set_center_view(CenterView::Arrangement),
            "piano" | "instrument" | "inst" => self.set_center_view(CenterView::Instrument),
            "session" | "pat" | "pattern" => self.set_center_view(CenterView::Session),
            "mixer" | "mix" => self.set_center_view(CenterView::Mixer),
            "reload" => {
                self.reload();
                self.status_message = "Design reloaded".into();
            }
            _ => self.status_message = format!("? {cmd}"),
        }
    }
}
