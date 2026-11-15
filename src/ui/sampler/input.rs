use crate::{
    instruments::sampler::{SAMPLER_ID, SampleAsset, Sampler},
    ui::DawApp,
};
use elevate::{
    devices::sampler::{self, PlayMode},
    musical::track::{TrackEdit, TrackKind},
};
use gpui::{AppContext, Context, PathPromptOptions};
use std::path::PathBuf;

impl DawApp {
    /// Cada inserción crea una instancia editable en su propia pista.
    pub(in crate::ui) fn insert_sampler_track(&mut self) {
        // Conserva remapeo de clips, selección, grupos y mezcla del controlador común.
        self.edit_track_structure(TrackEdit::Insert);
        if let Some(track) = self.tracks.get_mut(self.cursor_row) {
            track.name = "Hay Sampler".into();
            track.kind = TrackKind::Synth;
            self.add_sampler(self.cursor_row);
        }
    }

    pub(in crate::ui) fn sampler_selected(&self) -> bool {
        self.tracks
            .get(self.cursor_row)
            .and_then(|t| t.instrument)
            .is_some_and(|id| self.is_sampler_id(id))
    }
    pub(in crate::ui) fn add_sampler(&mut self, track: usize) {
        if track >= self.tracks.len() {
            return;
        }
        if self.tracks[track].kind == TrackKind::Group {
            self.status_message = "Añade el sampler a una pista hija del Hilo".into();
            return;
        }
        self.stop_all();
        let id = self
            .tracks
            .get(track)
            .and_then(|t| t.instrument)
            .filter(|id| self.is_sampler_id(*id))
            .unwrap_or_else(|| {
                if self.tracks.iter().any(|t| t.instrument == Some(SAMPLER_ID)) {
                    elevate::musical::model::InstrumentId::fresh()
                } else {
                    SAMPLER_ID
                }
            });
        if id != self.active_sampler_id && !self.sampler_instances.contains_key(&id) {
            self.sampler_instances.insert(id, Sampler::default());
        }
        self.tracks[track].instrument = Some(id);
        self.activate_sampler(id);
        self.track_revision = self.track_revision.wrapping_add(1);
        self.mix_vst();
        self.cursor_row = track;
        self.open_track_instrument(self.tracks[track].id);
        self.sync_midi_focus();
        self.status_message =
            "Hay Sampler · carga o arrastra un sample · M activa el teclado MIDI".into();
    }
    pub(in crate::ui) fn choose_sample(&mut self, cx: &mut Context<Self>) {
        if self.sampler_ui.picker.is_some() {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Cargar sample".into()),
        });
        self.sampler_ui.picker = Some(cx.spawn(async move |this, cx| {
            let result = prompt.await;
            let _ = this.update(cx, |this, cx| {
                this.sampler_ui.picker = None;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            this.load_sample(path.clone(), cx);
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => this.status_message = "No se pudo abrir el selector de samples".into(),
                }
                cx.notify();
            });
        }));
    }
    pub(in crate::ui) fn load_sample(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.sampler_ui.busy {
            self.status_message = "Espera a que termine de preparar el sample".into();
            return;
        }
        let Some(target) = self
            .tracks
            .get(self.cursor_row)
            .filter(|t| t.instrument == Some(self.active_sampler_id))
            .map(|t| t.id)
        else {
            return;
        };
        let target_instrument = self.active_sampler_id;
        self.sampler_ui.busy = true;
        let params = self.sampler.params.clone();
        self.status_message = format!("Cargando {}…", path.display());
        let work = cx.background_spawn(async move {
            let asset = SampleAsset::load(&path)?;
            let engine = Sampler::prepare(asset.clone(), params)?;
            Ok::<_, String>((asset, engine))
        });
        self.sampler_ui.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.sampler_ui.busy = false;
                this.sampler_ui.task = None;
                if !this
                    .tracks
                    .iter()
                    .any(|t| t.id == target && t.instrument == Some(target_instrument))
                {
                    this.status_message =
                        "Carga descartada: el sampler fue retirado de la pista".into();
                } else {
                    match result {
                        Ok((asset, engine)) => {
                            if target_instrument == this.active_sampler_id {
                                this.sampler.install(asset, engine);
                                this.status_message =
                                    format!("{} · listo para tocar", this.sampler.name());
                            } else if let Some(sampler) =
                                this.sampler_instances.get_mut(&target_instrument)
                            {
                                sampler.install(asset, engine);
                                this.status_message = "Sampler cargado en su pista".into();
                            }
                            this.track_revision = this.track_revision.wrapping_add(1);
                        }
                        Err(error) => this.status_message = error,
                    }
                }
                if this.sampler_ui.pending {
                    this.prepare_sampler(cx);
                }
                cx.notify();
            });
        }));
    }
    pub(in crate::ui) fn prepare_sampler(&mut self, cx: &mut Context<Self>) {
        if self.sampler_ui.busy {
            self.sampler_ui.pending = true;
            return;
        }
        self.sampler_ui.pending = false;
        let Some(asset) = self.sampler.asset.clone() else {
            return;
        };
        let target_instrument = self.active_sampler_id;
        self.sampler_ui.busy = true;
        let params = self.sampler.params.clone();
        let work = cx.background_spawn(async move {
            Sampler::prepare(asset.clone(), params).map(|engine| (asset, engine))
        });
        self.sampler_ui.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.sampler_ui.busy = false;
                this.sampler_ui.task = None;
                let pending = this.sampler_ui.pending;
                if !this
                    .tracks
                    .iter()
                    .any(|t| t.instrument == Some(target_instrument))
                {
                    this.status_message = "Preparación descartada: el sampler fue retirado".into();
                } else if !pending || target_instrument != this.active_sampler_id {
                    match result {
                        Ok((asset, engine)) => {
                            if target_instrument == this.active_sampler_id {
                                this.sampler.install(asset, engine);
                            } else if let Some(sampler) =
                                this.sampler_instances.get_mut(&target_instrument)
                            {
                                sampler.install(asset, engine);
                            }
                            this.track_revision = this.track_revision.wrapping_add(1);
                            this.status_message = "Sampler actualizado · toca para escuchar".into();
                        }
                        Err(e) => this.status_message = e,
                    }
                }
                if pending {
                    this.sampler_ui.pending = false;
                    this.prepare_sampler(cx);
                }
                cx.notify();
            });
        }));
    }
    pub(in crate::ui) fn set_sample_param(
        &mut self,
        id: usize,
        value: f32,
        cx: &mut Context<Self>,
    ) {
        self.sampler.params.set(id, value);
        self.track_revision = self.track_revision.wrapping_add(1);
        if let Some((cc, v)) = sampler::live_control(&self.sampler.params, id) {
            self.sampler.update_live_param(id);
            if let Some(player) = &self.player
                && player.has_instrument(self.active_sampler_id)
            {
                player.send_live_midi(
                    self.active_sampler_id,
                    elevate::musical::midi::MidiMessage::ControlChange {
                        cc,
                        value: (v.clamp(0.0, 1.0) * 127.0).round() as u8,
                    },
                );
            }
        } else if self.sampler_ui.drag.is_none() && self.sampler_ui.wave_drag.is_none() {
            self.prepare_sampler(cx);
        }
        let spec = sampler::PARAMS[id];
        self.status_message = format!(
            "{} · {:.2} {}",
            spec.name,
            self.sampler.params.get(id),
            spec.unit
        );
        cx.notify();
    }
    pub(in crate::ui) fn sample_mode(&mut self, mode: PlayMode, cx: &mut Context<Self>) {
        self.sampler.params.mode = mode;
        self.track_revision = self.track_revision.wrapping_add(1);
        self.prepare_sampler(cx);
        cx.notify();
    }
    pub(in crate::ui) fn finish_sample_drag(&mut self, cx: &mut Context<Self>) {
        if self.sampler_ui.wave_drag.take().is_some() {
            self.prepare_sampler(cx);
        }
        if let Some((id, _, _)) = self.sampler_ui.drag.take() {
            if sampler::live_control(&self.sampler.params, id).is_none() {
                self.prepare_sampler(cx);
            }
        }
    }
}
