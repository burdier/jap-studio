//! Acción de exportación y Bounce MIDI→WAV: captura estado propio y observa el worker.
use super::DawApp;
use crate::audio::{
    AudioClip, AudioFile,
    export::{self, Snapshot},
};
use elevate::{
    export::{BitDepth, ExportOptions, Format, Target},
    musical::{
        clip::ClipId,
        track::{Track, TrackKind},
    },
};
use std::{
    sync::{Arc, atomic::Ordering, mpsc::TryRecvError},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) struct BounceTarget {
    pub name: String,
    pub start_beat: f64,
    pub source_midi_ids: Vec<ClipId>,
    /// Si es `Some((track_id, instrument_id))`, carga el WAV resultante en ese Hay Sampler
    /// en vez de crear una pista de audio nueva.
    pub load_into_sampler: Option<(
        elevate::musical::track::TrackId,
        elevate::musical::model::InstrumentId,
    )>,
}

pub(super) struct FreezeTarget {
    pub track_id: elevate::musical::track::TrackId,
    pub name: String,
    pub start_beat: f64,
}

pub(super) struct FrozenTrack {
    pub clip: AudioClip,
    pub path: std::path::PathBuf,
}

impl DawApp {
    pub(in crate::ui) fn is_track_frozen(&self, id: elevate::musical::track::TrackId) -> bool {
        self.frozen_tracks.contains_key(&id)
    }

    pub(in crate::ui) fn toggle_freeze_track(&mut self, index: usize) {
        let Some(track) = self.tracks.get(index) else {
            return;
        };
        let track_id = track.id;
        if self.is_track_frozen(track_id) {
            self.unfreeze_track(track_id);
        } else {
            self.freeze_track(index);
        }
    }

    pub(in crate::ui) fn unfreeze_track(&mut self, track_id: elevate::musical::track::TrackId) {
        let Some(frozen) = self.frozen_tracks.remove(&track_id) else {
            return;
        };
        let _ = std::fs::remove_file(&frozen.path);
        let name = self
            .tracks
            .iter()
            .find(|t| t.id == track_id)
            .map(|t| t.name.to_string())
            .unwrap_or_else(|| "Pista".into());
        self.publish_clip_edit();
        self.status_message = format!("Pista «{name}» descongelada · instrumento y FX restaurados");
    }

    pub(in crate::ui) fn freeze_track(&mut self, index: usize) {
        if self.export_job.is_some() {
            self.status_message = "Ya hay un render o exportación en curso".into();
            return;
        }
        let Some(track) = self.tracks.get(index).cloned() else {
            return;
        };
        if track.is_return() {
            self.status_message = "No se puede congelar un bus Return (FX)".into();
            return;
        }
        if track.kind == TrackKind::Group {
            self.status_message = "Congela las pistas individuales dentro del grupo".into();
            return;
        }
        if self.is_track_frozen(track.id) {
            self.unfreeze_track(track.id);
            return;
        }
        let mut start_beat = f64::INFINITY;
        let mut end_beat = 0.0_f64;
        for clip in self
            .timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
        {
            if clip.track == index && !clip.props.muted {
                start_beat = start_beat.min(clip.start_beat);
                end_beat = end_beat.max(clip.end_beat(self.timeline.tempo.bpm()));
            }
        }
        for clip in self.timeline.midi_clips() {
            if clip.track == index && clip.drum_target.is_none() && !clip.props.muted {
                start_beat = start_beat.min(clip.start_beat);
                end_beat = end_beat.max(clip.end_beat());
            }
        }
        if !start_beat.is_finite() || end_beat <= start_beat {
            self.status_message = format!("«{}» no tiene clips activos para congelar", track.name);
            return;
        }
        let safe_name: String = track
            .name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .take(36)
            .collect();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let mut dir = self.settings.export.saved.directory.clone();
        dir.push("freezes");
        let options = ExportOptions {
            name: format!("{safe_name}_freeze_{stamp}"),
            formats: vec![Format::Wav],
            target: Target::Master,
            tracks: vec![index],
            directory: dir,
            sample_rate: self.settings.export.saved.sample_rate,
            bits: BitDepth::Float32,
            mp3: self.settings.export.saved.mp3,
            mono: false,
            start_beat,
            length_beats: Some(end_beat - start_beat),
            tail_seconds: 1.5,
            master_fx: Some(false),
            normalize_lufs: None,
            true_peak_limit_db: None,
        };
        self.stop_audio();
        match self.build_export_job_inner(options, Some(track.id)) {
            Ok(job) => {
                self.bounce_pending = None;
                self.freeze_pending = Some(FreezeTarget {
                    track_id: track.id,
                    name: track.name.to_string(),
                    start_beat,
                });
                self.export_job = Some(job);
                self.status_message =
                    format!("❄ Congelando «{}» (pre-fader, post-FX)…", track.name);
            }
            Err(error) => self.status_message = format!("Freeze: {error}"),
        }
    }

