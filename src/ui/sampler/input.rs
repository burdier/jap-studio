use crate::{
    instruments::sampler::{SAMPLER_ID, SampleAsset, Sampler},
    ui::DawApp,
};
use elevate::{
    devices::sampler::{self, PlayMode},
    musical::track::{TrackEdit, TrackKind, assign_instrument},
};
use gpui::{AppContext, Context, PathPromptOptions};
use std::path::PathBuf;

impl DawApp {
    /// La instancia actual es única: repetir la acción abre su pista sin reasignarla.
    pub(in crate::ui) fn insert_sampler_track(&mut self) {
        if let Some(index) = self
            .tracks
            .iter()
            .position(|track| track.instrument == Some(SAMPLER_ID))
        {
            self.select_track(index, false, false);
            if let Some(parent) = self.tracks[index].parent {
                if let Some(presentation) = self.track_presentation.get_mut(&parent) {
                    presentation.collapsed = false;
                }
            }
            self.refresh_track_layout();
            self.reveal_track();
            self.open_track_instrument(self.tracks[index].id);
            self.status_message =
                "Hay Sampler · pista existente · una instancia disponible por sesión".into();
            return;
        }
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
            .is_some_and(|t| t.instrument == Some(SAMPLER_ID))
    }
    pub(in crate::ui) fn add_sampler(&mut self, track: usize) {
        self.stop_all();
        assign_instrument(&mut self.tracks, track, SAMPLER_ID);
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
            .filter(|t| t.instrument == Some(SAMPLER_ID))
            .map(|t| t.id)
        else {
            return;
        };
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
                    .any(|t| t.id == target && t.instrument == Some(SAMPLER_ID))
                {
                    this.status_message =
                        "Carga descartada: el sampler fue retirado de la pista".into();
                } else {
                    match result {
                        Ok((asset, engine)) => {
                            this.sampler.install(asset, engine);
                            this.status_message =
                                format!("{} · listo para tocar", this.sampler.name());
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
                if this.sampler_track().is_none() {
                    this.sampler_ui.pending = false;
                } else if this.sampler_ui.pending {
                    this.prepare_sampler(cx);
                } else {
                    match result {
                        Ok((asset, engine)) => {
                            this.sampler.install(asset, engine);
                            this.status_message = "Sampler actualizado · toca para escuchar".into();
                        }
                        Err(e) => this.status_message = e,
                    }
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
        if sampler::live_control(&self.sampler.params, id).is_some() {
            self.sampler.update_live();
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
