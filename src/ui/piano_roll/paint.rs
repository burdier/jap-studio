//! Pincel continuo, una operación por celda de snap durante cada gesto.
use super::RollTool;
use crate::ui::DawApp;
use elevate::musical::time::PPQ;
use gpui::{MouseDownEvent, Pixels, Point};

impl DawApp {
    pub(in crate::ui) fn begin_roll_input(&mut self, event: &MouseDownEvent) {
        self.end_roll_drag();
        let grid = self.piano_roll.grid.get();
        if event.modifiers.control
            && self.piano_roll.tool == RollTool::Select
            && grid.notes.contains(&event.position)
            && let Some(clip) = &self.timeline.midi_clip
        {
            let hit = clip.notes().iter().rev().find(|note| {
                grid.note_bounds(note.start, note.duration, note.pitch)
                    .contains(&event.position)
            });
            if let Some(note) = hit {
                let selected = &mut self.piano_roll.selected_notes;
                if selected.contains(&note.id) {
                    selected.retain(|id| *id != note.id);
                } else {
                    selected.push(note.id);
                }
                self.piano_roll.selected = selected.last().copied();
                self.piano_roll.hint = format!("{} notas seleccionadas", selected.len());
            } else {
                self.piano_roll.drag = Some(super::RollDrag::Box {
                    origin: event.position,
                    current: event.position,
                    erase: false,
                    additive: self.piano_roll.selected_notes.clone(),
                });
            }
            return;
        }
        if self.handle_roll_pointer(event) {
            return;
        }
        let position = event.position;
        let grid = self.piano_roll.grid.get();
        if self.piano_roll.tool == RollTool::Pitch && grid.notes.contains(&position) {
            self.begin_roll_pitch(event);
            return;
        }
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

    /// Abre el menú de patrones si el clic derecho cae en el carril de propiedades de nota.
    pub(in crate::ui) fn open_lane_preset_menu(&mut self, position: Point<Pixels>) -> bool {
        let grid = self.piano_roll.grid.get();
        if !self.piano_roll.velocities
            || !grid.velocity.contains(&position)
            || position.x < grid.notes.left()
            || self.piano_roll.active_lane.note_property().is_none()
        {
            return false;
        }
        self.piano_roll.lane_menu = None;
        self.piano_roll.lane_preset_beat = grid.first_beat
            + f64::from(f32::from(position.x - grid.notes.left()) / grid.pixels_per_beat);
        self.piano_roll.lane_preset_menu = Some(position);
        true
    }

    /// Moldea la propiedad activa como una onda sobre las notas seleccionadas (o todo el clip).
    pub(in crate::ui) fn apply_lane_preset(&mut self, preset: super::LanePreset) {
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
        let chosen = &self.piano_roll.selected_notes;
        let scope = self.piano_roll.lane_scope;
        let bar = ((self.timeline.meter.beats_per_bar() * PPQ as f64).round() as i64).max(1);
        let clicked_bar = (self.piano_roll.lane_preset_beat * PPQ as f64).floor() as i64 / bar;
        let mut targets: Vec<_> = clip
            .notes()
            .iter()
            .filter(|note| chosen.len() < 2 || chosen.contains(&note.id))
            .filter(|note| scope != super::LaneScope::ThisBar || note.start.0 / bar == clicked_bar)
            .map(|note| (note.id, note.start.0, property.fraction(note)))
            .collect();
        if targets.is_empty() {
            self.piano_roll.hint = "No hay notas en ese alcance".into();
            return;
        }
        targets.sort_by_key(|target| (target.1, target.0));
        let first = targets[0].1 as f32;
        let span = (targets[targets.len() - 1].1 as f32 - first).max(1.0);
        let updates: Vec<_> = targets
            .iter()
            .enumerate()
            .map(|(index, (id, start, current))| {
                let (t, cycles) = if scope == super::LaneScope::All {
                    ((*start as f32 - first) / span, 2.0)
                } else {
                    ((start % bar) as f32 / bar as f32, 1.0)
                };
                (
                    *id,
                    property.value_at_fraction(preset.eval(t, cycles, index, *current)),
                )
            })
            .collect();
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
            "{} · {} aplicado a {count} notas",
            self.piano_roll.active_lane.short_label(),
            preset.label()
        );
    }
}