    fn build_export_job(&mut self, options: ExportOptions) -> Result<export::ExportJob, String> {
        self.build_export_job_inner(options, None)
    }

    fn build_export_job_inner(
        &mut self,
        mut options: ExportOptions,
        freeze_track_id: Option<elevate::musical::track::TrackId>,
    ) -> Result<export::ExportJob, String> {
        // Elegir una cabecera de grupo incluye también sus pistas hijas.
        if !options.tracks.is_empty() {
            let selected = options.tracks.clone();
            for index in selected {
                if let Some(group) = self.tracks.get(index)
                    && group.kind == TrackKind::Group
                {
                    for (child, track) in self.tracks.iter().enumerate() {
                        if track.parent == Some(group.id) && !options.tracks.contains(&child) {
                            options.tracks.push(child);
                        }
                    }
                }
            }
        }
        if let Ok(rest) = options.directory.strip_prefix("~") {
            options.directory = dirs::home_dir()
                .ok_or("No se encontró la carpeta personal")?
                .join(rest);
        }
        if options.directory.is_relative() {
            options.directory = std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(&options.directory);
        }
        let audio = self.audio_snapshot();
        let mut inputs = self.arrangement_inputs(&audio, &self.midi_snapshot(), Some(&options))?;
        if let Some(freeze_id) = freeze_track_id {
            // Freeze pre-fader, post-FX: ganancia unitaria, pan centrado y sin envíos/automatización
            // para conservar el control vivo de fader, pan, mute, solo y envíos tras congelar.
            inputs.retain(|input| input.id == freeze_id);
            for input in &mut inputs {
                input.parent = None;
                input.gain = 1.0;
                input.pan = 0.0;
                input.audible = true;
                input.source_audible = true;
                input.sends.clear();
                input.automation = Default::default();
            }
        }
        let session = &self.pattern_editor.session;
        let include_kit = freeze_track_id.is_none()
            && (self.has_drum_clips()
                || self.center_view == super::CenterView::Session
                || self.pattern_loop_beats.is_some());
        let drums = if include_kit
            && (options.tracks.is_empty()
                || session.source == elevate::musical::drums::DrumPlaybackSource::Arrangement)
        {
            self.drum_score_for_tracks(|i| options.includes_track(i))?
                .map(|score| crate::audio::drums::DrumInput {
                    score: Some(score),
                    kit: session.kit.clone(),
                })
        } else {
            None
        };
        let snapshot = Snapshot {
            inputs,
            drums,
            tracks: self
                .tracks
                .iter()
                .map(|t| (t.id, t.name.to_string()))
                .collect(),
            utility: self.utility.state(),
            master_fx_gain: self.master_fx_gain(),
            master_rack: None,
            bpm: self.timeline.tempo.bpm(),
            meter: self.timeline.meter,
        };
        let plugins = export::Plugins::capture(
            &snapshot.inputs,
            &self.tracks,
            options.includes_master_fx().then_some(&self.master_chain),
            &self.vst_effects.instances,
            &self.lua_components.instances,
        )?;
        export::start(snapshot, options, plugins)
    }

    pub(super) fn start_export(&mut self, command: &str) {
        if self.export_job.is_some() {
            self.status_message =
                "Ya hay una exportación activa · :export.cancel para cancelarla".into();
            return;
        }
        let result = ExportOptions::parse_with_defaults(command, &self.settings.export.saved)
            .and_then(|options| self.build_export_job(options));
        match result {
            Ok(job) => {
                self.bounce_pending = None;
                self.freeze_pending = None;
                self.export_job = Some(job);
                self.status_message = "Preparando exportación del Arrangement…".into();
            }
            Err(error) => self.status_message = format!("Export: {error}"),
        }
    }

    /// Renderiza el clip MIDI activo (o la selección MIDI de su pista) a WAV 32-bit float
    /// e inserta el resultado como clip de audio en una nueva pista del Arrangement.
    pub(in crate::ui) fn bounce_midi_to_audio(&mut self) {
        self.start_midi_bounce(None);
    }

