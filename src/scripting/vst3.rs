//! Una acción VST3 por script: Lua describe datos; el host valida y ejecuta por ID.
use crate::musical::{
    fx::{FxId, FxKind},
    track::{TrackDevice, TrackId, TrackSnapshot},
};
use mlua::{Table, Value, chunk::ChunkMode};
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub enum VstAction {
    InstrumentLoad {
        track: TrackId,
        path: String,
        class: Option<String>,
    },
    EffectLoad {
        track: TrackId,
        path: String,
        after: Option<FxId>,
    },
    EffectToggle {
        track: TrackId,
        fx: FxId,
    },
    EffectRemove {
        track: TrackId,
        fx: FxId,
    },
    EffectRename {
        track: TrackId,
        fx: FxId,
        name: String,
    },
    TrackLevel {
        track: TrackId,
        value: i32,
    },
    TrackPan {
        track: TrackId,
        value: i32,
    },
}

#[derive(Debug, Clone)]
pub struct VstPlan {
    pub origin: TrackSnapshot,
    pub action: VstAction,
}

pub fn build_action(source: &str, snapshot: &TrackSnapshot) -> Result<VstPlan, String> {
    if source.len() > super::sandbox::MAX_SOURCE_BYTES {
        return Err("El script Lua supera 256 KiB".into());
    }
    let lua = super::sandbox::new().map_err(|e| e.to_string())?;
    lua.globals()
        .raw_set("tracks", super::tracks::encode::snapshot(&lua, snapshot)?)
        .map_err(|e| e.to_string())?;
    lua.globals()
        .raw_set("track_revision", snapshot.revision)
        .map_err(|e| e.to_string())?;
    let row: Table = lua
        .load(source)
        .set_mode(ChunkMode::Text)
        .eval()
        .map_err(|e| format!("Lua VST3: {e}"))?;
    if row.metatable().is_some() {
        return Err("La acción VST3 no admite metatabla".into());
    }
    let op = string(&row, "op")?;
    let track = TrackId(positive(&row, "track")?);
    let entry = snapshot.get(track).ok_or("La pista indicada no existe")?;
    let action = match op.as_str() {
        "instrument.load" => {
            keys(&row, &["op", "track", "path", "class"])?;
            if !entry.track.accepts_clips() {
                return Err("El instrumento requiere una pista de audio o MIDI".into());
            }
            VstAction::InstrumentLoad {
                track,
                path: path(&row)?,
                class: optional_string(&row, "class")?,
            }
        }
        "effect.load" => {
            keys(&row, &["op", "track", "path", "after"])?;
            if !entry.track.accepts_clips() {
                return Err("El efecto requiere una pista de audio o MIDI".into());
            }
            let after = optional_fx(&row, "after")?;
            if after.is_some_and(|id| {
                entry
                    .track
                    .chain
                    .find(id)
                    .is_none_or(|node| node.children().is_some())
            }) {
                return Err("La conexión requiere un efecto existente en esa pista".into());
            }
            VstAction::EffectLoad {
                track,
                path: path(&row)?,
                after,
            }
        }
        "effect.toggle" | "effect.remove" | "effect.rename" => {
            if op == "effect.rename" {
                keys(&row, &["op", "track", "fx", "name"])?;
            } else {
                keys(&row, &["op", "track", "fx"])?;
            }
            let fx = FxId(u64::from(positive(&row, "fx")?));
            let node = entry
                .track
                .chain
                .find(fx)
                .ok_or("El FX indicado no existe")?;
            if !matches!(
                &node.kind,
                FxKind::Effect {
                    plugin_id: TrackDevice::Vst3 { .. },
                    ..
                }
            ) {
                return Err("La acción requiere un efecto VST3".into());
            }
            match op.as_str() {
                "effect.toggle" => VstAction::EffectToggle { track, fx },
                "effect.remove" => VstAction::EffectRemove { track, fx },
                _ => {
                    let name = string(&row, "name")?;
                    if name.trim().is_empty()
                        || name.len() > 128
                        || name.contains(['\n', '\r', '\0'])
                    {
                        return Err("Nombre FX inválido".into());
                    }
                    VstAction::EffectRename { track, fx, name }
                }
            }
        }
        "track.level" | "track.pan" => {
            keys(&row, &["op", "track", "value"])?;
            let value = integer(&row, "value")?;
            if op == "track.level" {
                if !(0..=100).contains(&value) {
                    return Err("Nivel fuera de rango (0–100)".into());
                }
                VstAction::TrackLevel { track, value }
            } else {
                if !(0..=100).contains(&value) {
                    return Err("Pan fuera de rango (0–100)".into());
                }
                VstAction::TrackPan { track, value }
            }
        }
        _ => return Err("Acción VST3 desconocida".into()),
    };
    Ok(VstPlan {
        origin: snapshot.clone(),
        action,
    })
}

fn raw(row: &Table, key: &str) -> Result<Value, String> {
    row.raw_get(key).map_err(|e| format!("{key}: {e}"))
}
fn string(row: &Table, key: &str) -> Result<String, String> {
    match raw(row, key)? {
        Value::String(value) => value
            .to_str()
            .map(|s| s.to_string())
            .map_err(|e| e.to_string()),
        _ => Err(format!("{key} debe ser texto")),
    }
}
fn optional_string(row: &Table, key: &str) -> Result<Option<String>, String> {
    if matches!(raw(row, key)?, Value::Nil) {
        Ok(None)
    } else {
        string(row, key).map(Some)
    }
}
fn integer(row: &Table, key: &str) -> Result<i32, String> {
    match raw(row, key)? {
        Value::Integer(value) => i32::try_from(value).map_err(|_| format!("{key} fuera de rango")),
        _ => Err(format!("{key} debe ser entero")),
    }
}
fn positive(row: &Table, key: &str) -> Result<u32, String> {
    let value = integer(row, key)?;
    u32::try_from(value)
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| format!("{key} debe ser entero positivo"))
}
fn optional_fx(row: &Table, key: &str) -> Result<Option<FxId>, String> {
    if matches!(raw(row, key)?, Value::Nil) {
        Ok(None)
    } else {
        Ok(Some(FxId(u64::from(positive(row, key)?))))
    }
}
fn path(row: &Table) -> Result<String, String> {
    let path = string(row, "path")?;
    if path.trim().is_empty() || path.contains('\0') {
        Err("Ruta VST3 inválida".into())
    } else {
        Ok(path)
    }
}
fn keys(row: &Table, allowed: &[&str]) -> Result<(), String> {
    for pair in row.clone().pairs::<Value, Value>() {
        let (key, _) = pair.map_err(|e| e.to_string())?;
        let Value::String(key) = key else {
            return Err("Clave de acción no textual".into());
        };
        if !allowed.contains(&key.to_str().map_err(|e| e.to_string())?.as_ref()) {
            return Err("Campo VST3 desconocido".into());
        }
    }
    Ok(())
}
