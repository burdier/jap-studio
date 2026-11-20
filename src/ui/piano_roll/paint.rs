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
        let grid = self.piano_roll.grid.get();
        if self.piano_roll.velocities
            && grid.velocity.contains(&position)
            && position.x >= grid.notes.left()
            && event.modifiers.alt
            && self.piano_roll.active_lane.note_property().is_some()
        {
            if self.piano_roll.lane_contour == super::ContourShape::StemsOnly {
                self.piano_roll.lane_contour = super::ContourShape::Linear;
            }
            self.piano_roll.lane_span = Some(super::LaneSpanDrag {
                origin: position,
                current: position,
                tension: if event.modifiers.shift { 0.45 } else { 0.0 },
            });
            self.apply_lane_span_to(position, event.modifiers.shift);
            return;
        }
        self.piano_roll.control_drag = (self.piano_roll.active_lane.controller().is_some()
            || self.piano_roll.active_lane.note_property().is_some())
            && grid.velocity.contains(&position);
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

    pub(in crate::ui) fn apply_lane_span_to(&mut self, position: Point<Pixels>, shift: bool) {
        let Some(mut span) = self.piano_roll.lane_span else {
            return;
        };
        span.current = position;
        span.tension = if shift { 0.45 } else { 0.0 };
        self.piano_roll.lane_span = Some(span);
        let Some(property) = self.piano_roll.active_lane.note_property() else {
            return;
        };
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        if clip.props.locked {
            self.piano_roll.hint = "Clip bloqueado".into();
            return;
        }
        let grid = self.piano_roll.grid.get();
        let lane_h = (f32::from(grid.velocity.size.height) - 16.0).max(1.0);
        let y_frac =
            |py: Pixels| ((f32::from(grid.velocity.bottom() - py)) / lane_h).clamp(0.0, 1.0);
        let (x0, y0, x1, y1) = if span.origin.x <= span.current.x {
            (
                f32::from(span.origin.x),
                y_frac(span.origin.y),
                f32::from(span.current.x),
                y_frac(span.current.y),
            )
        } else {
            (
                f32::from(span.current.x),
                y_frac(span.current.y),
                f32::from(span.origin.x),
                y_frac(span.origin.y),
            )
        };
        let shape = self.piano_roll.lane_contour;
        let dx = (x1 - x0).max(1.0);
        let updates: Vec<_> = clip
            .notes()
            .iter()
            .filter_map(|note| {
                let b = grid.note_bounds(note.start, note.duration, note.pitch);
                let stem_x = f32::from(b.left() + gpui::px(5.0));
                if stem_x >= x0 - 4.0 && stem_x <= x1 + 4.0 {
                    let t = ((stem_x - x0) / dx).clamp(0.0, 1.0);
                    let frac = shape.eval_span(y0, y1, t, span.tension);
                    Some((note.id, property.value_at_fraction(frac)))
                } else {
                    None
                }
            })
            .collect();
        if updates.is_empty() {
            return;
        }
        self.stop_audio();
        let count = updates.len();
        if let Some(clip) = self.timeline.midi_clip.as_mut() {
            clip.edit_pattern(|pattern| {
                for (id, value) in updates {
                    let _ = pattern.set_note_property(id, property, value);
                }
            });
        }
        self.piano_roll.hint = format!(
            "{} · {} moldeada sobre {count} notas",
            self.piano_roll.active_lane.short_label(),
            shape.label()
        );
    }
}
