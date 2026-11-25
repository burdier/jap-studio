//! Los puntos pertenecen a una nota, y el gesto se confirma al soltar el ratón.
use super::geometry::Grid;
use crate::ui::DawApp;
use elevate::musical::{
    clip::ClipId,
    model::{NoteEvent, NoteId},
    pitch_curve::{PITCH_RANGE_CENTS, PitchCurve, PitchPoint},
    time::PPQ,
};
use gpui::{MouseDownEvent, Pixels, Point, point, px};

pub(in crate::ui) struct PitchDrag {
    clip: ClipId,
    pub note: NoteId,
    base: Vec<PitchPoint>,
    anchor: Option<u16>,
    pub preview: PitchCurve,
}

pub(in crate::ui) fn pitch_position(
    grid: Grid,
    note: &NoteEvent,
    anchor: PitchPoint,
) -> Point<Pixels> {
    let offset = note.duration.0 as f64 * f64::from(anchor.at) / f64::from(u16::MAX);
    let beat = (note.start.0 as f64 + offset) / PPQ as f64;
    point(
        grid.x(beat),
        grid.y(note.pitch) + px(grid.row_height * (0.5 - f32::from(anchor.cents) / 100.0)),
    )
}

fn pitch_time(grid: Grid, note: &NoteEvent, x: Pixels) -> u16 {
    let left = grid.x(note.start.0 as f64 / PPQ as f64);
    let right = grid.x((note.start.0 + note.duration.0) as f64 / PPQ as f64);
    let width = f32::from(right - left).max(f32::EPSILON);
    (f32::from(x - left).clamp(0.0, width) / width * f32::from(u16::MAX)).round() as u16
}

impl DawApp {
    pub(super) fn begin_roll_pitch(&mut self, event: &MouseDownEvent) {
        let Some(clip) = &self.timeline.midi_clip else {
            return;
        };
        if clip.props.locked {
            self.piano_roll.hint = "Clip bloqueado".into();
            return;
        }
        let grid = self.piano_roll.grid.get();
        let pointer = event.position;
        let nearest = clip
            .notes()
            .iter()
            .rev()
            .filter_map(|note| {
                let bounds = grid.note_bounds(note.start, note.duration, note.pitch);
                if pointer.x < bounds.left() - px(6.0) || pointer.x > bounds.right() + px(6.0) {
                    return None;
                }
                let at = pitch_time(grid, note, pointer.x);
                let cents = note.pitch_curve.cents_at(at);
                let position = pitch_position(grid, note, PitchPoint { at, cents });
                let distance = f32::from((position.y - pointer.y).abs());
                (distance <= 6.0).then_some((note, distance))
            })
            .min_by(|(_, first), (_, second)| first.total_cmp(second))
            .map(|(note, _)| note);
        let note = nearest.or_else(|| {
            clip.notes().iter().find(|note| {
                let bounds = grid.note_bounds(note.start, note.duration, note.pitch);
                self.piano_roll.selected == Some(note.id)
                    && pointer.x >= bounds.left() - px(2.0)
                    && pointer.x <= bounds.right() + px(1.0)
            })
        });
        let Some(note) = note else {
            self.piano_roll.selected = None;
            self.piano_roll.selected_notes.clear();
            return;
        };
        let note_id = note.id;
        let mut base = note.pitch_curve.points().to_vec();
        let anchor = base.iter().position(|&anchor| {
            let at = pitch_position(grid, note, anchor);
            (at.x - pointer.x).abs() <= px(5.0) && (at.y - pointer.y).abs() <= px(6.0)
        });
        self.piano_roll.selected = Some(note_id);
        self.piano_roll.selected_notes = vec![note_id];
        if event.click_count == 2 {
            if let Some(index) = anchor {
                base.remove(index);
                if let Ok(curve) = PitchCurve::new(base) {
                    self.apply_roll_pitch(clip.id, note_id, curve);
                }
            }
            return;
        }
        if event.modifiers.alt {
            self.apply_roll_pitch(clip.id, note_id, PitchCurve::default());
            return;
        }
        let anchor = anchor.map(|index| base.remove(index).at);
        // Un clic en la línea selecciona. Arrastrarla o pulsar fuera de ella añade un punto.
        self.piano_roll.pitch_drag = Some(PitchDrag {
            clip: clip.id,
            note: note_id,
            base,
            anchor,
            preview: note.pitch_curve.clone(),
        });
        if nearest.is_none() {
            self.drag_roll_pitch(pointer, event.modifiers.shift);
        }
    }

    pub(in crate::ui) fn drag_roll_pitch(&mut self, position: Point<Pixels>, fine: bool) {
        let Some(drag) = &mut self.piano_roll.pitch_drag else {
            return;
        };
        let Some(clip) = self
            .timeline
            .midi_clip
            .as_ref()
            .filter(|clip| clip.id == drag.clip)
        else {
            return;
        };
        let Some(note) = clip.notes().iter().find(|note| note.id == drag.note) else {
            return;
        };
        let grid = self.piano_roll.grid.get();
        let center = grid.y(note.pitch) + px(0.5 * grid.row_height);
        let semitones = f32::from(center - position.y) / grid.row_height;
        let cents = if fine {
            (semitones * 100.0).round()
        } else {
            semitones.round() * 100.0
        };
        let cents = (cents as i16).clamp(-PITCH_RANGE_CENTS, PITCH_RANGE_CENTS);
        let at = match drag.anchor {
            Some(at @ (0 | u16::MAX)) => at,
            _ => pitch_time(grid, note, position.x),
        };
        let mut points = drag.base.clone();
        let point = PitchPoint { at, cents };
        match points.binary_search_by_key(&at, |point| point.at) {
            Ok(index) => points[index] = point,
            Err(index) => points.insert(index, point),
        }
        if let Ok(curve) = PitchCurve::new(points) {
            drag.preview = curve;
            self.piano_roll.hint =
                format!("{:+.2} st · Shift para cents", f32::from(cents) / 100.0);
        }
    }

    pub(super) fn finish_roll_pitch(&mut self) -> bool {
        let Some(drag) = self.piano_roll.pitch_drag.take() else {
            return false;
        };
        self.apply_roll_pitch(drag.clip, drag.note, drag.preview);
        true
    }

    fn apply_roll_pitch(&mut self, clip_id: ClipId, note: NoteId, curve: PitchCurve) {
        let Some(clip) = self
            .timeline
            .midi_clip
            .as_ref()
            .filter(|clip| clip.id == clip_id && !clip.props.locked)
        else {
            return;
        };
        if !clip
            .notes()
            .iter()
            .any(|event| event.id == note && event.pitch_curve != curve)
        {
            return;
        }
        self.stop_audio();
        if let Some(clip) = self.timeline.midi_clip.as_mut() {
            let _ = clip.edit_pattern(|pattern| pattern.set_pitch_curve(note, curve));
        }
    }
}
