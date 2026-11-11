//! Reasignación y copia de clips al modificar la colección de pistas.
use super::Timeline;

impl Timeline {
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
                let mut copy = clip.clone();
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
