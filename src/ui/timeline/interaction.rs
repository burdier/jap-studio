//! Picking, arrastre y scroll. Mismo viewport que utiliza paint.rs.
use super::{ClipHover, DragKind, Timeline, TimelineArea, clip_frame::ClipEdge, trim::EDGE_PX};
use crate::ui::navigation::{Navigation, pinch_factor, scroll_navigation};
use gpui::{PinchEvent, Pixels, Point, ScrollEvent, px};

impl Timeline {
    pub(crate) fn begin_clip_drag(&mut self, position: Point<Pixels>, track_height: f32) {
        self.clip_drag = None;
        self.slip_drag = None;
        let bounds = self.lanes_bounds.get();
        if !bounds.contains(&position) {
            return;
        }
        if self.begin_trim(position, track_height) {
            return;
        }
        let beat = self
            .viewport
            .beat_at(f64::from(f32::from(position.x - bounds.left())));
        let Some(row) = self.row_at_y(f32::from(position.y - bounds.top()), track_height) else {
            return;
        };
        let track = row.track;
        if let Some(midi_clip) = &self.midi_clip
            && track == midi_clip.track
            && beat >= midi_clip.start_beat
            && beat < midi_clip.end_beat()
        {
            if !midi_clip.props.locked {
                self.clip_drag =
                    Some((position, beat - midi_clip.start_beat, false, DragKind::Midi));
            }
            return;
        }
        if let Some(clip) = &self.clip
            && track == clip.track
            && beat >= clip.start_beat
            && beat < clip.end_beat(self.tempo.bpm())
        {
            if !clip.props.locked {
                self.clip_drag = Some((position, beat - clip.start_beat, false, DragKind::Audio));
            }
        }
    }
    pub(crate) fn move_clip_in_tracks(
        &mut self,
        position: Point<Pixels>,
        height: f32,
        tracks: &[elevate::musical::track::Track],
    ) -> Option<usize> {
        let bounds = self.lanes_bounds.get();
        if height > 0.0
            && !tracks.is_empty()
            && self.trim_drag.is_none()
            && self.slip_drag.is_none()
        {
            let index =
                self.drag_track_at_y(f32::from(position.y - bounds.top()), height, tracks.len())?;
            if !tracks[index].accepts_clips() {
                return None;
            }
        }
        self.move_clip(position, height, tracks.len())
    }
    pub(crate) fn move_clip(
        &mut self,
        position: Point<Pixels>,
        track_height: f32,
        track_count: usize,
    ) -> Option<usize> {
        if self.slip_drag.is_some() {
            return self.apply_slip(position);
        }
        if self.trim_drag.is_some() {
            return self.apply_trim(position);
        }
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
        let track = self.drag_track_at_y(
            f32::from(position.y - bounds.top()),
            track_height,
            track_count,
        )?;
        match kind {
            DragKind::Audio => {
                let start = self.magnet_start(beat - grab_offset);
                let clip = self.clip.as_mut()?;
                clip.start_beat = start;
                clip.track = track;
                self.edit_beat = start;
            }
            DragKind::Midi => {
                let start = self.magnet_start(beat - grab_offset);
                let clip = self.midi_clip.as_mut()?;
                clip.start_beat = start;
                clip.track = track;
                self.edit_beat = start;
            }
        }
        Some(track)
    }
    /// Inicio libre salvo cerca de una línea de la rejilla visible, donde se pega.
    fn magnet_start(&mut self, raw: f64) -> f64 {
        let raw = raw.max(0.0);
        let grid = self.viewport.grid();
        let hit = grid.magnet(raw, self.viewport.grid_step());
        let start = hit.filter(|hit| hit.captured).map_or(raw, |hit| hit.target);
        self.snap_guide.track(start, hit);
        start
    }
    pub(crate) fn end_clip_drag(&mut self) {
        if self.clip_drag.is_some_and(|(_, _, moved, _)| moved) {
            self.snap_guide.release();
        } else {
            self.snap_guide.land(None);
        }
        self.clip_drag = None;
        self.trim_drag = None;
        self.slip_drag = None;
        self.hover_dragging = false;
    }
    pub(crate) fn hover_track(&self) -> Option<usize> {
        self.hover_track
    }
    pub(crate) fn hover_dragging(&self) -> bool {
        self.hover_dragging || self.clip_drag.is_some()
    }
    pub(crate) fn update_hover(
        &mut self,
        position: Point<Pixels>,
        dragging: bool,
        track_height: f32,
        track_count: usize,
    ) -> bool {
        let bounds = self.lanes_bounds.get();
        let y_rel = f32::from(position.y - bounds.top());
        let x_ok = position.x >= px(0.0) && position.x <= bounds.right();
        let next_track = if x_ok && y_rel >= 0.0 && track_height > 0.0 {
            self.row_at_y(y_rel, track_height)
                .map(|r| r.track)
                .filter(|i| *i < track_count)
        } else {
            None
        };
        let next_drag = next_track.is_some() && (dragging || self.clip_drag.is_some());
        let next_clip = self.clip_hover_at(position, track_height);
        let changed = self.hover_track != next_track
            || self.hover_dragging != next_drag
            || self.hover_clip != next_clip;
        self.hover_track = next_track;
        self.hover_dragging = next_drag;
        self.hover_clip = next_clip;
        changed
    }

