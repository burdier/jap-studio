//! Pincel continuo, una operación por celda de snap durante cada gesto.
use super::RollTool;
use crate::ui::DawApp;
use gpui::{MouseDownEvent, Pixels, Point};

impl DawApp {
    pub(in crate::ui) fn begin_roll_input(&mut self, event: &MouseDownEvent) {
        self.end_roll_drag();
        if self.handle_roll_pointer(event) {
            return;
        }
        let position = event.position;
        self.piano_roll.control_drag = (self.piano_roll.active_lane.controller().is_some()
            || self.piano_roll.active_lane.note_property().is_some())
            && self.piano_roll.grid.get().velocity.contains(&position);
        self.piano_roll.paint_stroke = if self.piano_roll.tool == RollTool::Paint
            && self.piano_roll.grid.get().notes.contains(&position)
        {
            Some(Default::default())
        } else {
            None
        };
        if self.piano_roll.paint_stroke.is_some() {
            self.paint_roll_to(position);
        } else {
            self.edit_roll_at(position);
        }
        self.piano_roll
            .motion
            .borrow_mut()
            .begin(self.piano_roll.drag.as_ref());
    }

    pub(in crate::ui) fn paint_roll_to(&mut self, position: Point<Pixels>) {
        let Some(clip) = &self.timeline.midi_clip else {
            return;
        };
        if clip.props.locked {
            return;
        }
        let Some((tick, pitch)) = self
            .piano_roll
            .grid
            .get()
            .pick(position, clip.pattern.length())
        else {
            return;
        };
        let cell = (tick.0 / self.piano_roll.resolution.ticks().0, pitch);
        let Some(visited) = &mut self.piano_roll.paint_stroke else {
            return;
        };
        if visited.insert(cell) {
            self.edit_roll_at(position);
        }
    }
}
