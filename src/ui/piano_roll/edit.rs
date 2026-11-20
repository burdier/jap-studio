//! Operaciones MIDI del piano roll; botones, teclas y canvas llaman estos controladores.
use super::{RollDrag, RollTool};
use crate::ui::DawApp;
use elevate::musical::time::PPQ;
use gpui::{Pixels, Point};

impl DawApp {
    pub(in crate::ui) fn step_roll_clip(&mut self, direction: i32) {
        let Some(active) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        let active_id = active.id;
        let mut order: Vec<_> = self
            .timeline
            .other_midi_clips
            .iter()
            .chain(self.timeline.midi_clip.iter())
            .map(|clip| (clip.id, clip.track, clip.start_beat))
            .collect();
        if order.len() < 2 {
            return;
        }
        order.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| a.2.total_cmp(&b.2))
                .then_with(|| a.0.value().cmp(&b.0.value()))
        });
        let Some(current) = order.iter().position(|entry| entry.0 == active_id) else {
            return;
        };
        let next = (current as i32 + direction).rem_euclid(order.len() as i32) as usize;
        self.select_roll_clip(order[next].0);
    }

    pub(in crate::ui) fn edit_roll_at(&mut self, position: Point<Pixels>) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        let grid = self.piano_roll.grid.get();
        if self.piano_roll.velocities
            && grid.velocity.size.height > gpui::px(0.0)
            && position.y >= grid.velocity.top()
            && position.y <= grid.velocity.bottom()
        {
            if position.x < grid.notes.left() {
                if position.y >= grid.velocity.top() + gpui::px(21.0)
                    && position.y <= grid.velocity.top() + gpui::px(38.0)
                {
                    self.piano_roll.lane_contour = self.piano_roll.lane_contour.next();
                    self.piano_roll.hint = format!(
                        "Contorno del carril: {} · Alt+arrastre traza rampa/curva/onda sobre los palitos",
                        self.piano_roll.lane_contour.label()
                    );
                    return;
                }
                let current_index = super::lane::LANE_MENU_ITEMS
                    .iter()
                    .position(|item| item.lane == self.piano_roll.active_lane)
                    .unwrap_or(0);
                self.piano_roll.lane_menu = Some(super::lane::RollLaneMenu {
                    position,
                    selected: current_index,
                });
                return;
            }
            if self.piano_roll.active_lane.controller().is_some() {
                self.edit_roll_control_at(position);
                return;
            }
            let hit = clip.notes().iter().rev().find(|note| {
                let b = grid.note_bounds(note.start, note.duration, note.pitch);
                position.x >= b.left() - gpui::px(6.0) && position.x <= b.right() + gpui::px(6.0)
            });
            if let Some(note) = hit {
                let id = note.id;
                self.piano_roll.selected = Some(id);
                self.piano_roll.selected_notes = vec![id];
                let lane_h = (f32::from(grid.velocity.size.height) - 16.0).max(1.0);
                let frac =
                    ((f32::from(grid.velocity.bottom() - position.y)) / lane_h).clamp(0.0, 1.0);
                let lane = self.piano_roll.active_lane;
                if let Some(property) = lane.note_property() {
                    if clip.props.locked {
                        self.piano_roll.hint = "Clip bloqueado".into();
                        return;
                    }
                    self.stop_audio();
                    let value = property.value_at_fraction(frac);
                    if let Some(clip) = self.timeline.midi_clip.as_mut() {
                        let _ = clip
                            .edit_pattern(|pattern| pattern.set_note_property(id, property, value));
                    }
                    self.piano_roll.hint = format!(
                        "{}: {}{}",
                        lane.label(),
                        lane.format_value(frac),
                        if matches!(lane, super::RollLane::Pan) {
                            " · audio por nota: FluidSynth"
                        } else {
                            ""
                        }
                    );
                }
                return;
            }
        }
        self.piano_roll.lane_menu = None;
        let Some((tick, pitch)) = self
            .piano_roll
            .grid
            .get()
            .pick(position, clip.pattern.length())
        else {
            return;
        };
        let grid = self.piano_roll.grid.get();
        let hit = clip.notes().iter().rev().find(|note| {
            grid.note_bounds(note.start, note.duration, note.pitch)
                .contains(&position)
        });
        let hit_id = hit.map(|note| note.id);
        self.piano_roll.cursor = tick;
        self.piano_roll.selected = hit_id;
        if matches!(
            self.piano_roll.tool,
            RollTool::Select | RollTool::Box | RollTool::EraseRegion | RollTool::Length
        ) {
            if matches!(self.piano_roll.tool, RollTool::Box | RollTool::EraseRegion)
                || hit.is_none()
            {
                self.piano_roll.selected_notes.clear();
                self.piano_roll.selected = None;
                self.piano_roll.drag = Some(RollDrag::Box {
                    origin: position,
                    current: position,
                    erase: self.piano_roll.tool == RollTool::EraseRegion,
                });
            } else if let Some(note) = hit {
                if clip.props.locked {
                    self.piano_roll.selected_notes = vec![note.id];
                    self.piano_roll.hint = "Clip bloqueado".into();
                    return;
                }
                let id = note.id;
                if !self.piano_roll.selected_notes.contains(&id) {
                    self.piano_roll.selected_notes = vec![id];
                }
                let bounds = grid.note_bounds(note.start, note.duration, note.pitch);
                let edge = grid
                    .axis()
                    .edge_beats(7.0, note.duration.0 as f64 / PPQ as f64)
                    * f64::from(grid.pixels_per_beat);
                let on_right_edge = f64::from(f32::from(bounds.right() - position.x)) <= edge;
                self.piano_roll.drag = Some(
                    if self.piano_roll.tool == RollTool::Length || on_right_edge {
                        RollDrag::Length {
                            origin: position,
                            current: position,
                            original: clip.pattern.clone(),
                            note: id,
                        }
                    } else {
                        RollDrag::Move {
                            origin: position,
                            current: position,
                            original: clip.pattern.clone(),
                            notes: self.piano_roll.selected_notes.clone(),
                        }
                    },
                );
            }
            return;
        }
        self.piano_roll.selected_notes = hit_id.into_iter().collect();
        if clip.props.locked {
            self.piano_roll.hint = "Clip bloqueado".into();
            return;
        }
        if matches!(self.piano_roll.tool, RollTool::Draw | RollTool::Paint) && hit_id.is_none() {
            self.insert_roll_note(tick, pitch, self.piano_roll.draw_length.ticks());
            return;
        }
        if self.piano_roll.tool == RollTool::Erase {
            if let Some(id) = hit_id {
                self.delete_roll_notes(&[id]);
            }
            return;
        }
        self.stop_audio();
        let Some(clip) = self.timeline.midi_clip.as_mut() else {
            return;
        };
        match self.piano_roll.tool {
            RollTool::Paint => {
                if let Some(id) = hit_id {
                    let _ = clip.edit_pattern(|pattern| pattern.set_velocity(id, 100));
                    self.piano_roll.hint = "Velocity: 100".into();
                }
            }
            RollTool::Mute => {
                if let Some(id) = hit_id {
                    clip.detach_drum_link();
                    if !clip.muted_notes.insert(id) {
                        clip.muted_notes.remove(&id);
                    }
                    self.piano_roll.hint = "Silencio de nota alternado".into();
                }
            }
            _ => {}
        }
    }

    /// Usa la misma geometría de picking que el inicio del arrastre.
    pub(in crate::ui) fn hover_roll_at(&mut self, position: Point<Pixels>) -> bool {
        let grid = self.piano_roll.grid.get();
        let hit = self.timeline.midi_clip.as_ref().and_then(|clip| {
            if !grid.notes.contains(&position) {
                return None;
            }
            clip.notes().iter().rev().find_map(|note| {
                let bounds = grid.note_bounds(note.start, note.duration, note.pitch);
                if !bounds.contains(&position) {
                    return None;
                }
                let edge = grid
                    .axis()
                    .edge_beats(7.0, note.duration.0 as f64 / PPQ as f64)
                    * f64::from(grid.pixels_per_beat);
                let resize = f64::from(f32::from(bounds.right() - position.x)) <= edge;
                Some((note.id, resize))
            })
        });
        let next_note = hit.map(|(id, _)| id);
        let next_edge = hit.is_some_and(|(_, resize)| resize);
        let changed = self.piano_roll.hovered_note != next_note
            || self.piano_roll.hovered_resize_edge != next_edge;
        self.piano_roll.hovered_note = next_note;
        self.piano_roll.hovered_resize_edge = next_edge;
        changed
    }
}
