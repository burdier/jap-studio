//! Transformación exclusivamente visual beat ↔ píxel. No gobierna el transporte.
use super::BEATS_PER_BAR;

#[derive(Clone, Copy)]
pub(super) struct Viewport {
    pub first_beat: f64,
    pub pixels_per_beat: f64,
}
impl Viewport {
    pub fn beat_at(&self, x: f64) -> f64 {
        self.first_beat + x / self.pixels_per_beat
    }
    pub fn x_at(&self, beat: f64) -> f64 {
        (beat - self.first_beat) * self.pixels_per_beat
    }
    pub fn zoom_at(&mut self, x: f64, factor: f64) {
        let anchor = self.beat_at(x);
        self.pixels_per_beat = (self.pixels_per_beat * factor).clamp(8.0, 256.0);
        self.first_beat = (anchor - x / self.pixels_per_beat).max(0.0);
    }
    pub fn grid_step(&self) -> f64 {
        if self.pixels_per_beat >= 48.0 {
            0.25
        } else if self.pixels_per_beat >= 24.0 {
            0.5
        } else if self.pixels_per_beat >= 12.0 {
            1.0
        } else {
            BEATS_PER_BAR
        }
    }
}
