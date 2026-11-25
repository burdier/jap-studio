//! Solicita los estados al pulsar Exportar y reconstruye la cadena en el worker.
use crate::audio::{
    TrackInput,
    analyzer::tap::{Tap, TapPoint},
    effects::{self, ComponentEffect, Effect, Rack},
    piano::VoiceSource,
    vst3::{PendingState, SharedInstrument, VstEngine},
};
use elevate::{
    musical::{
        fx::{FxChain, FxId, FxKind, FxNode},
        track::{Track, TrackDevice, TrackId},
    },
    scripting::audio_components::AudioComponent,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

struct TrackPlugins {
    id: TrackId,
    synth: Option<PendingState>,
    chain: Option<FxChain>,
    effects: Vec<(FxId, Arc<Effect>, PendingState)>,
    components: Vec<(FxId, Arc<AudioComponent>, BTreeMap<u32, f32>)>,
    analyzers: Vec<FxId>,
}
pub(crate) struct Plugins {
    tracks: Vec<TrackPlugins>,
    master: Option<TrackPlugins>,
}
impl Plugins {
    pub fn capture(
        inputs: &[TrackInput],
        tracks: &[Track],
        master_chain: Option<&FxChain>,
        effects: &BTreeMap<FxId, Arc<Effect>>,
        components: &BTreeMap<FxId, Arc<ComponentEffect>>,
    ) -> Result<Self, String> {
        let mut plugins = Vec::new();
        for input in inputs {
            let Some(track) = tracks.iter().find(|t| t.id == input.id) else {
                return Err("Pista de exportación inexistente".into());
            };
            let synth = match input.midi.as_ref().map(|m| &m.source) {
                Some(VoiceSource::Vst(shared)) => Some(shared.editor.capture()?),
                _ => None,
            };
            let mut copy = TrackPlugins {
                id: track.id,
                synth,
                chain: None,
                effects: Vec::new(),
                components: Vec::new(),
                analyzers: Vec::new(),
            };
            if effects::needs_rack(&track.chain) {
                for node in track.chain.nodes() {
                    copy.capture_node(node, effects, components)?;
                }
                copy.chain = Some(track.chain.clone());
            }
            if copy.synth.is_some() || copy.chain.is_some() {
                plugins.push(copy);
            }
        }
        let master = if let Some(chain) = master_chain.filter(|c| effects::needs_rack(c)) {
            let mut copy = TrackPlugins {
                id: elevate::musical::track::MASTER_TRACK_ID,
                synth: None,
                chain: Some(chain.clone()),
                effects: Vec::new(),
                components: Vec::new(),
                analyzers: Vec::new(),
            };
            for node in chain.nodes() {
                copy.capture_node(node, effects, components)?;
            }
            Some(copy)
        } else {
            None
        };
        Ok(Self {
            tracks: plugins,
            master,
        })
    }
    pub fn prepare(
        self,
        inputs: &mut [TrackInput],
        master_rack: &mut Option<Arc<Rack>>,
        rate: u32,
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        for track in self.tracks {
            let input = inputs
                .iter_mut()
                .find(|i| i.id == track.id)
                .ok_or("Pista de exportación inexistente")?;
            if let Some(state) = track.synth {
                let vst = SharedInstrument::new(restore(state, rate, cancel)?, rate);
                vst.enter_arrangement()?;
                input.midi.as_mut().ok_or("Falta la fuente MIDI")?.source = VoiceSource::Vst(vst);
            }
            if let Some(chain) = track.chain {
                let mut effects = BTreeMap::new();
                for (id, _live, state) in track.effects {
                    effects.insert(id, Effect::from_engine(restore(state, rate, cancel)?, rate));
                }
                let mut components = BTreeMap::new();
                for (id, descriptor, params) in track.components {
                    let instance = ComponentEffect::prepare(descriptor, rate)?;
                    for (param_id, value) in params {
                        let _ = instance.set(param_id, value);
                    }
                    components.insert(id, instance);
                }
                let taps = track
                    .analyzers
                    .into_iter()
                    .map(|id| (TapPoint::Output(id), Tap::new().0))
                    .collect();
                let plan = effects::compile(&chain, &effects, &components, &taps, rate)?;
                input.rack = Some(Rack::new(plan, rate));
            }
        }
        if let Some(master) = self.master
            && let Some(chain) = master.chain
        {
            let mut effects = BTreeMap::new();
            for (id, _live, state) in master.effects {
                effects.insert(id, Effect::from_engine(restore(state, rate, cancel)?, rate));
            }
            let mut components = BTreeMap::new();
            for (id, descriptor, params) in master.components {
                let instance = ComponentEffect::prepare(descriptor, rate)?;
                for (param_id, value) in params {
                    let _ = instance.set(param_id, value);
                }
                components.insert(id, instance);
            }
            let taps = master
                .analyzers
                .into_iter()
                .map(|id| (TapPoint::Output(id), Tap::new().0))
                .collect();
            let plan = effects::compile(&chain, &effects, &components, &taps, rate)?;
            *master_rack = Some(Rack::new(plan, rate));
        }
        Ok(())
    }
}
impl TrackPlugins {
    fn capture_node(
        &mut self,
        node: &FxNode,
        effects: &BTreeMap<FxId, Arc<Effect>>,
        components: &BTreeMap<FxId, Arc<ComponentEffect>>,
    ) -> Result<(), String> {
        if !node.enabled {
            return Ok(());
        }
        match &node.kind {
            FxKind::Effect {
                plugin_id: TrackDevice::Vst3 { .. },
                ..
            } => {
                let effect = effects
                    .get(&node.id)
                    .ok_or_else(|| format!("{}: falta la instancia VST3", node.name))?;
                self.effects
                    .push((node.id, effect.clone(), effect.editor.capture()?));
            }
            FxKind::Effect {
                plugin_id: TrackDevice::Hay { catalog_id },
                parameters,
            } => {
                if catalog_id == elevate::devices::spectrum::ID {
                    self.analyzers.push(node.id);
                } else if let Some(comp) = components.get(&node.id) {
                    self.components
                        .push((node.id, comp.descriptor().clone(), parameters.clone()));
                }
            }
            _ => {
                if let Some(children) = node.children() {
                    for child in children {
                        self.capture_node(child, effects, components)?;
                    }
                }
            }
        }
        Ok(())
    }
}
fn restore(reply: PendingState, rate: u32, cancel: &AtomicBool) -> Result<VstEngine, String> {
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Exportación cancelada".into());
        }
        match reply.recv_timeout(Duration::from_millis(100)) {
            Ok(state) => {
                let mut engine = state?.instantiate(rate)?;
                engine.fix_export_latency();
                return Ok(engine);
            }
            Err(mpsc::RecvTimeoutError::Timeout) if started.elapsed() < Duration::from_secs(30) => {
                continue;
            }
            Err(e) => return Err(format!("VST3: no se pudo obtener el estado ({e})")),
        }
    }
}
