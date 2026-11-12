//! Adaptador de efectos externos: catálogo/acciones → mismo árbol FX y rutas de audio.
use crate::{
    audio::effects::{self, Effect, Rack},
    ui::{DawApp, fx::FxTarget},
};
use elevate::{
    catalog::plugins::InstalledInstrument,
    musical::{
        fx::{FxEdit, FxId, FxKind, FxNode},
        track::{TrackDevice, TrackId},
    },
};
use gpui::{AppContext, Context};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

#[derive(Default)]
pub(in crate::ui) struct VstEffects {
    pub instances: BTreeMap<FxId, Arc<Effect>>,
    racks: HashMap<TrackId, (u64, Arc<Rack>)>,
    pub task: Option<gpui::Task<()>>,
}
impl DawApp {
    pub(in crate::ui) fn effect_rack(&self, index: usize) -> Option<Arc<Rack>> {
        self.tracks
            .get(index)
            .and_then(|t| self.vst_effects.racks.get(&t.id))
            .map(|(_, rack)| rack.clone())
    }

    pub(in crate::ui) fn load_vst_effect_path(
        &mut self,
        path: std::path::PathBuf,
        target: FxTarget,
        cx: &mut Context<Self>,
    ) {
        if self.vst_effects.task.is_some() {
            self.status_message = "Espera a que termine de cargar el efecto".into();
            return;
        }
        let work = cx.background_spawn(async move {
            let candidate = InstalledInstrument {
                name: String::new(),
                bridged: false,
                bundle: path,
                class_id: String::new(),
                instrument: false,
            };
            crate::audio::vst3::scan::inspect(&candidate)?
                .into_iter()
                .find(|p| !p.instrument)
                .ok_or("El bundle no contiene un efecto VST3".into())
        });
        self.vst_effects.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.vst_effects.task = None;
                match result {
                    Ok(plugin) => this.load_vst_effect(plugin, target, cx),
                    Err(error) => this.status_message = error,
                }
                cx.notify();
            });
        }));
    }
    pub(in crate::ui) fn load_vst_effect(
        &mut self,
        plugin: InstalledInstrument,
        target: FxTarget,
        cx: &mut Context<Self>,
    ) {
        if self.vst_effects.task.is_some() {
            self.status_message = "Espera a que termine de cargar el efecto".into();
            return;
        }
        if self
            .tracks
            .iter()
            .find(|t| t.id == target.track)
            .is_none_or(|t| !t.accepts_clips())
        {
            self.status_message =
                "Selecciona una pista de audio o MIDI; los buses VST3 de grupo están pendientes"
                    .into();
            return;
        }
        let config = self.settings.audio.saved.clone();
        let expected = config.clone();
        self.status_message = format!("Cargando efecto {}…", plugin.name);
        let work = cx.background_spawn(async move {
            Effect::prepare(&plugin.bundle, &plugin.class_id, &config)
                .map(|effect| (effect, plugin.class_id))
        });
        self.vst_effects.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.vst_effects.task = None;
                if this.settings.audio.saved != expected {
                    this.status_message = "Salida cambiada; vuelve a cargar el efecto".into();
                    cx.notify();
                    return;
                }
                let result = result.and_then(|(effect, class_id)| {
                    let track = this
                        .tracks
                        .iter()
                        .find(|t| t.id == target.track)
                        .ok_or("La pista ya no existe")?;
                    if target.node.is_some_and(|id| track.chain.find(id).is_none()) {
                        return Err("Destino FX eliminado".into());
                    }
                    let placement = track.chain.insertion_after(target.node);
                    let node = FxNode::new(
                        effect.name.clone(),
                        FxKind::Effect {
                            plugin_id: TrackDevice::Vst3 { class_id },
                            parameters: Default::default(),
                        },
                    );
                    let id = node.id;
                    this.vst_effects.instances.insert(id, effect.clone());
                    if let Err(error) = this.edit_fx(target.track, FxEdit::Insert(node, placement))
                    {
                        this.vst_effects.instances.remove(&id);
                        return Err(error);
                    }
                    effect.editor.open();
                    this.status_message = format!(
                        "{} añadido a Track FX · doble clic: editor original",
                        effect.name
                    );
                    Ok(())
                });
                if let Err(error) = result {
                    this.status_message = format!("VST3: {error}");
                }
                cx.notify();
            });
        }));
    }
    pub(in crate::ui) fn sync_vst_effects(&mut self) {
        for track in &self.tracks {
            if !effects::needs_rack(&track.chain) {
                self.vst_effects.racks.remove(&track.id);
                continue;
            }
            if self
                .vst_effects
                .racks
                .get(&track.id)
                .is_some_and(|(rev, _)| *rev == track.chain.revision())
            {
                continue;
            }
            let rate = self
                .vst_effects
                .instances
                .iter()
                .find(|(id, _)| track.chain.find(**id).is_some())
                .map_or(0, |(_, e)| e.rate);
            self.fx.analyzers.prepare(&track.chain);
            self.fx.analyzers.changed(&track.chain);
            match effects::compile(
                &track.chain,
                &self.vst_effects.instances,
                &self.lua_components.instances,
                &self.fx.analyzers.taps,
                rate,
            ) {
                Ok(plan) => {
                    if let Some((rev, rack)) = self
                        .vst_effects
                        .racks
                        .get_mut(&track.id)
                        .filter(|(_, rack)| rack.rate == rate)
                    {
                        rack.publish(plan);
                        *rev = track.chain.revision();
                    } else {
                        self.vst_effects
                            .racks
                            .insert(track.id, (track.chain.revision(), Rack::new(plan, rate)));
                    }
                }
                Err(error) => {
                    self.status_message = error;
                    if let Some((rev, rack)) = self.vst_effects.racks.get_mut(&track.id) {
                        rack.publish(effects::Plan::Gain(0.));
                        *rev = track.chain.revision();
                    } else {
                        self.vst_effects.racks.insert(
                            track.id,
                            (
                                track.chain.revision(),
                                Rack::new(effects::Plan::Gain(0.), rate),
                            ),
                        );
                    }
                }
            }
        }
        self.vst_effects
            .racks
            .retain(|id, _| self.tracks.iter().any(|t| t.id == *id));
        if let Some(player) = &self.player {
            player.set_fx_rack(
                self.timeline
                    .clip
                    .as_ref()
                    .and_then(|c| self.effect_rack(c.track)),
            );
        }
        let piano_rack = self
            .tracks
            .iter()
            .position(|t| t.instrument == Some(self.instrument.id()))
            .and_then(|i| self.effect_rack(i));
        let sampler_rack = self.sampler_track().and_then(|i| self.effect_rack(i));
        let vst_rack = self.vst_track().and_then(|i| self.effect_rack(i));
        self.instrument.set_fx_rack(piano_rack);
        self.sampler.set_fx_rack(sampler_rack);
        self.vst.set_fx_rack(vst_rack);
        // Las rutas publicadas mantienen las instancias vivas hasta terminar el bloque activo.
        self.vst_effects
            .instances
            .retain(|id, _| self.tracks.iter().any(|t| t.chain.find(*id).is_some()));
        self.lua_components
            .instances
            .retain(|id, _| self.tracks.iter().any(|t| t.chain.find(*id).is_some()));
        for effect in self.vst_effects.instances.values() {
            if let Some(message) = effect.editor.take_message() {
                self.status_message = message;
            }
            if effect.failed.load(std::sync::atomic::Ordering::Relaxed) {
                self.status_message = format!("{}: fallo de procesamiento VST3", effect.name);
            }
        }
    }
}
