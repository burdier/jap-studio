//! Picking, arrastre y scroll. Mismo viewport que utiliza paint.rs.
use super::{ClipHover, DragKind, Timeline, TimelineArea, clip_frame::ClipEdge, trim::EDGE_PX};
use crate::ui::navigation::{Navigation, pinch_factor, scroll_navigation};
use gpui::{PinchEvent, Pixels, Point, ScrollEvent, px};

impl Timeline {
    #[cfg(feature = "p2p")]
    pub(in crate::ui) fn editing_project_clips(&self) -> bool {
        self.overlap.dragging()
            || self.clip_drag.is_some()
            || self.trim_drag.is_some()
            || self.slip_drag.is_some()
            || self.midi_box.is_some()
            || self.tempo_drag.is_some()
    }

    #[cfg(feature = "p2p")]
    pub(in crate::ui) fn active_clip_presence(&self) -> Option<(u64, &str, &'static str)> {
        let (kind, verb) = if let Some(drag) = self.trim_drag {
            (drag.kind, "Recortando")
        } else if let Some(drag) = self.slip_drag {
            (drag.kind, "Desplazando contenido")
        } else if let Some((_, _, moved, kind)) = self.clip_drag {
            (kind, if moved { "Moviendo" } else { "Selección" })
        } else if self.focused_audio.is_some() {
            let id = self.focused_audio?;
            let c = self
                .clip
                .iter()
                .chain(&self.other_audio_clips)
                .find(|c| c.id == id)?;
            return Some((c.id.value(), &c.name, "Selección"));
        } else {
            let c = self
                .midi_clip
                .as_ref()
                .filter(|c| self.selected_midi.contains(&c.id))?;
            return Some((c.id.value(), &c.name, "Selección"));
        };
        match kind {
            DragKind::Audio => self
                .clip
                .as_ref()
                .map(|c| (c.id.value(), c.name.as_str(), verb)),
            DragKind::Midi => self
                .midi_clip
                .as_ref()
                .map(|c| (c.id.value(), c.name.as_str(), verb)),
        }
    }

    pub(crate) fn begin_clip_drag(&mut self, position: Point<Pixels>, track_height: f32) {
        self.clip_drag = None;
        self.clip_drag_origin = None;
        self.group_drag = None;
        self.slip_drag = None;
        let bounds = self.lanes_bounds.get();
        if !bounds.contains(&position) {
            return;
        }
        if self.begin_trim(position, track_height) {
            if let Some(clip) = &self.midi_clip
                && self
                    .trim_drag
                    .is_some_and(|drag| drag.kind == DragKind::Midi)
            {
                self.begin_midi_gesture(&[clip.id]);
            }
            return;
        }
        let beat = self
            .viewport
            .beat_at(f64::from(f32::from(position.x - bounds.left())));
        let y = f32::from(position.y - bounds.top());
        let Some(row) = self.row_at_y(y, track_height) else {
            return;
        };
        let track = row.track;
        let total_selected = self.selected_audio.len() + self.selected_midi.len();
        if let Some(midi_clip) = &self.midi_clip
            && track == midi_clip.track
            && self.hits_layer(row, y, midi_clip.id)
            && beat >= midi_clip.start_beat
            && beat < midi_clip.end_beat()
        {
            let grab_offset = beat - midi_clip.start_beat;
            let orig = (midi_clip.start_beat, midi_clip.track);
            if total_selected > 1
                && self.selected_midi.contains(&midi_clip.id)
                && !self.selected_audio.is_empty()
            {
                if self.start_group_drag(midi_clip.start_beat, midi_clip.track) {
                    self.clip_drag = Some((position, grab_offset, false, DragKind::Midi));
                    self.clip_drag_origin = Some(orig);
                }
                return;
            }
            if self.begin_midi_gesture(&self.midi_edit_targets()) {
                self.clip_drag = Some((position, grab_offset, false, DragKind::Midi));
                self.clip_drag_origin = Some(orig);
            }
            return;
        }
        if let Some(clip) = &self.clip
            && track == clip.track
            && self.hits_layer(row, y, clip.id)
            && beat >= clip.start_beat
            && beat < clip.end_beat(self.tempo.bpm())
        {
            if !clip.props.locked {
                let grab_offset = beat - clip.start_beat;
                let orig = (clip.start_beat, clip.track);
                if total_selected > 1 && self.selected_audio.contains(&clip.id) {
                    if self.start_group_drag(clip.start_beat, clip.track) {
                        self.clip_drag = Some((position, grab_offset, false, DragKind::Audio));
                        self.clip_drag_origin = Some(orig);
                    }
                    return;
                }
                self.clip_drag = Some((position, grab_offset, false, DragKind::Audio));
                self.clip_drag_origin = Some(orig);
            }
        }
    }

