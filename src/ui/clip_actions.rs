//! Operaciones comunes a clips de audio y MIDI, independientes de su menú.
use super::{DawApp, timeline::DragKind};
use elevate::musical::clip::ClipProps;

impl DawApp {
    /// Clip activo en el punto de edición. El MIDI prevalece si audio y MIDI se solapan,
    /// igual que en el orden de dibujo y picking del Arrangement.
    pub(in crate::ui) fn clip_kind_at_cursor(&mut self) -> Option<DragKind> {
        let track = self.cursor_row;
        let beat = self.timeline.edit_beat();
        if self.timeline.midi_clip.as_ref().is_some_and(|clip| {
            clip.track == track && beat >= clip.start_beat && beat < clip.end_beat()
        }) {
            Some(DragKind::Midi)
        } else if let Some(index) = self.timeline.other_midi_clips.iter().rposition(|clip| {
            clip.track == track && beat >= clip.start_beat && beat < clip.end_beat()
        }) {
            let clip = self.timeline.other_midi_clips.remove(index);
            self.timeline.insert_midi_clip(clip);
            Some(DragKind::Midi)
        } else if self.timeline.clip.as_ref().is_some_and(|clip| {
            clip.track == track && beat >= clip.start_beat && beat < clip.end_beat(self.timeline.tempo.bpm())
        }) {
            Some(DragKind::Audio)
        } else if let Some(index) = self.timeline.other_audio_clips.iter().rposition(|clip| {
            clip.track == track && beat >= clip.start_beat && beat < clip.end_beat(self.timeline.tempo.bpm())
        }) {
            let clip = self.timeline.other_audio_clips.remove(index);
            self.timeline.insert_audio_clip(clip);
            Some(DragKind::Audio)
        } else {
            None
        }
    }

    /// Despacho de las opciones de clip del catálogo. Las operaciones que necesitan
    /// parámetros de texto/color siguen disponibles en el menú contextual.
    pub(in crate::ui) fn execute_clip_catalog_action(&mut self, command: &str) {
        if command == "clip.materialize" {
            self.materialize_midi_selection();
            return;
        }
        let Some(kind) = self.clip_kind_at_cursor() else {
            self.status_message = "Coloca el cursor sobre un clip para editarlo".into();
            return;
        };
        match command {
            "clip.duplicate" => self.duplicate_active_clip(kind),
            "clip.split" => {
                let beat = self.timeline.edit_beat();
                self.stop_audio();
                if !self.timeline.split_clip(kind, beat) {
                    self.status_message = "No se puede dividir el clip aquí".into();
                }
            }
            "clip.restore_trim" => {
                self.stop_audio();
                self.timeline.restore_clip_trim(kind);
            }
            "clip.mute" => self.toggle_clip_mute(kind),
            "clip.lock" => self.toggle_clip_lock(kind),
            "clip.gain.down" | "clip.gain.reset" | "clip.gain.up" => {
                self.stop_audio();
                if let Some(props) = self.active_clip_props_mut(kind) {
                    props.gain_db = match command {
                        "clip.gain.down" => (props.gain_db - 3.0).clamp(-24.0, 12.0),
                        "clip.gain.up" => (props.gain_db + 3.0).clamp(-24.0, 12.0),
                        _ => 0.0,
                    };
                    self.status_message = format!("Ganancia del clip: {:+.1} dB", props.gain_db);
                }
            }
            "clip.track.previous" => self.move_active_clip_track(kind, -1),
            "clip.track.next" => self.move_active_clip_track(kind, 1),
            "clip.delete" => self.delete_active_clip(kind),
            _ => self.status_message = format!("Acción de clip desconocida: {command}"),
        }
    }

    pub(in crate::ui) fn active_clip_name(&self, kind: DragKind) -> &str {
        match kind {
            DragKind::Midi => self.timeline.midi_clip.as_ref().map(|c| c.name.as_str()),
            DragKind::Audio => self.timeline.clip.as_ref().map(|c| c.name.as_str()),
        }
        .unwrap_or("Clip")
    }

