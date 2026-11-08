//! Inserción y selección sin reemplazar los clips MIDI que ya están en el Arrangement.
use super::Timeline;
use elevate::musical::clip::MidiClip;
use gpui::{Pixels, Point};

impl Timeline {
    pub(crate) fn insert_midi_clip(&mut self, clip: MidiClip) {
        if let Some(previous) = self.midi_clip.replace(clip) {
            self.other_midi_clips.push(previous);
        }
    }

    pub(crate) fn activate_midi_clip_at(
        &mut self,
        position: Point<Pixels>,
        track_height: f32,
    ) -> bool {
        let bounds = self.lanes_bounds.get();
        if !bounds.contains(&position) || track_height <= 0.0 {
            return false;
        }
        let beat = self
            .viewport
            .beat_at(f64::from(f32::from(position.x - bounds.left())));
        let track = (f32::from(position.y - bounds.top()) / track_height).floor() as usize;
        let hit = |clip: &MidiClip| {
            track == clip.track && beat >= clip.start_beat && beat < clip.end_beat()
        };
        if self.midi_clip.as_ref().is_some_and(hit) {
            return true;
        }
        if let Some(index) = self.other_midi_clips.iter().rposition(hit) {
            let clip = self.other_midi_clips.remove(index);
            self.insert_midi_clip(clip);
            return true;
        }
        false
    }
}
