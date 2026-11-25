//! Ediciones de Arrangement que conservan intactas las fuentes de audio y MIDI.
use super::{DragKind, Timeline};

impl Timeline {
    /// (inicio, final, recorte inicial en beats, tiene recorte).
    pub(crate) fn clip_extent(&self, kind: DragKind) -> Option<(f64, f64, f64, bool)> {
        let beats_per_second = self.tempo.beats_per_second();
        match kind {
            DragKind::Midi => self.midi_clip.as_ref().map(|c| {
                (
                    c.start_beat,
                    c.end_beat(),
                    c.trim_start,
                    c.trim_start > 0.0 || c.trim_len.is_some(),
                )
            }),
            DragKind::Audio => self.clip.as_ref().map(|c| {
                (
                    c.start_beat,
                    c.end_beat(self.tempo.bpm()),
                    c.trim_start * beats_per_second,
                    c.trim_start > 0.0 || c.trim_len.is_some(),
                )
            }),
        }
    }

    pub(crate) fn can_split_clip(&self, kind: DragKind, beat: f64) -> bool {
        self.clip_extent(kind).is_some_and(|(start, end, _, _)| {
            beat.is_finite() && beat - start >= 0.25 && end - beat >= 0.25
        })
    }

    /// Ambas partes mantienen sus offsets de fuente; no se decodifica ni se cortan notas.
    pub(crate) fn split_clip(&mut self, kind: DragKind, beat: f64) -> bool {
        if !self.can_split_clip(kind, beat) {
            return false;
        }
        match kind {
            DragKind::Midi => {
                let Some(id) = self.midi_clip.as_ref().map(|clip| clip.id) else {
                    return false;
                };
                let before = self.midi_edit_draft(&[id]);
                let Some(left) = self.midi_clip.as_mut().filter(|c| !c.props.locked) else {
                    return false;
                };
                let mut right = left.duplicate();
                let length = beat - left.start_beat;
                right.trim_start += length;
                right.trim_len = Some(left.end_beat() - beat);
                right.start_beat = beat;
                let new_id = right.id;
                left.trim_len = Some(length);
                self.insert_midi_clip(right);
                self.commit_midi_edit(before, &[id, new_id]);
            }
            DragKind::Audio => {
                let beats_per_second = self.tempo.beats_per_second();
                let Some(left) = self.clip.as_mut().filter(|c| !c.props.locked) else {
                    return false;
                };
                let mut right = left.duplicate();
                let root = left.crossfade_root();
                if left.processing.edit.crossfade_root.is_some()
                    || left.processing.edit.crossfade.is_some()
                    || self
                        .other_audio_clips
                        .iter()
                        .any(|c| c.processing.edit.crossfade.is_some_and(|f| f.from == root))
                {
                    left.processing.edit.crossfade_root = Some(root);
                    right.processing.edit.crossfade_root = Some(root);
                    right.processing.edit.crossfade = left.processing.edit.crossfade;
                }
                let length = (beat - left.start_beat) / beats_per_second;
                right.trim_start += length;
                right.trim_len = Some(left.length_seconds() - length);
                right.start_beat = beat;
                left.trim_len = Some(length);
                self.insert_audio_clip(right);
            }
        }
        true
    }

    /// Recupera el contenido oculto manteniendo su alineación temporal.
    /// Si el origen caería antes de cero, conserva ese recorte inicial.
    pub(crate) fn restore_clip_trim(&mut self, kind: DragKind) {
        let beats_per_second = self.tempo.beats_per_second();
        match kind {
            DragKind::Midi => {
                let Some(id) = self.midi_clip.as_ref().map(|clip| clip.id) else {
                    return;
                };
                let before = self.midi_edit_draft(&[id]);
                if let Some(c) = self.midi_clip.as_mut().filter(|c| !c.props.locked) {
                    let recovered = c.trim_start.min(c.start_beat);
                    c.start_beat -= recovered;
                    c.trim_start -= recovered;
                    c.trim_len = None;
                }
                self.commit_midi_edit(before, &[id]);
            }
            DragKind::Audio => {
                if let Some(c) = self.clip.as_mut().filter(|c| !c.props.locked) {
                    let recovered = c.trim_start.min(c.start_beat / beats_per_second);
                    c.start_beat -= recovered * beats_per_second;
                    c.trim_start -= recovered;
                    c.trim_len = None;
                }
            }
        }
    }
}
