//! Lua construye un lote tipado a partir de una lectura; Rust conserva la autoridad.
mod decode;
pub(crate) mod encode;
use crate::musical::track::{TrackBatch, TrackSnapshot};
use mlua::{Table, chunk::ChunkMode};

pub fn build_track_batch(source: &str, snapshot: &TrackSnapshot) -> Result<TrackBatch, String> {
    if source.len() > super::sandbox::MAX_SOURCE_BYTES {
        return Err("El script Lua supera 256 KiB".into());
    }
    let lua = super::sandbox::new().map_err(|error| error.to_string())?;
    lua.globals()
        .raw_set("tracks", encode::snapshot(&lua, snapshot)?)
        .map_err(|error| error.to_string())?;
    lua.globals()
        .raw_set("track_revision", snapshot.revision)
        .map_err(|error| error.to_string())?;
    let rows: Table = lua
        .load(source)
        .set_mode(ChunkMode::Text)
        .eval()
        .map_err(|error| format!("Lua pistas: {error}"))?;
    Ok(TrackBatch {
        origin: snapshot.clone(),
        edits: decode::operations(&rows)?,
    })
}
