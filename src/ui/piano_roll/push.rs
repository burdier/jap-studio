//! Coordina proyección continua, política Lua y confirmación con el mismo snap del grid.
use super::{RollDrag, geometry::Grid, motion::NoteMotion};
use crate::ui::{
    DawApp,
    time_grid::{TimeSpan, magnet::MagnetHit},
};
use elevate::{
    musical::{model::Pattern, time::PPQ},
    scripting::push::NotePush,
};
use gpui::{Pixels, Point, point, px};

pub(super) struct PushGesture {
    session: NotePush,
    span: TimeSpan,
    length: f64,
    min_pitch: i32,
    max_pitch: i32,
    resize: bool,
}

impl PushGesture {
    fn new(drag: &RollDrag) -> Result<Self, String> {
        let (pattern, ids, resize) = match drag {
            RollDrag::Move {
                original, notes, ..
            } => (original, notes.clone(), false),
            RollDrag::Length { original, note, .. } => (original, vec![*note], true),
            _ => return Err("El gesto no mueve notas".into()),
        };
        let ids_set: std::collections::HashSet<_> = ids.iter().copied().collect();
        let notes: Vec<_> = pattern
            .notes()
            .iter()
            .filter(|note| ids_set.contains(&note.id))
            .collect();
        if notes.is_empty() {
            return Err("No hay notas seleccionadas".into());
        }
        Ok(Self {
            session: NotePush::new(pattern, &ids)?,
            span: TimeSpan {
                start: notes.iter().map(|n| n.start.0).min().unwrap() as f64 / PPQ as f64,
                end: notes
                    .iter()
                    .map(|n| n.start.0 + n.duration.0)
                    .max()
                    .unwrap() as f64
                    / PPQ as f64,
            },
            length: pattern.length().0 as f64 / PPQ as f64,
            min_pitch: i32::from(notes.iter().map(|n| n.pitch).min().unwrap()),
            max_pitch: i32::from(notes.iter().map(|n| n.pitch).max().unwrap()),
            resize,
        })
    }

    fn parameters(&self, grid: Grid, offset: Point<Pixels>, step: f64) -> (f64, i32) {
        let delta = f64::from(f32::from(offset.x)) / f64::from(grid.pixels_per_beat);
        if self.resize {
            let end = if step > 0.0 {
                grid.axis()
                    .resize_end(self.span, delta, self.length, 1.0 / PPQ as f64, step)
                    .end
            } else {
                (self.span.end + delta).clamp(self.span.start + 1.0 / PPQ as f64, self.length)
            };
            ((end - self.span.end) * PPQ as f64, 0)
        } else {
            let moved = grid
                .axis()
                .move_span(self.span, delta, 0.0, self.length, step);
            let pitch = (-(f32::from(offset.y) / grid.row_height).round() as i32)
                .clamp(-self.min_pitch, 127 - self.max_pitch);
            ((moved.start - self.span.start) * PPQ as f64, pitch)
        }
    }

    /// Proyección continua con imán: cerca de una línea usa el mismo destino que el commit.
    /// Devuelve el borde visible (inicio al mover, final al redimensionar) y el imán.
    fn preview(
        &mut self,
        pattern: &Pattern,
        grid: Grid,
        offset: Point<Pixels>,
        step: f64,
        motion: &mut NoteMotion,
    ) -> Result<(f64, Option<MagnetHit>), String> {
        let (free, free_pitch) = self.parameters(grid, offset, 0.0);
        let (snapped, _) = self.parameters(grid, offset, step);
        let base = if self.resize {
            self.span.end
        } else {
            self.span.start
        };
        let ppq = PPQ as f64;
        let hit = grid
            .axis()
            .magnet_to(base + free / ppq, base + snapped / ppq, step);
        let delta = if hit.is_some_and(|hit| hit.captured) {
            snapped
        } else {
            free
        };
        let result = self.session.project(delta, free_pitch, self.resize)?;
        let y = if self.resize {
            px(0.0)
        } else {
            px(f32::from(offset.y).clamp(
                -(127 - self.max_pitch) as f32 * grid.row_height,
                self.min_pitch as f32 * grid.row_height,
            ))
        };
        motion.retarget(point(
            px(result.delta as f32 / PPQ as f32 * grid.pixels_per_beat),
            y,
        ));
        motion.push_neighbors(pattern, &result.neighbors, grid);
        Ok((base + result.delta as f64 / ppq, hit))
    }

    pub fn commit(
        &mut self,
        original: &Pattern,
        grid: Grid,
        offset: Point<Pixels>,
        step: f64,
    ) -> Result<Pattern, String> {
        let (delta, pitch) = self.parameters(grid, offset, step);
        self.session.commit(original, delta, pitch, self.resize)
    }
}

impl DawApp {
    pub(super) fn update_roll_push(&mut self) {
        if self.piano_roll.push_error.is_some() {
            return;
        }
        let Some(drag) = self.piano_roll.drag.as_ref() else {
            return;
        };
        let (origin, current, original) = match drag {
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
            } => (*origin, *current, original),
            _ => return,
        };
        let offset = current - origin;
        if self.piano_roll.push.is_none() {
            if offset.x.abs() < px(2.0)
                && (matches!(drag, RollDrag::Length { .. }) || offset.y.abs() < px(2.0))
            {
                return;
            }
            match PushGesture::new(drag) {
                Ok(push) => self.piano_roll.push = Some(push),
                Err(error) => {
                    self.piano_roll.hint = error.clone();
                    self.piano_roll.push_error = Some(error);
                    self.piano_roll.motion.borrow_mut().clear();
                    return;
                }
            }
        }
        let step = self.piano_roll.resolution.ticks().0 as f64 / PPQ as f64;
        if let Some(push) = &mut self.piano_roll.push {
            let result = push.preview(
                original,
                self.piano_roll.grid.get(),
                offset,
                step,
                &mut self.piano_roll.motion.borrow_mut(),
            );
            match result {
                Ok((edge, hit)) => self.piano_roll.snap_guide.track(edge, hit),
                Err(error) => {
                    self.piano_roll.hint = error.clone();
                    self.piano_roll.push_error = Some(error);
                    self.piano_roll.motion.borrow_mut().clear();
                    self.piano_roll.snap_guide.clear();
                }
            }
        }
    }
}
