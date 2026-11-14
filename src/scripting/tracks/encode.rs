//! Snapshot de solo lectura lógica: cambiar esta tabla Lua no altera el proyecto.
use crate::musical::{
    fx::{FxKind, FxNode},
    track::{TrackKind, TrackSnapshot},
};
use mlua::{Lua, Table};

pub(crate) fn snapshot(lua: &Lua, snapshot: &TrackSnapshot) -> Result<Table, String> {
    let rows = lua.create_table().map_err(|e| e.to_string())?;
    for (index, entry) in snapshot.entries.iter().enumerate() {
        let row = lua.create_table().map_err(|e| e.to_string())?;
        row.raw_set("id", entry.track.id.0)
            .map_err(|e| e.to_string())?;
        row.raw_set("name", entry.track.name.as_ref())
            .map_err(|e| e.to_string())?;
        row.raw_set("kind", kind(entry.track.kind))
            .map_err(|e| e.to_string())?;
        if let Some(instrument) = entry.track.instrument {
            row.raw_set("instrument", instrument.0)
                .map_err(|e| e.to_string())?;
        }
        if let Some(parent) = entry.track.parent {
            row.raw_set("parent", parent.0).map_err(|e| e.to_string())?;
        }
        row.raw_set("muted", entry.track.muted)
            .map_err(|e| e.to_string())?;
        row.raw_set("solo", entry.track.solo)
            .map_err(|e| e.to_string())?;
        row.raw_set("level", entry.mix.level)
            .map_err(|e| e.to_string())?;
        row.raw_set("pan", entry.mix.pan)
            .map_err(|e| e.to_string())?;
        row.raw_set("fx", nodes(lua, entry.track.chain.nodes())?)
            .map_err(|e| e.to_string())?;
        rows.raw_set(index + 1, row).map_err(|e| e.to_string())?;
    }
    Ok(rows)
}

fn kind(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Drum => "drum",
        TrackKind::Synth => "synth",
        TrackKind::Effect => "effect",
        TrackKind::Audio => "audio",
        TrackKind::Group => "group",
    }
}

fn nodes(lua: &Lua, nodes: &[FxNode]) -> Result<Table, String> {
    let rows = lua.create_table().map_err(|e| e.to_string())?;
    for (index, node) in nodes.iter().enumerate() {
        let row = lua.create_table().map_err(|e| e.to_string())?;
        row.raw_set("id", node.id.0).map_err(|e| e.to_string())?;
        row.raw_set("name", node.name.as_str())
            .map_err(|e| e.to_string())?;
        row.raw_set("enabled", node.enabled)
            .map_err(|e| e.to_string())?;
        let kind = match &node.kind {
            FxKind::Effect {
                plugin_id,
                parameters,
            } => {
                let plugin = match plugin_id {
                    crate::musical::track::TrackDevice::Hay { catalog_id } => catalog_id,
                    crate::musical::track::TrackDevice::Vst3 { class_id } => class_id,
                };
                row.raw_set("plugin", plugin.as_str())
                    .map_err(|e| e.to_string())?;
                let values = lua.create_table().map_err(|e| e.to_string())?;
                for (&id, &value) in parameters {
                    values.raw_set(id, value).map_err(|e| e.to_string())?;
                }
                row.raw_set("parameters", values)
                    .map_err(|e| e.to_string())?;
                "effect"
            }
            FxKind::SerialChain { children } => {
                row.raw_set("children", self::nodes(lua, children)?)
                    .map_err(|e| e.to_string())?;
                "serial"
            }
            FxKind::ParallelChain { branches } => {
                row.raw_set("children", self::nodes(lua, branches)?)
                    .map_err(|e| e.to_string())?;
                "parallel"
            }
        };
        row.raw_set("kind", kind).map_err(|e| e.to_string())?;
        rows.raw_set(index + 1, row).map_err(|e| e.to_string())?;
    }
    Ok(rows)
}
