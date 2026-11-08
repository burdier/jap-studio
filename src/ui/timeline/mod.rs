//! Estado de navegación del Arrangement. El canvas está en paint.rs.
mod interaction;
mod paint;
mod particles;
#[cfg(test)]
mod tests;
mod viewport;
mod waveform;

use super::DesignSystem;
use crate::audio::AudioClip;
use gpui::{Bounds, Pixels, Point};
use particles::ParticleTrail;
use std::{cell::Cell, rc::Rc};
use viewport::Viewport;

const BEATS_PER_BAR: f64 = 4.0;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum TimelineArea {
    Ruler,
    Lanes,
}

pub(crate) struct Timeline {
    clip_drag: Option<(Point<Pixels>, f64, bool)>,
    // Clip provisional. Migrar al modelo de proyecto al introducir múltiples clips.
    pub clip: Option<AudioClip>,
    pub playhead: Option<f64>,
    viewport: Viewport,
    edit_beat: f64,
    ruler_bounds: Rc<Cell<Bounds<Pixels>>>,
    lanes_bounds: Rc<Cell<Bounds<Pixels>>>,
    particles: ParticleTrail,
}

impl Timeline {
    pub(crate) fn new(pixels_per_beat: f32) -> Self {
        Self {
            clip_drag: None,
            clip: None,
            playhead: None,
            viewport: Viewport {
                first_beat: 0.0,
                pixels_per_beat: f64::from(pixels_per_beat).clamp(8.0, 256.0),
            },
            edit_beat: 0.0,
            ruler_bounds: Rc::default(),
            lanes_bounds: Rc::default(),
            particles: ParticleTrail::default(),
        }
    }
    pub(crate) fn set_scale(&mut self, pixels_per_beat: f32) {
        self.viewport.zoom_at(
            0.0,
            f64::from(pixels_per_beat) / self.viewport.pixels_per_beat,
        );
    }
    pub(crate) fn edit_beat(&self) -> f64 {
        self.edit_beat
    }
    pub(crate) fn set_edit_beat(&mut self, beat: f64) {
        self.edit_beat = beat.max(0.0);
        self.move_cursor(0.0);
    }
    pub(crate) fn position_label(&self) -> String {
        let rounded = (self.edit_beat * 100.0).round() / 100.0;
        let bar = (rounded / BEATS_PER_BAR).floor() as u64 + 1;
        format!("{bar}:{:.2}", rounded % BEATS_PER_BAR + 1.0)
    }
    pub(crate) fn go_to_start(&mut self) {
        self.edit_beat = 0.0;
        self.viewport.first_beat = 0.0;
    }
    pub(crate) fn move_cursor(&mut self, beats: f64) {
        self.edit_beat = (self.edit_beat + beats).max(0.0);
        let width = f64::from(f32::from(self.ruler_bounds.get().size.width));
        if width <= 0.0 {
            return;
        }
        let margin = (width / 4.0).min(self.viewport.pixels_per_beat);
        if self.viewport.x_at(self.edit_beat) < 0.0 {
            self.viewport.first_beat = self.edit_beat;
        } else if self.viewport.x_at(self.edit_beat) > width - margin {
            self.viewport.first_beat =
                (self.edit_beat - (width - margin) / self.viewport.pixels_per_beat).max(0.0);
        }
    }
    pub(crate) fn zoom_at_cursor(&mut self, factor: f64) {
        let width = f64::from(f32::from(self.ruler_bounds.get().size.width));
        let cursor = self.viewport.x_at(self.edit_beat);
        let anchor = if (0.0..=width).contains(&cursor) {
            cursor
        } else {
            width / 2.0
        };
        self.viewport.zoom_at(anchor, factor);
    }
    fn bounds(&self, area: TimelineArea) -> &Rc<Cell<Bounds<Pixels>>> {
        match area {
            TimelineArea::Ruler => &self.ruler_bounds,
            TimelineArea::Lanes => &self.lanes_bounds,
        }
    }
    pub(crate) fn advance_particles(&mut self, design: &DesignSystem, visible: bool) -> bool {
        let bounds = self.lanes_bounds.get();
        let width = f64::from(f32::from(bounds.size.width));
        let playhead = self
            .playhead
            .filter(|beat| visible && (0.0..width).contains(&self.viewport.x_at(*beat)));
        self.particles.advance(
            playhead,
            f64::from(f32::from(bounds.size.height)),
            design,
            visible,
        )
    }
}
