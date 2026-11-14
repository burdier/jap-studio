//! Inserción y control del instrumento externo; usa catálogo, pista y MIDI comunes.
mod commands;
pub(in crate::ui) mod effects;
mod view;
use super::DawApp;
use crate::instruments::vst::{VST_ID, VstInstrument};
use elevate::musical::{
    midi::MidiMessage,
    model::InstrumentId,
    track::{TrackEdit, TrackId},
};
use gpui::{AppContext, Context};
use std::path::PathBuf;

impl DawApp {
    pub(in crate::ui) fn is_vst_id(&self, id: InstrumentId) -> bool {
        id == self.vst.id || self.vst_instances.contains_key(&id)
    }
    pub(in crate::ui) fn activate_vst(&mut self, id: InstrumentId) {
        if id == self.vst.id {
            return;
        }
        let Some(next) = self.vst_instances.remove(&id) else {
            return;
        };
        let previous = std::mem::replace(&mut self.vst, next);
        self.vst_instances.insert(previous.id, previous);
    }
    pub(in crate::ui) fn prune_vst_instances(&mut self) {
        let live: std::collections::HashSet<_> = self
            .tracks
            .iter()
            .filter_map(|track| track.instrument)
            .filter(|id| self.is_vst_id(*id))
            .collect();
        self.vst_instances.retain(|id, _| live.contains(id));
        if !live.contains(&self.vst.id) {
            if let Some(next) = live.iter().next().copied() {
                let removed = self.vst.id;
                self.activate_vst(next);
                self.vst_instances.remove(&removed);
            } else {
                self.vst = VstInstrument::default();
                self.vst_instances.clear();
            }
        }
    }
    pub(in crate::ui) fn vst_name(&self, id: InstrumentId) -> &str {
        if id == self.vst.id {
            &self.vst.name
        } else {
            self.vst_instances
                .get(&id)
                .map_or("VST3", |instance| instance.name.as_str())
        }
    }
    pub(in crate::ui) fn vst_any_ready(&self) -> bool {
        self.vst.ready() || self.vst_instances.values().any(VstInstrument::ready)
    }
    pub(in crate::ui) fn vst_any_clip_playing(&self) -> bool {
        self.vst.clip_playing() || self.vst_instances.values().any(VstInstrument::clip_playing)
    }
    pub(in crate::ui) fn vst_selected(&self) -> bool {
        self.tracks
            .get(self.cursor_row)
            .and_then(|track| track.instrument)
            .is_some_and(|id| self.is_vst_id(id))
    }
    pub(in crate::ui) fn vst_track(&self) -> Option<usize> {
        self.tracks
            .iter()
            .position(|track| track.instrument == Some(self.vst.id))
    }
    pub(in crate::ui) fn load_vst(
        &mut self,
        bundle: PathBuf,
        target: Option<TrackId>,
        cx: &mut Context<Self>,
    ) {
        self.load_vst_class(bundle, target, None, cx);
    }
    pub(in crate::ui) fn load_vst_class(
        &mut self,
        bundle: PathBuf,
        target: Option<TrackId>,
        class: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.vst_task.is_some() {
            self.status_message = "Espera a que termine de cargar el VST3".into();
            return;
        }
        if let Some(track) = target.and_then(|id| self.tracks.iter().find(|track| track.id == id)) {
            if !track.accepts_clips() {
                self.status_message = "Selecciona una pista MIDI para el instrumento".into();
                return;
            }
        }
        let config = self.settings.audio.saved.clone();
        let selected_class = class.clone();
        let previous_instrument = target
            .and_then(|id| self.tracks.iter().find(|track| track.id == id))
            .map(|track| track.instrument);
        let expected = config.clone();
        let path = bundle.clone();
        self.status_message = "Cargando instrumento VST3 mediante yabridge…".into();
        let work =
            cx.background_spawn(async move { VstInstrument::prepare(&path, &config, class) });
        self.vst_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.vst_task = None;
                if this.settings.audio.saved != expected {
                    this.status_message =
                        "La salida cambió durante la carga; vuelve a añadir el VST3".into();
                    cx.notify();
                    return;
                }
                let index =
                    target.and_then(|id| this.tracks.iter().position(|track| track.id == id));
                if target.is_some() && index.is_none() {
                    this.status_message = "Pista retirada durante la carga VST3".into();
                    cx.notify();
                    return;
                }
                if index.map(|i| this.tracks[i].instrument) != previous_instrument {
                    this.status_message =
                        "El instrumento de destino cambió; carga VST3 descartada".into();
                    cx.notify();
                    return;
                }
                match result {
                    Ok((prepared, name)) => {
                        let gain = index.map_or(1., |i| this.track_gain(i));
                        let pan = index.map_or(0., |i| this.track_pan(i));
                        let fx = index.map_or(1., |i| this.track_fx_gain(i));
                        let id = index
                            .and_then(|i| this.tracks[i].instrument)
                            .filter(|id| this.is_vst_id(*id))
                            .unwrap_or_else(|| {
                                if this
                                    .tracks
                                    .iter()
                                    .all(|track| track.instrument != Some(VST_ID))
                                {
                                    VST_ID
                                } else {
                                    InstrumentId::fresh()
                                }
                            });
                        let mut candidate = VstInstrument::new(id);
                        match candidate.install(
                            prepared,
                            name,
                            bundle,
                            selected_class,
                            gain,
                            pan,
                            fx,
                            this.utility.clone(),
                        ) {
                            Ok(()) => {
                                this.stop_all();
                                let index = index.unwrap_or_else(|| {
                                    this.edit_track_structure(TrackEdit::Insert);
                                    this.cursor_row
                                });
                                this.tracks[index].instrument = Some(id);
                                this.track_revision = this.track_revision.wrapping_add(1);
                                if id == this.vst.id {
                                    this.vst = candidate;
                                } else {
                                    this.vst_instances.insert(id, candidate);
                                    this.activate_vst(id);
                                }
                                this.cursor_row = index;
                                this.open_track_instrument(this.tracks[index].id);
                                this.sync_midi_focus();
                                this.status_message = format!(
                                    "{} cargado · M: teclado MIDI · Espacio: Play/Stop",
                                    this.vst.name
                                );
                            }
                            Err(error) => this.status_message = format!("Salida VST3: {error}"),
                        }
                    }
                    Err(error) => this.status_message = format!("VST3: {error}"),
                }
                cx.notify();
            });
        }));
    }
    pub(in crate::ui) fn vst_midi(&mut self, message: MidiMessage) {
        let id = self
            .tracks
            .get(self.cursor_row)
            .and_then(|track| track.instrument)
            .filter(|id| self.is_vst_id(*id))
            .unwrap_or(self.vst.id);
        self.vst_midi_to(id, message);
    }
    pub(in crate::ui) fn vst_midi_to(&mut self, id: InstrumentId, message: MidiMessage) {
        let ready = if id == self.vst.id {
            self.vst.ready()
        } else {
            self.vst_instances
                .get(&id)
                .is_some_and(VstInstrument::ready)
        };
        if !ready {
            self.status_message = "VST3 sin cargar; selecciónalo desde [+]".into();
            return;
        }
        self.mix_vst();
        if id == self.vst.id {
            self.vst.send_midi(message);
        } else if let Some(vst) = self.vst_instances.get_mut(&id) {
            vst.send_midi(message);
        }
        self.midi_capture.record(
            message,
            self.session_epoch.elapsed().as_nanos() as u64,
            self.timeline.tempo.bpm(),
        );
    }
    pub(in crate::ui) fn play_vst_clip(
        &mut self,
        clip: &elevate::musical::clip::MidiClip,
        start: f64,
    ) {
        if let Some(id) = self.tracks.get(clip.track).and_then(|t| t.instrument) {
            self.activate_vst(id);
        }
        self.instrument.stop();
        self.sampler.silence();
        self.vst.mix(
            self.track_gain(clip.track) * clip.props.gain_linear(),
            self.track_pan(clip.track),
            self.track_fx_gain(clip.track),
        );
        match self.vst.play_clip(clip, start, self.timeline.tempo.bpm()) {
            Ok(()) => {
                self.timeline.playhead = Some(start);
                self.status_message = format!("▶ {} → {}", clip.name, self.vst.name);
            }
            Err(error) => self.status_message = error,
        }
    }
    pub(in crate::ui) fn mix_vst(&mut self) {
        for track in 0..self.tracks.len() {
            let Some(id) = self.tracks[track].instrument else {
                continue;
            };
            let playing = if id == self.vst.id {
                self.vst.clip_playing()
            } else {
                self.vst_instances
                    .get(&id)
                    .is_some_and(VstInstrument::clip_playing)
            };
            let clip_gain = if playing {
                self.timeline
                    .midi_clip
                    .as_ref()
                    .filter(|c| c.track == track)
                    .map_or(1., |c| c.props.gain_linear())
            } else {
                1.
            };
            let mix = (
                self.track_gain(track) * clip_gain,
                self.track_pan(track),
                self.track_fx_gain(track),
            );
            if id == self.vst.id {
                self.vst.mix(mix.0, mix.1, mix.2);
            } else if let Some(vst) = self.vst_instances.get(&id) {
                vst.mix(mix.0, mix.1, mix.2);
            }
        }
    }
    pub(in crate::ui) fn remove_vst(&mut self) {
        if let Some(id) = self.tracks.get(self.cursor_row).and_then(|t| t.instrument) {
            self.activate_vst(id);
        }
        self.vst_task = None;
        self.vst.unload();
        if let Some(index) = self.vst_track() {
            self.tracks[index].instrument = None;
            self.track_revision = self.track_revision.wrapping_add(1);
        }
        let removed = self.vst.id;
        if let Some(next) = self.vst_instances.keys().next().copied() {
            self.activate_vst(next);
            self.vst_instances.remove(&removed);
        } else {
            self.vst = VstInstrument::default();
        }
        self.fx.editor_track = None;
        self.sync_midi_focus();
        self.status_message = "Instrumento VST3 retirado".into();
    }
}
