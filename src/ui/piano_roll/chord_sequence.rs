//! Borrador de progresión, audición y confirmación atómica en el clip.
use super::chord::ChordPadState;
use crate::ui::DawApp;
use elevate::musical::{
    chord::{ChordAudition, ChordStep, MidiChordExpander, plan_sequence},
    midi::MidiTarget,
    model::NoteId,
    time::Tick,
};
use gpui::Context;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum ChordUnit {
    Beat,
    Bar,
}

pub(in crate::ui) struct ChordSequenceState {
    pub slots: Vec<Option<ChordPadState>>,
    pub unit: ChordUnit,
    pub selected: usize,
    pub audition: ChordAudition,
    pub midi_expander: MidiChordExpander,
}

impl Default for ChordSequenceState {
    fn default() -> Self {
        Self {
            slots: vec![None; 4],
            unit: ChordUnit::Bar,
            selected: 0,
            audition: ChordAudition::default(),
            midi_expander: MidiChordExpander::default(),
        }
    }
}

impl DawApp {
    pub(in crate::ui) fn release_chord_audition(&mut self) {
        let off = self.piano_roll.chord_sequence.audition.release();
        self.dispatch_routed_midi(off);
    }

    pub(in crate::ui) fn release_midi_chord_expander(&mut self) {
        let off = self.piano_roll.chord_sequence.midi_expander.release_all();
        if !off.is_empty() {
            let routed = self.midi_server.publish_inputs(off);
            self.dispatch_routed_midi(routed);
        }
    }

    pub(in crate::ui) fn audition_current_chord_pad(&mut self) {
        let pad = self.piano_roll.chord_pad;
        self.audition_chord_pad(pad);
    }

    pub(in crate::ui) fn audition_chord_slot(&mut self, index: usize) {
        let Some(pad) = self
            .piano_roll
            .chord_sequence
            .slots
            .get(index)
            .copied()
            .flatten()
        else {
            self.release_chord_audition();
            return;
        };
        self.audition_chord_pad(pad);
    }

    pub(in crate::ui) fn audition_chord_pad(&mut self, pad: ChordPadState) {
        self.release_chord_audition();
        let track_index = self
            .timeline
            .midi_clip
            .as_ref()
            .map(|clip| clip.track)
            .unwrap_or(self.cursor_row);
        let id = self
            .tracks
            .get(track_index)
            .and_then(|track| track.instrument)
            .unwrap_or(self.instrument.id());
        let (_, voices) =
            pad.preview_voices(self.piano_roll.scale_root, self.piano_roll.scale_minor);
        // No tomar propiedad de una nota que ya sostiene un teclado MIDI físico.
        let fresh: Vec<_> = voices
            .into_iter()
            .filter(|&note| {
                if id == self.instrument.id() {
                    !self.instrument.held(note)
                } else if id == self.active_sampler_id {
                    !self.sampler.held(note)
                } else if let Some(sampler) = self.sampler_instances.get(&id) {
                    !sampler.held(note)
                } else if id == self.vst.id {
                    !self.vst.held.contains(&note)
                } else if let Some(vst) = self.vst_instances.get(&id) {
                    !vst.held.contains(&note)
                } else {
                    false
                }
            })
            .collect();
        let on = self
            .piano_roll
            .chord_sequence
            .audition
            .press(MidiTarget::Instrument(id), &fresh);
        self.dispatch_routed_midi(on);
    }

    pub(super) fn insert_chord_sequence(&mut self, cx: &mut Context<Self>) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            self.status_message = "Abre un clip MIDI para insertar la secuencia".into();
            return;
        };
        if clip.props.locked {
            self.status_message = "Clip MIDI bloqueado".into();
            return;
        }
        let unit = match self.piano_roll.chord_sequence.unit {
            ChordUnit::Beat => self.timeline.meter.ticks_per_beat(),
            ChordUnit::Bar => self.timeline.meter.ticks_per_bar(),
        };
        let origin = Tick(self.piano_roll.cursor.0.max(0) / unit * unit);
        let slots: Vec<_> = self
            .piano_roll
            .chord_sequence
            .slots
            .iter()
            .map(|slot| {
                slot.map(|pad| ChordStep {
                    pitches: pad
                        .preview_voices(self.piano_roll.scale_root, self.piano_roll.scale_minor)
                        .1,
                    strum_ticks: i64::from(pad.strum_ticks),
                })
            })
            .collect();
        let notes = match plan_sequence(&slots, origin, Tick(unit), clip.pattern.length()) {
            Ok(notes) if !notes.is_empty() => notes,
            Ok(_) => {
                self.status_message = "Añade al menos un acorde a la secuencia".into();
                return;
            }
            Err(error) => {
                self.status_message = error;
                return;
            }
        };
        let id = self
            .tracks
            .get(clip.track)
            .and_then(|track| track.instrument)
            .unwrap_or(self.instrument.id());
        let mut pattern = clip.pattern.clone();
        let mut selected = Vec::<NoteId>::with_capacity(notes.len());
        for note in notes {
            match pattern.add_note(id, note.start, note.duration, note.pitch, 100) {
                Ok(note_id) => selected.push(note_id),
                Err(error) => {
                    self.status_message = error.to_string();
                    return;
                }
            }
        }
        self.release_chord_audition();
        self.stop_audio();
        if let Some(clip) = self.timeline.midi_clip.as_mut() {
            clip.replace_pattern(pattern);
        }
        self.piano_roll.selected = selected.first().copied();
        self.piano_roll.selected_notes = selected;
        let end = origin
            .0
            .saturating_add(unit.saturating_mul(slots.len() as i64));
        self.piano_roll.cursor =
            Tick(end.min(self.timeline.midi_clip.as_ref().unwrap().pattern.length().0 - 1));
        self.status_message = format!(
            "Secuencia insertada: {} notas",
            self.piano_roll.selected_notes.len()
        );
        self.piano_roll.hint = self.status_message.clone();
        cx.notify();
    }
}
