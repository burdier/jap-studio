//! No aceptar huecos o filas escondidas fuera de la secuencia que valida Rust.
use mlua::{Table, Value};

pub(super) fn length(table: &Table, max: usize) -> Result<usize, String> {
    let length = table.raw_len();
    if length > max {
        return Err(format!("El arreglo supera {max} filas"));
    }
    let mut count = 0;
    for pair in table.pairs::<Value, Value>() {
        let (key, _) = pair.map_err(|e| e.to_string())?;
        if !matches!(key, Value::Integer(index) if index >= 1 && index as u64 <= length as u64) {
            return Err("El arreglo debe usar índices consecutivos desde 1".into());
        }
        count += 1;
        if count > max {
            return Err(format!("El arreglo supera {max} filas"));
        }
    }
    if count != length {
        return Err("El arreglo contiene huecos".into());
    }
    Ok(length)
}
