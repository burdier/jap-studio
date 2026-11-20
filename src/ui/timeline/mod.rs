//! Estado de navegación del Arrangement. El canvas está en paint.rs.
mod appearance;
mod clip_edit;
#[cfg(test)]
mod clip_edit_tests;
mod clip_frame;
mod drum_clips;
#[cfg(test)]
mod gesture_tests;
mod import_preview;
mod interaction;
mod materialize;
mod midi_clips;
mod paint;
mod particles;
mod rows;
mod slip;
#[cfg(test)]
mod tests;
mod time_ruler;
mod track_edit;
mod trim;
mod viewport;
mod visual_transport;
mod waveform;

use super::DesignSystem;
use crate::audio::AudioClip;
use elevate::musical::clip::MidiClip;
use elevate::musical::time::{Meter, Tempo};
use gpui::{Bounds, Pixels, Point};
use particles::ParticleTrail;
use std::{cell::Cell, rc::Rc};
use viewport::Viewport;

#[derive(Clone, Copy, PartialEq)]
struct ClipHover {
    kind: DragKind,
    track: usize,
    start_beat: f64,
    edge: Option<clip_frame::ClipEdge>,
}

impl ClipHover {
    fn edge_for(
        self,
        kind: DragKind,
        track: usize,
        start_beat: f64,
    ) -> Option<clip_frame::ClipEdge> {
        (self.kind == kind && self.track == track && self.start_beat == start_beat)
            .then_some(self.edge)
            .flatten()
    }
}

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
    pub(crate) tempo: Tempo,
    pub(crate) meter: Meter,
    pub(crate) tempo_drag: Option<crate::ui::timing::TempoDrag>,
    pub(crate) layout: Option<crate::ui::track_layout::TrackLayout>,
    clip_drag: Option<(Point<Pixels>, f64, bool, DragKind)>,
    trim_drag: Option<trim::TrimDrag>,
    slip_drag: Option<slip::SlipDrag>,
    hover_track: Option<usize>,
    hover_dragging: bool,
    hover_clip: Option<ClipHover>,
    // Clips provisionales. Migrar al modelo de proyecto al introducir múltiples clips.
    pub clip: Option<AudioClip>,
    pub other_audio_clips: Vec<AudioClip>,
    pub midi_clip: Option<MidiClip>,
    pub other_midi_clips: Vec<MidiClip>,
    pub selected_midi: Vec<elevate::musical::clip::ClipId>,
    pub playhead: Option<f64>,
    visual_transport: visual_transport::VisualTransport,
    viewport: Viewport,
    edit_beat: f64,
    ruler_bounds: Rc<Cell<Bounds<Pixels>>>,
    time_ruler_bounds: Rc<Cell<Bounds<Pixels>>>,
    lanes_bounds: Rc<Cell<Bounds<Pixels>>>,
    particles: ParticleTrail,
    snap_guide: crate::ui::components::snap_guide::SnapGuide,
    pub(crate) import_preview: Option<import_preview::ImportPreview>,
}

impl Timeline {
    pub(crate) fn new(pixels_per_beat: f32) -> Self {
        Self {
            tempo: Tempo::default(),
            meter: Meter::default(),
            tempo_drag: None,
            layout: None,
            clip_drag: None,
            trim_drag: None,
            slip_drag: None,
            hover_track: None,
            hover_dragging: false,
            hover_clip: None,
            clip: None,
            other_audio_clips: Vec::new(),
            midi_clip: None,
            other_midi_clips: Vec::new(),
            selected_midi: Vec::new(),
            playhead: None,
            visual_transport: Default::default(),
            viewport: Viewport {
                first_beat: 0.0,
                pixels_per_beat: f64::from(pixels_per_beat).clamp(8.0, 256.0),
            },
            edit_beat: 0.0,
            ruler_bounds: Rc::default(),
            time_ruler_bounds: Rc::default(),
            lanes_bounds: Rc::default(),
            particles: ParticleTrail::default(),
            snap_guide: Default::default(),
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
        format!(
            "{}:{:.2}",
            self.meter.bar_at(rounded),
            self.meter.beat_at(rounded)
        )
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
    pub(crate) fn lanes_bounds(&self) -> Bounds<Pixels> {
        self.lanes_bounds.get()
    }
    /// Destello del imán todavía en curso; el render pide frames mientras dure.
    pub(crate) fn snap_animating(&self) -> bool {
        self.snap_guide.animating()
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
