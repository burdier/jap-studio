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
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

pub(in crate::ui) struct TrackTuner {
    pub track: TrackId,
    effect: Arc<Effect>,
    hz: Arc<AtomicU64>,
}
impl TrackTuner {
    pub(in crate::ui) fn reading(&self) -> String {
        let hz = f64::from_bits(self.hz.load(Ordering::Relaxed));
        let pitch = 69. + 12. * (hz / 440.).log2();
        let key = pitch.round();
        if !key.is_finite() || !(0.0..=127.0).contains(&key) {
            return "Sin tono".into();
        }
        let cents = ((pitch - key) * 100.).round() as i32;
        format!(
            "{}   {hz:.2} Hz   {cents:+} ¢",
            elevate::musical::pitch::note_name(key as u8)
        )
    }
}
#[derive(Default)]
pub(in crate::ui) struct VstEffects {
    pub instances: BTreeMap<FxId, Arc<Effect>>,
    racks: HashMap<TrackId, (u64, Arc<Rack>)>,
    pub(in crate::ui) tuner: Option<TrackTuner>,
    tuner_task: Option<gpui::Task<()>>,
    band_monitors: HashMap<TrackId, (Option<FxId>, Option<(FxId, u16)>)>,
    pub task: Option<gpui::Task<()>>,
}
impl DawApp {
    pub(in crate::ui) fn invalidate_changed_fx(
        &mut self,
        tracks: &[elevate::musical::track::Track],
    ) {
        for track in tracks {
            if self
                .tracks
                .iter()
                .find(|t| t.id == track.id)
                .is_some_and(|t| t.chain.nodes() != track.chain.nodes())
            {
                if let Some((rev, _)) = self.vst_effects.racks.get_mut(&track.id) {
                    *rev = u64::MAX;
                }
            }
        }
    }
    pub(in crate::ui) fn effect_rack(&self, index: usize) -> Option<Arc<Rack>> {
        self.tracks
            .get(index)
            .and_then(|t| self.vst_effects.racks.get(&t.id))
            .map(|(_, rack)| rack.clone())
    }
    pub(in crate::ui) fn master_effect_rack(&self) -> Option<Arc<Rack>> {
        self.vst_effects
            .racks
            .get(&elevate::musical::track::MASTER_TRACK_ID)
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
        if self.fx_chain(target.track).is_none() {
            self.status_message = "La pista de destino ya no existe".into();
            return;
        }
        let config = self.settings.audio.saved.clone();
        let expected = config.clone();
        self.status_message = format!("Cargando efecto {}…", plugin.name);
        let work = cx.background_spawn(async move {
            Effect::prepare(&plugin.bundle, &plugin.class_id, &config).map(|effect| {
                // Un efecto que no soporta captura aún puede usarse localmente.
                let _ = effect.editor.watch_state(None);
                (effect, plugin.class_id)
            })
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
                    let chain = this.fx_chain(target.track).ok_or("La pista ya no existe")?;
                    if target.node.is_some_and(|id| chain.find(id).is_none()) {
                        return Err("Destino FX eliminado".into());
                    }
                    let placement = chain.insertion_after(target.node);
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
                    let dest_name = if target.track == elevate::musical::track::MASTER_TRACK_ID {
                        "Master FX"
                    } else {
                        "Track FX"
                    };
                    this.status_message = format!(
                        "{} añadido a {dest_name} · doble clic: editor original",
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
    pub(in crate::ui) fn tuner_enabled(&self) -> bool {
        self.vst_effects.tuner.is_some() || self.vst_effects.tuner_task.is_some()
    }
    pub(in crate::ui) fn toggle_track_tuner(&mut self, cx: &mut Context<Self>) {
        const CLASS: &str = "2046504473616C636E75545300000000";
        if self.tuner_enabled() {
            self.set_track_tuner(None);
            self.status_message = "Afinador cerrado".into();
            cx.notify();
            return;
        }
        let Some(track) = self
            .tracks
            .get(self.cursor_row)
            .filter(|t| t.accepts_clips())
        else {
            self.status_message = "Selecciona una pista de audio o MIDI".into();
            return;
        };
        let target = track.id;
        let config = self.settings.audio.saved.clone();
        let expected = config.clone();
        self.status_message = format!("Abriendo StompTuner · {}", track.name);
        let work = cx.background_spawn(async move {
            let plugin = elevate::catalog::plugins::discover_instruments()
                .into_iter()
                .find(|p| !p.bridged && p.name.eq_ignore_ascii_case("StompTuner"))
                .ok_or("Instala StompTuner.vst3 en una carpeta VST3 de Linux")?;
            let effect = Effect::prepare(&plugin.bundle, CLASS, &config)?;
            let hz = effect.editor.watch_parameter("Frequency")?;
            Ok::<_, String>(TrackTuner {
                track: target,
                effect,
                hz,
            })
        });
        self.vst_effects.tuner_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let observed = this.update(cx, |this, cx| {
                match result {
                    Ok(tuner)
                        if this.settings.audio.saved == expected
                            && this.tracks.iter().any(|t| t.id == target) =>
                    {
                        this.set_track_tuner(Some(tuner));
                        this.status_message = "Afinador activo".into();
                    }
                    Ok(_) => this.status_message = "Cambió el destino del afinador".into(),
                    Err(e) => this.status_message = format!("StompTuner · {e}"),
                }
                cx.notify();
                let observed = this.vst_effects.tuner.as_ref().map(|t| t.hz.clone());
                if observed.is_none() {
                    this.vst_effects.tuner_task = None;
                }
                observed
            });
            let Ok(Some(hz)) = observed else {
                return;
            };
            let mut last = 0;
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(75))
                    .await;
                let next = hz.load(Ordering::Relaxed);
                if next != last && this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
                last = next;
            }
        }));
    }

    fn set_track_tuner(&mut self, next: Option<TrackTuner>) {
        if next.is_none() {
            self.vst_effects.tuner_task = None;
        }
        let previous = std::mem::replace(&mut self.vst_effects.tuner, next);
        for tuner in previous.iter().chain(&self.vst_effects.tuner) {
            if let Some((rev, _)) = self.vst_effects.racks.get_mut(&tuner.track) {
                *rev = u64::MAX;
            }
        }
    }

    pub(in crate::ui) fn sync_vst_effects(&mut self) {
        // Un cambio de lookahead pertenece al DSP local. Conserva el transporte y
        // recompila solo la ruta que contiene el procesador.
        for (id, effect) in &self.vst_effects.instances {
            if effect.editor.take_latency_change() {
                if self.master_chain.find(*id).is_some()
                    && let Some((revision, _)) = self
                        .vst_effects
                        .racks
                        .get_mut(&elevate::musical::track::MASTER_TRACK_ID)
                {
                    *revision = u64::MAX;
                } else if let Some(track) = self.tracks.iter().find(|t| t.chain.find(*id).is_some())
                    && let Some((revision, _)) = self.vst_effects.racks.get_mut(&track.id)
                {
                    *revision = u64::MAX;
                }
            }
        }
        if self
            .vst_effects
            .tuner
            .as_ref()
            .is_some_and(|tuner| !self.tracks.iter().any(|t| t.id == tuner.track))
        {
            self.set_track_tuner(None);
        }
        for track in &self.tracks {
            let tuner = self
                .vst_effects
                .tuner
                .as_ref()
                .filter(|tuner| tuner.track == track.id);
            if tuner.is_none() && !effects::needs_rack(&track.chain) {
                if let Some(p) = self.track_presentation.get_mut(&track.id) {
                    p.band_solo = None;
                }
                self.vst_effects.racks.remove(&track.id);
                continue;
            }
            if let Some(p) = self.track_presentation.get_mut(&track.id)
                && p.band_solo.is_some()
                && self
                    .vst_effects
                    .racks
                    .get(&track.id)
                    .is_none_or(|(rev, _)| *rev != track.chain.revision())
                && !track
                    .chain
                    .bands()
                    .iter()
                    .any(|b| Some(b.chain) == p.band_solo)
            {
                p.band_solo = None;
            }
            let solo = self
                .track_presentation
                .get(&track.id)
                .and_then(|p| p.band_solo);
            let draft = self.tejer_preview(track);
            let monitor = (solo, draft);
            let changed = self
                .vst_effects
                .band_monitors
                .get(&track.id)
                .copied()
                .unwrap_or_default()
                != monitor;
            if changed {
                if monitor == (None, None) {
                    self.vst_effects.band_monitors.remove(&track.id);
                } else {
                    self.vst_effects.band_monitors.insert(track.id, monitor);
                }
            }
            if let Some((rev, rack)) = self.vst_effects.racks.get(&track.id)
                && *rev == track.chain.revision()
            {
                if changed {
                    rack.set_band_monitor(&track.chain, solo, draft);
                }
                continue;
            }
            let rate = self
                .vst_effects
                .instances
                .iter()
                .find(|(id, _)| track.chain.find(**id).is_some())
                .map_or(0, |(_, e)| e.rate);
            self.fx
                .analyzers
                .prepare(&track.chain, &self.lua_components.instances);
            self.fx.analyzers.changed(&track.chain);
            match effects::compile(
                &track.chain,
                &self.vst_effects.instances,
                &self.lua_components.instances,
                &self.fx.analyzers.taps,
                rate,
            ) {
                Ok(plan) => {
                    let plan = if let Some(tuner) = tuner {
                        effects::Plan::Serial(vec![
                            plan,
                            effects::Plan::Observe(Box::new(effects::Plan::effect(
                                tuner.effect.clone(),
                            ))),
                        ])
                    } else {
                        plan
                    };
                    if solo.is_some() || draft.is_some() {
                        plan.set_band_monitor(&track.chain, solo, draft);
                    }
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
        // Compilar y publicar también la cadena FX del Bus Master.
        let master_id = elevate::musical::track::MASTER_TRACK_ID;
        if !effects::needs_rack(&self.master_chain) {
            self.vst_effects.racks.remove(&master_id);
        } else if self
            .vst_effects
            .racks
            .get(&master_id)
            .is_none_or(|(rev, _)| *rev != self.master_chain.revision())
        {
            let rate = self
                .vst_effects
                .instances
                .iter()
                .find(|(id, _)| self.master_chain.find(**id).is_some())
                .map_or(0, |(_, e)| e.rate);
            self.fx
                .analyzers
                .prepare(&self.master_chain, &self.lua_components.instances);
            self.fx.analyzers.changed(&self.master_chain);
            match effects::compile(
                &self.master_chain,
                &self.vst_effects.instances,
                &self.lua_components.instances,
                &self.fx.analyzers.taps,
                rate,
            ) {
                Ok(plan) => {
                    if let Some((rev, rack)) = self
                        .vst_effects
                        .racks
                        .get_mut(&master_id)
                        .filter(|(_, rack)| rack.rate == rate)
                    {
                        rack.publish(plan);
                        *rev = self.master_chain.revision();
                    } else {
                        self.vst_effects.racks.insert(
                            master_id,
                            (self.master_chain.revision(), Rack::new(plan, rate)),
                        );
                    }
                }
                Err(error) => {
                    self.status_message = format!("Master FX: {error}");
                    if let Some((rev, rack)) = self.vst_effects.racks.get_mut(&master_id) {
                        rack.publish(effects::Plan::Gain(1.));
                        *rev = self.master_chain.revision();
                    } else {
                        self.vst_effects.racks.insert(
                            master_id,
                            (
                                self.master_chain.revision(),
                                Rack::new(effects::Plan::Gain(1.), rate),
                            ),
                        );
                    }
                }
            }
        }
        self.vst_effects
            .racks
            .retain(|id, _| *id == master_id || self.tracks.iter().any(|t| t.id == *id));
        self.vst_effects
            .band_monitors
            .retain(|id, _| self.vst_effects.racks.contains_key(id));
        if let Some(player) = &self.player {
            let mut changed = false;
            for (index, track) in self.tracks.iter().enumerate() {
                changed |= player.set_fx_rack(track.id, self.effect_rack(index));
            }
            player.set_master_fx_rack(self.master_effect_rack());
            if changed && let Err(error) = player.refresh_track_delays() {
                self.status_message = error;
            }
        }
        let piano_rack = self.piano_output_track().and_then(|i| self.effect_rack(i));
        let sampler_rack = self.sampler_track().and_then(|i| self.effect_rack(i));
        self.instrument.set_fx_rack(piano_rack);
        let piano_racks: Vec<_> = self
            .piano_instances
            .keys()
            .filter_map(|id| {
                self.tracks
                    .iter()
                    .position(|track| track.instrument == Some(*id))
                    .map(|index| (*id, self.effect_rack(index)))
            })
            .collect();
        for (id, rack) in piano_racks {
            if let Some(piano) = self.piano_instances.get_mut(&id) {
                piano.set_fx_rack(rack);
            }
        }
        self.sampler.set_fx_rack(sampler_rack);
        let vst_rack = self.vst_track().and_then(|i| self.effect_rack(i));
        self.vst.set_fx_rack(vst_rack);
        let racks: Vec<_> = self
            .tracks
            .iter()
            .enumerate()
            .filter_map(|(i, track)| track.instrument.map(|id| (id, i)))
            .filter(|(id, _)| self.vst_instances.contains_key(id))
            .map(|(id, i)| (id, self.effect_rack(i)))
            .collect();
        for (id, rack) in racks {
            if let Some(vst) = self.vst_instances.get_mut(&id) {
                vst.set_fx_rack(rack);
            }
        }
        // Las rutas publicadas mantienen las instancias vivas hasta terminar el bloque activo.
        self.vst_effects.instances.retain(|id, _| {
            self.master_chain.find(*id).is_some()
                || self.tracks.iter().any(|t| t.chain.find(*id).is_some())
        });
        self.lua_components.instances.retain(|id, _| {
            self.master_chain.find(*id).is_some()
                || self.tracks.iter().any(|t| t.chain.find(*id).is_some())
        });
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
