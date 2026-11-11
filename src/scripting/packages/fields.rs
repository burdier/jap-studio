//! Lectura cruda y estricta: ninguna conversión Lua ni resolución de __index.
use super::{PackageError, PackagePhase};
use mlua::{Table, Value};
use std::path::Path;

pub(super) struct Location<'a> {
    pub path: &'a Path,
    pub id: Option<String>,
}

impl Location<'_> {
    pub fn error(&self, field: &str, message: impl ToString) -> PackageError {
        PackageError {
            phase: PackagePhase::Descriptor,
            path: self.path.to_owned(),
            id: self.id.clone(),
            field: Some(field.to_owned()),
            message: message.to_string(),
        }
    }
}

pub(super) struct Fields<'a> {
    pub table: Table,
    pub location: &'a Location<'a>,
    pub prefix: String,
}

impl Fields<'_> {
    pub fn error(&self, key: &str, message: impl ToString) -> PackageError {
        let field = if self.prefix.is_empty() {
            key.to_owned()
        } else if key.is_empty() {
            self.prefix.clone()
        } else {
            format!("{}.{key}", self.prefix)
        };
        self.location.error(&field, message)
    }

    pub fn check(&self, allowed: &[&str]) -> Result<(), PackageError> {
        if self.table.metatable().is_some() {
            return Err(self.error("", "No se admiten metatablas"));
        }
        for pair in self.table.pairs::<Value, Value>() {
            let (key, _) = pair.map_err(|e| self.error("", e))?;
            let Value::String(key) = key else {
                return Err(self.error("", "Los campos deben tener claves de texto"));
            };
            let key = key.to_str().map_err(|e| self.error("", e))?;
            if !allowed.contains(&key.as_ref()) {
                return Err(self.error(&key, "Campo desconocido"));
            }
        }
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Value, PackageError> {
        self.table.raw_get(key).map_err(|e| self.error(key, e))
    }

    pub fn text(&self, key: &str, max: usize, optional: bool) -> Result<String, PackageError> {
        let value = self.get(key)?;
        if optional && value == Value::Nil {
            return Ok(String::new());
        }
        let Value::String(value) = value else {
            return Err(self.error(key, "Se esperaba texto UTF-8"));
        };
        let value = value.to_str().map_err(|e| self.error(key, e))?;
        if value.len() > max
            || value.contains(['\0', '\r', '\n'])
            || (!value.is_empty() && value.trim().is_empty())
            || (!optional && value.is_empty())
        {
            return Err(self.error(
                key,
                format!("Texto inválido (máximo {max} bytes, una línea)"),
            ));
        }
        Ok(value.to_string())
    }

    pub fn number(&self, key: &str) -> Result<f64, PackageError> {
        let number = match self.get(key)? {
            Value::Integer(value) => value as f64,
            Value::Number(value) => value,
            _ => return Err(self.error(key, "Se esperaba un número")),
        };
        if !number.is_finite() {
            return Err(self.error(key, "Se esperaba un número finito"));
        }
        Ok(number)
    }

    pub fn boolean(&self, key: &str, optional: bool) -> Result<bool, PackageError> {
        match self.get(key)? {
            Value::Boolean(value) => Ok(value),
            Value::Nil if optional => Ok(false),
            _ => Err(self.error(key, "Se esperaba un booleano")),
        }
    }
}

pub(super) fn identifier(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}
