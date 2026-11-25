//! Adaptador de topología FX; Loro conserva cada nodo y su lista de hijos por ID.
use crate::musical::{
    fx::{BranchMix, FxChain, FxId, FxKind, FxNode, SidechainSource},
    track::{TrackDevice, TrackId},
};
use loro::{LoroDoc, LoroMovableList, ToJson};
use std::collections::{BTreeMap, HashSet};

pub(super) fn write_chain(doc: &LoroDoc, track: TrackId, chain: &FxChain) -> Result<(), String> {
    write_children(doc, track, None, chain.nodes())
}

fn write_children(
    doc: &LoroDoc,
    track: TrackId,
    parent: Option<FxId>,
    nodes: &[FxNode],
) -> Result<(), String> {
    let order = doc.get_movable_list(order_key(track, parent));
    let keep: HashSet<_> = nodes.iter().map(|node| node.id.0).collect();
    for index in (0..order.len()).rev() {
        if !order_id(&order, index).is_some_and(|id| keep.contains(&id)) {
            order.delete(index, 1).map_err(|error| error.to_string())?;
        }
    }
    for node in nodes {
        if !(0..order.len()).any(|index| order_id(&order, index) == Some(node.id.0)) {
            let id = i64::try_from(node.id.0).map_err(|_| "ID FX fuera de rango")?;
            order
                .insert(order.len(), id)
                .map_err(|error| error.to_string())?;
        }
    }
    for (target, node) in nodes.iter().enumerate() {
        let source = (target..order.len())
            .find(|&index| order_id(&order, index) == Some(node.id.0))
            .ok_or("Orden FX incompleto")?;
        if source != target {
            order
                .mov(source, target)
                .map_err(|error| error.to_string())?;
        }
        let map = doc.get_map(node_key(track, node.id));
        map.insert("name", node.name.as_str())
            .map_err(|error| error.to_string())?;
        map.insert("enabled", node.enabled)
            .map_err(|error| error.to_string())?;
        if let Some(ir) = &node.cabinet_ir {
            map.insert("ir_name", ir.name.as_str())
                .map_err(|e| e.to_string())?;
            map.insert("ir_rate", ir.rate as i64)
                .map_err(|e| e.to_string())?;
            if map.get("ir_pcm").is_none_or(|v| {
                v.get_deep_value()
                    .as_binary()
                    .is_none_or(|b| b.as_slice() != ir.pcm.as_ref())
            }) {
                map.insert("ir_pcm", ir.pcm.to_vec())
                    .map_err(|e| e.to_string())?;
            }
        } else if map.get("ir_pcm").is_some() {
            for key in ["ir_name", "ir_rate", "ir_pcm"] {
                map.delete(key).map_err(|e| e.to_string())?;
            }
        }
        let stored = map.get("vst_state").map(|v| v.get_deep_value());
        if let Some(preset) = &node.vst_preset {
            if stored
                .as_ref()
                .and_then(|v| v.as_binary())
                .is_none_or(|v| v.as_slice() != preset.bytes())
            {
                map.insert("vst_state", preset.bytes().to_vec())
                    .map_err(|e| e.to_string())?;
            }
        } else if stored.is_some() {
            map.delete("vst_state").map_err(|e| e.to_string())?;
        }
        let db10 = i64::from(node.branch_mix.db10);
        if map
            .get("branch_db10")
            .and_then(|v| v.get_deep_value().to_json_value().as_i64())
            .unwrap_or(0)
            != db10
        {
            map.insert("branch_db10", db10).map_err(|e| e.to_string())?;
        }
        if map
            .get("branch_solo")
            .and_then(|v| v.get_deep_value().to_json_value().as_bool())
            .unwrap_or(false)
            != node.branch_mix.solo
        {
            map.insert("branch_solo", node.branch_mix.solo)
                .map_err(|e| e.to_string())?;
        }
        let sc_encoded = node.sidechain.map(|sc| match sc {
            SidechainSource::Drums => "drums".to_owned(),
            SidechainSource::Track(tid) => format!("track:{}", tid.0),
        });
        let stored_sc = map.get("sidechain").and_then(|v| {
            v.get_deep_value()
                .to_json_value()
                .as_str()
                .map(str::to_owned)
        });
        if stored_sc != sc_encoded {
            if let Some(val) = sc_encoded {
                map.insert("sidechain", val.as_str())
                    .map_err(|e| e.to_string())?;
            } else {
                map.delete("sidechain").map_err(|e| e.to_string())?;
            }
        }
        let (kind, plugin_type, plugin_id) = match &node.kind {
            FxKind::Effect {
                plugin_id: TrackDevice::Hay { catalog_id },
                ..
            } => (0, 0, catalog_id.as_str()),
            FxKind::Effect {
                plugin_id: TrackDevice::Vst3 { class_id },
                ..
            } => (0, 1, class_id.as_str()),
            FxKind::Crossover { hz, .. } => {
                let stored = map
                    .get("hz")
                    .and_then(|v| v.get_deep_value().to_json_value().as_u64());
                if stored != Some(u64::from(*hz)) {
                    map.insert("hz", i64::from(*hz))
                        .map_err(|e| e.to_string())?;
                }
                (3, -1, "")
            }
            FxKind::SerialChain { .. } => (1, -1, ""),
            FxKind::ParallelChain { .. } => (2, -1, ""),
        };
        map.insert("kind", kind)
            .map_err(|error| error.to_string())?;
        map.insert("plugin_type", plugin_type)
            .map_err(|error| error.to_string())?;
        map.insert("plugin_id", plugin_id)
            .map_err(|error| error.to_string())?;
        match &node.kind {
            FxKind::Effect { parameters, .. } => {
                let params = doc.get_map(params_key(track, node.id));
                let mut stale = Vec::new();
                params.for_each(|key, _| {
                    if key
                        .parse::<u32>()
                        .ok()
                        .is_none_or(|id| !parameters.contains_key(&id))
                    {
                        stale.push(key.to_owned());
                    }
                });
                for key in stale {
                    params.delete(&key).map_err(|error| error.to_string())?;
                }
                for (&id, &value) in parameters {
                    if !value.is_finite() {
                        return Err("Parámetro FX no finito".into());
                    }
                    params
                        .insert(&id.to_string(), f64::from(value))
                        .map_err(|error| error.to_string())?;
                }
            }
            FxKind::SerialChain { children } => {
                write_children(doc, track, Some(node.id), children)?
            }
            FxKind::ParallelChain { branches } | FxKind::Crossover { branches, .. } => {
                write_children(doc, track, Some(node.id), branches)?
            }
        }
    }
    Ok(())
}

