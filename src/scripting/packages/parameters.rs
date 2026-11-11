use super::{
    PackageError,
    fields::{Fields, identifier},
};
use crate::actions::{ActionParameter, Argument, ParameterKind};
use mlua::Value;
use std::collections::HashSet;

pub(super) fn read(parent: &Fields<'_>) -> Result<Vec<ActionParameter<String>>, PackageError> {
    let Value::Table(table) = parent.get("parameters")? else {
        return Err(parent.error("parameters", "Se esperaba un arreglo denso"));
    };
    if table.metatable().is_some() {
        return Err(parent.error("parameters", "No se admiten metatablas"));
    }
    let mut count = 0;
    let mut last = 0;
    for pair in table.pairs::<Value, Value>() {
        let (key, _) = pair.map_err(|e| parent.error("parameters", e))?;
        let Value::Integer(index @ 1..=32) = key else {
            return Err(parent.error("parameters", "Usa índices consecutivos 1–32"));
        };
        last = last.max(index);
        count += 1;
    }
    if last != count {
        return Err(parent.error("parameters", "El arreglo contiene huecos"));
    }
    let mut parameters = Vec::with_capacity(count as usize);
    let mut keys = HashSet::new();
    for index in 1..=count {
        let prefix = format!("parameters[{index}]");
        let value = table
            .raw_get::<Value>(index)
            .map_err(|e| parent.error(&prefix, e))?;
        let Value::Table(table) = value else {
            return Err(parent.error(&prefix, "Se esperaba un descriptor de parámetro"));
        };
        let fields = Fields {
            table,
            location: parent.location,
            prefix,
        };
        let parameter = read_parameter(&fields)?;
        if !keys.insert(parameter.key.clone()) {
            return Err(fields.error("key", "Clave duplicada"));
        }
        parameters.push(parameter);
    }
    Ok(parameters)
}

fn read_parameter(fields: &Fields<'_>) -> Result<ActionParameter<String>, PackageError> {
    // Rechazar metatablas antes de interpretar ningún campo del parámetro.
    fields.check(&[
        "key",
        "label",
        "hint",
        "kind",
        "default",
        "min",
        "max",
        "step",
        "whole",
        "min_exclusive",
        "unit",
    ])?;
    let key = fields.text("key", 64, false)?;
    if !identifier(&key) {
        return Err(fields.error("key", "Usa [a-z][a-z0-9_]*"));
    }
    let label = fields.text("label", 128, false)?;
    let hint = fields.text("hint", 256, true)?;
    let (kind, default) = match fields.text("kind", 16, false)?.as_str() {
        "number" => number(fields)?,
        kind @ ("text" | "bool") => {
            fields.check(&["key", "label", "hint", "kind", "default"])?;
            if kind == "text" {
                (ParameterKind::Text, fields.text("default", 1024, false)?)
            } else {
                (
                    ParameterKind::Bool,
                    fields.boolean("default", false)?.to_string(),
                )
            }
        }
        _ => return Err(fields.error("kind", "Tipo desconocido; usa number, text o bool")),
    };
    let parameter = ActionParameter {
        key,
        label,
        hint,
        kind,
        default,
        empty_value: None,
        argument: Argument::Positional,
    };
    // Límites y tipos de valores comparten el validador de las acciones nativas.
    parameter
        .parse(&parameter.default)
        .map_err(|e| fields.error("default", e))?;
    Ok(parameter)
}

fn number(fields: &Fields<'_>) -> Result<(ParameterKind<String>, String), PackageError> {
    let min = fields.number("min")?;
    let max = fields.number("max")?;
    let step = fields.number("step")?;
    let default = fields.number("default")?;
    let whole = fields.boolean("whole", true)?;
    let min_exclusive = fields.boolean("min_exclusive", true)?;
    let unit = fields.text("unit", 32, true)?;
    if min >= max {
        return Err(fields.error("max", "Debe ser mayor que min"));
    }
    if step <= 0.0 {
        return Err(fields.error("step", "Debe ser mayor que cero"));
    }
    if whole {
        for (key, value) in [
            ("min", min),
            ("max", max),
            ("step", step),
            ("default", default),
        ] {
            if value.fract() != 0.0 {
                return Err(fields.error(key, "Debe ser entero cuando whole es true"));
            }
        }
    }
    Ok((
        ParameterKind::Number {
            min,
            max,
            step,
            whole,
            min_exclusive,
            unit,
        },
        default.to_string(),
    ))
}
