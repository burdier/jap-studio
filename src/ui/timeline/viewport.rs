//! Transformación exclusivamente visual beat ↔ píxel. No gobierna el transporte.
use crate::ui::time_grid::TimeGrid;

pub(super) const MAX_SCALE: f64 = 32_768.0;

#[derive(Clone, Copy)]
pub(super) struct Viewport {
    pub first_beat: f64,
    pub pixels_per_beat: f64,
}
impl Viewport {
    pub fn beat_at(&self, x: f64) -> f64 {
        self.grid().beat_at(x)
    }
    pub fn x_at(&self, beat: f64) -> f64 {
        self.grid().x_at(beat)
    }
    pub fn grid(&self) -> TimeGrid {
        TimeGrid {
            first_beat: self.first_beat,
            pixels_per_beat: self.pixels_per_beat,
        }
    }
    pub fn zoom_at(&mut self, x: f64, factor: f64) {
        if !x.is_finite() || !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let anchor = self.beat_at(x);
        self.pixels_per_beat = (self.pixels_per_beat * factor).clamp(8.0, MAX_SCALE);
        self.first_beat = (anchor - x / self.pixels_per_beat).max(0.0);
    }
    pub fn grid_step(&self) -> f64 {
        self.grid().grid_step()
    }
}
