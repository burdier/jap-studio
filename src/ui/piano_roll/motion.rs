//! Interpolación breve de la vista; el gesto y el patrón conservan sus coordenadas exactas.
use super::{RollDrag, geometry::Grid};
use elevate::musical::{
    model::{NoteId, Pattern},
    time::PPQ,
};
use gpui::{Pixels, Point, point, px};
use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

#[derive(Default)]
pub(in crate::ui) struct NeighborMotion {
    /// Desplazamiento del borde inicial/final, en píxeles respecto al patrón original.
    pub offset: Point<Pixels>,
    target: Point<Pixels>,
}

pub(in crate::ui) struct NotePreview {
    pub notes: HashSet<NoteId>,
    pub resize: bool,
    pub offset: Point<Pixels>,
    pub dragging: bool,
    pub neighbors: HashMap<NoteId, NeighborMotion>,
}

pub(in crate::ui) struct NoteMotion {
    pub preview: Option<NotePreview>,
    target: Point<Pixels>,
    frame_at: Instant,
}

impl Default for NoteMotion {
    fn default() -> Self {
        Self {
            preview: None,
            target: Point::default(),
            frame_at: Instant::now(),
        }
    }
}

impl NoteMotion {
    pub fn clear(&mut self) {
        self.preview = None;
    }

    pub fn begin(&mut self, drag: Option<&RollDrag>) {
        *self = Self::default();
        let (notes, resize) = match drag {
            Some(RollDrag::Move { notes, .. }) => (notes.iter().copied().collect(), false),
            Some(RollDrag::Length { note, .. }) => (HashSet::from([*note]), true),
            _ => return,
        };
        self.preview = Some(NotePreview {
            notes,
            resize,
            offset: Point::default(),
            dragging: true,
            neighbors: HashMap::new(),
        });
    }

    pub fn retarget(&mut self, offset: Point<Pixels>) {
        self.target = offset;
        if self.preview.as_ref().is_some_and(|preview| preview.resize) {
            self.target.y = px(0.0);
        }
    }

    pub fn push_neighbors(
        &mut self,
        pattern: &Pattern,
        spans: &HashMap<NoteId, (f64, f64)>,
        grid: Grid,
    ) {
        let Some(preview) = &mut self.preview else {
            return;
        };
        for neighbor in preview.neighbors.values_mut() {
            neighbor.target = Point::default();
        }
        let scale = f64::from(grid.pixels_per_beat) / PPQ as f64;
        for note in pattern.notes() {
            if let Some((start, end)) = spans.get(&note.id) {
                preview.neighbors.entry(note.id).or_default().target = point(
                    px(((start - note.start.0 as f64) * scale) as f32),
                    px(((end - (note.start.0 + note.duration.0) as f64) * scale) as f32),
                );
            }
        }
    }

    /// Devuelve demanda de un frame adicional solo mientras queda distancia visual.
    pub fn advance(&mut self) -> bool {
        let now = Instant::now();
        let dt = now.duration_since(self.frame_at).as_secs_f32().min(0.05);
        self.frame_at = now;
        let Some(preview) = &mut self.preview else {
            return false;
        };
        let response = if preview.dragging { 0.012 } else { 0.020 };
        let alpha = 1.0 - (-dt / response).exp();
        let mut distance = self.target - preview.offset;
        // No arrastrar una estela: incluso con movimientos rápidos el retraso es <= 6 px.
        if preview.dragging {
            distance.x = px(f32::from(distance.x).clamp(-6.0, 6.0));
            distance.y = px(f32::from(distance.y).clamp(-6.0, 6.0));
        }
        preview.offset = self.target - distance * (1.0 - alpha);
        let mut pending = (self.target.x - preview.offset.x).abs() > px(0.1)
            || (self.target.y - preview.offset.y).abs() > px(0.1);
        for neighbor in preview.neighbors.values_mut() {
            let mut distance = neighbor.target - neighbor.offset;
            if preview.dragging {
                distance.x = px(f32::from(distance.x).clamp(-6.0, 6.0));
                distance.y = px(f32::from(distance.y).clamp(-6.0, 6.0));
            }
            neighbor.offset = neighbor.target - distance * (1.0 - alpha);
            let changing = (neighbor.target.x - neighbor.offset.x).abs() > px(0.1)
                || (neighbor.target.y - neighbor.offset.y).abs() > px(0.1);
            pending |= changing;
            if !changing {
                neighbor.offset = neighbor.target;
            }
        }
        if !pending {
            preview.offset = self.target;
            if !preview.dragging {
                self.preview = None;
            }
        }
        pending
    }

    /// El modelo ya está ajustado a rejilla; solo el dibujo termina de acomodarse.
    pub fn settle(&mut self, before: &Pattern, after: &Pattern, grid: Grid) {
        let Some(preview) = &mut self.preview else {
            return;
        };
        let Some(old) = before
            .notes()
            .iter()
            .find(|note| preview.notes.contains(&note.id))
        else {
            self.clear();
            return;
        };
        let Some(new) = after.notes().iter().find(|note| note.id == old.id) else {
            self.clear();
            return;
        };
        let committed = if preview.resize {
            point(
                px((new.duration.0 - old.duration.0) as f32 / PPQ as f32 * grid.pixels_per_beat),
                px(0.0),
            )
        } else {
            point(
                px((new.start.0 - old.start.0) as f32 / PPQ as f32 * grid.pixels_per_beat),
                px((f32::from(old.pitch) - f32::from(new.pitch)) * grid.row_height),
            )
        };
        preview.offset -= committed;
        let after: HashMap<_, _> = after.notes().iter().map(|note| (note.id, note)).collect();
        for old in before.notes() {
            if !preview.notes.contains(&old.id)
                && after
                    .get(&old.id)
                    .is_some_and(|new| new.start != old.start || new.duration != old.duration)
            {
                preview.neighbors.entry(old.id).or_default();
            }
            if let Some(edges) = preview.neighbors.get_mut(&old.id) {
                if let Some(new) = after.get(&old.id) {
                    let scale = grid.pixels_per_beat / PPQ as f32;
                    edges.offset -= point(
                        px((new.start.0 - old.start.0) as f32 * scale),
                        px(
                            (new.start.0 + new.duration.0 - old.start.0 - old.duration.0) as f32
                                * scale,
                        ),
                    );
                    edges.target = Point::default();
                }
            }
        }
        preview.dragging = false;
        self.target = Point::default();
        self.frame_at = Instant::now();
    }
}
