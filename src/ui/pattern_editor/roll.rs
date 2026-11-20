//! Gestos del roll ligero sobre las notas de la pieza, identificadas en el patrón original.
use super::PatternAction;
use crate::ui::{
    DawApp,
    piano_roll::{
        geometry::{FIRST_PITCH, Grid, LAST_PITCH},
        navigation::RollViewport,
    },
    time_grid::TimeSpan,
};
use elevate::musical::{
    drums::{DrumPatternId, DrumPiece},
    grid::NoteGrid,
    model::{InstrumentId, NoteEvent, NoteId, Pattern},
    time::{PPQ, Tick},
};
use gpui::{Pixels, Point, px};
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
pub(in crate::ui) struct DrumRoll {
    pub piece: Option<InstrumentId>,
    pub selected: Option<NoteId>,
    pub viewport: RollViewport,
    pub snap: NoteGrid,
    pub grid: Rc<Cell<Grid>>,
    drag: Option<DrumRollDrag>,
    pub preview: Option<NoteEvent>,
}

#[derive(Clone, Copy)]
enum DrumRollDrag {
    Velocity(NoteId),
    Note {
        pattern: DrumPatternId,
        origin: Point<Pixels>,
        note: NoteEvent,
        resize: bool,
    },
}

impl DrumRoll {
    pub(super) fn focus(&mut self, piece: &DrumPiece, pattern: &Pattern) {
        self.piece = Some(piece.id);
        self.reset_selection();
        let root = piece.base_note();
        let (low, high) = pattern
            .notes()
            .iter()
            .filter(|note| note.instrument == piece.id)
            .fold((root, root), |(low, high), note| {
                (low.min(note.pitch), high.max(note.pitch))
            });
        let low = f32::from(low.saturating_sub(3));
        let high = f32::from((high + 3).min(127));
        let visible = (high - low + 1.0).max(17.0);
        self.viewport = RollViewport {
            top_pitch: (f32::from(root) + 8.0)
                .max(high)
                .min(low + visible - 1.0)
                .clamp(visible - 1.0, 127.0),
            vertical_zoom: f32::from(LAST_PITCH - FIRST_PITCH + 1) / visible,
            ..Default::default()
        };
        self.grid.set(Grid::default());
    }

    pub(super) fn reset_selection(&mut self) {
        self.selected = None;
        self.drag = None;
        self.preview = None;
    }
}

impl DawApp {
    pub(in crate::ui) fn begin_drum_roll_input(&mut self, position: Point<Pixels>, clicks: usize) {
        let editor = &mut self.pattern_editor;
        let Some(piece) = editor.roll.piece else {
            return;
        };
        let grid = editor.roll.grid.get();
        let length = editor.pattern().length();
        let notes = editor
            .pattern()
            .notes()
            .iter()
            .filter(|note| note.instrument == piece);

        if grid.velocity.contains(&position) && grid.velocity.size.height > px(16.0) {
            let target = notes
                .filter_map(|note| {
                    let bounds = grid.note_bounds(note.start, note.duration, note.pitch);
                    let distance = (position.x - (bounds.left() + px(5.0))).abs();
                    (distance <= px(9.0)).then_some((distance, note.id))
                })
                .min_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(_, id)| id);
            if let Some(id) = target {
                editor.roll.selected = Some(id);
                editor.roll.drag = Some(DrumRollDrag::Velocity(id));
                self.move_drum_roll_input(position);
            }
            return;
        }

