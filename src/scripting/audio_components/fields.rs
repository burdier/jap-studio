//! Lectura estricta de tablas y valores Lua.
use mlua::{Table, Value};
pub(super) fn identifier(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
pub(super) fn as_table(value: Value, path: &str) -> Result<Table, String> {
    match value {
        Value::Table(t) if t.metatable().is_none() => Ok(t),
        _ => Err(format!("{path}: se esperaba tabla sin metatabla")),
    }
}
pub(super) fn table(parent: &Table, key: &str) -> Result<Table, String> {
    as_table(parent.raw_get(key).map_err(|e| e.to_string())?, key)
}
pub(super) fn string(parent: &Table, key: &str) -> Result<String, String> {
    match parent.raw_get::<Value>(key).map_err(|e| e.to_string())? {
        Value::String(s) => s.to_str().map(|s| s.to_string()).map_err(|e| e.to_string()),
        _ => Err(format!("{key}: se esperaba texto")),
    }
}
pub(super) fn number(parent: &Table, key: &str) -> Result<f64, String> {
    match parent.raw_get::<Value>(key).map_err(|e| e.to_string())? {
        Value::Integer(n) => Ok(n as f64),
        Value::Number(n) if n.is_finite() => Ok(n),
        _ => Err(format!("{key}: se esperaba número finito")),
    }
}
pub(super) fn integer(parent: &Table, key: &str) -> Result<i64, String> {
    match parent.raw_get::<Value>(key).map_err(|e| e.to_string())? {
        Value::Integer(value) => Ok(value),
        _ => Err(format!("{key}: se esperaba entero")),
    }
}
pub(super) fn label(parent: &Table, key: &str, max: usize) -> Result<String, String> {
    let value = string(parent, key)?;
    if value.is_empty()
        || value.len() > max
        || value.contains(['\0', '\r', '\n'])
        || value.trim().is_empty()
    {
        Err(format!("{key}: texto inválido"))
    } else {
        Ok(value)
    }
}
pub(super) fn fields(table: &Table, allowed: &[&str], path: &str) -> Result<(), String> {
    for pair in table.clone().pairs::<Value, Value>() {
        let (key, _) = pair.map_err(|e| e.to_string())?;
        let Value::String(key) = key else {
            return Err(format!("{path}: clave no textual"));
        };
        let key = key.to_str().map_err(|e| e.to_string())?;
        if !allowed.contains(&key.as_ref()) {
            return Err(format!("{path}.{key}: campo desconocido"));
        }
    }
    Ok(())
}
pub(super) fn array(
    table: &Table,
    min: usize,
    max: usize,
    path: &str,
) -> Result<Vec<Table>, String> {
    let len = table.raw_len();
    if len < min || len > max {
        return Err(format!("{path}: tamaño fuera de {min}..{max}"));
    }
    let mut rows = Vec::with_capacity(len);
    for i in 1..=len {
        rows.push(as_table(
            table.raw_get(i).map_err(|e| e.to_string())?,
            &format!("{path}[{i}]"),
        )?);
    }
    let mut count = 0;
    for pair in table.clone().pairs::<Value, Value>() {
        let (key, _) = pair.map_err(|e| e.to_string())?;
        if !matches!(key, Value::Integer(i) if i >= 1 && i as usize <= len) {
            return Err(format!("{path}: arreglo disperso o con claves adicionales"));
        }
        count += 1;
    }
    if count != len {
        return Err(format!("{path}: arreglo disperso"));
    }
    Ok(rows)
}
