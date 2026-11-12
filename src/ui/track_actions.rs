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
        let levels = sources
            .iter()
            .map(|source| source.map_or(75, |i| self.track_levels[i]))
            .collect();
        let pans = sources
            .iter()
            .map(|source| source.map_or(50, |i| self.track_pans[i]))
            .collect();
        let preserve_midi = matches!(edit, TrackEdit::Move { .. });
        self.commit_track_plan(
            tracks,
            sources,
            levels,
            pans,
            selected,
            preserve_midi,
            true,
            None,
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
