//! Selección y borrado de notas: mismas operaciones desde teclado, catálogo y ratón.
use crate::ui::DawApp;
use elevate::musical::model::NoteId;
use std::collections::HashSet;

impl DawApp {
    pub(super) fn select_all_roll_notes(&mut self) {
        self.end_roll_drag();
        self.piano_roll.selected_notes = self
            .timeline
            .midi_clip
            .as_ref()
            .map(|clip| clip.notes().iter().map(|note| note.id).collect())
            .unwrap_or_default();
        self.piano_roll.selected = self.piano_roll.selected_notes.first().copied();
        self.piano_roll.hint = format!(
            "{} notas seleccionadas",
            self.piano_roll.selected_notes.len()
        );
    }

    pub(super) fn clear_roll_selection(&mut self) {
        self.end_roll_drag();
        self.piano_roll.selected = None;
        self.piano_roll.selected_notes.clear();
        self.piano_roll.hint = "Selección despejada".into();
    }

    pub(super) fn delete_roll_selection(&mut self) {
        if self.piano_roll.drag.is_some() {
            self.piano_roll.hint = "Suelta el ratón para terminar la selección".into();
            return;
        }
        let mut ids = self.piano_roll.selected_notes.clone();
        if ids.is_empty() {
            ids.extend(self.piano_roll.selected);
        }
        self.delete_roll_notes(&ids);
    }

    pub(super) fn delete_roll_notes(&mut self, ids: &[NoteId]) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        if clip.props.locked {
            self.piano_roll.hint = "Clip bloqueado".into();
            return;
        }
        let ids: HashSet<_> = ids.iter().copied().collect();
        let remaining: Vec<_> = clip
            .notes()
            .iter()
            .filter(|note| !ids.contains(&note.id))
            .copied()
            .collect();
        let count = clip.notes().len() - remaining.len();
        if count == 0 {
            self.piano_roll.hint = "Selecciona notas o arrastra una región para borrar".into();
            return;
        }
        // Una sola reconstrucción conserva expresión, CC e IDs; evita N borrados lineales.
        let Ok(pattern) = clip.pattern.with_notes(remaining, clip.pattern.next_id()) else {
            return;
        };
        self.stop_audio();
        if let Some(clip) = self.timeline.midi_clip.as_mut() {
            clip.replace_pattern(pattern);
            clip.muted_notes.retain(|id| !ids.contains(id));
        }
        self.piano_roll
            .selected_notes
            .retain(|id| !ids.contains(id));
        self.piano_roll.selected = self
            .piano_roll
            .selected
            .filter(|id| !ids.contains(id))
            .or_else(|| self.piano_roll.selected_notes.first().copied());
        if self
            .piano_roll
            .hovered_note
            .is_some_and(|id| ids.contains(&id))
        {
            self.piano_roll.hovered_note = None;
            self.piano_roll.hovered_resize_edge = false;
        }
        self.piano_roll.motion.borrow_mut().clear();
        self.piano_roll.hint = format!("{count} notas eliminadas");
    }
}
