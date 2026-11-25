//! Vista previa de reordenamiento: sigue el puntero y confirma una sola edición al soltar.
use super::{
    DawApp,
    track_layout::{TrackLayout, TrackRow},
};
use elevate::musical::track::{TrackEdit, TrackId, TrackKind};
use gpui::{Pixels, Point, px};

#[derive(Clone, Copy)]
pub(super) struct TrackReorderDrag {
    pub source: TrackId,
    pub start: Point<Pixels>,
    pub current: Point<Pixels>,
    pub row_top: f32,
    pub target: Option<(TrackId, bool)>,
    pub moved: bool,
}

#[derive(Clone)]
pub(super) struct TrackReorderVisual {
    pub layout: TrackLayout,
    pub source_track: usize,
    pub source_rows: Vec<usize>,
    pub placeholder_top: f32,
    pub floating_top: f32,
    pub floating_height: f32,
    pub float_offset: f32,
}

impl TrackReorderDrag {
    pub fn preview_top(self) -> f32 {
        self.row_top + f32::from(self.current.y - self.start.y)
    }
}

impl DawApp {
    pub(in crate::ui) fn begin_track_reorder_drag(
        &mut self,
        index: usize,
        position: Point<Pixels>,
    ) {
        let Some(track) = self.tracks.get(index) else {
            return;
        };
        let Some(row) = self.track_layout().row(index) else {
            return;
        };
        self.track_reorder_drag = Some(TrackReorderDrag {
            source: track.id,
            start: position,
            current: position,
            row_top: row.top,
            target: None,
            moved: false,
        });
    }

    pub(in crate::ui) fn update_track_reorder_drag(
        &mut self,
        position: Point<Pixels>,
        dragging: bool,
    ) -> bool {
        let Some(mut drag) = self.track_reorder_drag else {
            return false;
        };
        if !dragging {
            self.track_reorder_drag = None;
            return true;
        }
        drag.current = position;
        drag.moved |= (position.y - drag.start.y).abs() >= px(3.0);
        if drag.moved {
            drag.target = self.track_drop_target(drag.source, position);
        }
        self.track_reorder_drag = Some(drag);
        true
    }

    pub(in crate::ui) fn finish_track_reorder_drag(&mut self, position: Point<Pixels>) {
        if self.track_reorder_drag.is_none() {
            return;
        }
        self.update_track_reorder_drag(position, true);
        let Some(drag) = self.track_reorder_drag.take() else {
            return;
        };
        let Some((target, after)) = drag.target.filter(|_| drag.moved) else {
            return;
        };
        let Some(index) = self.tracks.iter().position(|track| track.id == drag.source) else {
            return;
        };
        self.select_track(index, false, false);
        self.edit_track_structure(TrackEdit::Move { target, after });
    }

    pub(super) fn track_reorder_visual(&self) -> Option<TrackReorderVisual> {
        let drag = self.track_reorder_drag.filter(|drag| drag.moved)?;
        let source_index = self
            .tracks
            .iter()
            .position(|track| track.id == drag.source)?;
        let source = &self.tracks[source_index];
        let original = self.track_layout();
        let source_indices: Vec<_> = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| {
                track.id == drag.source
                    || (source.kind == TrackKind::Group && track.parent == Some(drag.source))
            })
            .map(|(index, _)| index)
            .collect();
        let source_rows: Vec<_> = original
            .rows
            .iter()
            .filter(|row| source_indices.contains(&row.track))
            .map(|row| row.track)
            .collect();
        let floating_height = original
            .rows
            .iter()
            .filter(|row| source_rows.contains(&row.track))
            .map(|row| row.height)
            .sum();
        let mut order: Vec<_> = (0..self.tracks.len()).collect();
        if let Some((target, after)) = drag.target {
            order.retain(|index| !source_indices.contains(index));
            let destination = order
                .iter()
                .position(|index| self.tracks[*index].id == target)?;
            let insert_at = if !after {
                destination
            } else if self.tracks[order[destination]].kind == TrackKind::Group {
                order.iter().rposition(|index| {
                    self.tracks[*index].id == target || self.tracks[*index].parent == Some(target)
                })? + 1
            } else {
                destination + 1
            };
            order.splice(insert_at..insert_at, source_indices);
        }
        let mut layout = TrackLayout {
            slots: original.slots.clone(),
            counts: original.counts.clone(),
            ..Default::default()
        };
        for index in order {
            if let Some(row) = original.row(index) {
                layout.rows.push(TrackRow {
                    lanes: row.lanes,
                    bands: row.bands,
                    selected_band: row.selected_band,
                    track: index,
                    top: layout.height,
                    height: row.height,
                });
                layout.height += row.height;
            }
        }
        let floating_top = drag.preview_top();
        Some(TrackReorderVisual {
            source_track: source_index,
            placeholder_top: layout.row(source_index)?.top,
            float_offset: floating_top - original.row(source_index)?.top,
            layout,
            source_rows,
            floating_top,
            floating_height,
        })
    }

    fn track_drop_target(
        &self,
        source_id: TrackId,
        position: Point<Pixels>,
    ) -> Option<(TrackId, bool)> {
        let source = self.tracks.iter().find(|track| track.id == source_id)?;
        let bounds = self.timeline.lanes_bounds();
        if position.x < bounds.left() - px(self.ds.track_header_width + 7.0)
            || position.x >= bounds.left() - px(7.0)
        {
            return None;
        }
        let y = f32::from(position.y - bounds.top());
        let layout = self.track_layout();
        let row = layout.at_y(y)?;
        let hovered = &self.tracks[row.track];
        let target = if source.parent.is_none() {
            hovered.parent.unwrap_or(hovered.id)
        } else {
            hovered.id
        };
        let target_index = self.tracks.iter().position(|track| track.id == target)?;
        let destination = &self.tracks[target_index];
        if destination.parent != source.parent || target == source_id {
            return None;
        }
        let target_row = layout.row(target_index)?;
        let end = if destination.kind == TrackKind::Group && destination.parent.is_none() {
            layout
                .rows
                .iter()
                .find(|row| row.top > target_row.top && self.tracks[row.track].parent.is_none())
                .map_or(layout.height, |row| row.top)
        } else {
            target_row.top + target_row.height
        };
        let after = y >= (target_row.top + end) * 0.5;
        Some((target, after))
    }
}
