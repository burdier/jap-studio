//! Lectura estricta de referencias y campos; no evalúa metatablas del resultado.
mod operations;
use crate::musical::{
    fx::{FxId, FxPlacement},
    track::{TrackId, TrackRef},
};
use mlua::{Table, Value};
pub(super) use operations::operations;

fn raw(row: &Table, key: &str) -> Result<Value, String> {
    row.raw_get(key).map_err(|e| format!("{key}: {e}"))
}

fn string(row: &Table, key: &str) -> Result<String, String> {
    match raw(row, key)? {
        Value::String(value) => value
            .to_str()
            .map(|v| v.to_string())
            .map_err(|e| e.to_string()),
        _ => Err(format!("{key} debe ser texto")),
    }
}

fn optional_string(row: &Table, key: &str, default: &str) -> Result<String, String> {
    if matches!(raw(row, key)?, Value::Nil) {
        Ok(default.into())
    } else {
        string(row, key)
    }
}

fn integer(row: &Table, key: &str) -> Result<i32, String> {
    match raw(row, key)? {
        Value::Integer(value) => i32::try_from(value).map_err(|_| format!("{key} fuera de rango")),
        _ => Err(format!("{key} debe ser entero")),
    }
}

fn number(row: &Table, key: &str) -> Result<f32, String> {
    let value = match raw(row, key)? {
        Value::Integer(value) => value as f64,
        Value::Number(value) => value,
        _ => return Err(format!("{key} debe ser número")),
    };
    let value = value as f32;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{key} debe ser finito"))
    }
}

fn boolean(row: &Table, key: &str) -> Result<bool, String> {
    match raw(row, key)? {
        Value::Boolean(value) => Ok(value),
        _ => Err(format!("{key} debe ser booleano")),
    }
}

fn reference_value(value: Value) -> Result<TrackRef, String> {
    match value {
        Value::Integer(id) if id > 0 => u32::try_from(id)
            .map(TrackId)
            .map(TrackRef::Id)
            .map_err(|_| "ID de pista fuera de rango".into()),
        Value::String(handle) => {
            let handle = handle.to_str().map_err(|e| e.to_string())?.to_string();
            if handle.is_empty() {
                Err("Handle vacío".into())
            } else {
                Ok(TrackRef::Handle(handle))
            }
        }
        _ => Err("Referencia de pista: ID entero positivo o handle".into()),
    }
}

fn reference(row: &Table, key: &str) -> Result<TrackRef, String> {
    reference_value(raw(row, key)?)
}

fn optional_reference(row: &Table, key: &str) -> Result<Option<TrackRef>, String> {
    match raw(row, key)? {
        Value::Nil => Ok(None),
        value => reference_value(value).map(Some),
    }
}

fn fx_id(row: &Table, key: &str) -> Result<FxId, String> {
    match raw(row, key)? {
        Value::Integer(id) if id > 0 => Ok(FxId(id as u64)),
        _ => Err(format!("{key}: ID FX entero positivo requerido")),
    }
}

fn placement(row: &Table) -> Result<FxPlacement, String> {
    let inside = raw(row, "inside")?;
    let before = raw(row, "before")?;
    let after = raw(row, "after")?;
    let count = [inside.clone(), before.clone(), after.clone()]
        .into_iter()
        .filter(|value| !matches!(value, Value::Nil))
        .count();
    if count > 1 {
        return Err("Usa solo uno de inside, before o after".into());
    }
    if !matches!(inside, Value::Nil) {
        return fx_id(row, "inside").map(|id| FxPlacement::Inside(Some(id)));
    }
    if !matches!(before, Value::Nil) {
        return fx_id(row, "before").map(FxPlacement::Before);
    }
    if !matches!(after, Value::Nil) {
        return fx_id(row, "after").map(FxPlacement::After);
    }
    Ok(FxPlacement::Inside(None))
}

fn keys(row: &Table, allowed: &[&str]) -> Result<(), String> {
    if row.metatable().is_some() {
        return Err("Las operaciones no admiten metatablas".into());
    }
    for pair in row.pairs::<Value, Value>() {
        let (key, _) = pair.map_err(|e| e.to_string())?;
        let Value::String(key) = key else {
            return Err("Clave de operación no textual".into());
        };
        let key = key.to_str().map_err(|e| e.to_string())?;
        if !allowed.contains(&key.as_ref()) {
            return Err(format!("Campo desconocido: {key}"));
        }
    }
    Ok(())
}
