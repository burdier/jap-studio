//! Presentación de pistas por identidad. No modifica pistas, clips ni audio.
use elevate::musical::track::{Track, TrackId};
use std::collections::HashMap;

pub(in crate::ui) const COMPACT_TRACK_HEIGHT: f32 = 40.0;

#[derive(Clone, Copy, Default)]
pub(super) struct TrackPresentation {
    pub collapsed: bool,
    pub height: Option<f32>,
}

#[derive(Clone, Copy)]
pub(crate) struct TrackRow {
    pub track: usize,
    pub top: f32,
    pub height: f32,
}

#[derive(Clone, Default)]
pub(crate) struct TrackLayout {
    pub rows: Vec<TrackRow>,
    pub height: f32,
}

impl TrackLayout {
    pub(super) fn build(
        tracks: &[Track],
        presentation: &HashMap<TrackId, TrackPresentation>,
        default_height: f32,
    ) -> Self {
        let mut layout = Self::default();
        for (track, data) in tracks.iter().enumerate() {
            if data
                .parent
                .is_some_and(|id| presentation.get(&id).is_some_and(|p| p.collapsed))
            {
                continue;
            }
            let height = presentation
                .get(&data.id)
                .and_then(|p| p.height)
                .unwrap_or(default_height)
                .clamp(32.0, 240.0);
            layout.rows.push(TrackRow {
                track,
                top: layout.height,
                height,
            });
            layout.height += height;
        }
        layout
    }

    pub(crate) fn uniform(count: usize, height: f32) -> Self {
        Self {
            rows: (0..count)
                .map(|track| TrackRow {
                    track,
                    top: track as f32 * height,
                    height,
                })
                .collect(),
            height: count as f32 * height,
        }
    }

    pub(crate) fn row(&self, track: usize) -> Option<TrackRow> {
        self.rows.iter().find(|row| row.track == track).copied()
    }

    pub(crate) fn at_y(&self, y: f32) -> Option<TrackRow> {
        self.rows
            .iter()
            .find(|row| y >= row.top && y < row.top + row.height)
            .copied()
    }
}
