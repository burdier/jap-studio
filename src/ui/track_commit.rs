//! Publicación de un plan estructural ya validado; conserva estado por TrackId.
use super::DawApp;
use elevate::musical::track::{Track, TrackId};

/// Solo retiros de hijas: los carriles y procesadores de las pistas que quedan
/// siguen usando el mismo snapshot que arrancó el transporte.
fn removed_hilo_branches(
    old: &[Track],
    new: &[Track],
    sources: &[Option<usize>],
) -> Option<Vec<TrackId>> {
    if new.len() >= old.len() || new.len() != sources.len() {
        return None;
    }
    let mut next_kept = sources.iter().zip(new).peekable();
    let mut removed = Vec::new();
    for (index, track) in old.iter().enumerate() {
        if next_kept
            .peek()
            .is_some_and(|(source, kept)| **source == Some(index) && kept.id == track.id)
        {
            next_kept.next();
        } else if track
            .parent
            .is_some_and(|parent| new.iter().any(|p| p.id == parent && p.hilo.is_some()))
        {
            removed.push(track.id);
        } else {
            return None;
        }
    }
    (next_kept.next().is_none()).then_some(removed)
}

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
        // El mezclador conserva sus voces, racks y buses por TrackId. El orden
        // visual únicamente cambia los índices que usa el próximo plan musical.
        let reordered = tracks.len() == self.tracks.len()
            && tracks.len() == sources.len()
            && tracks.iter().zip(&sources).all(|(track, source)| {
                source.and_then(|index| self.tracks.get(index)) == Some(track)
            })
            && tracks
                .iter()
                .map(|track| track.id)
                .ne(self.tracks.iter().map(|track| track.id));
        let retired = if reordered {
            0
        } else if revision.is_none()
            && self.pattern_loop_beats.is_none()
            && let Some(player) = &self.player
            && !player.finished()
            && !player.failed()
            && let Some(removed) = removed_hilo_branches(&self.tracks, &tracks, &sources)
        {
            player.retire_tracks(&removed);
            removed.len()
        } else {
            self.stop_all();
            0
        };
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
        self.prune_midi_bindings_for(&tracks);
        self.frozen_tracks
            .retain(|id, _| tracks.iter().any(|t| t.id == *id));
        if reordered
            && let Some(clip) = &mut self.piano_roll.opened_clip
            && let Some(Some(track)) = original.get(clip.track)
        {
            clip.track = *track;
        }
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
        let live_hilos: std::collections::HashSet<_> = self
            .tracks
            .iter()
            .filter(|track| track.hilo.is_some())
            .map(|track| track.id)
            .collect();
        for clip in self
            .timeline
            .midi_clip
            .iter_mut()
            .chain(&mut self.timeline.other_midi_clips)
        {
            if clip
                .hilo_target
                .is_some_and(|target| !live_hilos.contains(&target))
            {
                clip.hilo_target = None;
            }
        }
        let live_pianos: std::collections::HashSet<_> = self
            .tracks
            .iter()
            .filter_map(|track| track.instrument)
            .collect();
        self.piano_instances
            .retain(|id, _| live_pianos.contains(id));
        if !live_pianos.contains(&self.instrument.id()) {
            if let Some(id) = self.piano_instances.keys().copied().next() {
                self.activate_piano(id);
                self.piano_instances
                    .retain(|id, _| live_pianos.contains(id));
            }
        }
        self.midi_armed_tracks.retain(|id| {
            self.tracks.iter().any(|track| {
                track.id == *id && (track.instrument.is_some() || track.hilo.is_some())
            })
        });
        // El mezclador conserva su voz preparada mientras procesa la cola;
        // los recursos de audición de la pista salen de la UI aquí.
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
        let live_synths: std::collections::HashSet<_> = self
            .tracks
            .iter()
            .filter_map(|track| track.instrument)
            .filter(|id| self.is_synth_id(*id))
            .collect();
        self.synth_instances
            .retain(|id, _| live_synths.contains(id));
        if !live_synths.contains(&self.active_synth_id) {
            if let Some(next) = live_synths.iter().next().copied() {
                let removed = self.active_synth_id;
                self.activate_synth(next);
                self.synth_instances.remove(&removed);
            } else {
                self.synth = Default::default();
                self.active_synth_id = crate::instruments::synth::SYNTH_ID;
                self.synth
                    .configure_audio(self.settings.audio.saved.clone());
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
        self.status_message = if reordered
            && (self.player.is_some() || self.file_playback.is_some())
        {
            "Pista reordenada · Play continúa".into()
        } else if retired > 0 {
            format!(
                "Ramas retiradas: {retired} · Play continúa · el reparto de clips se actualiza al volver a pulsar Play"
            )
        } else {
            format!(
                "{} pistas · Ctrl+clic: selección múltiple · Shift+clic: rango",
                self.tracks.len()
            )
        };
        if reordered {
            self.publish_clip_edit();
        }
    }
}
