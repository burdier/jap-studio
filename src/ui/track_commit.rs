//! Publicación de un plan estructural ya validado; conserva estado por TrackId.
use super::DawApp;
use elevate::musical::track::{Track, TrackId};

impl DawApp {
    pub(in crate::ui) fn commit_track_plan(
        &mut self,
        tracks: Vec<Track>,
        sources: Vec<Option<usize>>,
        levels: Vec<i32>,
        pans: Vec<i32>,
        selected: Vec<TrackId>,
        preserve_midi: bool,
        select_new: bool,
        revision: Option<u64>,
    ) {
        self.stop_all();
        if self.midi_capture.is_recording() {
            self.midi_capture.stop_recording(
                self.session_epoch
                    .elapsed()
                    .as_nanos()
                    .min(u64::MAX as u128) as u64,
                120.0,
            );
        }
        self.track_control_drag = None;
        self.track_reorder_drag = None;
        self.clip_menu = None;
        self.track_menu = None;
        self.track_color_menu = None;
        self.palette.close();
        if !preserve_midi {
            self.piano_roll.open = false;
        }
        let old_cursor = self.tracks.get(self.cursor_row).map(|t| t.id);
        let original: Vec<_> = self
            .tracks
            .iter()
            .map(|old| tracks.iter().position(|t| t.id == old.id))
            .collect();
        self.timeline
            .remap_tracks_preserving_selection(&sources, &original, preserve_midi);
        self.midi_mapper.remap_tracks(&original);
        self.track_levels = levels;
        self.track_pans = pans;
        self.ds.track_color_override = sources
            .iter()
            .map(|source| {
                source.and_then(|i| self.ds.track_color_override.get(i).copied().flatten())
            })
            .collect();
        let new_ids: Vec<_> = tracks
            .iter()
            .filter(|t| !self.tracks.iter().any(|old| old.id == t.id))
            .map(|t| t.id)
            .collect();
        self.track_presentation = tracks
            .iter()
            .zip(&sources)
            .filter_map(|(track, source)| {
                let mut view =
                    source.and_then(|i| self.track_presentation.get(&self.tracks[i].id).copied());
                // Compactar al agrupar o insertar dentro de un grupo. Las copias
                // conservan la presentación de su origen, incluida la altura elegida.
                let enters_group = track.parent.is_some()
                    && source.is_none_or(|i| {
                        self.tracks[i].id == track.id && self.tracks[i].parent != track.parent
                    });
                if enters_group {
                    view.get_or_insert_with(Default::default).height =
                        Some(super::track_layout::COMPACT_TRACK_HEIGHT);
                }
                view.map(|view| (track.id, view))
            })
            .collect();
        self.tracks = tracks;
        self.midi_armed_tracks.retain(|id| {
            self.tracks.iter().any(|track| {
                track.id == *id && (track.instrument.is_some() || track.hilo.is_some())
            })
        });
        // El borrado de una pista retira su instancia; los NoteOff pendientes ya
        // fueron liberados por stop_all antes de publicar el nuevo plan.
        let live_samplers: std::collections::HashSet<_> = self
            .tracks
            .iter()
            .filter_map(|track| track.instrument)
            .filter(|id| self.is_sampler_id(*id))
            .collect();
        self.sampler_instances
            .retain(|id, _| live_samplers.contains(id));
        if !live_samplers.contains(&self.active_sampler_id) {
            if let Some(next) = live_samplers.iter().next().copied() {
                let removed = self.active_sampler_id;
                self.activate_sampler(next);
                self.sampler_instances.remove(&removed);
            } else {
                self.sampler = Default::default();
                self.active_sampler_id = crate::instruments::sampler::SAMPLER_ID;
            }
        }
        self.prune_vst_instances();
        self.selected_tracks = if new_ids.is_empty() || !select_new {
            selected
                .into_iter()
                .filter(|id| self.tracks.iter().any(|t| t.id == *id))
                .collect()
        } else {
            new_ids
        };
        self.cursor_row = self
            .selected_tracks
            .first()
            .copied()
            .or(old_cursor)
            .and_then(|id| self.tracks.iter().position(|t| t.id == id))
            .unwrap_or(self.cursor_row.min(self.tracks.len().saturating_sub(1)));
        self.track_anchor = self.tracks.get(self.cursor_row).map(|t| t.id);
        // Insertar o duplicar una pista hija debe hacer visible el resultado.
        if let Some(parent) = self
            .tracks
            .get(self.cursor_row)
            .and_then(|track| track.parent)
        {
            if let Some(presentation) = self.track_presentation.get_mut(&parent) {
                presentation.collapsed = false;
            }
        }
        self.refresh_track_layout();
        self.reveal_track();
        self.sync_midi_focus();
        self.apply_track_gains();
        self.track_revision = revision.unwrap_or_else(|| self.track_revision.wrapping_add(1));
        self.status_message = format!(
            "{} pistas · Ctrl+clic: selección múltiple · Shift+clic: rango",
            self.tracks.len()
        );
    }
}
