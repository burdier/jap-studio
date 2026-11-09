//! Controlador compartido por menú contextual, toolbar y teclado del mock.
use super::{PianoRoll, RollAction, RollTool, demo};
use crate::ui::DawApp;
use elevate::musical::time::{PPQ, Tick};
use gpui::{Context, Window};

impl DawApp {
    pub(in crate::ui) fn insert_midi_clip_from_menu(&mut self) {
        let Some(target) = self.clip_menu.take() else {
            return;
        };
        if target.clip_kind.is_some() {
            return;
        }
        self.insert_midi_clip_at(target.track, target.beat);
    }

    pub(in crate::ui) fn insert_midi_clip_at(&mut self, track: usize, beat: f64) {
        if !self.track_accepts_clips(track) {
            self.status_message = "Selecciona una pista hija para insertar MIDI".into();
            return;
        }
        self.stop_audio();
        let clip = demo::reference_clip(
            self.active_instrument_name().to_string(),
            track,
            beat,
            self.tracks[track]
                .instrument
                .unwrap_or(self.instrument.id()),
        );
        self.timeline.insert_midi_clip(clip);
        self.open_piano_roll();
    }

    pub(in crate::ui) fn open_piano_roll(&mut self) {
        let Some(clip) = &self.timeline.midi_clip else {
            return;
        };
        self.cursor_row = clip.track;
        let cursor = Tick((13 * PPQ).min(clip.pattern.length().0 - 1));
        let copied_note = self.piano_roll.copied_note;
        self.piano_roll = PianoRoll {
            open: true,
            cursor,
            copied_note,
            ..PianoRoll::default()
        };
        self.timeline.end_clip_drag();
    }

    pub(in crate::ui) fn apply_roll_action(
        &mut self,
        action: RollAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            RollAction::ZoomWidth(factor) => {
                if let Some(clip) = &self.timeline.midi_clip {
                    let grid = self.piano_roll.grid.get();
                    self.piano_roll.viewport.zoom_at(
                        grid,
                        grid.notes.center(),
                        clip.pattern_beats(),
                        factor,
                    );
                }
            }
            RollAction::ZoomHeight(factor) => {
                let grid = self.piano_roll.grid.get();
                self.piano_roll
                    .viewport
                    .zoom_vertical_at(grid, grid.notes.center(), factor);
            }
            RollAction::ResetView => self.piano_roll.viewport = Default::default(),
            RollAction::Close if self.piano_roll_window.is_some() => {
                self.toggle_piano_roll_window(cx)
            }
            RollAction::Close => {
                self.piano_roll.open = false;
                window.focus(&self.focus_handle, cx);
            }
            RollAction::ToggleWindow => self.toggle_piano_roll_window(cx),
            RollAction::PreviousClip => self.step_roll_clip(-1),
            RollAction::NextClip => self.step_roll_clip(1),
            RollAction::Tool(tool) => {
                self.piano_roll.tool = tool;
                self.piano_roll.hint = "Herramienta MIDI activa".into();
            }
            RollAction::Quantize => self.roll_quantize(),
            RollAction::Humanize => self.roll_humanize(),
            RollAction::HumanizeAmount(amount) => {
                self.piano_roll.humanize_amount = amount;
                self.piano_roll.hint = format!("Humanize: {amount}%");
            }
            RollAction::Copy => self.roll_copy(false),
            RollAction::Cut => self.roll_copy(true),
            RollAction::Paste => self.roll_paste(),
            RollAction::GhostNotes => self.piano_roll.ghost_notes = !self.piano_roll.ghost_notes,
            RollAction::Follow => self.piano_roll.follow = !self.piano_roll.follow,
            RollAction::Velocities => self.piano_roll.velocities = !self.piano_roll.velocities,
            RollAction::HighlightScale => {
                self.piano_roll.highlight_scale = !self.piano_roll.highlight_scale
            }
            RollAction::Preview(label) => self.piano_roll.hint = format!("Mock visual · {label}"),
        }
    }

    pub(in crate::ui) fn handle_roll_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let action = match key {
            "0" => Some(RollAction::ResetView),
            "escape" => Some(RollAction::Close),
            "v" => Some(RollAction::Tool(RollTool::Select)),
            "b" => Some(RollAction::Tool(RollTool::Draw)),
            "e" | "E" => Some(RollAction::Tool(RollTool::Erase)),
            "p" => Some(RollAction::Tool(RollTool::Paint)),
            "m" => Some(RollAction::Tool(RollTool::Mute)),
            "q" => Some(RollAction::Quantize),
            "H" => Some(RollAction::Humanize),
            "h" | "left" | "l" | "right" => {
                let direction = if matches!(key, "h" | "left") { -1 } else { 1 };
                if let Some(clip) = &self.timeline.midi_clip {
                    self.piano_roll.cursor.0 = (self.piano_roll.cursor.0 + direction * PPQ / 4)
                        .clamp(0, clip.pattern.length().0 - 1);
                }
                None
            }
            "j" | "down" | "k" | "up" => {
                if let Some(clip) = &self.timeline.midi_clip {
                    let notes = clip.notes();
                    if !notes.is_empty() {
                        let current = notes
                            .iter()
                            .position(|n| Some(n.id) == self.piano_roll.selected);
                        let next = match (current, matches!(key, "j" | "down")) {
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
