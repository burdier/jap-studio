use super::ComponentEffect;
use super::Effect;
use super::convolver::{Cabinet, StreamDsp};
use crate::audio::analyzer::tap::{Tap, TapPoint};
use crate::audio::vst3::TransportContext;
use elevate::musical::{
    fx::{FxChain, FxId, FxKind, FxNode, SidechainSource},
    track::TrackDevice,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{collections::BTreeMap, sync::Arc};
pub(crate) enum Plan {
    Gain(f32),
    Cabinet(FxId, Cabinet, u64),
    BranchGain(FxId, f32),
    Effect(FxId, Arc<Effect>, u32, Option<SidechainSource>),
    Delay(FxId, usize),
    Component(FxId, Arc<ComponentEffect>, Option<SidechainSource>),
    Analyzer(Arc<Tap>),
    Observe(Box<Plan>),
    Serial(Vec<Plan>),
    Parallel(Vec<Plan>),
    // Bits bajos: corte en Hz. Bits altos: ambas ramas (0), graves (1), agudos (2).
    Crossover(FxId, AtomicU32, Vec<Plan>),
}
impl Plan {
    pub(crate) fn effect(id: FxId, effect: Arc<Effect>) -> Self {
        Self::effect_with_sidechain(id, effect, None)
    }
    pub(crate) fn effect_with_sidechain(
        id: FxId,
        effect: Arc<Effect>,
        sidechain: Option<SidechainSource>,
    ) -> Self {
        let latency = effect.editor.latency_samples();
        Self::Effect(id, effect, latency, sidechain)
    }

    pub(crate) fn has_track_sidechain(&self) -> bool {
        match self {
            Self::Effect(_, _, _, Some(SidechainSource::Track(_)))
            | Self::Component(_, _, Some(SidechainSource::Track(_))) => true,
            Self::Observe(plan) => plan.has_track_sidechain(),
            Self::Serial(nodes) | Self::Parallel(nodes) | Self::Crossover(_, _, nodes) => {
                nodes.iter().any(Self::has_track_sidechain)
            }
            _ => false,
        }
    }

    pub(super) fn latency_samples(&self) -> u64 {
        match self {
            Self::Effect(_, _, latency, _) => u64::from(*latency),
            Self::Delay(_, samples) => *samples as u64,
            Self::Serial(nodes) => nodes.iter().map(Self::latency_samples).sum(),
            Self::Parallel(nodes) | Self::Crossover(_, _, nodes) => {
                nodes.iter().map(Self::latency_samples).max().unwrap_or(0)
            }
            _ => 0,
        }
    }

    pub(super) fn visit_delays(&self, visit: &mut impl FnMut(FxId, usize)) {
        match self {
            Self::Delay(id, length) => visit(*id, *length),
            Self::Serial(nodes) | Self::Parallel(nodes) | Self::Crossover(_, _, nodes) => {
                for node in nodes {
                    node.visit_delays(visit);
                }
            }
            Self::Observe(plan) => plan.visit_delays(visit),
            _ => {}
        }
    }

    pub(crate) fn set_band_monitor(
        &self,
        chain: &FxChain,
        solo: Option<FxId>,
        draft: Option<(FxId, u16)>,
    ) {
        let children = match self {
            Self::Crossover(id, control, branches) => {
                if let Some(FxNode {
                    kind:
                        FxKind::Crossover {
                            hz,
                            branches: model,
                        },
                    ..
                }) = chain.find(*id)
                {
                    let hz = draft.filter(|(cut, _)| cut == id).map_or(*hz, |(_, hz)| hz);
                    let side = solo
                        .and_then(|band| {
                            model.iter().position(|branch| branch.find(band).is_some())
                        })
                        .map_or(0, |i| i as u32 + 1);
                    control.store(u32::from(hz) | (side << 16), Ordering::Relaxed);
                }
                branches
            }
            Self::Parallel(children) | Self::Serial(children) => children,
            Self::Observe(plan) => return plan.set_band_monitor(chain, solo, draft),
            _ => return,
        };
        for child in children {
            child.set_band_monitor(chain, solo, draft);
        }
    }

    pub(super) fn failed(&self) -> bool {
        match self {
            Self::Effect(_, effect, _, _) => {
                effect.failed.load(std::sync::atomic::Ordering::Relaxed)
            }
            Self::Crossover(_, _, nodes) | Self::Parallel(nodes) | Self::Serial(nodes) => {
                nodes.iter().any(Self::failed)
            }
            _ => false,
        }
    }
    pub(super) fn tail_frames(&self) -> u64 {
        match self {
            Self::Cabinet(_, _, tail) => *tail,
            Self::Effect(_, effect, _, _) => effect.tail_frames,
            Self::Serial(nodes) => nodes
                .iter()
                .map(Self::tail_frames)
                .fold(0, u64::saturating_add),
            Self::Crossover(_, _, nodes) | Self::Parallel(nodes) => {
                nodes.iter().map(Self::tail_frames).max().unwrap_or(0)
            }
            _ => 0,
        }
    }
    /// Preparación y retiro exclusivamente en el publicador, nunca en process.
    pub fn prepare_rate(&self, rate: u32) {
        match self {
            Self::Component(_, effect, _) => effect.prepare_rate(rate),
            Self::Observe(plan) => plan.prepare_rate(rate),
            Self::Crossover(_, _, nodes) | Self::Serial(nodes) | Self::Parallel(nodes) => {
                for node in nodes {
                    node.prepare_rate(rate);
                }
            }
            _ => {}
        }
    }
    #[cfg(test)]
    pub fn process(&self, frames: &mut [[f32; 2]], rate: u32) {
        self.process_timed_automated(
            frames,
            rate,
            None,
            &mut super::CrossoverHistory::new(),
            &StreamDsp::prepare(self, None, rate),
            None,
            None,
        );
    }
    pub(super) fn process_timed_automated(
        &self,
        frames: &mut [[f32; 2]],
        rate: u32,
        time: Option<TransportContext>,
        history: &mut super::CrossoverHistory,
        delays: &StreamDsp,
        automation: Option<(&elevate::musical::track_automation::TrackAutomation, f64)>,
        sidechain: Option<super::SidechainInputs<'_>>,
    ) {
        match self {
            Self::Cabinet(id, _, _) => delays.convolve(*id, frames),
            Self::Delay(id, _) => delays.delays.process(*id, frames),
            Self::BranchGain(id, gain) => history.apply_branch_gain(*id, *gain, frames, rate),
            Self::Gain(gain) => {
                for f in frames {
                    f[0] *= gain;
                    f[1] *= gain;
                }
            }
            Self::Effect(fx_id, effect, _, sc_src) => {
                if rate != effect.rate {
                    frames.fill([0.; 2]);
                    return;
                }
                let mut params = [(0u32, 0.0f64); 32];
                let count = automation.map_or(0, |(auto, beat)| {
                    auto.collect_fx_params(*fx_id, beat, &mut params)
                });
                let sc_frames = sc_src.and_then(|src| sidechain.and_then(|s| s.resolve(src)));
                if time.is_some() || count > 0 || sc_frames.is_some() {
                    effect.process_timed_params_sidechain(
                        frames,
                        time,
                        &params[..count],
                        sc_frames,
                    );
                } else {
                    effect.process(frames);
                }
            }
            Self::Component(fx_id, effect, sc_src) => {
                let sc_frames = sc_src.and_then(|src| sidechain.and_then(|s| s.resolve(src)));
                effect.process_automated_sidechain(
                    frames,
                    rate,
                    automation.map(|(a, beat)| (a, *fx_id, beat)),
                    sc_frames,
                );
            }
            Self::Analyzer(tap) => tap.capture(frames, rate),
            // Las cadenas Utility conservan su salida escalar y su rampa existentes.
            // Evaluar sus puntos de análisis sobre una copia evita aplicar la ganancia
            // dos veces, incluso en el bloque donde la UI inserta/retira un Spectrum.
            Self::Observe(plan) => {
                for (index, block) in frames.chunks(512).enumerate() {
                    let mut copy = [[0.; 2]; 512];
                    copy[..block.len()].copy_from_slice(block);
                    plan.process_timed_automated(
                        &mut copy[..block.len()],
                        rate,
                        time.map(|time| time.offset(index * 512)),
                        history,
                        delays,
                        automation,
                        sidechain.map(|s| s.subslice(index * 512, block.len())),
                    );
                }
            }
            Self::Crossover(id, hz, nodes) => {
                let [low, high] = nodes.as_slice() else {
                    return;
                };
                let setting = hz.load(Ordering::Relaxed);
                history.process(
                    *id,
                    setting as u16,
                    setting >> 16,
                    low,
                    high,
                    frames,
                    rate,
                    time,
                    delays,
                    automation,
                    sidechain,
                );
            }
            Self::Serial(nodes) => {
                for node in nodes {
                    node.process_timed_automated(
                        frames, rate, time, history, delays, automation, sidechain,
                    );
                }
            }
            Self::Parallel(nodes) if nodes.is_empty() => {}
            Self::Parallel(nodes) => {
                for (index, block) in frames.chunks_mut(512).enumerate() {
                    let mut input = [[0.; 2]; 512];
                    input[..block.len()].copy_from_slice(block);
                    block.fill([0.; 2]);
                    let sc = sidechain.map(|s| s.subslice(index * 512, block.len()));
                    for node in nodes {
                        let mut branch = input;
                        node.process_timed_automated(
                            &mut branch[..block.len()],
                            rate,
                            time.map(|time| time.offset(index * 512)),
                            history,
                            delays,
                            automation,
                            sc,
                        );
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
        matches!(node.kind, FxKind::Crossover { .. })
            || matches!(
                node.kind,
                FxKind::Effect {
                    plugin_id: TrackDevice::Vst3 { .. },
                    ..
                }
            )
            || matches!(&node.kind, FxKind::Effect { plugin_id: TrackDevice::Hay { catalog_id }, .. } if catalog_id != "hay.utility")
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
// 32 MiB de memoria estéreo como máximo por ruta, asignados fuera del callback.
const MAX_DELAY_FRAMES: u64 = 4 * 1024 * 1024;

fn align_branches(branches: &[FxNode], plans: &mut [Plan]) -> Result<(), String> {
    let max = plans.iter().map(Plan::latency_samples).max().unwrap_or(0);
    if max > MAX_DELAY_FRAMES {
        return Err("Latencia VST3 demasiado grande para compensar".into());
    }
    for (branch, plan) in branches.iter().zip(plans) {
        let wait = (max - plan.latency_samples()) as usize;
        if wait != 0 {
            let fx = std::mem::replace(plan, Plan::Gain(1.));
            *plan = Plan::Serial(vec![fx, Plan::Delay(branch.id, wait)]);
        }
    }
    Ok(())
}

pub(crate) fn compile(
    chain: &FxChain,
    effects: &BTreeMap<FxId, Arc<Effect>>,
    components: &BTreeMap<FxId, Arc<ComponentEffect>>,
    taps: &BTreeMap<TapPoint, Arc<Tap>>,
    rate: u32,
) -> Result<Plan, String> {
    fn node(
        n: &FxNode,
        effects: &BTreeMap<FxId, Arc<Effect>>,
        components: &BTreeMap<FxId, Arc<ComponentEffect>>,
        taps: &BTreeMap<TapPoint, Arc<Tap>>,
        rate: u32,
        depth: usize,
    ) -> Result<Plan, String> {
        if depth > 16 {
            return Err("Máximo 16 niveles de procesamiento FX".into());
        }
        let processor = if !n.enabled {
            Plan::Gain(1.)
        } else {
            match &n.kind {
                FxKind::Effect {
                    plugin_id: TrackDevice::Vst3 { .. },
                    ..
                } => {
                    let effect = effects.get(&n.id).ok_or_else(|| {
                        format!("{}: vuelve a cargar este VST3 desde [+]", n.name)
                    })?;
                    if effect.rate != rate {
                        return Err(
                            "La frecuencia de audio cambió; vuelve a cargar los efectos VST3"
                                .into(),
                        );
                    }
                    Plan::effect_with_sidechain(n.id, effect.clone(), n.sidechain)
                }
                FxKind::Effect {
                    plugin_id: TrackDevice::Hay { catalog_id },
                    parameters,
                } if catalog_id == elevate::musical::fx::cabinet::ID => {
                    use elevate::musical::fx::cabinet;
                    let model = parameters.get(&0).copied().unwrap_or(0.) as usize;
                    if model >= cabinet::MODELS.len() {
                        return Err("Preset de convolver inválido".into());
                    }
                    let mic = cabinet::mic_index(parameters.get(&cabinet::MIC_PARAMETER))?;
                    let mix = cabinet::mix_value(
                        parameters.get(&cabinet::MIX_PARAMETER),
                        n.cabinet_ir.is_some(),
                        model,
                    );
                    let gain_db = cabinet::gain_db_value(parameters.get(&cabinet::GAIN_PARAMETER));
                    let predelay_ms =
                        cabinet::predelay_ms_value(parameters.get(&cabinet::PREDELAY_PARAMETER));
                    let decay = cabinet::decay_value(parameters.get(&cabinet::DECAY_PARAMETER));
                    let spec = Cabinet {
                        model,
                        mic,
                        mix,
                        gain_db,
                        predelay_ms,
                        decay,
                        ir: n.cabinet_ir.clone(),
                    };
                    let tail = spec.tail(if rate == 0 { 48000 } else { rate });
                    Plan::Cabinet(n.id, spec, tail)
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
                    taps.get(&TapPoint::Output(n.id))
                        .ok_or("Spectrum sin captura preparada")?
                        .clone(),
                ),
                FxKind::Effect {
                    plugin_id: TrackDevice::Hay { .. },
                    ..
                } if components.contains_key(&n.id) => {
                    Plan::Component(n.id, components[&n.id].clone(), n.sidechain)
                }
                FxKind::Effect { .. } => {
                    return Err(format!("{}: procesador no disponible", n.name));
                }
                FxKind::SerialChain { children } => Plan::Serial(
                    children
                        .iter()
                        .map(|n| node(n, effects, components, taps, rate, depth + 1))
                        .collect::<Result<_, _>>()?,
                ),
                FxKind::Crossover { branches, .. } | FxKind::ParallelChain { branches } => {
                    let mut plans = branches
                        .iter()
                        .map(|b| node(b, effects, components, taps, rate, depth + 1))
                        .collect::<Result<Vec<_>, _>>()?;
                    align_branches(branches, &mut plans)?;
                    if let FxKind::Crossover { hz, .. } = &n.kind {
                        Plan::Crossover(n.id, AtomicU32::new(u32::from(*hz)), plans)
                    } else {
                        let solo = branches.iter().any(|b| b.branch_mix.solo);
                        Plan::Parallel(
                            plans
                                .into_iter()
                                .zip(branches)
                                .map(|(fx, b)| {
                                    Plan::Serial(vec![
                                        fx,
                                        Plan::BranchGain(b.id, b.branch_mix.gain(solo)),
                                    ])
                                })
                                .collect(),
                        )
                    }
                }
            }
        };
        if matches!(processor, Plan::Analyzer(_)) {
            return Ok(processor);
        }
        let input = taps.get(&TapPoint::Input(n.id));
        let output = taps.get(&TapPoint::Output(n.id));
        if input.is_none() && output.is_none() {
            return Ok(processor);
        }
        let mut monitored = Vec::with_capacity(3);
        if let Some(tap) = input {
            monitored.push(Plan::Analyzer(tap.clone()));
        }
        monitored.push(processor);
        if let Some(tap) = output {
            monitored.push(Plan::Analyzer(tap.clone()));
        }
        Ok(Plan::Serial(monitored))
    }
    let plan = Plan::Serial(
        chain
            .nodes()
            .iter()
            .map(|n| node(n, effects, components, taps, rate, 0))
            .collect::<Result<_, _>>()?,
    );
    let mut storage = 0u64;
    plan.visit_delays(&mut |_, length| storage += length as u64);
    if storage > MAX_DELAY_FRAMES || plan.latency_samples() > MAX_DELAY_FRAMES {
        return Err("La compensación de latencia supera la capacidad del rack".into());
    }
    Ok(
        if !chain.bands().is_empty()
            || contains_vst(chain)
            || contains_component(chain, components)
            || elevate::devices::fx_gain::compile(chain).is_err()
        {
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
    fn delayed_parallel_branches_stay_aligned_across_blocks_and_republication() {
        let clean = FxNode::new("Limpia", FxKind::SerialChain { children: vec![] });
        let wet = FxNode::new("Procesada", FxKind::SerialChain { children: vec![] });
        let branches = [clean, wet];
        // Dos procesadores consecutivos con un retardo conocido en la rama procesada.
        let make_plan = || {
            let mut plans = [
                Plan::Gain(1.),
                Plan::Serial(vec![
                    Plan::Delay(FxId(9001), 257),
                    Plan::Delay(FxId(9002), 258),
                ]),
            ];
            align_branches(&branches, &mut plans).unwrap();
            Plan::Parallel(Vec::from(plans))
        };
        let rack = super::super::Rack::new(make_plan(), 48_000);
        assert_eq!(rack.latency_samples(), 515);
        let routing = super::super::Routing::new();
        routing.set(Some(rack.clone()), 48_000);
        let mut first = [[0.; 2]; 300];
        first[0] = [0.5, -0.25];
        routing.process(&mut first);
        assert_eq!(first, [[0.; 2]; 300]);

        rack.publish(make_plan());
        let other = super::super::Routing::new();
        other.set(Some(rack), 48_000);
        let mut silent = [[0.; 2]; 128];
        other.process(&mut silent);
        let mut rest = [[0.; 2]; 700];
        routing.process(&mut rest);
        assert_eq!(rest[215], [1., -0.5]);
        rest[215] = [0.; 2];
        assert_eq!(rest, [[0.; 2]; 700]);
    }

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
