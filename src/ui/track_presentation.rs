//! Plegado y tamaño de filas: controladores de presentación compartidos por comandos y menú.
use super::{DawApp, track_layout::TrackLayout};
use elevate::musical::track::TrackKind;

impl DawApp {
    pub(in crate::ui) fn track_layout(&self) -> TrackLayout {
        TrackLayout::build(
            &self.tracks,
            &self.track_presentation,
            self.ds.track_header_height,
        )
    }

    pub(in crate::ui) fn refresh_track_layout(&mut self) {
        let layout = self.track_layout();
        // Un destino asíncrono puede seguir existiendo dentro de un grupo recién plegado.
        if self.center_view == super::CenterView::Arrangement
            && layout.row(self.cursor_row).is_none()
        {
            if let Some(parent) = self
                .tracks
                .get(self.cursor_row)
                .and_then(|track| track.parent)
                && let Some(index) = self.tracks.iter().position(|track| track.id == parent)
            {
                self.select_track(index, false, false);
            }
        }
        self.timeline.layout = Some(layout);
    }

    pub(in crate::ui) fn track_collapsed(&self, index: usize) -> bool {
        self.tracks
            .get(index)
            .and_then(|t| self.track_presentation.get(&t.id))
            .is_some_and(|p| p.collapsed)
    }

    pub(in crate::ui) fn track_height(&self, index: usize) -> f32 {
        self.tracks
            .get(index)
            .and_then(|t| self.track_presentation.get(&t.id))
            .and_then(|p| p.height)
            .unwrap_or(self.ds.track_header_height)
            .clamp(32.0, 240.0)
    }

    pub(in crate::ui) fn toggle_track_fold(&mut self, index: usize) {
        let Some(track) = self
            .tracks
            .get(index)
            .filter(|t| t.kind == TrackKind::Group)
        else {
            return;
        };
        let id = track.id;
        let presentation = self.track_presentation.entry(id).or_default();
        presentation.collapsed = !presentation.collapsed;
        self.timeline.end_clip_drag();
        self.timeline.import_preview = None;
        self.track_control_drag = None;
        self.clip_menu = None;
        self.select_track(index, false, false);
        self.refresh_track_layout();
        let layout = self.track_layout();
        self.timeline.selected_midi.retain(|id| {
            self.timeline
                .other_midi_clips
                .iter()
                .chain(self.timeline.midi_clip.iter())
                .any(|clip| clip.id == *id && layout.row(clip.track).is_some())
        });
        self.reveal_track();
        self.status_message = if self.track_collapsed(index) {
            "Grupo plegado"
        } else {
            "Grupo desplegado"
        }
        .into();
    }

    pub(in crate::ui) fn set_selected_track_height(&mut self, height: Option<f32>) {
        if height.is_some_and(|h| !h.is_finite() || !(32.0..=240.0).contains(&h)) {
            self.status_message = "Altura de pista: 32–240 px".into();
            return;
        }
        let selected: Vec<_> = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(i, _)| self.track_selected(*i))
            .map(|(_, t)| t.id)
            .collect();
        for id in selected {
            self.track_presentation.entry(id).or_default().height = height;
        }
        self.timeline.end_clip_drag();
        self.track_control_drag = None;
        self.refresh_track_layout();
        self.reveal_track();
        self.status_message = "Altura aplicada a las pistas seleccionadas".into();
    }

    pub(in crate::ui) fn move_visible_track(&mut self, forward: bool) {
        let layout = self.track_layout();
        let Some(position) = layout.rows.iter().position(|r| r.track == self.cursor_row) else {
            return;
        };
        let next = if forward {
            (position + 1).min(layout.rows.len() - 1)
        } else {
            position.saturating_sub(1)
        };
        self.select_track(layout.rows[next].track, false, false);
        self.reveal_track();
    }
}