        let Some((at, pitch)) = grid.pick(position, length) else {
            return;
        };
        let hit = notes
            .rev()
            .find(|note| {
                grid.note_bounds(note.start, note.duration, note.pitch)
                    .contains(&position)
            })
            .copied();
        editor.roll.selected = hit.map(|note| note.id);
        if let Some(note) = hit {
            if clicks == 2 {
                self.apply_pattern_action(PatternAction::DeleteNote(note.id));
            } else {
                let bounds = grid.note_bounds(note.start, note.duration, note.pitch);
                let handle = (bounds.size.width / 3.0).min(px(7.0));
                editor.roll.drag = Some(DrumRollDrag::Note {
                    pattern: editor.session.active,
                    origin: position,
                    note,
                    resize: position.x >= bounds.right() - handle && bounds.size.width >= px(9.0),
                });
            }
        } else if clicks == 2 {
            let step = editor.roll.snap.ticks().0;
            let at = Tick(at.0 / step * step);
            self.apply_pattern_action(PatternAction::InsertNote {
                piece,
                at,
                duration: Tick(step.min(length.0 - at.0)),
                pitch,
            });
        }
    }

    pub(in crate::ui) fn move_drum_roll_input(&mut self, position: Point<Pixels>) -> bool {
        let editor = &mut self.pattern_editor;
        let grid = editor.roll.grid.get();
        let Some(drag) = editor.roll.drag else {
            return false;
        };
        if let DrumRollDrag::Velocity(id) = drag {
            let usable = (f32::from(grid.velocity.size.height) - 16.0).max(1.0);
            let fraction =
                (f32::from(grid.velocity.bottom() - position.y) / usable).clamp(0.0, 1.0);
            let value = (fraction * 126.0).round() as u8 + 1;
            if editor
                .pattern()
                .notes()
                .iter()
                .any(|note| note.id == id && note.velocity != value)
            {
                self.apply_pattern_action(PatternAction::NoteVelocity { id, value });
            }
            return true;
        }

        let DrumRollDrag::Note {
            origin,
            mut note,
            resize,
            ..
        } = drag
        else {
            unreachable!()
        };
        let delta = position - origin;
        if grid.pixels_per_beat <= 0.0
            || grid.row_height <= 0.0
            || (delta.x.abs() < px(2.0) && (resize || delta.y.abs() < px(2.0)))
        {
            editor.roll.preview = None;
            return true;
        }
        let beat_delta = f64::from(f32::from(delta.x) / grid.pixels_per_beat);
        let ppq = PPQ as f64;
        let step = editor.roll.snap.ticks().0 as f64 / ppq;
        let end = editor.pattern().length().0 as f64 / ppq;
        let span = TimeSpan {
            start: note.start.0 as f64 / ppq,
            end: (note.start.0 + note.duration.0) as f64 / ppq,
        };
        if resize {
            let span = grid
                .axis()
                .resize_end(span, beat_delta, end, 1.0 / ppq, step);
            note.duration = Tick((span.length() * ppq).round() as i64);
        } else {
            let moved = grid.axis().move_span(span, beat_delta, 0.0, end, step);
            note.start = Tick((moved.start * ppq).round() as i64);
            let pitch =
                i32::from(note.pitch) - (f32::from(delta.y) / grid.row_height).round() as i32;
            note.pitch = pitch.clamp(0, 127) as u8;
        }
        editor.roll.preview = Some(note);
        true
    }

    pub(in crate::ui) fn finish_drum_roll_input(&mut self, position: Point<Pixels>) -> bool {
        let drag = self.pattern_editor.roll.drag;
        if matches!(drag, Some(DrumRollDrag::Note { .. })) {
            self.move_drum_roll_input(position);
        }
        self.pattern_editor.roll.drag = None;
        let preview = self.pattern_editor.roll.preview.take();
        if let (Some(DrumRollDrag::Note { pattern, note, .. }), Some(edit)) = (drag, preview)
            && pattern == self.pattern_editor.session.active
            && self.pattern_editor.pattern().notes().contains(&note)
            && note != edit
        {
            self.apply_pattern_action(PatternAction::MoveNote(edit));
        }
        drag.is_some()
    }

    pub(super) fn handle_drum_roll_key(&mut self, key: &str) -> bool {
        let editor = &mut self.pattern_editor;
        if key == "escape" {
            if editor.roll.drag.is_some() {
                editor.roll.drag = None;
                editor.roll.preview = None;
            } else {
                editor.roll.piece = None;
            }
            return true;
        }
        let Some(mut note) = editor
            .pattern()
            .notes()
            .iter()
            .find(|note| Some(note.id) == editor.roll.selected)
            .copied()
        else {
            return false;
        };
        let original = note;
        match key {
            "delete" | "backspace" => {
                self.apply_pattern_action(PatternAction::DeleteNote(note.id));
                return true;
            }
            "up" | "k" => note.pitch = note.pitch.saturating_add(1).min(127),
            "down" | "j" => note.pitch = note.pitch.saturating_sub(1),
            "left" | "h" => note.start.0 = (note.start.0 - editor.roll.snap.ticks().0).max(0),
            "right" | "l" => {
                let end = editor.pattern().length().0 - note.duration.0;
                note.start.0 = (note.start.0 + editor.roll.snap.ticks().0).min(end);
            }
            _ => return false,
        }
        if note != original {
            self.apply_pattern_action(PatternAction::MoveNote(note));
        }
        true
    }
}