    /// Renderiza el clip MIDI activo (o selección) a WAV y lo carga directamente en Hay Sampler.
    pub(in crate::ui) fn bounce_midi_to_sampler(
        &mut self,
        target_track: elevate::musical::track::TrackId,
        target_sampler: elevate::musical::model::InstrumentId,
    ) {
        self.start_midi_bounce(Some((target_track, target_sampler)));
    }

    fn start_midi_bounce(
        &mut self,
        load_into_sampler: Option<(
            elevate::musical::track::TrackId,
            elevate::musical::model::InstrumentId,
        )>,
    ) {
        if self.export_job.is_some() {
            self.status_message = "Ya hay un render o exportación en curso".into();
            return;
        }
        let target_ids = self.timeline.midi_edit_targets();
        let anchor_track = self
            .timeline
            .midi_clip
            .as_ref()
            .map(|c| c.track)
            .or_else(|| {
                self.timeline
                    .midi_clips()
                    .find(|c| target_ids.contains(&c.id))
                    .map(|c| c.track)
            });
        let Some(track) = anchor_track else {
            self.status_message = "Selecciona un clip MIDI para hacer Bounce a Audio".into();
            return;
        };
        let clips: Vec<_> = self
            .timeline
            .midi_clips()
            .filter(|c| c.track == track && target_ids.contains(&c.id) && !c.props.muted)
            .collect();
        if clips.is_empty() {
            self.status_message =
                "El clip MIDI seleccionado está silenciado o vacío para hacer Bounce".into();
            return;
        }
        let start_beat = clips
            .iter()
            .map(|c| c.start_beat)
            .fold(f64::INFINITY, f64::min);
        let end_beat = clips.iter().map(|c| c.end_beat()).fold(0.0_f64, f64::max);
        if !start_beat.is_finite() || end_beat <= start_beat {
            self.status_message = "Rango MIDI inválido para Bounce".into();
            return;
        }
        let base_name = self
            .timeline
            .midi_clip
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_else(|| self.track_title(track));
        let safe_name: String = base_name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .take(36)
            .collect();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let mut dir = self.settings.export.saved.directory.clone();
        dir.push("bounces");
        let options = ExportOptions {
            name: format!("{safe_name}_bounce_{stamp}"),
            formats: vec![Format::Wav],
            target: Target::Master,
            tracks: vec![track],
            directory: dir,
            sample_rate: self.settings.export.saved.sample_rate,
            bits: BitDepth::Float32,
            mp3: self.settings.export.saved.mp3,
            mono: false,
            start_beat,
            length_beats: Some(end_beat - start_beat),
            tail_seconds: if load_into_sampler.is_some() {
                0.6
            } else {
                1.0
            },
            master_fx: Some(false),
            normalize_lufs: None,
            true_peak_limit_db: None,
        };
        let source_midi_ids: Vec<ClipId> = clips.iter().map(|c| c.id).collect();
        self.stop_audio();
        match self.build_export_job(options) {
            Ok(job) => {
                let for_sampler = load_into_sampler.is_some();
                self.bounce_pending = Some(BounceTarget {
                    name: base_name.clone(),
                    start_beat,
                    source_midi_ids,
                    load_into_sampler,
                });
                self.export_job = Some(job);
                self.status_message = if for_sampler {
                    format!("⚡ Haciendo Bounce de «{base_name}» para Hay Sampler…")
                } else {
                    format!("Renderizando Bounce de «{base_name}» a WAV…")
                };
            }
            Err(error) => self.status_message = format!("Bounce: {error}"),
        }
    }

    pub(super) fn cancel_export(&mut self) {
        if let Some(job) = &self.export_job {
            job.cancel.store(true, Ordering::Relaxed);
            self.bounce_pending = None;
            self.freeze_pending = None;
            self.status_message = "Cancelando render…".into();
        } else {
            self.status_message = "No hay exportación activa".into();
        }
    }

