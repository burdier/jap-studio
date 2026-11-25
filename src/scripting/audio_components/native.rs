//! Lista explícita de primitivas admitidas en descriptores; no carga plugins.
use super::*;
use crate::{
    PrimitiveInfo,
    primitives::{
        amplitude::Volume, compressor::Compressor, echo::Echo, filter::Filter, limiter::Limiter,
        reverb::Reverb,
    },
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeKind {
    Volume,
    Filter,
    Compressor,
    Limiter,
    Echo,
    Reverb,
}
#[derive(Debug, Clone)]
pub struct AudioNode {
    pub id: String,
    pub kind: NativeKind,
    pub values: Vec<f32>,
}
impl AudioNode {
    pub fn info(&self) -> &'static PrimitiveInfo {
        match self.kind {
            NativeKind::Volume => Volume::INFO,
            NativeKind::Filter => Filter::INFO,
            NativeKind::Compressor => Compressor::INFO,
            NativeKind::Limiter => Limiter::INFO,
            NativeKind::Echo => Echo::INFO,
            NativeKind::Reverb => Reverb::INFO,
        }
    }
}
pub(super) fn parse_node(
    row: &Table,
    id: String,
    version: i64,
    path: &str,
) -> Result<AudioNode, String> {
    let kind = match string(row, "primitive")?.as_str() {
        "elevate.amplitude.volume" => NativeKind::Volume,
        "elevate.filter.svf" if version == 2 => NativeKind::Filter,
        "elevate.dynamics.compressor" if version == 2 => NativeKind::Compressor,
        "elevate.dynamics.limiter" if version == 2 => NativeKind::Limiter,
        "elevate.time.echo" if version == 2 => NativeKind::Echo,
        "elevate.space.reverb" if version == 2 => NativeKind::Reverb,
        _ => {
            return Err(format!(
                "{path}.primitive: primitiva desconocida o versión incompatible"
            ));
        }
    };
    let mut node = AudioNode {
        id,
        kind,
        values: Vec::new(),
    };
    let params = table(row, "parameters")?;
    fields(
        &params,
        &node
            .info()
            .parameters
            .iter()
            .map(|p| p.key)
            .collect::<Vec<_>>(),
        path,
    )?;
    for spec in node.info().parameters {
        let raw = if kind == NativeKind::Filter
            && spec.key == "gain"
            && matches!(params.raw_get::<Value>("gain"), Ok(Value::Nil))
        {
            f64::from(spec.default)
        } else {
            number(&params, spec.key)?
        };
        let value = raw as f32;
        if value < spec.min || value > spec.max {
            return Err(format!("{path}.{}: fuera de rango", spec.key));
        }
        spec.validate(value).map_err(|e| format!("{path}: {e}"))?;
        if kind == NativeKind::Filter {
            Filter::validate(spec.id, value).map_err(|e| format!("{path}: {e}"))?;
        }
        node.values.push(value);
    }
    Ok(node)
}
pub(super) fn choices(row: &Table, kind: &str, version: i64) -> Result<Vec<String>, String> {
    if kind == "number" && matches!(row.raw_get::<Value>("choices"), Ok(Value::Nil)) {
        return Ok(Vec::new());
    }
    if kind != "choice" || version != 2 {
        return Err("kind requiere number o choice (v2)".into());
    }
    let t = table(row, "choices")?;
    let n = t.raw_len();
    if !(2..=16).contains(&n) {
        return Err("choices requiere 2..16 etiquetas".into());
    }
    let mut result = Vec::new();
    let mut count = 0;
    for pair in t.clone().pairs::<Value, Value>() {
        let (key, _) = pair.map_err(|e| e.to_string())?;
        if !matches!(key, Value::Integer(i) if i >= 1 && i as usize <= n) {
            return Err("choices disperso".into());
        }
        count += 1;
    }
    if count != n {
        return Err("choices disperso".into());
    }
    for i in 1..=n {
        let value = match t.raw_get::<Value>(i).map_err(|e| e.to_string())? {
            Value::String(s) => s.to_str().map_err(|e| e.to_string())?.to_string(),
            _ => return Err("choices requiere etiquetas textuales".into()),
        };
        if value.trim().is_empty() || value.len() > 64 || value.contains(['\0', '\r', '\n']) {
            return Err("Etiqueta choice inválida".into());
        }
        result.push(value);
    }
    Ok(result)
}
