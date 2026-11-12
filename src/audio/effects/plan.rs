use super::ComponentEffect;
use super::Effect;
use crate::audio::analyzer::tap::Tap;
use elevate::musical::{
    fx::{FxChain, FxId, FxKind, FxNode},
    track::TrackDevice,
};
use std::{collections::BTreeMap, sync::Arc};
pub(crate) enum Plan {
    Gain(f32),
    Effect(Arc<Effect>),
    Component(Arc<ComponentEffect>),
    Analyzer(Arc<Tap>),
    Observe(Box<Plan>),
    Serial(Vec<Plan>),
    Parallel(Vec<Plan>),
}
impl Plan {
    pub fn process(&self, frames: &mut [[f32; 2]], rate: u32) {
        match self {
            Self::Gain(gain) => {
                for f in frames {
                    f[0] *= gain;
                    f[1] *= gain;
                }
            }
            Self::Effect(effect) => effect.process(frames),
            Self::Component(effect) => effect.process(frames),
            Self::Analyzer(tap) => tap.capture(frames, rate),
            // Las cadenas Utility conservan su salida escalar y su rampa existentes.
            // Evaluar sus puntos de análisis sobre una copia evita aplicar la ganancia
            // dos veces, incluso en el bloque donde la UI inserta/retira un Spectrum.
            Self::Observe(plan) => {
                for block in frames.chunks(512) {
                    let mut copy = [[0.; 2]; 512];
                    copy[..block.len()].copy_from_slice(block);
                    plan.process(&mut copy[..block.len()], rate);
                }
            }
            Self::Serial(nodes) => {
                for node in nodes {
                    node.process(frames, rate);
                }
            }
            Self::Parallel(nodes) if nodes.is_empty() => {}
            Self::Parallel(nodes) => {
                for block in frames.chunks_mut(512) {
                    let mut input = [[0.; 2]; 512];
                    input[..block.len()].copy_from_slice(block);
                    block.fill([0.; 2]);
                    for node in nodes {
                        let mut branch = input;
                        node.process(&mut branch[..block.len()], rate);
                        for (out, value) in block.iter_mut().zip(branch) {
                            out[0] += value[0];
                            out[1] += value[1];
                        }
                    }
                }
            }
        }
    }
}
pub(crate) fn needs_rack(chain: &FxChain) -> bool {
    fn visit(node: &FxNode) -> bool {
        matches!(
            node.kind,
            FxKind::Effect {
                plugin_id: TrackDevice::Vst3 { .. },
                ..
            }
        ) || matches!(&node.kind, FxKind::Effect { plugin_id: TrackDevice::Hay { catalog_id }, .. } if catalog_id != "hay.utility")
            || node
                .children()
                .is_some_and(|children| children.iter().any(visit))
    }
    chain.nodes().iter().any(visit)
}
/// Export aún no tiene un host VST3 offline; un tap transparente sí se puede omitir.
pub(crate) fn contains_vst(chain: &FxChain) -> bool {
    fn visit(node: &FxNode) -> bool {
        matches!(
            node.kind,
            FxKind::Effect {
                plugin_id: TrackDevice::Vst3 { .. },
                ..
            }
        ) || node
            .children()
            .is_some_and(|children| children.iter().any(visit))
    }
    chain.nodes().iter().any(visit)
}
pub(crate) fn contains_component(
    chain: &FxChain,
    components: &BTreeMap<FxId, Arc<ComponentEffect>>,
) -> bool {
    fn visit(node: &FxNode, components: &BTreeMap<FxId, Arc<ComponentEffect>>) -> bool {
        components.contains_key(&node.id)
            || node
                .children()
                .is_some_and(|children| children.iter().any(|child| visit(child, components)))
    }
    chain.nodes().iter().any(|node| visit(node, components))
}
pub(crate) fn compile(
    chain: &FxChain,
    effects: &BTreeMap<FxId, Arc<Effect>>,
    components: &BTreeMap<FxId, Arc<ComponentEffect>>,
    taps: &BTreeMap<FxId, Arc<Tap>>,
    rate: u32,
) -> Result<Plan, String> {
    fn node(
        n: &FxNode,
        effects: &BTreeMap<FxId, Arc<Effect>>,
        components: &BTreeMap<FxId, Arc<ComponentEffect>>,
        taps: &BTreeMap<FxId, Arc<Tap>>,
        rate: u32,
        depth: usize,
    ) -> Result<Plan, String> {
        if depth > 16 {
            return Err("Máximo 16 niveles de procesamiento FX".into());
        }
        if !n.enabled {
            return Ok(Plan::Gain(1.));
        }
        Ok(match &n.kind {
            FxKind::Effect {
                plugin_id: TrackDevice::Vst3 { .. },
                ..
            } => {
                let effect = effects
                    .get(&n.id)
                    .ok_or_else(|| format!("{}: vuelve a cargar este VST3 desde [+]", n.name))?;
                if effect.rate != rate {
                    return Err(
                        "La frecuencia de audio cambió; vuelve a cargar los efectos VST3".into(),
                    );
                }
                Plan::Effect(effect.clone())
            }
            FxKind::Effect {
                plugin_id: TrackDevice::Hay { catalog_id },
                parameters,
            } if catalog_id == "hay.utility" => {
                let gain = parameters.get(&0).copied().unwrap_or(1.);
                if !gain.is_finite() || !(0. ..=2.).contains(&gain) {
                    return Err("Ganancia Utility inválida".into());
                }
                Plan::Gain(gain)
            }
            FxKind::Effect {
                plugin_id: TrackDevice::Hay { catalog_id },
                ..
            } if catalog_id == elevate::devices::spectrum::ID => Plan::Analyzer(
                taps.get(&n.id)
                    .ok_or("Spectrum sin captura preparada")?
                    .clone(),
            ),
            FxKind::Effect {
                plugin_id: TrackDevice::Hay { .. },
                ..
            } if components.contains_key(&n.id) => Plan::Component(components[&n.id].clone()),
            FxKind::Effect { .. } => return Err(format!("{}: procesador no disponible", n.name)),
            FxKind::SerialChain { children } => Plan::Serial(
                children
                    .iter()
                    .map(|n| node(n, effects, components, taps, rate, depth + 1))
                    .collect::<Result<_, _>>()?,
            ),
            FxKind::ParallelChain { branches } => Plan::Parallel(
                branches
                    .iter()
                    .map(|n| node(n, effects, components, taps, rate, depth + 1))
                    .collect::<Result<_, _>>()?,
            ),
        })
    }
    let plan = Plan::Serial(
        chain
            .nodes()
            .iter()
            .map(|n| node(n, effects, components, taps, rate, 0))
            .collect::<Result<_, _>>()?,
    );
    Ok(
        if contains_vst(chain) || contains_component(chain, components) {
            plan
        } else {
            Plan::Observe(Box::new(plan))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use elevate::scripting::audio_components::parse;

    #[test]
    fn lua_component_changes_real_rack_audio_and_keeps_utility_order() {
        let descriptor = Arc::new(
            parse(include_str!(
                "../../../assets/scripts/components/audio/double_volume.lua"
            ))
            .unwrap(),
        );
        let effect = ComponentEffect::prepare(descriptor.clone(), 48_000).unwrap();
        let mut chain = FxChain::new();
        let utility = FxNode::new(
            "Utility",
            FxKind::Effect {
                plugin_id: TrackDevice::Hay {
                    catalog_id: "hay.utility".into(),
                },
                parameters: [(0, 0.5)].into(),
            },
        );
        chain
            .apply(elevate::musical::fx::FxEdit::Insert(
                utility,
                elevate::musical::fx::FxPlacement::Inside(None),
            ))
            .unwrap();
        let node = FxNode::new(
            descriptor.name.clone(),
            FxKind::Effect {
                plugin_id: TrackDevice::Hay {
                    catalog_id: descriptor.id.clone(),
                },
                parameters: [(0, 1.0)].into(),
            },
        );
        let id = node.id;
        chain
            .apply(elevate::musical::fx::FxEdit::Insert(
                node,
                elevate::musical::fx::FxPlacement::Inside(None),
            ))
            .unwrap();
        let components = BTreeMap::from([(id, effect.clone())]);
        let plan = compile(&chain, &BTreeMap::new(), &components, &BTreeMap::new(), 0).unwrap();
        effect.set(0, 0.5).unwrap();
        let mut frames = vec![[0.8, -0.4]; 4096];
        plan.process(&mut frames, 48_000);
        assert!((frames.last().unwrap()[0] - 0.1).abs() < 0.005);
        assert!((frames.last().unwrap()[1] + 0.05).abs() < 0.005);
    }
}
