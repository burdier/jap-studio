//! Correspondencia acotada de notas originales y transformadas, fuera de audio RT.
use super::live::{LiveEvent, LiveProcessor, MAX_EVENTS_PER_BLOCK};
use crate::musical::{midi::MidiMessage, model::NoteId};
mod safety;

pub const MAX_ACTIVE_NOTES: usize = 1024;

#[derive(Clone, Copy)]
struct ActiveNote {
    id: Option<NoteId>,
    source_channel: u8,
    source_pitch: u8,
    output_channel: u8,
    output_pitch: u8,
}

/// Lua decide la transformación; Rust conserva a qué nota debe volver cada
/// liberación. Una instancia mantiene su propio estado y su propio script.
pub struct LiveNoteProcessor {
    lua: LiveProcessor,
    active: Vec<ActiveNote>,
    pending: Vec<ActiveNote>,
    output: Vec<LiveEvent>,
    disabled: bool,
    last_error: Option<String>,
}

impl LiveNoteProcessor {
    pub fn prepare(source: &str) -> Result<Self, String> {
        Ok(Self {
            lua: LiveProcessor::prepare(source)?,
            active: Vec::with_capacity(MAX_ACTIVE_NOTES),
            pending: Vec::with_capacity(MAX_ACTIVE_NOTES),
            output: Vec::with_capacity(MAX_ACTIVE_NOTES.max(MAX_EVENTS_PER_BLOCK)),
            disabled: false,
            last_error: None,
        })
    }

    /// Publica el bloque completo o conserva la propiedad anterior sin cambios.
    /// Aún se ejecuta fuera del callback hasta auditar asignaciones/GC de `mlua`.
    pub fn process_block(
        &mut self,
        input: &[LiveEvent],
        frames: u32,
    ) -> Result<&[LiveEvent], String> {
        if self.disabled {
            return Err("Lua live: instancia desactivada".into());
        }
        self.output.clear();
        let transformed = self.lua.process_block(input, frames)?;
        self.pending.clear();
        self.pending.extend_from_slice(&self.active);
        let result = project(input, transformed, &mut self.pending, &mut self.output);
        if let Err(error) = result {
            self.output.clear();
            return Err(error);
        }
        std::mem::swap(&mut self.active, &mut self.pending);
        Ok(&self.output)
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }
}

fn project(
    input: &[LiveEvent],
    transformed: &[LiveEvent],
    active: &mut Vec<ActiveNote>,
    output: &mut Vec<LiveEvent>,
) -> Result<(), String> {
    for (&source, &mapped) in input.iter().zip(transformed) {
        match (source.message, mapped.message) {
            (
                MidiMessage::NoteOn {
                    note: source_pitch, ..
                },
                MidiMessage::NoteOn {
                    note: output_pitch,
                    velocity,
                },
            ) => {
                if source.offset != mapped.offset || velocity == 0 {
                    return Err("Lua live: NoteOn cambió de offset o tiene velocidad cero".into());
                }
                if active.len() == MAX_ACTIVE_NOTES
                    || source
                        .note_id
                        .is_some_and(|id| active.iter().any(|n| n.id == Some(id)))
                {
                    return Err("Lua live: demasiadas notas activas o NoteId repetido".into());
                }
                active.push(ActiveNote {
                    id: source.note_id,
                    source_channel: source.channel,
                    source_pitch,
                    output_channel: mapped.channel,
                    output_pitch,
                });
                output.push(mapped);
            }
            (MidiMessage::NoteOff { note: source_pitch }, MidiMessage::NoteOff { .. }) => {
                let index = active
                    .iter()
                    .rposition(|n| {
                        n.id == source.note_id
                            && n.source_channel == source.channel
                            && n.source_pitch == source_pitch
                    })
                    .ok_or("Lua live: NoteOff sin NoteOn correspondiente")?;
                let owner = active.remove(index);
                output.push(LiveEvent {
                    offset: source.offset,
                    channel: owner.output_channel,
                    message: MidiMessage::NoteOff {
                        note: owner.output_pitch,
                    },
                    note_id: source.note_id,
                });
            }
            (MidiMessage::AllNotesOff, MidiMessage::AllNotesOff) => {
                active.clear();
                output.push(mapped);
            }
            (
                MidiMessage::ControlChange { .. }
                | MidiMessage::PitchBend { .. }
                | MidiMessage::ProgramChange { .. },
                MidiMessage::ControlChange { .. }
                | MidiMessage::PitchBend { .. }
                | MidiMessage::ProgramChange { .. },
            ) => output.push(mapped),
            _ => return Err("Lua live: transformación incompatible con el tipo de evento".into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: Option<u64>, offset: u32, pitch: u8, on: bool) -> LiveEvent {
        LiveEvent {
            offset,
            channel: 1,
            message: if on {
                MidiMessage::NoteOn {
                    note: pitch,
                    velocity: 90,
                }
            } else {
                MidiMessage::NoteOff { note: pitch }
            },
            note_id: id.map(NoteId),
        }
    }

    #[test]
    fn lua_transposes_notes_but_host_preserves_ids_offsets_and_releases() {
        let source = include_str!("../../assets/scripts/components/midi/transposer.lua");
        let mut processor = LiveNoteProcessor::prepare(source).unwrap();
        let on = processor
            .process_block(
                &[note(Some(11), 3, 60, true), note(Some(12), 7, 60, true)],
                64,
            )
            .unwrap();
        assert_eq!(on[0], note(Some(11), 3, 67, true));
        assert_eq!(on[1], note(Some(12), 7, 67, true));
        assert_eq!(processor.active_count(), 2);
        let off = processor
            .process_block(
                &[note(Some(11), 9, 60, false), note(Some(12), 12, 60, false)],
                64,
            )
            .unwrap();
        assert_eq!(off[0], note(Some(11), 9, 67, false));
        assert_eq!(off[1], note(Some(12), 12, 67, false));
        assert_eq!(processor.active_count(), 0);
    }

    #[test]
    fn repeated_physical_pitch_uses_lifo_even_when_lua_state_changes() {
        let source = "local step = 0; return function(kind, channel, a, b, offset) \
            if kind == 1 then step = step + 1; a = a + step \
            elseif kind == 2 then a = 100 end \
            return kind, channel, a, b, offset end";
        let mut processor = LiveNoteProcessor::prepare(source).unwrap();
        let on = processor
            .process_block(&[note(None, 1, 60, true), note(None, 2, 60, true)], 64)
            .unwrap();
        assert_eq!(
            [on[0].message, on[1].message],
            [
                note(None, 1, 61, true).message,
                note(None, 2, 62, true).message
            ]
        );
        let off = processor
            .process_block(&[note(None, 3, 60, false), note(None, 4, 60, false)], 64)
            .unwrap();
        assert_eq!(
            [off[0].message, off[1].message],
            [
                note(None, 3, 62, false).message,
                note(None, 4, 61, false).message
            ]
        );
        assert_eq!(processor.active_count(), 0);
    }

    #[test]
    fn invalid_lua_output_rolls_back_the_whole_note_registry() {
        let source = "return function(kind, channel, a, b, offset) \
            if offset == 2 then return 99, channel, a, b, offset end \
            return kind, channel, a, b, offset end";
        let mut processor = LiveNoteProcessor::prepare(source).unwrap();
        assert!(
            processor
                .process_block(
                    &[note(Some(1), 1, 60, true), note(Some(2), 2, 62, true)],
                    64
                )
                .is_err()
        );
        assert_eq!(processor.active_count(), 0);
        assert!(processor.output.is_empty());
    }
}
