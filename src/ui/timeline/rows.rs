//! Coordenadas compartidas por picking, hover, arrastre y recorte.
use super::Timeline;
use crate::ui::track_layout::TrackRow;

impl Timeline {
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
