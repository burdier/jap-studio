//! Coordenadas compartidas por picking, hover, arrastre y recorte.
use super::Timeline;
use crate::ui::track_layout::TrackRow;

impl Timeline {
    pub(in crate::ui) fn holds_clip_layout(&self) -> bool {
        self.clip_drag.is_some()
            || self.trim_drag.is_some()
            || self.slip_drag.is_some()
            || self.midi_box.is_some()
    }

    fn pointer_slot(row: TrackRow, y: f32) -> usize {
        (((y - row.top) / (row.height / row.lanes as f32)) as usize).min(row.lanes - 1)
    }

    pub(super) fn lane_top(row: TrackRow, y: f32) -> f32 {
        row.top + Self::pointer_slot(row, y) as f32 * (row.height / row.lanes as f32)
    }

    pub(super) fn hits_layer(
        &self,
        row: TrackRow,
        y: f32,
        id: elevate::musical::clip::ClipId,
    ) -> bool {
        self.layout
            .as_ref()
            .is_none_or(|layout| layout.clip_slot(row, id) == Self::pointer_slot(row, y))
    }

    pub(super) fn row_at_y(&self, y: f32, default_height: f32) -> Option<TrackRow> {
        if let Some(layout) = &self.layout {
            return layout.at_y(y);
        }
        if y < 0.0 || default_height <= 0.0 {
            return None;
        }
        let track = (y / default_height).floor() as usize;
        Some(TrackRow {
            track,
            lanes: 1,
            bands: 0,
            selected_band: None,
            top: track as f32 * default_height,
            height: default_height,
        })
    }

    pub(super) fn drag_track_at_y(
        &self,
        y: f32,
        default_height: f32,
        count: usize,
    ) -> Option<usize> {
        if let Some(layout) = &self.layout {
            let y = y.clamp(0.0, (layout.height - 0.01).max(0.0));
            return layout.at_y(y).map(|r| r.track);
        }
        self.row_at_y(y.max(0.0), default_height)
            .map(|r| r.track.min(count.saturating_sub(1)))
    }
}
