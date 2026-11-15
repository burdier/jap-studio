//! Alcance revisado PR-08: Lua transforma el snapshot MIDI antes de Play.
use super::{live::LiveEvent, live_notes::LiveNoteProcessor};
use crate::musical::{
    midi::MidiMessage,
    model::{NoteId, Pattern},
};

/// Procesa eventos en orden musical. Los bloques de un evento son sintéticos:
/// el patrón original conserva ticks y duración; Lua solo decide el tono.
pub fn transform_pattern(pattern: &Pattern, source: &str) -> Result<Pattern, String> {
    let mut processor = LiveNoteProcessor::prepare(source)?;
    let mut events = Vec::with_capacity(pattern.notes().len() * 2);
    for note in pattern.notes() {
        events.push((note.start.0, false, note.id, note.pitch, note.velocity));
        let end = note
            .start
            .0
            .checked_add(note.duration.0)
            .ok_or("Lua pre-Play: tiempo de nota agotado")?;
        events.push((end, true, note.id, note.pitch, note.velocity));
    }
    // En una frontera compartida se libera antes de pulsar la siguiente nota.
    events.sort_by_key(|event| (event.0, event.1));
    let mut pitches = Vec::<(NoteId, u8)>::with_capacity(pattern.notes().len());
    for (_, off, id, pitch, velocity) in events {
        let input = LiveEvent {
            offset: 0,
            channel: 0,
            message: if off {
                MidiMessage::NoteOff { note: pitch }
            } else {
                MidiMessage::NoteOn {
                    note: pitch,
                    velocity,
                }
            },
            note_id: Some(id),
        };
        let block = processor.process_or_disable(&[input], 1);
        if let Some(error) = block.error {
            return Err(format!("Lua pre-Play: {error}"));
        }
        if !off {
            let [
                LiveEvent {
                    message: MidiMessage::NoteOn { note, .. },
                    ..
                },
            ] = block.events
            else {
                return Err("Lua pre-Play: salida de nota inválida".into());
            };
            pitches.push((id, *note));
        }
    }
    if processor.active_count() != 0 {
        return Err("Lua pre-Play: quedaron notas sin liberar".into());
    }
    pattern
        .with_note_pitches(&pitches)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musical::{model::InstrumentId, time::Tick};

    #[test]
    fn preplay_transposes_a_snapshot_and_preserves_source_ids_and_timing() {
        let mut pattern = Pattern::new(Tick(960)).unwrap();
        let id = pattern
            .add_note(InstrumentId(1), Tick(100), Tick(200), 60, 90)
            .unwrap();
        let script = include_str!("../../assets/scripts/components/midi/transposer.lua");
        let result = transform_pattern(&pattern, script).unwrap();
        assert_eq!(result.notes()[0].id, id);
        assert_eq!(
            (result.notes()[0].start, result.notes()[0].duration),
            (Tick(100), Tick(200))
        );
        assert_eq!(result.notes()[0].pitch, 67);
        assert_eq!(pattern.notes()[0].pitch, 60);
        assert_eq!(result.next_id(), pattern.next_id());
    }

    #[test]
    fn a_failed_script_cannot_publish_a_partial_snapshot() {
        let mut pattern = Pattern::new(Tick(960)).unwrap();
        pattern
            .add_note(InstrumentId(1), Tick(0), Tick(100), 60, 90)
            .unwrap();
        pattern
            .add_note(InstrumentId(1), Tick(200), Tick(100), 125, 90)
            .unwrap();
        let script = include_str!("../../assets/scripts/components/midi/transposer.lua");
        assert!(transform_pattern(&pattern, script).is_err());
        assert_eq!(
            [pattern.notes()[0].pitch, pattern.notes()[1].pitch],
            [60, 125]
        );
    }
}
