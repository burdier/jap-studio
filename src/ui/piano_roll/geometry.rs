//! Geometría compartida por dibujo y picking, también al cambiar de panel o de ventana.
use super::navigation::RollViewport;
use elevate::musical::time::{PPQ, Tick};
use gpui::{Bounds, Pixels, Point, point, px, size};

pub(in crate::ui) const FIRST_PITCH: u8 = 48;
pub(in crate::ui) const LAST_PITCH: u8 = 84;
pub(in crate::ui) const KEYBOARD_WIDTH: f32 = 96.0;
const RULER_HEIGHT: f32 = 44.0;

#[derive(Clone, Copy, Default)]
pub(in crate::ui) struct Grid {
    pub all: Bounds<Pixels>,
    pub notes: Bounds<Pixels>,
    pub velocity: Bounds<Pixels>,
    pub row_height: f32,
    pub pixels_per_beat: f32,
    pub first_beat: f64,
    pub top_pitch: f32,
}

impl Grid {
    pub fn new(all: Bounds<Pixels>, beats: f64, velocities: bool, viewport: RollViewport) -> Self {
        let width = (f32::from(all.size.width) - KEYBOARD_WIDTH).max(1.0);
        let available = f32::from(all.size.height);
        let velocity_height = if velocities {
            (available * 0.25).min(94.0)
        } else {
            0.0
        };
        let height = (available - RULER_HEIGHT - velocity_height - 10.0).max(1.0);
        let notes = Bounds::new(
            all.origin + point(px(KEYBOARD_WIDTH), px(RULER_HEIGHT)),
            size(px(width), px(height)),
        );
        let velocity = Bounds::new(
            point(notes.left(), notes.bottom() + px(4.0)),
            size(notes.size.width, px((velocity_height - 4.0).max(0.0))),
        );
        Self {
            all,
            notes,
            velocity,
            row_height: viewport.row_height(height),
            pixels_per_beat: width / beats as f32 * viewport.zoom as f32,
            first_beat: viewport
                .first_beat
                .clamp(0.0, (beats - beats / viewport.zoom).max(0.0)),
            top_pitch: viewport.bounded_top_pitch(),
        }
    }

    pub fn x(&self, beat: f64) -> Pixels {
        self.notes.left() + px((beat - self.first_beat) as f32 * self.pixels_per_beat)
    }

    pub fn y(&self, pitch: u8) -> Pixels {
        self.notes.top() + px((self.top_pitch - f32::from(pitch)) * self.row_height)
    }

    pub fn pick(&self, position: Point<Pixels>, length: Tick) -> Option<(Tick, u8)> {
        if !self.notes.contains(&position)
            || position.x >= self.notes.right()
            || position.y >= self.notes.bottom()
            || self.notes.size.width <= px(0.0)
            || self.row_height <= 0.0
        {
            return None;
        }
        let beat = self.first_beat
            + f64::from(f32::from(position.x - self.notes.left()) / self.pixels_per_beat);
        let tick = Tick(((beat * PPQ as f64).floor() as i64).clamp(0, (length.0 - 1).max(0)));
        let row = f32::from(position.y - self.notes.top()) / self.row_height;
        Some((tick, (self.top_pitch - row).ceil().clamp(0.0, 127.0) as u8))
    }
}
