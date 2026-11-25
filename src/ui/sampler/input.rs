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
        self.load_sample_with_root(path, None, cx);
    }

    pub(in crate::ui) fn load_sample_with_root(
        &mut self,
        path: PathBuf,
        root: Option<(u8, i32)>,
        cx: &mut Context<Self>,
    ) {
        if elevate::explorer::EntryKind::for_file(&path) == elevate::explorer::EntryKind::Hydrogen {
            self.import_hydrogen(path, None, cx);
            cx.notify();
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
        self.load_sample_into(target, target_instrument, path, root, cx);
    }

    /// Carga un sample en una pista/instancia concreta de Hay Sampler,
    /// aunque el cursor esté en otra pista (p. ej. al arrastrar desde el Arrangement).
    pub(in crate::ui) fn load_sample_into(
        &mut self,
        target: elevate::musical::track::TrackId,
        target_instrument: elevate::musical::model::InstrumentId,
        path: PathBuf,
        root: Option<(u8, i32)>,
        cx: &mut Context<Self>,
    ) {
        if self.sampler_ui.busy {
            self.status_message = "Espera a que termine de preparar el sample".into();
            return;
        }
        self.sampler_ui.busy = true;
        let mut params = if target_instrument == self.active_sampler_id {
            self.sampler.params.clone()
        } else {
            self.sampler_instances
                .get(&target_instrument)
                .map(|s| s.params.clone())
                .unwrap_or_default()
        };
        if let Some((note, cents)) = root {
            params.set(sampler::ROOT, f32::from(note));
            params.set(sampler::TUNE, -(cents as f32) / 100.0);
            params.keytrack = true;
        }
        self.status_message = format!("Cargando {}…", path.display());
        let work = cx.background_spawn(async move {
            let asset = SampleAsset::load(&path)?;
            let engine = Sampler::prepare(asset.clone(), params.clone())?;
            Ok::<_, String>((asset, engine, params))
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
                        Ok((asset, engine, params)) => {
                            if target_instrument == this.active_sampler_id {
                                if root.is_some() {
                                    this.sampler.params = params;
                                }
                                this.sampler_ui.wave_zoom = None;
                                this.sampler_ui.wave_hover = None;
                                this.sampler.install(asset, engine);
                                this.status_message =
                                    format!("{} · listo en Hay Sampler", this.sampler.name());
                            } else if let Some(sampler) =
                                this.sampler_instances.get_mut(&target_instrument)
                            {
                                if root.is_some() {
                                    sampler.params = params;
                                }
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

    /// Devuelve el `(TrackId, InstrumentId)` del Hay Sampler visible actualmente (o el primero existente).
    pub(in crate::ui) fn active_sampler_target(
        &self,
    ) -> Option<(
        elevate::musical::track::TrackId,
        elevate::musical::model::InstrumentId,
    )> {
        if let Some(track_id) = self.fx.editor_track
            && let Some(track) = self.tracks.iter().find(|t| t.id == track_id)
            && let Some(inst) = track.instrument
            && self.is_sampler_id(inst)
        {
            return Some((track.id, inst));
        }
        if let Some(track) = self
            .tracks
            .iter()
            .find(|t| t.instrument == Some(self.active_sampler_id))
        {
            return Some((track.id, self.active_sampler_id));
        }
        self.tracks.iter().find_map(|t| {
            t.instrument
                .filter(|id| self.is_sampler_id(*id))
                .map(|id| (t.id, id))
        })
    }

    /// Envía el clip activo del Arrangement (Audio o MIDI) a Hay Sampler.
    /// Si es Audio recortado/procesado escribe un WAV exacto del recorte; si es MIDI le hace Bounce.
    pub(in crate::ui) fn drop_arrangement_clip_into_sampler(
        &mut self,
        kind: crate::ui::timeline::DragKind,
        cx: &mut Context<Self>,
    ) {
        let (target_track, target_sampler) = match self.active_sampler_target() {
            Some(pair) => pair,
            None => {
                self.insert_sampler_track();
                let Some(pair) = self.active_sampler_target() else {
                    return;
                };
                pair
            }
        };
        self.activate_sampler(target_sampler);
        self.open_track_instrument(target_track);

        match kind {
            crate::ui::timeline::DragKind::Midi => {
                self.bounce_midi_to_sampler(target_track, target_sampler);
            }
            crate::ui::timeline::DragKind::Audio => {
                let Some(clip) = self.timeline.clip.clone() else {
                    self.status_message =
                        "Selecciona un clip de audio para enviar al Sampler".into();
                    return;
                };
                let untrimmed = clip.trim_start <= 0.0005
                    && clip.trim_len.is_none()
                    && clip.processing.edit == Default::default()
                    && clip.props.pitch_semitones == 0;
                if untrimmed
                    && let Some(path) = clip
                        .source_path_override
                        .as_ref()
                        .or(clip.file.source_path.as_ref())
                {
                    self.load_sample_into(target_track, target_sampler, path.clone(), None, cx);
                    return;
                }
                let rate = clip.file.sample_rate;
                let total = clip.file.frames.len();
                let start = ((clip.trim_start * f64::from(rate)).round() as usize).min(total);
                let end = (((clip.trim_start + clip.length_seconds()) * f64::from(rate)).round()
                    as usize)
                    .clamp(start, total);
                if end <= start {
                    self.status_message = "El clip de audio está vacío".into();
                    return;
                }
                let gain = clip.props.gain_linear();
                let frames: Vec<[f32; 2]> = clip.file.frames[start..end]
                    .iter()
                    .map(|&[l, r]| [l * gain, r * gain])
                    .collect();
                let safe_name: String = clip
                    .name
                    .chars()
                    .map(|ch| {
                        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                            ch
                        } else {
                            '_'
                        }
                    })
                    .take(32)
                    .collect();
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                let mut dir = self.settings.export.saved.directory.clone();
                if let Ok(rest) = dir.strip_prefix("~")
                    && let Some(home) = dirs::home_dir()
                {
                    dir = home.join(rest);
                }
                dir.push("bounces");
                let out_path = dir.join(format!("{safe_name}_sample_{stamp}.wav"));
                self.status_message = format!("Preparando «{}» para Hay Sampler…", clip.name);
                let job = cx.background_spawn(async move {
                    if let Some(parent) = out_path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    write_wav_i16(&out_path, rate, &frames)?;
                    Ok::<PathBuf, String>(out_path)
                });
                cx.spawn(async move |this, cx| {
                    let result = job.await;
                    let _ = this.update(cx, |this, cx| {
                        match result {
                            Ok(path) => {
                                this.load_sample_into(target_track, target_sampler, path, None, cx)
                            }
                            Err(e) => this.status_message = format!("Sampler: {e}"),
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
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

fn write_wav_i16(
    path: &std::path::Path,
    sample_rate: u32,
    frames: &[[f32; 2]],
) -> Result<(), String> {
    use std::io::Write;
    let data_bytes = (frames.len() * 4) as u32;
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    file.write_all(b"RIFF").map_err(|e| e.to_string())?;
    file.write_all(&(36 + data_bytes).to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(b"WAVEfmt ").map_err(|e| e.to_string())?;
    file.write_all(&16u32.to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(&1u16.to_le_bytes())
        .map_err(|e| e.to_string())?; // PCM
    file.write_all(&2u16.to_le_bytes())
        .map_err(|e| e.to_string())?; // Stereo
    file.write_all(&sample_rate.to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(&(sample_rate * 4).to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(&4u16.to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(&16u16.to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(b"data").map_err(|e| e.to_string())?;
    file.write_all(&data_bytes.to_le_bytes())
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::with_capacity(frames.len() * 4);
    for &[l, r] in frames {
        buf.extend_from_slice(&((l.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
        buf.extend_from_slice(&((r.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    file.write_all(&buf).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}