    fn clip_hover_at(&self, position: Point<Pixels>, track_height: f32) -> Option<ClipHover> {
        let bounds = self.lanes_bounds.get();
        if !bounds.contains(&position) {
            return None;
        }
        let track = self
            .row_at_y(f32::from(position.y - bounds.top()), track_height)?
            .track;
        let beat = self
            .viewport
            .beat_at(f64::from(f32::from(position.x - bounds.left())));
        let hit = |kind, clip_track, start, end, locked| {
            if clip_track != track || beat < start || beat >= end {
                return None;
            }
            let edge_beats = self.viewport.grid().edge_beats(EDGE_PX, end - start);
            let edge = if locked {
                None
            } else if beat - start <= edge_beats {
                Some(ClipEdge::Left)
            } else if end - beat <= edge_beats {
                Some(ClipEdge::Right)
            } else {
                None
            };
            Some(ClipHover {
                kind,
                track,
                start_beat: start,
                edge,
            })
        };
        if let Some(clip) = self.midi_clip.as_ref()
            && let Some(hover) = hit(
                DragKind::Midi,
                clip.track,
                clip.start_beat,
                clip.end_beat(),
                clip.props.locked,
            )
        {
            return Some(hover);
        }
        for clip in self.other_midi_clips.iter().rev() {
            if let Some(hover) = hit(
                DragKind::Midi,
                clip.track,
                clip.start_beat,
                clip.end_beat(),
                clip.props.locked,
            ) {
                return Some(hover);
            }
        }
        if let Some(clip) = self.clip.as_ref()
            && let Some(hover) = hit(
                DragKind::Audio,
                clip.track,
                clip.start_beat,
                clip.end_beat(self.tempo.bpm()),
                clip.props.locked,
            )
        {
            return Some(hover);
        }
        self.other_audio_clips.iter().rev().find_map(|clip| {
            hit(
                DragKind::Audio,
                clip.track,
                clip.start_beat,
                clip.end_beat(self.tempo.bpm()),
                clip.props.locked,
            )
        })
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
            let track = self
                .row_at_y(f32::from(position.y - bounds.top()), track_height)?
                .track;
            (track < track_count).then_some(track)
        } else {
            None
        }
    }
    /// Desplaza el tiempo y devuelve el delta vertical para el scroll común de pistas.
    pub(crate) fn scroll(&mut self, event: &ScrollEvent, area: TimelineArea) -> f32 {
        match scroll_navigation(event) {
            Navigation::Zoom(factor) => {
                let x = f64::from(f32::from(event.position.x - self.bounds(area).get().left()));
                self.viewport.zoom_at(x, factor);
                0.0
            }
            Navigation::Pan { x, y } => {
                let amount = if area != TimelineArea::Lanes && x.abs() < y.abs() {
                    y
                } else {
                    x
                };
                self.viewport.first_beat =
                    (self.viewport.first_beat - amount / self.viewport.pixels_per_beat).max(0.0);
                if area == TimelineArea::Lanes {
                    y as f32
                } else {
                    0.0
                }
            }
        }
    }

    pub(crate) fn pinch(&mut self, event: &PinchEvent, area: TimelineArea) {
        if let Some(factor) = pinch_factor(event) {
            let x = f64::from(f32::from(event.position.x - self.bounds(area).get().left()));
            self.viewport.zoom_at(x, factor);
        }
    }
}
