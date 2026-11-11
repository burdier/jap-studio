use super::{
    PackageDescriptor, PackageError,
    fields::{Fields, Location, identifier},
    parameters,
};
use crate::scripting::sandbox;
use mlua::{Function, Lua, Value, chunk::ChunkMode};
use std::path::Path;

pub(super) fn read(
    source: &str,
    path: &Path,
    bundled: bool,
) -> Result<PackageDescriptor, PackageError> {
    let lua = sandbox::new().map_err(|e| PackageError::load(path, e))?;
    evaluate(&lua, source, path, bundled).map(|(descriptor, _)| descriptor)
}

/// Misma evaluación y lectura al registrar y ejecutar; el llamador conserva la VM
/// para que evaluación y run compartan un único presupuesto.
pub(super) fn evaluate(
    lua: &Lua,
    source: &str,
    path: &Path,
    bundled: bool,
) -> Result<(PackageDescriptor, Function), PackageError> {
    let value: Value = lua
        .load(source)
        .set_name(path.to_string_lossy())
        .set_mode(ChunkMode::Text)
        .eval()
        .map_err(|e| PackageError::load(path, e))?;
    let mut location = Location { path, id: None };
    let Value::Table(table) = value else {
        return Err(location.error("descriptor", "El paquete debe devolver una tabla"));
    };
    if let Ok(Value::String(id)) = table.raw_get::<Value>("id") {
        location.id = id.to_str().ok().map(|s| s.to_string());
    }
    let fields = Fields {
        table,
        location: &location,
        prefix: String::new(),
    };
    fields.check(&[
        "api_version",
        "id",
        "name",
        "description",
        "parameters",
        "run",
    ])?;
    let version = fields.number("api_version")?;
    if version != 1.0 {
        return Err(fields.error("api_version", "Solo se admite la API 1"));
    }
    let id = fields.text("id", 64, false)?;
    if id.len() < 3 || !id.contains('.') || !id.split('.').all(identifier) {
        return Err(fields.error(
            "id",
            "Usa al menos dos segmentos [a-z][a-z0-9_]* separados por puntos",
        ));
    }
    if !bundled && id.starts_with("hay.") {
        return Err(fields.error(
            "id",
            "El prefijo hay.* está reservado a paquetes distribuidos",
        ));
    }
    let name = fields.text("name", 128, false)?;
    let description = fields.text("description", 512, false)?;
    let Value::Function(run) = fields.get("run")? else {
        return Err(fields.error("run", "Se esperaba una función Lua"));
    };
    if run.info().what != "Lua" {
        return Err(fields.error("run", "Se esperaba una función Lua"));
    }
    let parameters = parameters::read(&fields)?;
    Ok((
        PackageDescriptor {
            api_version: 1,
            id,
            name,
            description,
            parameters,
        },
        run,
    ))
}
