//! Herramientas Lua fuera del hilo de audio: copia de notas -> patrón validado.
pub mod actions;
mod builtin;
pub use builtin::{edit_builtin, inline_subdivision};
mod controls;
mod notes;
mod output;
mod target;
pub use target::MidiScriptTarget;
mod arrays;
pub mod audio_components;
pub mod packages;
pub mod push;
mod sandbox;
pub mod tracks;
use crate::musical::{
    model::{NoteId, Pattern},
    time::PPQ,
};
use actions::MidiOptions;
use mlua::chunk::ChunkMode;
pub use output::MidiEdit;

/// API v1: `notes` contiene identidad, instrumento, tiempo, pitch, expresión y selección.
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

    let notes = notes::encode(&lua, pattern, selected)?;
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
