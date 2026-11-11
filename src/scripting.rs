//! Herramientas Lua fuera del hilo de audio: copia de notas -> patrón validado.
pub mod actions;
mod builtin;
pub use builtin::{edit_builtin, inline_subdivision};
mod controls;
mod output;
pub mod packages;
pub mod push;
mod sandbox;
use crate::musical::{
    model::{NoteId, Pattern},
    time::PPQ,
};
use actions::MidiOptions;
use mlua::chunk::ChunkMode;
pub use output::MidiEdit;

/// API v1: `notes` contiene id, start, duration, pitch, velocity y selected.
/// `start`, `duration` y `length` se miden en ticks; `ppq` indica ticks por negra.
/// Compatibilidad para consumidores que solo necesitan el patrón transformado.
pub fn transform_midi(
    source: &str,
    pattern: &Pattern,
    selected: &[NoteId],
) -> Result<Pattern, String> {
    edit_midi(source, pattern, selected, MidiOptions::default()).map(|edit| edit.pattern)
}

/// Además de editar notas, `note.cuts` solicita cortes absolutos en ticks.
/// Rust asigna identidades y limita el resultado; Lua nunca recibe el clip mutable.
pub fn edit_midi(
    source: &str,
    pattern: &Pattern,
    selected: &[NoteId],
    options: MidiOptions,
) -> Result<MidiEdit, String> {
    if pattern.notes().len() > output::MAX_NOTES || options.amount > 100 {
        return Err("Demasiadas notas o intensidad fuera de 0–100".into());
    }
    if source.len() > sandbox::MAX_SOURCE_BYTES {
        return Err("El script Lua supera 256 KiB".into());
    }
    let lua = sandbox::new().map_err(|error| error.to_string())?;
    let globals = lua.globals();

    let selected: std::collections::HashSet<_> = selected.iter().copied().collect();
    let notes = lua
        .create_table_with_capacity(pattern.notes().len(), 0)
        .map_err(|error| error.to_string())?;
    for (index, note) in pattern.notes().iter().enumerate() {
        let row = lua
            .create_table_with_capacity(0, 10)
            .map_err(|error| error.to_string())?;
        row.set("id", note.id.0)
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
    globals
        .set("notes", notes.clone())
        .map_err(|error| error.to_string())?;
    globals
        .set("length", pattern.length().0)
        .map_err(|error| error.to_string())?;
    globals.set("ppq", PPQ).map_err(|error| error.to_string())?;
    globals
        .set("grid_ticks", options.grid.ticks().0)
        .map_err(|e| e.to_string())?;
    globals
        .set("amount", options.amount)
        .map_err(|e| e.to_string())?;
    globals.set("pan", options.pan).map_err(|e| e.to_string())?;
    globals
        .set("value", options.value)
        .map_err(|e| e.to_string())?;
    globals
        .set("strength", options.strength)
        .map_err(|e| e.to_string())?;
    globals
        .set("scale_root", options.scale_root)
        .map_err(|e| e.to_string())?;
    globals
        .set("scale_intervals", options.scale_intervals)
        .map_err(|e| e.to_string())?;
    let controls = controls::encode(&lua, pattern)?;
    globals
        .set("controls", controls.clone())
        .map_err(|e| e.to_string())?;
    lua.load(source)
        .set_mode(ChunkMode::Text)
        .exec()
        .map_err(|error| error.to_string())?;
    let mut result = output::decode(notes, pattern)?;
    controls::decode(controls, pattern, &mut result.pattern)?;
    Ok(result)
}
