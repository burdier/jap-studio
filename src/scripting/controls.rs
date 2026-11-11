//! Tabla Lua de controles: IDs existentes conservados y nuevos asignados por Rust.
use crate::musical::{
    automation::{ControlLane, ControlPoint},
    model::Pattern,
    time::Tick,
};
use mlua::{Lua, Table};

pub(super) const MAX_CONTROLS: usize = 65_536;

pub(super) fn encode(lua: &Lua, pattern: &Pattern) -> Result<Table, String> {
    if pattern.controls().points().len() > MAX_CONTROLS {
        return Err("El script supera 65536 puntos de control".into());
    }
    let rows = lua.create_table().map_err(|e| e.to_string())?;
    for (index, point) in pattern.controls().points().iter().enumerate() {
        let row = lua.create_table().map_err(|e| e.to_string())?;
        row.set("id", point.id).map_err(|e| e.to_string())?;
        row.set("start", point.at.0).map_err(|e| e.to_string())?;
        row.set("lane", point.lane.key())
            .map_err(|e| e.to_string())?;
        row.set("value", point.value).map_err(|e| e.to_string())?;
        rows.raw_set(index + 1, row).map_err(|e| e.to_string())?;
    }
    Ok(rows)
}

pub(super) fn decode(rows: Table, original: &Pattern, target: &mut Pattern) -> Result<(), String> {
    let length = super::arrays::length(&rows, MAX_CONTROLS)?;
    let old_ids: std::collections::HashSet<_> =
        original.controls().points().iter().map(|p| p.id).collect();
    let mut next_id = original.controls().next_id();
    let mut points = Vec::with_capacity(length);
    for index in 1..=length {
        let row: Table = rows.raw_get(index).map_err(|e| e.to_string())?;
        let id = match row
            .raw_get::<Option<u64>>("id")
            .map_err(|e| e.to_string())?
        {
            Some(id) if old_ids.contains(&id) => id,
            Some(_) => return Err("ID de control desconocido: omite id al crear un punto".into()),
            None => {
                let id = next_id;
                next_id = next_id.checked_add(1).ok_or("IDs de control agotados")?;
                id
            }
        };
        let key: String = row.raw_get("lane").map_err(|e| e.to_string())?;
        points.push(ControlPoint {
            id,
            lane: ControlLane::parse(&key).ok_or("Usa cc0–cc127, pitch o pressure")?,
            at: Tick(row.raw_get("start").map_err(|e| e.to_string())?),
            value: row.raw_get("value").map_err(|e| e.to_string())?,
        });
    }
    target.restore_controls(points, next_id)
}
