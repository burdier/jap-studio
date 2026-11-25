//! Presentación de pistas por identidad. No modifica pistas, clips ni audio.
use elevate::musical::clip::ClipId;
use elevate::musical::track::{Track, TrackId};
use std::collections::HashMap;

pub(in crate::ui) const COMPACT_TRACK_HEIGHT: f32 = 40.0;

#[derive(Clone, Copy, Default)]
pub(super) struct TrackPresentation {
    pub collapsed: bool,
    pub z_folded: bool,
    pub band_solo: Option<elevate::musical::fx::FxId>,
    pub height: Option<f32>,
}

#[derive(Clone, Copy)]
pub(crate) struct TrackRow {
    pub track: usize,
    pub lanes: usize,
    pub bands: usize,
    pub selected_band: Option<usize>,
    pub top: f32,
    pub height: f32,
}

#[derive(Clone, Default)]
pub(crate) struct TrackLayout {
    pub rows: Vec<TrackRow>,
    pub(super) spans: std::rc::Rc<Vec<(usize, f64, f64, ClipId)>>,
    pub(super) slots: std::rc::Rc<HashMap<ClipId, usize>>,
    pub(super) counts: std::rc::Rc<HashMap<usize, usize>>,
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
                lanes: 1,
                bands: 0,
                selected_band: None,
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
                    lanes: 1,
                    bands: 0,
                    selected_band: None,
                    top: track as f32 * height,
                    height,
                })
                .collect(),
            height: count as f32 * height,
            ..Self::default()
        }
    }

    pub(super) fn assign_layers(&mut self, mut clips: Vec<(usize, f64, f64, ClipId)>) {
        clips.sort_unstable_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.total_cmp(&b.1))
                .then_with(|| a.3.value().cmp(&b.3.value()))
        });
        let mut ends = Vec::new();
        let mut previous = None;
        for (track, start, end, id) in clips {
            if previous != Some(track) {
                ends.clear();
                previous = Some(track);
            }
            let slot = ends
                .iter()
                .position(|last| *last <= start)
                .unwrap_or(ends.len());
            if slot == ends.len() {
                ends.push(end);
                std::rc::Rc::make_mut(&mut self.counts).insert(track, ends.len());
            } else {
                ends[slot] = end;
            }
            if slot > 0 {
                std::rc::Rc::make_mut(&mut self.slots).insert(id, slot);
            }
        }
    }

    pub(crate) fn clip_slot(&self, row: TrackRow, id: ClipId) -> usize {
        self.slots.get(&id).copied().unwrap_or(0).min(row.lanes - 1)
    }

    pub(crate) fn clip_row(&self, track: usize, id: ClipId) -> Option<TrackRow> {
        let mut row = self.row(track)?;
        row.height /= row.lanes as f32;
        row.top += self.clip_slot(row, id) as f32 * row.height;
        Some(row)
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