pub(super) fn read_chain(doc: &LoroDoc, track: TrackId) -> Result<FxChain, String> {
    let mut ids = HashSet::new();
    FxChain::restore(read_children(doc, track, None, 0, &mut ids)?)
}

fn read_children(
    doc: &LoroDoc,
    track: TrackId,
    parent: Option<FxId>,
    depth: usize,
    ids: &mut HashSet<u64>,
) -> Result<Vec<FxNode>, String> {
    if depth > 32 {
        return Err("Máximo 32 niveles de FX".into());
    }
    let order = doc.get_movable_list(order_key(track, parent));
    let mut nodes = Vec::with_capacity(order.len());
    for index in 0..order.len() {
        let id = order_id(&order, index).ok_or("ID FX inválido")?;
        if id == 0 || id > i64::MAX as u64 || !ids.insert(id) || ids.len() > 256 {
            return Err("IDs FX repetidos o excesivos".into());
        }
        let map = doc.get_map(node_key(track, FxId(id)));
        let read_int = |key| {
            map.get(key)
                .and_then(|value| value.get_deep_value().to_json_value().as_i64())
                .ok_or_else(|| format!("Falta {key} en FX"))
        };
        let read_text = |key| {
            map.get(key)
                .and_then(|value| {
                    value
                        .get_deep_value()
                        .to_json_value()
                        .as_str()
                        .map(str::to_owned)
                })
                .ok_or_else(|| format!("Falta {key} en FX"))
        };
        let kind = match read_int("kind")? {
            0 => {
                let plugin_id = match read_int("plugin_type")? {
                    0 => TrackDevice::Hay {
                        catalog_id: read_text("plugin_id")?,
                    },
                    1 => TrackDevice::Vst3 {
                        class_id: read_text("plugin_id")?,
                    },
                    _ => return Err("Tipo de plugin inválido".into()),
                };
                let params = doc.get_map(params_key(track, FxId(id)));
                let mut parameters = BTreeMap::new();
                let mut error = None;
                params.for_each(|key, value| {
                    let entry = key
                        .parse::<u32>()
                        .ok()
                        .zip(value.get_deep_value().to_json_value().as_f64())
                        .filter(|(_, number)| {
                            number.is_finite()
                                && *number >= f32::MIN as f64
                                && *number <= f32::MAX as f64
                        });
                    if let Some((key, value)) = entry {
                        parameters.insert(key, value as f32);
                    } else {
                        error = Some("Parámetro FX inválido".to_owned());
                    }
                });
                if let Some(error) = error {
                    return Err(error);
                }
                FxKind::Effect {
                    plugin_id,
                    parameters,
                }
            }
            1 => FxKind::SerialChain {
                children: read_children(doc, track, Some(FxId(id)), depth + 1, ids)?,
            },
            3 => FxKind::Crossover {
                hz: u16::try_from(read_int("hz")?).map_err(|_| "Cruce fuera de rango")?,
                branches: read_children(doc, track, Some(FxId(id)), depth + 1, ids)?,
            },
            2 => FxKind::ParallelChain {
                branches: read_children(doc, track, Some(FxId(id)), depth + 1, ids)?,
            },
            _ => return Err("Tipo de nodo FX inválido".into()),
        };
        let enabled = map
            .get("enabled")
            .and_then(|value| value.get_deep_value().to_json_value().as_bool())
            .ok_or("Falta enabled en FX")?;
        nodes.push(FxNode {
            id: FxId(id),
            name: read_text("name")?,
            enabled,
            cabinet_ir: map
                .get("ir_pcm")
                .map(|v| {
                    let value = v.get_deep_value();
                    let data = value.as_binary().ok_or("PCM de IR inválido")?;
                    if data.len() > 768000 {
                        return Err("IR demasiado larga".into());
                    }
                    crate::musical::fx::cabinet::CabinetImpulse::new(
                        read_text("ir_name")?,
                        u32::try_from(read_int("ir_rate")?)
                            .map_err(|_| "Frecuencia IR inválida")?,
                        data.to_vec(),
                    )
                })
                .transpose()?,
            vst_preset: map
                .get("vst_state")
                .map(|v| {
                    let v = v.get_deep_value();
                    let bytes = v.as_binary().ok_or("Estado VST3 inválido")?;
                    if bytes.len() > crate::musical::fx::VstPreset::LIMIT {
                        return Err("Estado VST3 demasiado grande".into());
                    }
                    crate::musical::fx::VstPreset::new(bytes.to_vec())
                })
                .transpose()?,
            branch_mix: BranchMix {
                db10: map
                    .get("branch_db10")
                    .map(|v| {
                        v.get_deep_value()
                            .to_json_value()
                            .as_i64()
                            .and_then(|n| i16::try_from(n).ok())
                            .ok_or("Nivel de rama inválido")
                    })
                    .transpose()?
                    .unwrap_or(0),
                solo: map
                    .get("branch_solo")
                    .map(|v| {
                        v.get_deep_value()
                            .to_json_value()
                            .as_bool()
                            .ok_or("Solo de rama inválido")
                    })
                    .transpose()?
                    .unwrap_or(false),
            },
            sidechain: map
                .get("sidechain")
                .and_then(|v| {
                    v.get_deep_value()
                        .to_json_value()
                        .as_str()
                        .map(str::to_owned)
                })
                .and_then(|raw| {
                    if raw == "drums" {
                        Some(SidechainSource::Drums)
                    } else {
                        raw.strip_prefix("track:")
                            .and_then(|n| n.parse::<u32>().ok())
                            .map(|id| SidechainSource::Track(TrackId(id)))
                    }
                }),
            kind,
        });
    }
    Ok(nodes)
}

fn order_key(track: TrackId, parent: Option<FxId>) -> String {
    format!("fx-order:{}:{}", track.0, parent.map_or(0, |id| id.0))
}
fn node_key(track: TrackId, id: FxId) -> String {
    format!("fx-node:{}:{}", track.0, id.0)
}
fn params_key(track: TrackId, id: FxId) -> String {
    format!("fx-params:{}:{}", track.0, id.0)
}
fn order_id(order: &LoroMovableList, index: usize) -> Option<u64> {
    order
        .get(index)
        .and_then(|value| value.get_deep_value().to_json_value().as_u64())
}
