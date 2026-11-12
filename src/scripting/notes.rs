//! Copia común de notas para scripts de archivo y paquetes declarativos.
use crate::musical::model::{NoteId, Pattern};
use mlua::{Lua, Table};

pub(super) fn encode(lua: &Lua, pattern: &Pattern, selected: &[NoteId]) -> Result<Table, String> {
    let selected: std::collections::HashSet<_> = selected.iter().copied().collect();
    let notes = lua
        .create_table_with_capacity(pattern.notes().len(), 0)
        .map_err(|error| error.to_string())?;
    for (index, note) in pattern.notes().iter().enumerate() {
        let row = lua
            .create_table_with_capacity(0, 11)
            .map_err(|error| error.to_string())?;
        row.set("id", note.id.0)
            .map_err(|error| error.to_string())?;
        row.set("instrument", note.instrument.0)
            .map_err(|error| error.to_string())?;
        row.set("start", note.start.0)
            .map_err(|error| error.to_string())?;
        row.set("duration", note.duration.0)
            .map_err(|error| error.to_string())?;
        row.set("pitch", note.pitch)
            .map_err(|error| error.to_string())?;
        row.set("release_velocity", note.expression.release_velocity)
            .map_err(|e| e.to_string())?;
        row.set("pan", note.expression.pan)
            .map_err(|e| e.to_string())?;
        row.set("probability", note.expression.probability)
            .map_err(|e| e.to_string())?;
        row.set("velocity", note.velocity)
            .map_err(|error| error.to_string())?;
        row.set(
            "selected",
            selected.is_empty() || selected.contains(&note.id),
        )
        .map_err(|error| error.to_string())?;
        notes
            .raw_set(index + 1, row)
            .map_err(|error| error.to_string())?;
    }
    Ok(notes)
}