    fn start_group_drag(&mut self, anchor_beat: f64, anchor_track: usize) -> bool {
        let audio: Vec<_> = self
            .clip
            .iter()
            .chain(&self.other_audio_clips)
            .filter(|c| !c.props.locked && self.selected_audio.contains(&c.id))
            .map(|c| (c.id, c.start_beat, c.track))
            .collect();
        let midi: Vec<_> = self
            .midi_clips()
            .filter(|c| !c.props.locked && self.selected_midi.contains(&c.id))
            .map(|c| (c.id, c.start_beat, c.track))
            .collect();
        if audio.len() + midi.len() <= 1 {
            return false;
        }
        if !midi.is_empty() {
            let midi_ids: Vec<_> = midi.iter().map(|(id, _, _)| *id).collect();
            self.begin_midi_gesture(&midi_ids);
        }
        self.group_drag = Some(super::GroupDrag {
            anchor_beat,
            anchor_track,
            audio,
            midi,
        });
        true
    }
    pub(crate) fn move_clip_in_tracks(
        &mut self,
        position: Point<Pixels>,
        height: f32,
        tracks: &[elevate::musical::track::Track],
    ) -> Option<usize> {
        let bounds = self.lanes_bounds.get();
        if self.midi_box.is_some() {
            self.update_midi_box(position, height);
            return None;
        }
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
            if let Some((origin, offset, moved, kind)) = self.clip_drag
                && self.group_drag.is_some()
            {
                if !moved
                    && (position.x - origin.x).abs() < px(3.0)
                    && (position.y - origin.y).abs() < px(3.0)
                {
                    return None;
                }
                let beat = self.viewport.beat_at(f64::from(position.x - bounds.left()));
                let raw_anchor = self.magnet_start((beat - offset).max(0.0));
                let group = self.group_drag.as_ref().unwrap();
                let min_start = group
                    .audio
                    .iter()
                    .chain(&group.midi)
                    .map(|&(_, start, _)| start)
                    .fold(f64::INFINITY, f64::min);
                let delta_beat = (raw_anchor - group.anchor_beat).max(-min_start);
                let candidate_track_delta = index as isize - group.anchor_track as isize;
                let tracks_valid =
                    group
                        .audio
                        .iter()
                        .chain(&group.midi)
                        .all(|&(_, _, orig_track)| {
                            let dest = orig_track as isize + candidate_track_delta;
                            dest >= 0
                                && (dest as usize) < tracks.len()
                                && tracks[dest as usize].accepts_clips()
                        });
                let delta_track = if tracks_valid {
                    candidate_track_delta
                } else {
                    0
                };
                let audio_moves: Vec<_> = group
                    .audio
                    .iter()
                    .map(|&(id, start, orig)| {
                        (
                            id,
                            start + delta_beat,
                            (orig as isize + delta_track) as usize,
                        )
                    })
                    .collect();
                let midi_moves: Vec<_> = group
                    .midi
                    .iter()
                    .map(|&(id, start, orig)| {
                        (
                            id,
                            start + delta_beat,
                            (orig as isize + delta_track) as usize,
                        )
                    })
                    .collect();
                for clip in self.clip.iter_mut().chain(&mut self.other_audio_clips) {
                    if let Some(&(_, start, trk)) =
                        audio_moves.iter().find(|(id, _, _)| *id == clip.id)
                    {
                        clip.start_beat = start;
                        clip.track = trk;
                    }
                }
                for clip in self.midi_clips_mut() {
                    if let Some(&(_, start, trk)) =
                        midi_moves.iter().find(|(id, _, _)| *id == clip.id)
                    {
                        clip.start_beat = start;
                        clip.track = trk;
                    }
                }
                self.edit_beat =
                    (self.group_drag.as_ref().unwrap().anchor_beat + delta_beat).max(0.0);
                self.clip_drag = Some((origin, offset, true, kind));
                return Some(index);
            }
            if let Some((origin, offset, moved, DragKind::Midi)) = self.clip_drag
                && self
                    .midi_gesture
                    .as_ref()
                    .is_some_and(|gesture| gesture.clips.len() > 1)
            {
                if !moved
                    && (position.x - origin.x).abs() < px(3.0)
                    && (position.y - origin.y).abs() < px(3.0)
                {
                    return None;
                }
                let beat = self.viewport.beat_at(f64::from(position.x - bounds.left()));
                let result = self.move_midi_group(beat - offset, index, tracks);
                if result.is_some() {
                    self.clip_drag = Some((origin, offset, true, DragKind::Midi));
                }
                return result;
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
    pub(super) fn magnet_start(&mut self, raw: f64) -> f64 {
        let raw = raw.max(0.0);
        let grid = self.viewport.grid();
        let hit = grid.magnet(raw, self.viewport.grid_step());
        let start = hit.filter(|hit| hit.captured).map_or(raw, |hit| hit.target);
        self.snap_guide.track(start, hit);
        start
    }
    pub(in crate::ui) fn clip_moved(&self) -> bool {
        self.clip_drag.is_some_and(|(_, _, moved, _)| moved)
            || self.trim_drag.is_some()
            || self.slip_drag.is_some()
    }
    pub(in crate::ui) fn active_clip_drag_kind(&self) -> Option<DragKind> {
        self.clip_drag
            .and_then(|(_, _, moved, kind)| moved.then_some(kind))
    }
    /// Devuelve el clip arrastrado a su posición original (útil al soltarlo en el Sampler).
    pub(in crate::ui) fn restore_clip_drag_origin(&mut self) {
        let Some((_, _, _, kind)) = self.clip_drag else {
            return;
        };
        if let Some(group) = self.group_drag.take() {
            for (id, start, trk) in group.audio {
                if let Some(c) = self
                    .clip
                    .iter_mut()
                    .chain(&mut self.other_audio_clips)
                    .find(|c| c.id == id)
                {
                    c.start_beat = start;
                    c.track = trk;
                }
            }
            for (id, start, trk) in group.midi {
                if let Some(c) = self.midi_clips_mut().find(|c| c.id == id) {
                    c.start_beat = start;
                    c.track = trk;
                }
            }
            return;
        }
        if let Some((start, trk)) = self.clip_drag_origin {
            match kind {
                DragKind::Audio => {
                    if let Some(c) = &mut self.clip {
                        c.start_beat = start;
                        c.track = trk;
                    }
                }
                DragKind::Midi => {
                    if !self.cancel_midi_gesture()
                        && let Some(c) = &mut self.midi_clip
                    {
                        c.start_beat = start;
                        c.track = trk;
                    }
                }
            }
        }
    }
    pub(crate) fn end_clip_drag(&mut self) -> bool {
        let slip = self.finish_slip_selection();
        let edited = self.commit_midi_gesture();
        let selected = self.finish_midi_box();
        if self.clip_drag.is_some_and(|(_, _, moved, _)| moved) {
            self.snap_guide.release();
        } else {
            self.snap_guide.land(None);
        }
        self.clip_drag = None;
        self.clip_drag_origin = None;
        self.group_drag = None;
        self.trim_drag = None;
        self.slip_drag = None;
        self.region_drag = None;
        self.loop_drag = None;
        self.hover_dragging = false;
        let gain = self.overlap.finish_gain();
        let fade = self.fade_drag.take().is_some();
        edited || selected || slip || gain || fade
    }
    pub(crate) fn hover_track(&self) -> Option<usize> {
        self.hover_track
    }
    pub(crate) fn hover_dragging(&self) -> bool {
        self.hover_dragging || self.clip_drag.is_some()
    }
    pub(crate) fn hovering_clip_edge(&self) -> bool {
        self.trim_drag.is_some() || self.hover_clip.is_some_and(|hover| hover.edge.is_some())
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
        let probe_point = next_clip
            .filter(|hit| {
                !dragging
                    && self
                        .clip
                        .iter()
                        .chain(&self.other_audio_clips)
                        .any(|clip| clip.id == hit.id)
            })
            .map(|_| position);
        let point_changed = self.probe.pointer != probe_point;
        self.probe.pointer = probe_point;
        let changed = point_changed
            || self.hover_track != next_track
            || self.hover_dragging != next_drag
            || self.hover_clip != next_clip;
        self.hover_track = next_track;
        self.hover_dragging = next_drag;
        self.hover_clip = next_clip;
        changed
    }

    pub(super) fn clip_hover_at(
        &self,
        position: Point<Pixels>,
        track_height: f32,
    ) -> Option<ClipHover> {
        let bounds = self.lanes_bounds.get();
        if !bounds.contains(&position) {
            return None;
        }
        let y = f32::from(position.y - bounds.top());
        let row = self.row_at_y(y, track_height)?;
        let beat = self
            .viewport
            .beat_at(f64::from(f32::from(position.x - bounds.left())));
        let hit = |id, clip_track, start, end, locked| {
            if clip_track != row.track
                || !self.hits_layer(row, y, id)
                || beat < start
                || beat >= end
            {
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
            Some(ClipHover { id, edge })
        };
        if let Some(clip) = self.midi_clip.as_ref()
            && let Some(hover) = hit(
                clip.id,
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
                clip.id,
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
                clip.id,
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
                clip.id,
                clip.track,
                clip.start_beat,
                clip.end_beat(self.tempo.bpm()),
                clip.props.locked,
            )
        })
    }
    /// Devuelve `true` si el puntero está sobre la barra de título superior de un clip
    /// (en Ableton Live la barra de título siempre permite arrastrar el clip aunque la herramienta Región esté activa).
    pub(crate) fn is_over_clip_title(
        &self,
        position: Point<Pixels>,
        track_height: f32,
        design: &super::DesignSystem,
    ) -> bool {
        let bounds = self.lanes_bounds.get();
        if !bounds.contains(&position) {
            return false;
        }
        let y = f32::from(position.y - bounds.top());
        let Some(row) = self.row_at_y(y, track_height) else {
            return false;
        };
        let title_h = super::clip_frame::title_height(design);
        if y - row.top > title_h {
            return false;
        }
        let beat = self
            .viewport
            .beat_at(f64::from(f32::from(position.x - bounds.left())));
        let bpm = self.tempo.bpm();
        self.midi_clips().any(|c| {
            c.track == row.track
                && self.hits_layer(row, y, c.id)
                && beat >= c.start_beat
                && beat < c.end_beat()
        }) || self.clip.iter().chain(&self.other_audio_clips).any(|c| {
            c.track == row.track
                && self.hits_layer(row, y, c.id)
                && beat >= c.start_beat
                && beat < c.end_beat(bpm)
        })
    }

    pub(crate) fn beat_at_position(
        &self,
        position: Point<Pixels>,
        area: TimelineArea,
    ) -> Option<f64> {
        let bounds = self.bounds(area).get();
        bounds.contains(&position).then(|| {
            self.viewport
                .beat_at(f64::from(position.x - bounds.left()))
                .max(0.0)
        })
    }

    pub(crate) fn place_cursor(
        &mut self,
        position: Point<Pixels>,
        area: TimelineArea,
        track_height: f32,
        track_count: usize,
    ) -> Option<usize> {
        self.edit_beat = self.beat_at_position(position, area)?;
        if area == TimelineArea::Lanes {
            let bounds = self.bounds(area).get();
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
