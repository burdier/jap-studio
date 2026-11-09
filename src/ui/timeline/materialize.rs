//! Selección MIDI por identidad y sustitución atómica de sus fuentes materializadas.
use super::Timeline;
use crate::ui::DawApp;
use elevate::musical::materialize::materialize;

impl Timeline {
    pub(crate) fn select_active_midi(&mut self, toggle: bool) {
        let Some(id) = self.midi_clip.as_ref().map(|c| c.id) else {
            return;
        };
        if !toggle {
            self.selected_midi = vec![id];
        } else if self.selected_midi.contains(&id) {
            self.selected_midi.retain(|selected| *selected != id);
        } else {
            self.selected_midi.push(id);
        }
    }

    pub(crate) fn materialize_selected_midi(&mut self) -> Result<usize, String> {
        let clips: Vec<_> = self
            .other_midi_clips
            .iter()
            .chain(self.midi_clip.iter())
            .filter(|clip| self.selected_midi.contains(&clip.id))
            .collect();
        let count = clips.len();
        let result = materialize(&clips)?;
        self.other_midi_clips
            .retain(|clip| !self.selected_midi.contains(&clip.id));
        if self
            .midi_clip
            .as_ref()
            .is_some_and(|clip| self.selected_midi.contains(&clip.id))
        {
            self.midi_clip = None;
        }
        self.edit_beat = result.start_beat;
        self.insert_midi_clip(result);
        Ok(count)
    }
}

impl DawApp {
    pub(in crate::ui) fn materialize_midi_selection(&mut self) {
        self.stop_audio();
        self.timeline.end_clip_drag();
        match self.timeline.materialize_selected_midi() {
            Ok(count) => {
                self.cursor_row = self.timeline.midi_clip.as_ref().unwrap().track;
                if self.piano_roll.open {
                    self.open_piano_roll();
                }
                self.status_message =
                    format!("Materialize · {count} clips MIDI → 1 · recorte consolidado");
            }
            Err(error) => self.status_message = error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elevate::musical::{
        clip::MidiClip,
        model::Pattern,
        time::{PPQ, Tick},
    };
    use gpui::{Bounds, point, px, size};

    fn clip(start: f64) -> MidiClip {
        MidiClip::new("MIDI", 0, start, Pattern::new(Tick(4 * PPQ)).unwrap())
    }

    #[test]
    fn selection_survives_activation_and_only_selected_sources_are_replaced() {
        let mut timeline = Timeline::new(32.0);
        timeline.lanes_bounds.set(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(800.0), px(72.0)),
        ));
        let untouched = clip(16.0);
        let untouched_id = untouched.id;
        timeline.insert_midi_clip(untouched);
        let first = clip(0.0);
        let first_id = first.id;
        timeline.insert_midi_clip(first);
        timeline.insert_midi_clip(clip(8.0));
        assert!(timeline.activate_midi_clip_at(point(px(20.0), px(30.0)), 72.0));
        timeline.select_active_midi(true);
        assert_eq!(timeline.selected_midi.len(), 2);
        assert!(timeline.selected_midi.contains(&first_id));
        assert_eq!(timeline.materialize_selected_midi().unwrap(), 2);
        assert_eq!(timeline.other_midi_clips.len(), 1);
        assert_eq!(timeline.other_midi_clips[0].id, untouched_id);
        let result = timeline.midi_clip.as_ref().unwrap();
        assert_eq!(result.duration_beats(), 12.0);
        assert_eq!(timeline.selected_midi, vec![result.id]);
        timeline.select_active_midi(true);
        assert!(timeline.selected_midi.is_empty());
    }

    #[test]
    fn failure_preserves_selection_sources_and_cursor() {
        let mut timeline = Timeline::new(32.0);
        timeline.insert_midi_clip(clip(0.0));
        let id = timeline.midi_clip.as_ref().unwrap().id;
        let mut locked = clip(8.0);
        locked.props.locked = true;
        timeline.insert_midi_clip(locked);
        timeline.selected_midi.push(id);
        let selection = timeline.selected_midi.clone();
        timeline.edit_beat = 10.0;
        assert!(timeline.materialize_selected_midi().is_err());
        assert_eq!(timeline.selected_midi, selection);
        assert_eq!(timeline.other_midi_clips[0].id, id);
        assert_eq!(timeline.midi_clip.as_ref().unwrap().start_beat, 8.0);
        assert_eq!(timeline.edit_beat, 10.0);
    }
}
