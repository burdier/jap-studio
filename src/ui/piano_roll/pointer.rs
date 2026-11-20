//! Pointer conserva selección/arrastre; el doble clic crea o borra una sola nota.
use super::RollTool;
use crate::ui::DawApp;
use elevate::musical::time::Tick;
use gpui::MouseDownEvent;

impl DawApp {
    pub(super) fn handle_roll_pointer(&mut self, event: &MouseDownEvent) -> bool {
        if self.piano_roll.tool != RollTool::Select || event.click_count != 2 {
            return false;
        }
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return false;
        };
        let grid = self.piano_roll.grid.get();
        let Some((tick, pitch)) = grid.pick(event.position, clip.pattern.length()) else {
            return false;
        };
        let hit = clip
            .notes()
            .iter()
            .rev()
            .find(|note| {
                grid.note_bounds(note.start, note.duration, note.pitch)
                    .contains(&event.position)
            })
            .map(|note| note.id);
        self.end_roll_drag();
        if let Some(id) = hit {
            self.delete_roll_notes(&[id]);
        } else {
            self.insert_roll_note(tick, pitch, self.piano_roll.resolution.ticks());
        }
        true
    }

    pub(super) fn insert_roll_note(&mut self, tick: Tick, pitch: u8, length: Tick) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        if clip.props.locked {
            self.piano_roll.hint = "Clip bloqueado".into();
            return;
        }
        let step = self.piano_roll.resolution.ticks().0;
        let start = tick.0 / step * step;
        let duration = length.0.min(clip.pattern.length().0 - start);
        let instrument = self
            .tracks
            .get(clip.track)
            .and_then(|track| track.instrument)
            .unwrap_or(self.instrument.id());
        self.stop_audio();
        if let Some(clip) = self.timeline.midi_clip.as_mut() {
            if let Ok(id) = clip.edit_pattern(|pattern| {
                pattern.add_note(instrument, Tick(start), Tick(duration), pitch, 100)
            }) {
                self.piano_roll.cursor = Tick(start);
                self.piano_roll.selected = Some(id);
                self.piano_roll.selected_notes = vec![id];
                self.piano_roll.hint = "Nota insertada".into();
            }
        }
    }
}