    pub(super) fn poll_export(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(job) = &self.export_job else {
            return;
        };
        match job.result.try_recv() {
            Ok(result) => {
                self.export_job = None;
                if let Some(freeze) = self.freeze_pending.take() {
                    self.finish_freeze(freeze, result.map(|r| r.paths));
                    return;
                }
                if let Some(bounce) = self.bounce_pending.take() {
                    self.finish_bounce(bounce, result.map(|r| r.paths), cx);
                    return;
                }
                self.finish_export_form(result.as_ref().map_err(String::as_str));
                self.status_message = match result {
                    Ok(report) => {
                        let folder = report
                            .paths
                            .first()
                            .and_then(|p| p.parent())
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        let mut metrics = Vec::new();
                        if let Some(lufs) = report.loudness.integrated_lufs {
                            metrics.push(format!("{lufs:.1} LUFS"));
                        }
                        if let Some(tp) = report.loudness.true_peak_dbtp {
                            metrics.push(format!("{tp:.1} dBTP"));
                        }
                        if report.loudness.applied_gain_db.abs() >= 0.05 {
                            metrics
                                .push(format!("gain {:+.1} dB", report.loudness.applied_gain_db));
                        }
                        if report.loudness.max_gain_reduction_db >= 0.05 {
                            metrics.push(format!(
                                "GR -{:.1} dB",
                                report.loudness.max_gain_reduction_db
                            ));
                        }
                        let metrics_text = if metrics.is_empty() {
                            String::new()
                        } else {
                            format!(" · {}", metrics.join(" · "))
                        };
                        let peak = if report.overs == 0 {
                            String::new()
                        } else {
                            format!(
                                " · {} muestras >0 dBFS (PCM entero/MP3 saturan; float conserva)",
                                report.overs
                            )
                        };
                        format!(
                            "Exportados {} archivos → {folder}{metrics_text}{peak}",
                            report.paths.len()
                        )
                    }
                    Err(error) => format!("Export: {error}"),
                };
            }
            Err(TryRecvError::Empty) => {
                if !job.cancel.load(Ordering::Relaxed) {
                    let prefix = if let Some(freeze) = &self.freeze_pending {
                        format!("❄ Congelando «{}»", freeze.name)
                    } else if self.bounce_pending.is_some() {
                        "Bounce a WAV".to_string()
                    } else {
                        "Exportando Arrangement".to_string()
                    };
                    self.status_message = format!(
                        "{prefix} · {}% · :export.cancel",
                        job.progress.load(Ordering::Relaxed)
                    );
                }
            }
            Err(TryRecvError::Disconnected) => {
                self.export_job = None;
                let was_freeze = self.freeze_pending.take().is_some();
                let was_bounce = self.bounce_pending.take().is_some();
                if !was_bounce && !was_freeze {
                    self.finish_export_form(Err("La exportación terminó sin generar archivos"));
                }
                self.status_message = if was_freeze {
                    "Freeze: el worker terminó sin resultado".into()
                } else if was_bounce {
                    "Bounce: el worker terminó sin resultado".into()
                } else {
                    "Export: el worker terminó sin resultado".into()
                };
            }
        }
    }

    fn finish_freeze(
        &mut self,
        freeze: FreezeTarget,
        result: Result<Vec<std::path::PathBuf>, String>,
    ) {
        let paths = match result {
            Ok(p) => p,
            Err(error) => {
                self.status_message = format!("Freeze fallido: {error}");
                return;
            }
        };
        let Some(path) = paths.first().cloned() else {
            self.status_message = "Freeze no generó archivo WAV".into();
            return;
        };
        let Some(track_index) = self.tracks.iter().position(|t| t.id == freeze.track_id) else {
            let _ = std::fs::remove_file(&path);
            return;
        };
        let file = match AudioFile::decode(&path) {
            Ok(file) => file,
            Err(error) => {
                self.status_message = format!("Freeze: no se pudo leer el WAV ({error})");
                return;
            }
        };
        let clip = AudioClip {
            processing: Default::default(),
            id: ClipId::fresh(),
            name: format!("{} (Freeze)", freeze.name),
            file: Arc::new(file),
            source_path_override: None,
            track: track_index,
            start_beat: freeze.start_beat,
            props: Default::default(),
            trim_start: 0.0,
            trim_len: None,
        };
        self.frozen_tracks
            .insert(freeze.track_id, FrozenTrack { clip, path });
        self.publish_clip_edit();
        self.status_message = format!(
            "❄ «{}» congelada · instrumento y FX en reposo (fader, pan y envíos activos)",
            freeze.name
        );
    }

