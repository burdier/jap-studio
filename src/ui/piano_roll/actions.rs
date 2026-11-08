//! Controlador compartido por menú contextual, toolbar y teclado del mock.
use super::{InsertMenu, PianoRoll, RollAction, RollTool, demo};
use crate::ui::{DawApp, timeline::TimelineArea};
use elevate::musical::time::{PPQ, Tick};
use gpui::{Context, Pixels, Point, Size, Window, point, px};

impl DawApp {
    pub(in crate::ui) fn open_midi_clip_menu(
        &mut self,
        position: Point<Pixels>,
        viewport: Size<Pixels>,
    ) {
        self.timeline.end_clip_drag();
        let Some(track) = self.timeline.place_cursor(
            position,
            TimelineArea::Lanes,
            self.ds.track_header_height,
            self.tracks.len(),
        ) else {
            return;
        };
        self.cursor_row = track;
        self.sync_midi_focus();
        self.piano_roll.menu = Some(InsertMenu {
            position: point(
                position.x.min((viewport.width - px(248.0)).max(px(0.0))),
                position.y.min((viewport.height - px(94.0)).max(px(0.0))),
            ),
            track,
            beat: self.timeline.edit_beat(),
        });
    }

    pub(in crate::ui) fn insert_midi_clip_from_menu(&mut self) {
        let Some(target) = self.piano_roll.menu.take() else {
            return;
        };
        self.stop_audio();
        let clip = demo::reference_clip(
            self.instrument.active_name().to_string(),
            target.track,
            target.beat,
            self.instrument.id(),
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
        self.piano_roll = PianoRoll {
            open: true,
            cursor,
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
            RollAction::Close if self.piano_roll_window.is_some() => {
                self.toggle_piano_roll_window(cx)
            }
            RollAction::Close => {
                self.piano_roll.open = false;
                window.focus(&self.focus_handle, cx);
            }
            RollAction::ToggleWindow => self.toggle_piano_roll_window(cx),
            RollAction::Tool(tool) => {
                self.piano_roll.tool = tool;
                self.piano_roll.hint =
                    "Mock visual · selección disponible; edición MIDI pendiente".into();
            }
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
            "escape" => Some(RollAction::Close),
            "v" => Some(RollAction::Tool(RollTool::Select)),
            "b" => Some(RollAction::Tool(RollTool::Draw)),
            "e" => Some(RollAction::Tool(RollTool::Erase)),
            "p" => Some(RollAction::Tool(RollTool::Paint)),
            "m" => Some(RollAction::Tool(RollTool::Mute)),
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

    pub(in crate::ui) fn select_roll_note(&mut self, position: Point<Pixels>) {
        let Some(clip) = &self.timeline.midi_clip else {
            return;
        };
        let Some((tick, pitch)) = self
            .piano_roll
            .grid
            .get()
            .pick(position, clip.pattern.length())
        else {
            return;
        };
        self.piano_roll.cursor = tick;
        self.piano_roll.selected = clip
            .notes()
            .iter()
            .find(|note| {
                note.pitch == pitch && tick >= note.start && tick.0 < note.start.0 + note.duration.0
            })
            .map(|note| note.id);
    }
}
