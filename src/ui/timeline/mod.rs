//! Estado de navegación del Arrangement. El canvas está en paint.rs.
mod appearance;
mod automation;
mod clip_edit;
#[cfg(test)]
mod clip_edit_tests;
mod clip_frame;
mod drum_clips;
mod fades;
#[cfg(test)]
mod gesture_tests;
mod import_preview;
mod interaction;
mod materialize;
mod midi_clips;
mod midi_edit;
mod navigation;
mod overlap;
mod paint;
mod particles;
mod pitch_probe;
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
use navigation::{LoopDrag, RegionDrag};
pub(in crate::ui) use navigation::{MarkerEdit, loop_beats};
use particles::ParticleTrail;
use std::{cell::Cell, rc::Rc};
use viewport::Viewport;

#[derive(Clone, Copy, PartialEq)]
struct ClipHover {
    id: elevate::musical::clip::ClipId,
    edge: Option<clip_frame::ClipEdge>,
}

impl ClipHover {
    fn edge_for(self, id: elevate::musical::clip::ClipId) -> Option<clip_frame::ClipEdge> {
        (self.id == id).then_some(self.edge).flatten()
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

#[derive(Clone, Copy, Debug)]
pub(in crate::ui) struct LiveRecordPreview {
    pub track: usize,
    pub start_beat: f64,
    pub end_beat: f64,
    pub peak: f32,
    pub counting_in: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipBarHit {
    Inspector(DragKind),
    Mute(DragKind),
    Lock(DragKind),
    Loop(DragKind),
}

struct GroupDrag {
    anchor_beat: f64,
    anchor_track: usize,
    audio: Vec<(elevate::musical::clip::ClipId, f64, usize)>,
    midi: Vec<(elevate::musical::clip::ClipId, f64, usize)>,
}

pub(crate) struct Timeline {
    pub(in crate::ui) navigation: elevate::project::ArrangementNavigation,
    pub(in crate::ui) automation: elevate::musical::track_automation::ArrangementAutomation,
    pub(in crate::ui) modulation: elevate::musical::modulation::ModulationMatrix,
    pub(in crate::ui) automation_mode: bool,
    pub(in crate::ui) automation_param: elevate::musical::track_automation::AutomationParam,
    automation_drag: Option<automation::AutomationDrag>,
    automation_hover: Option<automation::AutomationHover>,
    pub(in crate::ui) marker_edit: Option<MarkerEdit>,
    pub(in crate::ui) region_tool: bool,
    pub(in crate::ui) time_selection: Option<(i64, i64)>,
    region_drag: Option<RegionDrag>,
    loop_drag: Option<LoopDrag>,
    wave_zoom: f32,
    pub(super) sample_editor: crate::ui::sample_editor::SampleEditor,
    #[cfg(feature = "p2p")]
    pub(in crate::ui) peer_clip: Option<(u64, String)>,
    pub(crate) tempo: Tempo,
    pub(crate) meter: Meter,
    pub(crate) tempo_drag: Option<crate::ui::timing::TempoDrag>,
    pub(crate) layout: Option<crate::ui::track_layout::TrackLayout>,
    clip_drag: Option<(Point<Pixels>, f64, bool, DragKind)>,
    clip_drag_origin: Option<(f64, usize)>,
    group_drag: Option<GroupDrag>,
    trim_drag: Option<trim::TrimDrag>,
    fade_drag: Option<fades::FadeDrag>,
    slip_drag: Option<slip::SlipDrag>,
    pub(in crate::ui) overlap: overlap::Overlap,
    probe: pitch_probe::PitchProbe,
    hover_track: Option<usize>,
    hover_dragging: bool,
    hover_clip: Option<ClipHover>,
    // Clips provisionales. Migrar al modelo de proyecto al introducir múltiples clips.
    pub clip: Option<AudioClip>,
    pub other_audio_clips: Vec<AudioClip>,
    pub midi_clip: Option<MidiClip>,
    pub other_midi_clips: Vec<MidiClip>,
    pub(super) focused_audio: Option<elevate::musical::clip::ClipId>,
    pub selected_audio: Vec<elevate::musical::clip::ClipId>,
    pub selected_midi: Vec<elevate::musical::clip::ClipId>,
    midi_history: midi_edit::History,
    midi_gesture: Option<midi_edit::EditDraft>,
    midi_box: Option<midi_edit::SelectionBox>,
    pub(super) midi_clipboard: Vec<(elevate::musical::track::TrackId, MidiClip)>,
    pub playhead: Option<f64>,
    pub(super) last_stop_beat: Option<f64>,
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
            navigation: Default::default(),
            automation: Default::default(),
            modulation: Default::default(),
            automation_mode: false,
            automation_param: elevate::musical::track_automation::AutomationParam::Volume,
            automation_drag: None,
            automation_hover: None,
            marker_edit: None,
            region_tool: false,
            time_selection: None,
            region_drag: None,
            loop_drag: None,
            wave_zoom: 1.0,
            #[cfg(feature = "p2p")]
            peer_clip: None,
            sample_editor: crate::ui::sample_editor::SampleEditor {
                tab: 2,
                ..Default::default()
            },
            tempo: Tempo::default(),
            meter: Meter::default(),
            tempo_drag: None,
            layout: None,
            clip_drag: None,
            clip_drag_origin: None,
            group_drag: None,
            trim_drag: None,
            fade_drag: None,
            slip_drag: None,
            overlap: Default::default(),
            probe: Default::default(),
            hover_track: None,
            hover_dragging: false,
            hover_clip: None,
            clip: None,
            other_audio_clips: Vec::new(),
            midi_clip: None,
            other_midi_clips: Vec::new(),
            focused_audio: None,
            selected_audio: Vec::new(),
            selected_midi: Vec::new(),
            midi_history: Default::default(),
            midi_gesture: None,
            midi_box: None,
            midi_clipboard: Vec::new(),
            playhead: None,
            last_stop_beat: None,
            visual_transport: Default::default(),
            viewport: Viewport {
                first_beat: 0.0,
                pixels_per_beat: f64::from(pixels_per_beat).clamp(8.0, viewport::MAX_SCALE),
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
        let rounded = (self.playhead.unwrap_or(self.edit_beat) * 100.0).round() / 100.0;
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
