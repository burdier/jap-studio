//! Portapapeles de notas con expresión, offsets y mute conservados.
use crate::ui::DawApp;
use elevate::musical::time::Tick;

impl DawApp {
    pub(super) fn duplicate_roll_notes(&mut self) {
        let clipboard = std::mem::take(&mut self.piano_roll.copied_notes);
        self.roll_copy(false);
        if let Some(end) = self
            .piano_roll
            .copied_notes
            .iter()
            .map(|(note, _)| note.start.0 + note.duration.0)
            .max()
        {
            let cursor = std::mem::replace(&mut self.piano_roll.cursor, Tick(end));
            self.roll_paste();
            self.piano_roll.cursor = cursor;
        }
        self.piano_roll.copied_notes = clipboard;
    }

    pub(in crate::ui) fn roll_copy(&mut self, cut: bool) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        if cut && (clip.props.locked || self.piano_roll.drag.is_some()) {
            self.piano_roll.hint =
                "Termina el arrastre y desbloquea el clip antes de cortar".into();
            return;
        }
        let notes: Vec<_> = clip
            .notes()
            .iter()
            .filter(|note| {
                self.piano_roll.selected_notes.contains(&note.id)
                    || (self.piano_roll.selected_notes.is_empty()
                        && self.piano_roll.selected == Some(note.id))
            })
            .map(|note| (note.clone(), clip.muted_notes.contains(&note.id)))
            .collect();
        if notes.is_empty() {
            self.piano_roll.hint = "Selecciona notas para copiar".into();
            return;
        }
        let count = notes.len();
        if cut {
            let ids = [clip.id];
            let before = self.timeline.midi_edit_draft(&ids);
            self.stop_audio();
            if let Some(clip) = self.timeline.midi_clip.as_mut() {
                clip.edit_pattern(|pattern| {
                    for (note, _) in &notes {
                        let _ = pattern.remove_note(note.id);
                    }
                });
                for (note, _) in &notes {
                    clip.muted_notes.remove(&note.id);
                }
            }
            self.piano_roll.selected = None;
            self.piano_roll.selected_notes.clear();
            self.timeline.commit_midi_edit(before, &ids);
        }
        self.piano_roll.copied_notes = notes;
        self.piano_roll.hint = format!(
            "{count} notas {}",
            if cut { "cortadas" } else { "copiadas" }
        );
    }

    pub(in crate::ui) fn roll_paste(&mut self) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        if clip.props.locked || self.piano_roll.drag.is_some() {
            self.piano_roll.hint = "Termina el arrastre y desbloquea el clip antes de pegar".into();
            return;
        }
        let Some(origin) = self
            .piano_roll
            .copied_notes
            .iter()
            .map(|(note, _)| note.start.0)
            .min()
        else {
            return;
        };
        let offset = self.piano_roll.cursor.0.max(0) - origin;
        let mut pattern = clip.pattern.clone();
        let mut muted = clip.muted_notes.clone();
        let mut selected = Vec::new();
        for (note, is_muted) in &self.piano_roll.copied_notes {
            let instrument = self
                .tracks
                .get(clip.track)
                .and_then(|track| track.instrument)
                .unwrap_or(note.instrument);
            match pattern.add_note_copy(
                note,
                Tick(note.start.0 + offset),
                note.duration,
                instrument,
            ) {
                Ok(id) => {
                    selected.push(id);
                    if *is_muted {
                        muted.insert(id);
                    }
                }
                Err(_) => {
                    self.piano_roll.hint =
                        "La selección no cabe: amplía el clip o mueve el cursor".into();
                    return;
                }
            }
        }
        let ids = [clip.id];
        let before = self.timeline.midi_edit_draft(&ids);
        self.stop_audio();
        if let Some(clip) = self.timeline.midi_clip.as_mut() {
            clip.replace_pattern(pattern);
            clip.muted_notes = muted;
        }
        self.timeline.commit_midi_edit(before, &ids);
        self.piano_roll.hint = format!("{} notas pegadas", selected.len());
        self.piano_roll.selected = selected.first().copied();
        self.piano_roll.selected_notes = selected;
    }
}
