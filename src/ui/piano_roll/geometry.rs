//! Geometría compartida por dibujo y picking, también al cambiar de panel o de ventana.
use elevate::musical::time::Tick;
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
}

impl Grid {
    pub fn new(all: Bounds<Pixels>, beats: f64, velocities: bool) -> Self {
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
            row_height: height / f32::from(LAST_PITCH - FIRST_PITCH + 1),
            pixels_per_beat: width / beats as f32,
        }
    }

    pub fn x(&self, beat: f64) -> Pixels {
        self.notes.left() + px(beat as f32 * self.pixels_per_beat)
    }

    pub fn y(&self, pitch: u8) -> Pixels {
        self.notes.top() + px(f32::from(LAST_PITCH.saturating_sub(pitch)) * self.row_height)
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
        let fraction = f32::from(position.x - self.notes.left()) / f32::from(self.notes.size.width);
        let tick = Tick((fraction as f64 * length.0 as f64) as i64);
        let row = (f32::from(position.y - self.notes.top()) / self.row_height) as u8;
        Some((tick, LAST_PITCH.saturating_sub(row)))
    }
}
