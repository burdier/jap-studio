//! Inserción, selección y botones de cabecera por clip (Audio y MIDI) en el Arrangement.
use super::{ClipBarHit, DragKind, Timeline};
use crate::audio::AudioClip;
use elevate::musical::clip::MidiClip;
use gpui::{Pixels, Point};

impl Timeline {
    pub(crate) fn insert_midi_clip(&mut self, clip: MidiClip) {
        self.selected_midi = vec![clip.id];
        if let Some(previous) = self.midi_clip.replace(clip) {
            self.other_midi_clips.push(previous);
        }
    }

    pub(crate) fn insert_audio_clip(&mut self, clip: AudioClip) {
        if let Some(previous) = self.clip.replace(clip) {
            self.other_audio_clips.push(previous);
        }
    }

    pub(crate) fn activate_audio_clip_at(
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
        let Some(row) = self.row_at_y(f32::from(position.y - bounds.top()), track_height) else {
            return false;
        };
        let track = row.track;
        let hit = |clip: &AudioClip| {
            track == clip.track && beat >= clip.start_beat && beat < clip.end_beat(self.tempo.bpm())
        };
        if self.clip.as_ref().is_some_and(hit) {
            return true;
        }
        if let Some(index) = self.other_audio_clips.iter().rposition(hit) {
            let clip = self.other_audio_clips.remove(index);
            self.insert_audio_clip(clip);
            return true;
        }
        false
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
        let Some(row) = self.row_at_y(f32::from(position.y - bounds.top()), track_height) else {
            return false;
        };
        let track = row.track;
        let hit = |clip: &MidiClip| {
            track == clip.track && beat >= clip.start_beat && beat < clip.end_beat()
        };
        if self.midi_clip.as_ref().is_some_and(hit) {
            return true;
        }
        if let Some(index) = self.other_midi_clips.iter().rposition(hit) {
            let clip = self.other_midi_clips.remove(index);
            let selection = std::mem::take(&mut self.selected_midi);
            self.insert_midi_clip(clip);
            self.selected_midi = selection;
            return true;
        }
        false
    }

    /// Detecta si el clic cayó sobre la píldora superior estilo REAPER `[ i | M | L | ⟳ ]` de un clip.
    pub(crate) fn hit_clip_bar(
        &mut self,
        position: Point<Pixels>,
        track_height: f32,
    ) -> Option<ClipBarHit> {
        let bounds = self.lanes_bounds.get();
        if !bounds.contains(&position) || track_height <= 0.0 {
            return None;
        }
        let rel_y = f32::from(position.y - bounds.top());
        let row = self.row_at_y(rel_y, track_height)?;
        let local_y = rel_y - row.top;
        // La cápsula comienza en el borde superior del carril.
        let top = super::clip_frame::CONTROL_TOP;
        if !(top..=top + super::clip_frame::CONTROL_HEIGHT).contains(&local_y) {
            return None;
        }
        let kind = if self.activate_midi_clip_at(position, track_height) {
            DragKind::Midi
        } else if self.activate_audio_clip_at(position, track_height) {
            DragKind::Audio
        } else {
            return None;
        };
        let (start_beat, end_beat) = match kind {
            DragKind::Midi => {
                let clip = self.midi_clip.as_ref()?;
                (clip.start_beat, clip.end_beat())
            }
            DragKind::Audio => {
                let clip = self.clip.as_ref()?;
                (clip.start_beat, clip.end_beat(self.tempo.bpm()))
            }
        };
        let clip_left_x = self.viewport.x_at(start_beat).max(0.0) as f32;
        let clip_right_x = self
            .viewport
            .x_at(end_beat)
            .min(f64::from(f32::from(bounds.size.width))) as f32;
        if clip_right_x - clip_left_x < super::clip_frame::CONTROLS_MIN_WIDTH {
            return None;
        }
        let click_x = f32::from(position.x - bounds.left());
        let pill_offset = click_x - (clip_left_x + super::clip_frame::CONTROL_LEFT);
        if !(0.0..=super::clip_frame::CONTROL_WIDTH).contains(&pill_offset) {
            return None;
        }
        let slot = (pill_offset / super::clip_frame::CONTROL_SLOT).floor() as usize;
        Some(match slot {
            0 => ClipBarHit::Inspector(kind),
            1 => ClipBarHit::Mute(kind),
            2 => ClipBarHit::Lock(kind),
            _ => ClipBarHit::Loop(kind),
        })
    }
}