    fn finish_bounce(
        &mut self,
        bounce: BounceTarget,
        result: Result<Vec<std::path::PathBuf>, String>,
        cx: &mut gpui::Context<Self>,
    ) {
        let paths = match result {
            Ok(p) => p,
            Err(error) => {
                self.status_message = format!("Bounce fallido: {error}");
                return;
            }
        };
        let Some(path) = paths.first() else {
            self.status_message = "Bounce no generó archivo WAV".into();
            return;
        };
        if let Some((target_track, target_sampler)) = bounce.load_into_sampler {
            self.load_sample_into(target_track, target_sampler, path.clone(), None, cx);
            self.status_message =
                format!("⚡ Bounce de «{}» cargando en Hay Sampler…", bounce.name);
            return;
        }
        let file = match AudioFile::decode(path) {
            Ok(file) => file,
            Err(error) => {
                self.status_message = format!("Bounce: no se pudo leer el WAV ({error})");
                return;
            }
        };
        let new_track = self.tracks.len();
        let mut tracks = self.tracks.clone();
        let mut sources: Vec<Option<usize>> = (0..self.tracks.len()).map(Some).collect();
        let mut levels = self.track_levels.clone();
        let mut pans = self.track_pans.clone();
        let bounce_track = Track::new(format!("{} (Bounce)", bounce.name), TrackKind::Audio);
        let bounce_track_id = bounce_track.id;
        tracks.push(bounce_track);
        sources.push(None);
        levels.push(75);
        pans.push(50);
        self.commit_track_plan(
            tracks,
            sources,
            levels,
            pans,
            vec![bounce_track_id],
            true,
            true,
            None,
        );
        for clip in self
            .timeline
            .midi_clip
            .iter_mut()
            .chain(&mut self.timeline.other_midi_clips)
        {
            if bounce.source_midi_ids.contains(&clip.id) {
                clip.props.muted = true;
            }
        }
        let clip_id = ClipId::fresh();
        let clip = AudioClip {
            processing: Default::default(),
            id: clip_id,
            name: format!("{} (WAV)", bounce.name),
            file: Arc::new(file),
            source_path_override: None,
            track: new_track,
            start_beat: bounce.start_beat,
            props: Default::default(),
            trim_start: 0.0,
            trim_len: None,
        };
        self.timeline.selected_midi.clear();
        self.timeline.insert_audio_clip(clip);
        self.timeline.selected_audio = vec![clip_id];
        self.timeline.focused_audio = Some(clip_id);
        self.select_track(new_track, false, false);
        self.publish_clip_edit();
        self.refresh_project_dirty();
        self.status_message = format!(
            "Bounce listo: «{}» → pista Audio (WAV 32-bit float · MIDI original silenciado)",
            bounce.name
        );
    }

    /// Exporta el clip MIDI activo a un archivo Standard MIDI File (`.mid`).
    pub(in crate::ui) fn export_midi_clip_dialog(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(clip) = self.timeline.midi_clip.clone() else {
            self.status_message = "Selecciona un clip MIDI para exportar a .mid".into();
            return;
        };
        let bytes = match elevate::musical::midi_export::export_clip_to_smf(
            &clip,
            self.timeline.tempo.bpm(),
            self.timeline.meter,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.status_message = format!("Exportar MIDI: {error}");
                return;
            }
        };
        self.save_smf_with_dialog(bytes, format!("{}.mid", sanitize_filename(&clip.name)), cx);
    }

    /// Exporta todos los clips MIDI activos del Arrangement a un archivo `.mid` multipista.
    pub(in crate::ui) fn export_arrangement_midi_dialog(&mut self, cx: &mut gpui::Context<Self>) {
        let clips = self.midi_snapshot();
        let bytes = match elevate::musical::midi_export::export_arrangement_to_smf(
            &self.tracks,
            &clips,
            self.timeline.tempo.bpm(),
            self.timeline.meter,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.status_message = format!("Exportar MIDI: {error}");
                return;
            }
        };
        let suggested = self
            .project
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| format!("{}.mid", sanitize_filename(&s.to_string_lossy())))
            .unwrap_or_else(|| "arrangement.mid".into());
        self.save_smf_with_dialog(bytes, suggested, cx);
    }

    fn save_smf_with_dialog(
        &mut self,
        bytes: Vec<u8>,
        suggested_name: String,
        cx: &mut gpui::Context<Self>,
    ) {
        let dir = self
            .project
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(std::path::Path::to_path_buf)
            .or_else(|| {
                let mut d = self.settings.export.saved.directory.clone();
                if let Ok(rest) = d.strip_prefix("~")
                    && let Some(home) = dirs::home_dir()
                {
                    d = home.join(rest);
                }
                Some(d)
            })
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let prompt = cx.prompt_for_new_path(&dir, Some(&suggested_name));
        cx.spawn(async move |this, cx| {
            let result = prompt.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(Some(mut path))) => {
                        if path.extension().is_none() {
                            path.set_extension("mid");
                        }
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        match std::fs::write(&path, &bytes) {
                            Ok(()) => {
                                this.status_message =
                                    format!("MIDI exportado → {}", path.display());
                            }
                            Err(e) => {
                                this.status_message = format!("Exportar MIDI: {e}");
                            }
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => {
                        this.status_message =
                            "No se pudo abrir el diálogo de exportación MIDI".into()
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' {
                ch
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "midi".into()
    } else {
        trimmed.to_owned()
    }
}
