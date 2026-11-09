//! Estado de navegación del Arrangement. El canvas está en paint.rs.
mod clip_edit;
#[cfg(test)]
mod clip_edit_tests;
mod clip_frame;
#[cfg(test)]
mod gesture_tests;
mod import_preview;
mod interaction;
mod materialize;
mod midi_clips;
mod paint;
mod particles;
mod rows;
#[cfg(test)]
mod tests;
mod time_ruler;
mod track_edit;
mod trim;
mod viewport;
mod waveform;

use super::DesignSystem;
use crate::audio::AudioClip;
use elevate::musical::clip::MidiClip;
use gpui::{Bounds, Pixels, Point};
use particles::ParticleTrail;
use std::{cell::Cell, rc::Rc};
use viewport::Viewport;

const BEATS_PER_BAR: f64 = 4.0;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum TimelineArea {
    Ruler,
    TimeRuler,
    Lanes,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DragKind {
    Audio,
    Midi,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipBarHit {
    Inspector(DragKind),
    Mute(DragKind),
    Lock(DragKind),
    Loop(DragKind),
}

pub(crate) struct Timeline {
    pub(crate) layout: Option<crate::ui::track_layout::TrackLayout>,
    clip_drag: Option<(Point<Pixels>, f64, bool, DragKind)>,
    trim_drag: Option<trim::TrimDrag>,
    hover_track: Option<usize>,
    hover_dragging: bool,
    // Clips provisionales. Migrar al modelo de proyecto al introducir múltiples clips.
    pub clip: Option<AudioClip>,
    pub other_audio_clips: Vec<AudioClip>,
    pub midi_clip: Option<MidiClip>,
    pub other_midi_clips: Vec<MidiClip>,
    pub selected_midi: Vec<elevate::musical::clip::ClipId>,
    pub playhead: Option<f64>,
    viewport: Viewport,
    edit_beat: f64,
    ruler_bounds: Rc<Cell<Bounds<Pixels>>>,
    time_ruler_bounds: Rc<Cell<Bounds<Pixels>>>,
    lanes_bounds: Rc<Cell<Bounds<Pixels>>>,
    particles: ParticleTrail,
    pub(crate) import_preview: Option<import_preview::ImportPreview>,
}

impl Timeline {
    pub(crate) fn new(pixels_per_beat: f32) -> Self {
        Self {
            layout: None,
            clip_drag: None,
            trim_drag: None,
            hover_track: None,
            hover_dragging: false,
            clip: None,
            other_audio_clips: Vec::new(),
            midi_clip: None,
            other_midi_clips: Vec::new(),
            selected_midi: Vec::new(),
            playhead: None,
            viewport: Viewport {
                first_beat: 0.0,
                pixels_per_beat: f64::from(pixels_per_beat).clamp(8.0, 256.0),
            },
            edit_beat: 0.0,
            ruler_bounds: Rc::default(),
            time_ruler_bounds: Rc::default(),
            lanes_bounds: Rc::default(),
            particles: ParticleTrail::default(),
            import_preview: None,
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
            TimelineArea::TimeRuler => &self.time_ruler_bounds,
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
