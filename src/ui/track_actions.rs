//! Operaciones de pistas compartidas por menú, catálogo y comandos.
use super::{CenterView, DawApp, VimMode};
use elevate::musical::track::{TrackEdit, TrackKind, edit_tracks};

impl DawApp {
    pub(in crate::ui) fn track_selected(&self, index: usize) -> bool {
        if self.selected_tracks.is_empty() {
            index == self.cursor_row
        } else {
            self.tracks
                .get(index)
                .is_some_and(|track| self.selected_tracks.contains(&track.id))
        }
    }

    /// Operador Vim dd y Delete usan la misma eliminación que menú/catálogo.
    pub(in crate::ui) fn handle_track_delete_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        pending: bool,
    ) -> bool {
        let modifiers = event.keystroke.modifiers;
        if self.mode != VimMode::Normal
            || self.center_view != CenterView::Arrangement
            || modifiers.control
            || modifiers.alt
            || modifiers.platform
            || modifiers.shift
        {
            return false;
        }
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(&event.keystroke.key);
        match key {
            "delete" => {
                if !event.is_held {
                    self.execute_command("track.delete");
                }
                true
            }
            "d" if self.vim_enabled => {
                if self.controller.enabled() {
                    self.track_delete_pending = false;
                    return false;
                }
                if event.is_held {
                    self.track_delete_pending = pending;
                } else if pending {
                    self.execute_command("track.delete");
                } else {
                    self.track_delete_pending = true;
                    self.status_message =
                        "d · pulsa d para eliminar las pistas seleccionadas · Esc cancela".into();
                }
                true
            }
            _ => false,
        }
    }

    pub(in crate::ui) fn select_track(&mut self, index: usize, toggle: bool, range: bool) {
        self.track_delete_pending = false;
        let Some(track) = self.tracks.get(index) else {
            return;
        };
        let id = track.id;
        if range {
            let anchor = self
                .track_anchor
                .and_then(|id| self.tracks.iter().position(|t| t.id == id))
                .unwrap_or(index);
            self.selected_tracks = self
                .track_layout()
                .rows
                .iter()
                .filter(|row| (anchor.min(index)..=anchor.max(index)).contains(&row.track))
                .map(|row| self.tracks[row.track].id)
                .collect();
        } else if toggle {
            if self.selected_tracks.contains(&id) {
                self.selected_tracks.retain(|old| *old != id);
            } else {
                self.selected_tracks.push(id);
            }
            self.track_anchor = Some(id);
        } else {
            self.selected_tracks = vec![id];
            self.track_anchor = Some(id);
        }
        self.cursor_row = index;
        self.sync_midi_focus();
    }

    pub(in crate::ui) fn edit_track_structure(&mut self, edit: TrackEdit) {
        self.track_delete_pending = false;
        let mut selected = self.selected_tracks.clone();
        if selected.is_empty() {
            if let Some(track) = self.tracks.get(self.cursor_row) {
                selected.push(track.id);
            }
        }
        let (tracks, sources) = match edit_tracks(&self.tracks, &selected, edit, self.cursor_row) {
            Ok(plan) => plan,
            Err(error) => {
                self.status_message = error;
                return;
            }
        };
        if matches!(edit, TrackEdit::Move { .. })
            && tracks
                .iter()
                .map(|track| track.id)
                .eq(self.tracks.iter().map(|track| track.id))
        {
            return;
        }
        let active_midi = self.timeline.midi_clip.as_ref().map(|clip| clip.id);
        let selected_midi = self.timeline.selected_midi.clone();
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
        if !matches!(edit, TrackEdit::Move { .. }) {
            self.piano_roll.open = false;
        }
        let old_cursor = self.tracks.get(self.cursor_row).map(|t| t.id);
        let original: Vec<_> = self
            .tracks
            .iter()
            .map(|old| tracks.iter().position(|t| t.id == old.id))
            .collect();
        self.timeline.remap_tracks(&sources, &original);
        if matches!(edit, TrackEdit::Move { .. }) {
            if let Some(id) = active_midi {
                self.timeline.focus_midi_clip(id);
            }
            self.timeline.selected_midi = selected_midi;
        }
        self.midi_mapper.remap_tracks(&original);
        self.track_levels = sources
            .iter()
            .map(|source| source.map_or(75, |i| self.track_levels[i]))
            .collect();
        self.track_pans = sources
            .iter()
            .map(|source| source.map_or(50, |i| self.track_pans[i]))
            .collect();
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
        self.selected_tracks = if new_ids.is_empty() {
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
        self.status_message = format!(
            "{} pistas · Ctrl+clic: selección múltiple · Shift+clic: rango",
            self.tracks.len()
        );
    }

    pub(in crate::ui) fn track_accepts_clips(&self, index: usize) -> bool {
        self.tracks.get(index).is_some_and(|t| t.accepts_clips())
    }

    pub(in crate::ui) fn track_title(&self, index: usize) -> String {
        let t = &self.tracks[index];
        if t.kind == TrackKind::Group {
            let children: Vec<_> = self
                .tracks
                .iter()
                .filter(|child| child.parent == Some(t.id))
                .collect();
            let active = children
                .iter()
                .filter(|child| !child.muted && !t.muted)
                .count();
            format!("{} [{active}/{}]", t.name, children.len())
        } else {
            format!("{}{}", if t.parent.is_some() { "↳ " } else { "" }, t.name)
        }
    }
}
