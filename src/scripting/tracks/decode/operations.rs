//! Operaciones Lua permitidas; el ejecutor musical vuelve a validar el lote completo.
use super::{
    boolean, fx_id, integer, keys, number, optional_reference, optional_string, raw, reference,
    reference_value, string,
};
use crate::musical::{
    fx::{FxEdit, FxKind, FxNode},
    track::{TrackBatchEdit, TrackDevice},
};
use mlua::{Table, Value};
use std::collections::BTreeMap;

pub(in crate::scripting::tracks) fn operations(
    rows: &Table,
) -> Result<Vec<TrackBatchEdit>, String> {
    if rows.metatable().is_some() {
        return Err("El lote no admite metatabla".into());
    }
    let count = crate::scripting::arrays::length(rows, 128)?;
    if count == 0 {
        return Err("El lote no contiene operaciones".into());
    }
    (1..=count)
        .map(|index| {
            let row: Table = rows
                .raw_get(index)
                .map_err(|e| format!("Operación {index}: {e}"))?;
            let op = string(&row, "op").map_err(|e| format!("Operación {index}: {e}"))?;
            decode(&row, &op).map_err(|e| format!("Operación {index} ({op}): {e}"))
        })
        .collect()
}

fn decode(row: &Table, op: &str) -> Result<TrackBatchEdit, String> {
    let edit = match op {
        "insert_audio" => {
            keys(row, &["op", "handle", "name", "after", "parent"])?;
            TrackBatchEdit::InsertAudio {
                handle: string(row, "handle")?,
                name: string(row, "name")?,
                after: optional_reference(row, "after")?,
                parent: optional_reference(row, "parent")?,
            }
        }
        "group" => {
            keys(row, &["op", "handle", "name", "members"])?;
            let members: Table = row.raw_get("members").map_err(|e| e.to_string())?;
            if members.metatable().is_some() {
                return Err("members no admite metatabla".into());
            }
            let count = crate::scripting::arrays::length(&members, 64)?;
            let members = (1..=count)
                .map(|i| reference_value(members.raw_get(i).map_err(|e| e.to_string())?))
                .collect::<Result<Vec<_>, _>>()?;
            TrackBatchEdit::Group {
                handle: string(row, "handle")?,
                name: string(row, "name")?,
                members,
            }
        }
        "rename" => {
            keys(row, &["op", "track", "name"])?;
            TrackBatchEdit::Rename {
                track: reference(row, "track")?,
                name: string(row, "name")?,
            }
        }
        "move" => {
            keys(row, &["op", "track", "after"])?;
            TrackBatchEdit::Move {
                track: reference(row, "track")?,
                after: reference(row, "after")?,
            }
        }
        "mute" | "solo" => {
            keys(row, &["op", "track", "value"])?;
            let track = reference(row, "track")?;
            let value = boolean(row, "value")?;
            if op == "mute" {
                TrackBatchEdit::Mute { track, value }
            } else {
                TrackBatchEdit::Solo { track, value }
            }
        }
        "level" | "pan" => {
            keys(row, &["op", "track", "value"])?;
            let track = reference(row, "track")?;
            let value = integer(row, "value")?;
            if op == "level" {
                TrackBatchEdit::Level { track, value }
            } else {
                TrackBatchEdit::Pan { track, value }
            }
        }
        "fx.insert_utility" => {
            keys(
                row,
                &["op", "track", "name", "gain", "inside", "before", "after"],
            )?;
            let gain = if matches!(raw(row, "gain")?, Value::Nil) {
                1.0
            } else {
                number(row, "gain")?
            };
            let node = FxNode::new(
                optional_string(row, "name", "Utility")?,
                FxKind::Effect {
                    plugin_id: TrackDevice::Hay {
                        catalog_id: "hay.utility".into(),
                    },
                    parameters: BTreeMap::from([(0, gain)]),
                },
            );
            TrackBatchEdit::Fx {
                track: reference(row, "track")?,
                edit: FxEdit::Insert(node, super::placement(row)?),
            }
        }
        "fx.insert_serial" | "fx.insert_parallel" => {
            keys(row, &["op", "track", "name", "inside", "before", "after"])?;
            let (default, kind) = if op == "fx.insert_serial" {
                ("Serie", FxKind::SerialChain { children: vec![] })
            } else {
                ("Paralelo", FxKind::ParallelChain { branches: vec![] })
            };
            let node = FxNode::new(optional_string(row, "name", default)?, kind);
            TrackBatchEdit::Fx {
                track: reference(row, "track")?,
                edit: FxEdit::Insert(node, super::placement(row)?),
            }
        }
        "fx.rename" => {
            keys(row, &["op", "track", "fx", "name"])?;
            TrackBatchEdit::Fx {
                track: reference(row, "track")?,
                edit: FxEdit::Rename(fx_id(row, "fx")?, string(row, "name")?),
            }
        }
        "fx.toggle" | "fx.remove" | "fx.duplicate" => {
            keys(row, &["op", "track", "fx"])?;
            let id = fx_id(row, "fx")?;
            let edit = match op {
                "fx.toggle" => FxEdit::Toggle(id),
                "fx.remove" => FxEdit::Remove(id),
                _ => FxEdit::Duplicate(id),
            };
            TrackBatchEdit::Fx {
                track: reference(row, "track")?,
                edit,
            }
        }
        "fx.parameter" => {
            keys(row, &["op", "track", "fx", "parameter", "value"])?;
            let parameter = u32::try_from(integer(row, "parameter")?)
                .map_err(|_| "Parámetro FX fuera de rango")?;
            TrackBatchEdit::Fx {
                track: reference(row, "track")?,
                edit: FxEdit::Parameter(fx_id(row, "fx")?, parameter, number(row, "value")?),
            }
        }
        "fx.move" => {
            keys(row, &["op", "track", "fx", "inside", "before", "after"])?;
            TrackBatchEdit::Fx {
                track: reference(row, "track")?,
                edit: FxEdit::Move(fx_id(row, "fx")?, super::placement(row)?),
            }
        }
        _ => return Err("Operación de pista desconocida".into()),
    };
    Ok(edit)
}
