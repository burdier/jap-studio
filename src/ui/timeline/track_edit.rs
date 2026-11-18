//! Reasignación y copia de clips al modificar la colección de pistas.
use super::Timeline;

impl Timeline {
    pub(crate) fn remap_tracks_preserving_selection(
        &mut self,
        sources: &[Option<usize>],
        original: &[Option<usize>],
        preserve: bool,
    ) {
        let active = self.midi_clip.as_ref().map(|clip| clip.id);
        let selected = self.selected_midi.clone();
        self.remap_tracks(sources, original);
        if preserve {
            if let Some(id) = active {
                self.focus_midi_clip(id);
            }
            self.selected_midi = selected;
        }
    }

    pub(crate) fn focus_midi_clip(&mut self, id: elevate::musical::clip::ClipId) {
        if self.midi_clip.as_ref().is_some_and(|clip| clip.id == id) {
            return;
        }
        if let Some(index) = self.other_midi_clips.iter().position(|clip| clip.id == id) {
            let clip = self.other_midi_clips.remove(index);
            if let Some(previous) = self.midi_clip.replace(clip) {
                self.other_midi_clips.push(previous);
            }
        }
    }
    pub(crate) fn remap_tracks(&mut self, sources: &[Option<usize>], original: &[Option<usize>]) {
        self.end_clip_drag();
        self.import_preview = None;
        self.hover_track = None;
        self.hover_clip = None;
        let audio: Vec<_> = self
            .other_audio_clips
            .drain(..)
            .chain(self.clip.take())
            .collect();
        let midi: Vec<_> = self
            .other_midi_clips
            .drain(..)
            .chain(self.midi_clip.take())
            .collect();
        for (index, source) in sources.iter().enumerate() {
            let Some(source) = source else { continue };
            for clip in audio.iter().filter(|c| c.track == *source) {
                let mut copy = if original[*source] == Some(index) {
                    clip.clone()
                } else {
                    clip.duplicate()
                };
                copy.track = index;
                self.other_audio_clips.push(copy);
            }
            for clip in midi.iter().filter(|c| c.track == *source) {
                let mut copy = if original[*source] == Some(index) {
                    clip.clone()
                } else {
                    clip.duplicate()
                };
                copy.track = index;
                self.other_midi_clips.push(copy);
            }
        }
        self.clip = self.other_audio_clips.pop();
        self.midi_clip = self.other_midi_clips.pop();
        self.selected_midi.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::Timeline;
    use crate::audio::{AudioClip, AudioFile};
    use elevate::musical::{clip::MidiClip, model::Pattern, time::Tick};
    use std::sync::Arc;

    #[test]
    fn remapping_keeps_clip_id_focus_and_selection_when_new_tracks_are_inserted() {
        let mut timeline = Timeline::new(64.0);
        let pattern = Pattern::new(Tick(960)).unwrap();
        let first = MidiClip::new("Primero", 0, 0.0, pattern.clone());
        let second = MidiClip::new("Segundo", 1, 0.0, pattern);
        let first_id = first.id;
        let second_id = second.id;
        timeline.other_midi_clips.push(first);
        timeline.midi_clip = Some(second);
        timeline.selected_midi = vec![first_id, second_id];

        timeline.remap_tracks_preserving_selection(
            &[Some(1), None, Some(0)],
            &[Some(2), Some(0)],
            true,
        );

        assert_eq!(timeline.midi_clip.as_ref().unwrap().id, second_id);
        assert_eq!(timeline.midi_clip.as_ref().unwrap().track, 0);
        assert_eq!(timeline.other_midi_clips[0].id, first_id);
        assert_eq!(timeline.other_midi_clips[0].track, 2);
        assert_eq!(timeline.selected_midi, vec![first_id, second_id]);
    }

    #[test]
    fn duplicating_track_assigns_new_audio_clip_id() {
        let mut timeline = Timeline::new(64.0);
        let original = AudioClip {
            id: elevate::musical::clip::ClipId::fresh(),
            name: "Toma".into(),
            file: Arc::new(AudioFile {
                name: "toma.wav".into(),
                source_path: None,
                sample_rate: 48_000,
                frames: vec![[0.0, 0.0]],
                peaks: vec![[0.0, 0.0]],
                max_peak: 0.0,
            }),
            source_path_override: None,
            track: 0,
            start_beat: 0.0,
            props: Default::default(),
            trim_start: 0.0,
            trim_len: None,
        };
        timeline.insert_audio_clip(original.clone());
        timeline.remap_tracks(&[Some(0), Some(0)], &[Some(0)]);
        let clips: Vec<_> = timeline
            .other_audio_clips
            .iter()
            .chain(timeline.clip.iter())
            .collect();
        assert_eq!(clips.len(), 2);
        assert!(clips.iter().any(|clip| clip.id == original.id));
        assert_ne!(clips[0].id, clips[1].id);
        assert!(Arc::ptr_eq(&clips[0].file, &clips[1].file));
    }
}
