//! Arrastre y selección rectangular; snap musical al soltar, preview visual independiente.
use super::RollDrag;
use crate::ui::DawApp;
use elevate::musical::{
    model::{NoteId, Pattern},
    time::PPQ,
};
use gpui::{Pixels, Point};

impl DawApp {
    pub(in crate::ui) fn drag_roll_to(&mut self, position: Point<Pixels>) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        let grid = self.piano_roll.grid.get();
        match self.piano_roll.drag.as_mut() {
            Some(RollDrag::Box {
                origin, current, ..
            }) => {
                *current = position;
                let left = origin.x.min(position.x);
                let right = origin.x.max(position.x);
                let top = origin.y.min(position.y);
                let bottom = origin.y.max(position.y);
                self.piano_roll.selected_notes = clip
                    .notes()
                    .iter()
                    .filter(|note| {
                        let bounds = grid.note_bounds(note.start, note.duration, note.pitch);
                        bounds.left() <= right
                            && bounds.right() >= left
                            && bounds.top() <= bottom
                            && bounds.bottom() >= top
                    })
                    .map(|note| note.id)
                    .collect();
                self.piano_roll.selected = self.piano_roll.selected_notes.first().copied();
            }
            Some(RollDrag::Move { current, .. } | RollDrag::Length { current, .. }) => {
                *current = position;
            }
            None => {}
        }
        self.update_roll_push();
    }

    pub(in crate::ui) fn finish_roll_drag(&mut self, position: Point<Pixels>) {
        self.piano_roll.paint_stroke = None;
        self.piano_roll.control_drag = false;
        self.piano_roll.lane_span = None;
        let step = self.piano_roll.resolution.ticks().0;
        self.drag_roll_to(position);
        let Some(drag) = self.piano_roll.drag.take() else {
            return;
        };
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        if clip.props.locked {
            self.piano_roll.snap_guide.land(None);
            self.piano_roll.motion.borrow_mut().clear();
            self.piano_roll.push = None;
            self.piano_roll.push_error = None;
            self.piano_roll.hint = "Clip bloqueado".into();
            return;
        }
        let grid = self.piano_roll.grid.get();
        let resize = matches!(&drag, RollDrag::Length { .. });
        let moved_ids = match &drag {
            RollDrag::Move { notes, .. } => notes.clone(),
            RollDrag::Length { note, .. } => vec![*note],
            RollDrag::Box { .. } => Vec::new(),
        };
        let replacement = match drag {
            RollDrag::Box {
                origin,
                current,
                erase,
            } => {
                if erase
                    && ((current.x - origin.x).abs() >= gpui::px(2.0)
                        || (current.y - origin.y).abs() >= gpui::px(2.0))
                {
                    self.delete_roll_selection();
                } else {
                    self.piano_roll.hint = format!(
                        "{} notas seleccionadas · Delete: borrar",
                        self.piano_roll.selected_notes.len()
                    );
                }
                return;
            }
            RollDrag::Move {
                origin,
                current,
                original,
                ..
            }
            | RollDrag::Length {
                origin,
                current,
                original,
                ..
            } => {
                let offset = current - origin;
                if offset.x.abs() < gpui::px(2.0) && (resize || offset.y.abs() < gpui::px(2.0)) {
                    self.piano_roll.snap_guide.land(None);
                    self.piano_roll.motion.borrow_mut().clear();
                    self.piano_roll.push = None;
                    self.piano_roll.push_error = None;
                    return;
                }
                let result = if let Some(error) = self.piano_roll.push_error.take() {
                    Err(error)
                } else if clip.pattern != original {
                    Err("El clip cambió durante el arrastre".into())
                } else if let Some(push) = &mut self.piano_roll.push {
                    push.commit(&original, grid, offset, step as f64 / PPQ as f64)
                } else {
                    Err("No se pudo preparar el empuje".into())
                };
                self.piano_roll.push = None;
                match result {
                    Ok(pattern) => Some(pattern),
                    Err(error) => {
                        self.piano_roll.hint = error;
                        None
                    }
                }
            }
        };
        // El commit siempre ajusta a la rejilla: confirmar el borde donde quedó.
        let landed = replacement
            .as_ref()
            .and_then(|pattern| landed_edge(pattern, &moved_ids, resize));
        self.piano_roll.snap_guide.land(landed);
        if let Some(pattern) = replacement {
            self.piano_roll
                .motion
                .borrow_mut()
                .settle(&clip.pattern, &pattern, grid);
            if clip.pattern == pattern {
                return;
            }
            self.stop_audio();
            if let Some(clip) = self.timeline.midi_clip.as_mut() {
                clip.pattern = pattern;
            }
        } else {
            self.piano_roll.motion.borrow_mut().clear();
        }
    }

    pub(in crate::ui) fn end_roll_drag(&mut self) {
        self.piano_roll.snap_guide.clear();
        self.piano_roll.paint_stroke = None;
        self.piano_roll.control_drag = false;
        self.piano_roll.lane_span = None;
        self.piano_roll.drag = None;
        self.piano_roll.push = None;
        self.piano_roll.push_error = None;
        self.piano_roll.motion.borrow_mut().clear();
    }
}

/// Inicio (mover) o final (redimensionar) del grupo de notas tras el commit, en beats.
fn landed_edge(pattern: &Pattern, ids: &[NoteId], resize: bool) -> Option<f64> {
    let notes = pattern.notes().iter().filter(|note| ids.contains(&note.id));
    let tick = if resize {
        notes.map(|note| note.start.0 + note.duration.0).max()
    } else {
        notes.map(|note| note.start.0).min()
    }?;
    Some(tick as f64 / PPQ as f64)
}
