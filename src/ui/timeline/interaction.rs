//! Picking, arrastre y scroll. Mismo viewport que utiliza paint.rs.
use super::{DragKind, Timeline, TimelineArea};
use crate::audio::BEATS_PER_SECOND;
use gpui::{Pixels, Point, ScrollEvent, px};

impl Timeline {
    pub(crate) fn begin_clip_drag(&mut self, position: Point<Pixels>, track_height: f32) {
        self.clip_drag = None;
        let bounds = self.lanes_bounds.get();
        if !bounds.contains(&position) {
            return;
        }
        let beat = self
            .viewport
            .beat_at(f64::from(f32::from(position.x - bounds.left())));
        let track = (f32::from(position.y - bounds.top()) / track_height).floor() as usize;
        if let Some(midi_clip) = &self.midi_clip
            && track == midi_clip.track
            && beat >= midi_clip.start_beat
            && beat < midi_clip.end_beat()
        {
            self.clip_drag = Some((position, beat - midi_clip.start_beat, false, DragKind::Midi));
            return;
        }
        if let Some(clip) = &self.clip
            && track == clip.track
            && beat >= clip.start_beat
            && beat < clip.start_beat + clip.file.duration() * BEATS_PER_SECOND
        {
            self.clip_drag = Some((position, beat - clip.start_beat, false, DragKind::Audio));
        }
    }
    pub(crate) fn move_clip(
        &mut self,
        position: Point<Pixels>,
        track_height: f32,
        track_count: usize,
    ) -> Option<usize> {
        let (origin, grab_offset, moved, kind) = self.clip_drag?;
        if track_count == 0
            || (!moved
                && (position.x - origin.x).abs() < px(3.0)
                && (position.y - origin.y).abs() < px(3.0))
        {
            return None;
        }
        self.clip_drag = Some((origin, grab_offset, true, kind));
        let bounds = self.lanes_bounds.get();
        let beat = self
            .viewport
            .beat_at(f64::from(f32::from(position.x - bounds.left())));
        let track = ((f32::from(position.y - bounds.top()).max(0.0) / track_height).floor()
            as usize)
            .min(track_count - 1);
        match kind {
            DragKind::Audio => {
                let clip = self.clip.as_mut()?;
                clip.start_beat = (beat - grab_offset).max(0.0);
                clip.track = track;
                self.edit_beat = clip.start_beat;
            }
            DragKind::Midi => {
                let clip = self.midi_clip.as_mut()?;
                clip.start_beat = (beat - grab_offset).max(0.0);
                clip.track = track;
                self.edit_beat = clip.start_beat;
            }
        }
        Some(track)
    }
    pub(crate) fn end_clip_drag(&mut self) {
        self.clip_drag = None;
    }
    pub(crate) fn place_cursor(
        &mut self,
        position: Point<Pixels>,
        area: TimelineArea,
        track_height: f32,
        track_count: usize,
    ) -> Option<usize> {
        let bounds = self.bounds(area).get();
        if !bounds.contains(&position) {
            return None;
        }
        self.edit_beat = self
            .viewport
            .beat_at(f64::from(f32::from(position.x - bounds.left())))
            .max(0.0);
        if area == TimelineArea::Lanes {
            let track = (f32::from(position.y - bounds.top()) / track_height).floor() as usize;
            (track < track_count).then_some(track)
        } else {
            None
        }
    }
    pub(crate) fn scroll(&mut self, event: &ScrollEvent, area: TimelineArea) -> bool {
        let delta = event.delta.pixel_delta(px(24.0));
        let horizontal = f64::from(f32::from(delta.x));
        let vertical = f64::from(f32::from(delta.y));
        let amount = if horizontal.abs() > vertical.abs() {
            horizontal
        } else {
            vertical
        };
        if event.modifiers.control {
            let x = f64::from(f32::from(event.position.x - self.bounds(area).get().left()));
            self.viewport.zoom_at(x, (amount * 0.005).exp());
        } else if event.modifiers.shift
            || horizontal.abs() > vertical.abs()
            || area == TimelineArea::Ruler
        {
            self.viewport.first_beat =
                (self.viewport.first_beat - amount / self.viewport.pixels_per_beat).max(0.0);
        } else {
            return false;
        }
        true
    }
}
