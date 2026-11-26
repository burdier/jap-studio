//! Los bundles se resuelven en el catálogo local. Loro sólo transporta clase y estado.
use crate::audio::{
    device::AudioConfig,
    effects::{ComponentEffect, Effect},
    vst3::scan,
};
use elevate::{
    musical::{
        fx::{FxId, FxKind, FxNode},
        track::{Track, TrackDevice},
    },
    scripting::audio_components::AudioComponent,
};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

pub(super) type PreparedEffects = BTreeMap<FxId, Arc<Effect>>;

pub(super) fn unique_ids(nodes: &[FxNode], ids: &mut HashSet<FxId>) -> bool {
    nodes
        .iter()
        .all(|n| ids.insert(n.id) && n.children().is_none_or(|c| unique_ids(c, ids)))
}

pub(super) fn validate(nodes: &[FxNode]) -> Result<(), String> {
    for node in nodes {
        match &node.kind {
            FxKind::Effect {
                plugin_id: TrackDevice::Vst3 { class_id },
                ..
            } => {
                if class_id.len() != 32
                    || !class_id.bytes().all(|b| b.is_ascii_hexdigit())
                    || node.vst_preset.is_none()
                {
                    return Err(format!(
                        "{}: falta el estado recuperable del efecto VST3",
                        node.name
                    ));
                }
            }
            FxKind::Effect {
                plugin_id: TrackDevice::Hay { catalog_id },
                parameters,
            } => {
                if catalog_id == "hay.utility" {
                    if parameters.get(&0).is_some_and(|v| !(0.0..=2.).contains(v)) {
                        return Err("Nivel Utility inválido".into());
                    }
                } else if catalog_id == elevate::musical::fx::cabinet::ID {
                    elevate::musical::fx::cabinet::mic_index(
                        parameters.get(&elevate::musical::fx::cabinet::MIC_PARAMETER),
                    )?;
                    if ![0., 1., 2., 3.].contains(parameters.get(&0).unwrap_or(&0.)) {
                        return Err("Gabinete inválido".into());
                    }
                } else if catalog_id != elevate::devices::spectrum::ID {
                    if catalog_id.is_empty() || parameters.values().any(|v| !v.is_finite()) {
                        return Err(format!("{}: parámetros de efecto inválidos", node.name));
                    }
                }
            }
            _ => {
                if let Some(children) = node.children() {
                    validate(children)?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn prepare(
    tracks: &[Track],
    previous: &[Track],
    master_chain: &elevate::musical::fx::FxChain,
    previous_master: Option<&elevate::musical::fx::FxChain>,
    config: &AudioConfig,
) -> Result<PreparedEffects, String> {
    let (ready, _) =
        prepare_inner(tracks, previous, master_chain, previous_master, config, false)?;
    Ok(ready)
}

/// Prepara efectos VST3 en modo tolerante (para P2P): los plugins ausentes en esta
/// máquina quedan en bypass local y devuelven un aviso sin interrumpir la sesión.
pub(super) fn prepare_tolerant(
    tracks: &[Track],
    previous: &[Track],
    master_chain: &elevate::musical::fx::FxChain,
    previous_master: Option<&elevate::musical::fx::FxChain>,
    config: &AudioConfig,
) -> Result<(PreparedEffects, Vec<String>), String> {
    prepare_inner(tracks, previous, master_chain, previous_master, config, true)
}

fn prepare_inner(
    tracks: &[Track],
    previous: &[Track],
    master_chain: &elevate::musical::fx::FxChain,
    previous_master: Option<&elevate::musical::fx::FxChain>,
    config: &AudioConfig,
    tolerant: bool,
) -> Result<(PreparedEffects, Vec<String>), String> {
    fn visit<'a>(
        nodes: &'a [FxNode],
        old: Option<&elevate::musical::fx::FxChain>,
        changed: &mut Vec<(&'a FxNode, &'a str)>,
    ) {
        for node in nodes {
            if let FxKind::Effect {
                plugin_id: TrackDevice::Vst3 { class_id },
                ..
            } = &node.kind
            {
                let same = old.and_then(|c| c.find(node.id)).is_some_and(|prev| {
                    prev.kind == node.kind && prev.vst_preset == node.vst_preset
                });
                if !same {
                    changed.push((node, class_id));
                }
            }
            if let Some(children) = node.children() {
                visit(children, old, changed);
            }
        }
    }
    let mut changed = Vec::new();
    for t in tracks {
        visit(
            t.chain.nodes(),
            previous.iter().find(|old| old.id == t.id).map(|t| &t.chain),
            &mut changed,
        );
    }
    visit(master_chain.nodes(), previous_master, &mut changed);
    let mut ready = PreparedEffects::new();
    let mut warnings = Vec::new();
    if changed.is_empty() {
        return Ok((ready, warnings));
    }
    let plugins = scan::discover();
    for (node, class) in changed {
        let Some(plugin) = plugins
            .iter()
            .filter(|p| !p.instrument && p.class_id.eq_ignore_ascii_case(class))
            .min_by_key(|p| p.bridged)
        else {
            if tolerant {
                warnings.push(format!("Falta efecto VST3 {} (en bypass local)", node.name));
                continue;
            }
            return Err(format!(
                "Falta el efecto VST3 {} ({class}) en esta máquina",
                node.name
            ));
        };
        let Some(preset) = node.vst_preset.clone() else {
            if tolerant {
                warnings.push(format!("{} sin preset VST3 (en bypass local)", node.name));
                continue;
            }
            return Err("Falta el estado VST3".into());
        };
        match Effect::restore_project(
            plugin.bundle.clone(),
            plugin.class_id.clone(),
            preset,
            config,
        ) {
            Ok(effect) => {
                ready.insert(node.id, effect);
            }
            Err(e) if tolerant => {
                warnings.push(format!("VST3 {} en bypass ({e})", node.name));
            }
            Err(e) => return Err(format!("{}: {e}", node.name)),
        }
    }
    Ok((ready, warnings))
}

pub(super) fn prepare_components(
    tracks: &[Track],
    master_chain: &elevate::musical::fx::FxChain,
    registry: &BTreeMap<String, Arc<AudioComponent>>,
    rate: u32,
) -> Result<BTreeMap<FxId, Arc<ComponentEffect>>, String> {
    fn visit(
        nodes: &[FxNode],
        registry: &BTreeMap<String, Arc<AudioComponent>>,
        rate: u32,
        out: &mut BTreeMap<FxId, Arc<ComponentEffect>>,
    ) -> Result<(), String> {
        for node in nodes {
            if let FxKind::Effect {
                plugin_id: TrackDevice::Hay { catalog_id },
                parameters,
            } = &node.kind
                && let Some(descriptor) = registry.get(catalog_id)
            {
                let effect = ComponentEffect::prepare(descriptor.clone(), rate)?;
                for (&param_id, &value) in parameters {
                    let _ = effect.set(param_id, value);
                }
                out.insert(node.id, effect);
            }
            if let Some(children) = node.children() {
                visit(children, registry, rate, out)?;
            }
        }
        Ok(())
    }
    let mut ready = BTreeMap::new();
    for track in tracks {
        visit(track.chain.nodes(), registry, rate, &mut ready)?;
    }
    visit(master_chain.nodes(), registry, rate, &mut ready)?;
    Ok(ready)
}

#[cfg(feature = "p2p")]
pub(super) fn competing_presets(base: &[Track], local: &[Track], remote: &[Track]) -> bool {
    fn conflict(nodes: &[FxNode], local: &Track, remote: &Track) -> bool {
        nodes.iter().any(|n| {
            let a = local.chain.find(n.id).and_then(|p| p.vst_preset.as_ref());
            let b = remote.chain.find(n.id).and_then(|p| p.vst_preset.as_ref());
            (a != b && a != n.vst_preset.as_ref() && b != n.vst_preset.as_ref())
                || n.children().is_some_and(|c| conflict(c, local, remote))
        })
    }
    base.iter()
        .zip(local)
        .zip(remote)
        .any(|((b, l), r)| conflict(b.chain.nodes(), l, r))
}