    pub(in crate::ui) fn rename_active_clip(&mut self, kind: DragKind, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        match kind {
            DragKind::Midi => {
                if let Some(c) = &mut self.timeline.midi_clip {
                    c.name = name.into();
                }
            }
            DragKind::Audio => {
                if let Some(c) = &mut self.timeline.clip {
                    c.name = name.into();
                }
            }
        }
        self.status_message = format!("Clip renombrado: {name}");
    }

    pub(in crate::ui) fn toggle_clip_mute(&mut self, kind: DragKind) {
        self.stop_audio();
        if let Some(props) = self.active_clip_props_mut(kind) {
            props.muted = !props.muted;
        }
    }

    pub(in crate::ui) fn toggle_clip_lock(&mut self, kind: DragKind) {
        if let Some(props) = self.active_clip_props_mut(kind) {
            props.locked = !props.locked;
        }
    }

    pub(in crate::ui) fn active_clip_props(&self, kind: DragKind) -> Option<ClipProps> {
        match kind {
            DragKind::Midi => self.timeline.midi_clip.as_ref().map(|c| c.props),
            DragKind::Audio => self.timeline.clip.as_ref().map(|c| c.props),
        }
    }

    pub(in crate::ui) fn active_clip_props_mut(
        &mut self,
        kind: DragKind,
    ) -> Option<&mut ClipProps> {
        match kind {
            DragKind::Midi => self.timeline.midi_clip.as_mut().map(|c| &mut c.props),
            DragKind::Audio => self.timeline.clip.as_mut().map(|c| &mut c.props),
        }
    }

    pub(in crate::ui) fn duplicate_active_clip(&mut self, kind: DragKind) {
        match kind {
            DragKind::Midi => {
                if let Some(mut copy) = self
                    .timeline
                    .midi_clip
                    .as_ref()
                    .map(|clip| clip.duplicate())
                {
                    copy.start_beat = copy.end_beat();
                    self.status_message = format!(
                        "Clip MIDI «{}» duplicado en {:.2} beats",
                        copy.name, copy.start_beat
                    );
                    self.timeline.insert_midi_clip(copy);
                }
            }
            DragKind::Audio => {
                if let Some(mut copy) = self.timeline.clip.clone() {
                    copy.start_beat = copy.end_beat(self.timeline.tempo.bpm());
                    self.status_message = format!(
                        "Clip de audio «{}» duplicado en {:.2} beats",
                        copy.name, copy.start_beat
                    );
                    self.timeline.insert_audio_clip(copy);
                }
            }
        }
    }

    pub(in crate::ui) fn delete_active_clip(&mut self, kind: DragKind) {
        self.stop_audio();
        match kind {
            DragKind::Midi => {
                if let Some(clip) = &self.timeline.midi_clip {
                    self.timeline.selected_midi.retain(|id| *id != clip.id);
                }
                self.timeline.midi_clip = self.timeline.other_midi_clips.pop();
                if self.timeline.midi_clip.is_none() {
                    self.piano_roll.open = false;
                }
                self.status_message = "Clip MIDI eliminado".into();
            }
            DragKind::Audio => {
                self.timeline.clip = self.timeline.other_audio_clips.pop();
                self.status_message = "Clip de audio eliminado".into();
            }
        }
    }

    pub(in crate::ui) fn move_active_clip_track(&mut self, kind: DragKind, delta: isize) {
        if self.active_clip_props(kind).is_some_and(|p| p.locked) {
            return;
        }
        self.stop_audio();
        let current = match kind {
            DragKind::Midi => self.timeline.midi_clip.as_ref().map(|c| c.track),
            DragKind::Audio => self.timeline.clip.as_ref().map(|c| c.track),
        };
        let Some(current) = current else { return };
        let destination = if delta < 0 {
            (0..current).rev().find(|i| self.track_accepts_clips(*i))
        } else {
            (current + 1..self.tracks.len()).find(|i| self.track_accepts_clips(*i))
        };
        let Some(destination) = destination else {
            return;
        };
        match kind {
            DragKind::Midi => {
                if let Some(clip) = self.timeline.midi_clip.as_mut() {
                    clip.track = destination;
                    self.cursor_row = clip.track;
                }
            }
            DragKind::Audio => {
                if let Some(clip) = self.timeline.clip.as_mut() {
                    clip.track = destination;
                    self.cursor_row = clip.track;
                }
            }
        }
    }
}
