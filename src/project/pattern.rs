//! Notas y automatización de los patrones de Arrangement y de batería.
use super::{
    midi::{order_id, reconcile_order},
    track::{read_bool, read_i64},
};
use crate::musical::{
    model::{InstrumentId, NoteEvent, NoteId, Pattern},
    time::Tick,
};
use loro::{LoroDoc, LoroMap};
use std::collections::HashSet;

pub(super) fn write(
    doc: &LoroDoc,
    prefix: &str,
    id: u64,
    map: &LoroMap,
    pattern: &Pattern,
    muted_notes: &HashSet<NoteId>,
) -> Result<(), String> {
    map.insert("pattern_len", pattern.length().0).map_err(err)?;
    map.insert(
        "next_note_id",
        i64::try_from(pattern.next_id()).map_err(|_| "ID de nota fuera de rango")?,
    )
    .map_err(err)?;
    super::midi_controls::write(doc, prefix, id, map, pattern)?;
    let notes = pattern.notes();
    let note_order = doc.get_movable_list(format!("{prefix}-notes:{id}"));
    reconcile_order(
        &note_order,
        &notes.iter().map(|note| note.id.0).collect::<Vec<_>>(),
    )?;
    for note in notes {
        let map = doc.get_map(format!("{prefix}-note:{id}:{}", note.id.0));
        map.insert("instrument", i64::from(note.instrument.0))
            .map_err(err)?;
        map.insert("start", note.start.0).map_err(err)?;
        map.insert("duration", note.duration.0).map_err(err)?;
        map.insert("pitch", i64::from(note.pitch)).map_err(err)?;
        map.insert(
            "release_velocity",
            i64::from(note.expression.release_velocity),
        )
        .map_err(err)?;
        map.insert("probability", i64::from(note.expression.probability))
            .map_err(err)?;
        map.insert("pan", i64::from(note.expression.pan))
            .map_err(err)?;
        map.insert("velocity", i64::from(note.velocity))
            .map_err(err)?;
        map.insert("muted", muted_notes.contains(&note.id))
            .map_err(err)?;
    }

    Ok(())
}

pub(super) fn read(
    doc: &LoroDoc,
    prefix: &str,
    id: u64,
    map: &LoroMap,
) -> Result<(Pattern, HashSet<NoteId>), String> {
    let note_order = doc.get_movable_list(format!("{prefix}-notes:{id}"));
    let mut notes = Vec::with_capacity(note_order.len());
    let mut note_ids = HashSet::new();
    let mut muted_notes = HashSet::new();
    for index in 0..note_order.len() {
        let note_id = order_id(&note_order, index).ok_or("ID de nota inválido")?;
        if !note_ids.insert(note_id) {
            return Err("ID de nota duplicado".into());
        }
        let note = doc.get_map(format!("{prefix}-note:{id}:{note_id}"));
        if read_bool(&note, "muted")? {
            muted_notes.insert(NoteId(note_id));
        }
        notes.push(NoteEvent {
            id: NoteId(note_id),
            instrument: InstrumentId(
                u32::try_from(read_i64(&note, "instrument")?)
                    .map_err(|_| "Instrumento MIDI inválido")?,
            ),
            start: Tick(read_i64(&note, "start")?),
            duration: Tick(read_i64(&note, "duration")?),
            pitch: u8::try_from(read_i64(&note, "pitch")?).map_err(|_| "Pitch MIDI inválido")?,
            expression: crate::musical::note_expression::NoteExpression {
                pan: if note.get("pan").is_none() {
                    0
                } else {
                    i8::try_from(read_i64(&note, "pan")?).map_err(|_| "Pan de nota inválido")?
                },
                release_velocity: if note.get("release_velocity").is_none() {
                    64
                } else {
                    u8::try_from(read_i64(&note, "release_velocity")?)
                        .map_err(|_| "Release velocity inválida")?
                },
                probability: if note.get("probability").is_none() {
                    100
                } else {
                    u8::try_from(read_i64(&note, "probability")?)
                        .map_err(|_| "Probabilidad inválida")?
                },
            },
            velocity: u8::try_from(read_i64(&note, "velocity")?)
                .map_err(|_| "Velocity MIDI inválida")?,
        });
    }
    let next_note_id = u64::try_from(read_i64(map, "next_note_id")?)
        .map_err(|_| "Siguiente ID de nota inválido")?;
    let mut pattern =
        Pattern::restore_with_next_id(Tick(read_i64(map, "pattern_len")?), notes, next_note_id)
            .map_err(|error| error.to_string())?;
    super::midi_controls::read(doc, prefix, id, map, &mut pattern)?;
    Ok((pattern, muted_notes))
}

fn err(error: loro::LoroError) -> String {
    error.to_string()
}
