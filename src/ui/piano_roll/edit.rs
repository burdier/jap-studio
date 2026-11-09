//! Operaciones MIDI del piano roll; botones, teclas y canvas llaman estos controladores.
use super::{RollDrag, RollTool};
use crate::ui::DawApp;
use crate::ui::time_grid::TimeSpan;
use elevate::musical::{
    model::Pattern,
    time::{PPQ, Tick},
};
use gpui::{Pixels, Point};

const STEP: i64 = PPQ / 4; // 1/16 de compás en 4/4.
const DRAW_LENGTH: i64 = PPQ / 2; // 1/8.

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
        let Some(index) = self
            .timeline
            .other_midi_clips
            .iter()
            .position(|clip| clip.id == order[next].0)
        else {
            return;
        };
        let clip = self.timeline.other_midi_clips.remove(index);
        let previous = self.timeline.midi_clip.replace(clip).expect("clip activo");
        self.timeline.other_midi_clips.push(previous);
        self.open_piano_roll();
        self.piano_roll.hint = "Clip MIDI activo cambiado".into();
    }

    pub(in crate::ui) fn edit_roll_at(&mut self, position: Point<Pixels>) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
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
            RollTool::Select | RollTool::Box | RollTool::Length
        ) {
            if self.piano_roll.tool == RollTool::Box || hit.is_none() {
                self.piano_roll.selected_notes.clear();
                self.piano_roll.selected = None;
                self.piano_roll.drag = Some(RollDrag::Box {
                    origin: position,
                    current: position,
                });
            } else if let Some(note) = hit {
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
        let instrument = self
            .tracks
            .get(clip.track)
            .and_then(|track| track.instrument)
            .unwrap_or(self.instrument.id());
        self.stop_audio();
        let Some(clip) = self.timeline.midi_clip.as_mut() else {
            return;
        };
        if clip.props.locked {
            self.piano_roll.hint = "Clip bloqueado".into();
            return;
        }
        match self.piano_roll.tool {
            RollTool::Draw | RollTool::Paint if hit_id.is_none() => {
                let start = tick.0 / STEP * STEP;
                let duration = DRAW_LENGTH.min(clip.pattern.length().0 - start);
                if let Ok(id) =
                    clip.pattern
                        .add_note(instrument, Tick(start), Tick(duration), pitch, 100)
                {
                    self.piano_roll.selected = Some(id);
                    self.piano_roll.selected_notes = vec![id];
                    self.piano_roll.hint = "Nota insertada".into();
                }
            }
            RollTool::Paint => {
                if let Some(id) = hit_id {
                    let _ = clip.pattern.set_velocity(id, 100);
                    self.piano_roll.hint = "Velocity: 100".into();
                }
            }
            RollTool::Erase => {
                if let Some(id) = hit_id {
                    let _ = clip.pattern.remove_note(id);
                    clip.muted_notes.remove(&id);
                    self.piano_roll.selected = None;
                    self.piano_roll.selected_notes.clear();
                    self.piano_roll.hint = "Nota borrada".into();
                }
            }
            RollTool::Mute => {
                if let Some(id) = hit_id {
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

    pub(in crate::ui) fn drag_roll_to(&mut self, position: Point<Pixels>) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        let grid = self.piano_roll.grid.get();
        match self.piano_roll.drag.as_mut() {
            Some(RollDrag::Box { origin, current }) => {
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
    }

    pub(in crate::ui) fn finish_roll_drag(&mut self, position: Point<Pixels>) {
        self.drag_roll_to(position);
        let Some(drag) = self.piano_roll.drag.take() else {
            return;
        };
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        if clip.props.locked {
            return;
        }
        let grid = self.piano_roll.grid.get();
        let replacement = match drag {
            RollDrag::Box { .. } => return,
            RollDrag::Move {
                origin,
                current,
                original,
                notes,
            } => {
                let delta =
                    f64::from(f32::from(current.x - origin.x)) / f64::from(grid.pixels_per_beat);
                let selected: Vec<_> = original
                    .notes()
                    .iter()
                    .filter(|note| notes.contains(&note.id))
                    .collect();
                let min_start = selected.iter().map(|note| note.start.0).min().unwrap_or(0);
                let max_end = selected
                    .iter()
                    .map(|note| note.start.0 + note.duration.0)
                    .max()
                    .unwrap_or(0);
                let span = TimeSpan {
                    start: min_start as f64 / PPQ as f64,
                    end: max_end as f64 / PPQ as f64,
                };
                let moved = grid.axis().move_span(
                    span,
                    delta,
                    0.0,
                    original.length().0 as f64 / PPQ as f64,
                    STEP as f64 / PPQ as f64,
                );
                let ticks = ((moved.start - span.start) * PPQ as f64).round() as i64;
                let mut pitch_delta =
                    (f32::from(origin.y - current.y) / grid.row_height).round() as i32;
                let min_pitch = selected.iter().map(|note| note.pitch).min().unwrap_or(0) as i32;
                let max_pitch = selected.iter().map(|note| note.pitch).max().unwrap_or(127) as i32;
                pitch_delta = pitch_delta.clamp(-min_pitch, 127 - max_pitch);
                let mut changed = original.notes().to_vec();
                for note in &mut changed {
                    if notes.contains(&note.id) {
                        note.start.0 += ticks;
                        note.pitch = (i32::from(note.pitch) + pitch_delta) as u8;
                    }
                }
                Pattern::restore_with_next_id(original.length(), changed, original.next_id()).ok()
            }
            RollDrag::Length {
                origin,
                current,
                original,
                note,
            } => {
                let delta =
                    f64::from(f32::from(current.x - origin.x)) / f64::from(grid.pixels_per_beat);
                let mut changed = original.notes().to_vec();
                if let Some(target) = changed.iter_mut().find(|entry| entry.id == note) {
                    let span = TimeSpan {
                        start: target.start.0 as f64 / PPQ as f64,
                        end: (target.start.0 + target.duration.0) as f64 / PPQ as f64,
                    };
                    let resized = grid.axis().resize_end(
                        span,
                        delta,
                        original.length().0 as f64 / PPQ as f64,
                        1.0 / PPQ as f64,
                        STEP as f64 / PPQ as f64,
                    );
                    target.duration.0 = ((resized.end - resized.start) * PPQ as f64).round() as i64;
                    Pattern::restore_with_next_id(original.length(), changed, original.next_id())
                        .ok()
                } else {
                    None
                }
            }
        };
        if let Some(pattern) = replacement {
            if clip.pattern.notes() == pattern.notes() {
                return;
            }
            self.stop_audio();
            if let Some(clip) = self.timeline.midi_clip.as_mut() {
                clip.pattern = pattern;
            }
        }
    }

    pub(in crate::ui) fn end_roll_drag(&mut self) {
        self.piano_roll.drag = None;
    }

    pub(in crate::ui) fn roll_quantize(&mut self) {
        self.transform_roll_notes(
            |note, length| {
                note.start.0 =
                    ((note.start.0 + STEP / 2) / STEP * STEP).min(length - note.duration.0);
            },
            "Notas cuantizadas a 1/16",
        );
    }

    pub(in crate::ui) fn roll_humanize(&mut self) {
        let amount = self.piano_roll.humanize_amount;
        self.transform_roll_notes(
            move |note, length| {
                let seed = note.id.0.wrapping_mul(0x9e3779b97f4a7c15);
                // Jitter de tiempo: ±12 ticks a 100 %; escala linealmente con `amount`.
                let raw_jitter = ((seed >> 24) % 25) as i64 - 12;
                let jitter = raw_jitter * i64::from(amount) / 100;
                note.start.0 = (note.start.0 + jitter).clamp(0, length - note.duration.0);
                // Jitter de velocity: ±8 a 100 %; escala linealmente con `amount`.
                let raw_vel = (((seed >> 40) % 17) as i16) - 8;
                let vel_jitter = raw_vel * i16::from(amount) / 100;
                let velocity = i16::from(note.velocity) + vel_jitter;
                note.velocity = velocity.clamp(1, 127) as u8;
            },
            "Tiempo y velocity humanizados",
        );
    }

    fn transform_roll_notes(
        &mut self,
        mut edit: impl FnMut(&mut elevate::musical::model::NoteEvent, i64),
        label: &str,
    ) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        if clip.props.locked {
            self.piano_roll.hint = "Clip bloqueado".into();
            return;
        }
        let selected = self.piano_roll.selected_notes.clone();
        let mut notes = clip.notes().to_vec();
        let length = clip.pattern.length().0;
        let next_id = clip.pattern.next_id();
        for note in &mut notes {
            if selected.is_empty() || selected.contains(&note.id) {
                edit(note, length);
            }
        }
        if let Ok(pattern) = Pattern::restore_with_next_id(Tick(length), notes, next_id) {
            self.stop_audio();
            if let Some(clip) = self.timeline.midi_clip.as_mut() {
                clip.pattern = pattern;
            }
            self.piano_roll.hint = label.into();
        }
    }

    pub(in crate::ui) fn roll_copy(&mut self, cut: bool) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        if cut && clip.props.locked {
            self.piano_roll.hint = "Clip bloqueado".into();
            return;
        }
        let Some(note) = clip
            .notes()
            .iter()
            .find(|note| Some(note.id) == self.piano_roll.selected)
            .copied()
        else {
            return;
        };
        self.piano_roll.copied_note = Some(note);
        if cut && !clip.props.locked {
            self.stop_audio();
            if let Some(clip) = self.timeline.midi_clip.as_mut() {
                let _ = clip.pattern.remove_note(note.id);
                clip.muted_notes.remove(&note.id);
            }
            self.piano_roll.selected = None;
            self.piano_roll.selected_notes.clear();
        }
        self.piano_roll.hint = if cut { "Nota cortada" } else { "Nota copiada" }.into();
    }

    pub(in crate::ui) fn roll_paste(&mut self) {
        let Some(note) = self.piano_roll.copied_note else {
            return;
        };
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        if clip.props.locked {
            self.piano_roll.hint = "Clip bloqueado".into();
            return;
        }
        let start = self.piano_roll.cursor.0.max(0);
        let duration = note.duration.0.min(clip.pattern.length().0 - start);
        let instrument = self
            .tracks
            .get(clip.track)
            .and_then(|track| track.instrument)
            .unwrap_or(note.instrument);
        if duration <= 0 {
            return;
        }
        self.stop_audio();
        if let Some(clip) = self.timeline.midi_clip.as_mut() {
            if let Ok(id) = clip.pattern.add_note(
                instrument,
                Tick(start),
                Tick(duration),
                note.pitch,
                note.velocity,
            ) {
                self.piano_roll.selected = Some(id);
                self.piano_roll.selected_notes = vec![id];
                self.piano_roll.hint = "Nota pegada".into();
            }
        }
    }
}
