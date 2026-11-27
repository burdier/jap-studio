//! Gestión local de proyectos: guardar, preparar candidato y publicar sin perder la sesión actual.
pub(in crate::ui) mod autosave;
mod dialogs;
mod effects;
mod install;
#[cfg(feature = "p2p")]
mod live;
mod location;
pub(in crate::ui) mod recent;
mod snapshot;
#[cfg(test)]
mod tests;
use super::DawApp;
use gpui::{AppContext, Context};
use snapshot::Snapshot;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct ProjectUi {
    pub path: Option<PathBuf>,
    pub dirty: bool,
    saved: Option<String>,
    document: Option<Vec<u8>>,
    checked: Option<Instant>,
    task: Option<gpui::Task<()>>,
    dialog: Option<gpui::Task<()>>,
    pub autosave_last: Option<Instant>,
    pub autosave_hash: u64,
    pub autosave_label: Option<String>,
    pub autosave_task: Option<gpui::Task<()>>,
    pub recovery_available: Option<(PathBuf, String)>,
    pub recovery_dismissed: bool,
    pub recent: Vec<PathBuf>,
    #[cfg(feature = "p2p")]
    shared: Option<live::SharedProject>,
    #[cfg(feature = "p2p")]
    alias: Option<String>,
    #[cfg(feature = "p2p")]
    identity_dialog: Option<live::IdentityDialog>,
    #[cfg(feature = "p2p")]
    share_invite: Option<live::ShareInvite>,
}

impl ProjectUi {
    pub fn new() -> Self {
        Self {
            recovery_available: autosave::detect_recovery(),
            recent: recent::load(),
            ..Default::default()
        }
    }
}

impl DawApp {
    fn session_snapshot(&self) -> Result<Snapshot, String> {
        let mut tracks = self.tracks.clone();
        let mut master_chain = self.master_chain.clone();
        for (id, effect) in &self.vst_effects.instances {
            if master_chain.find(*id).is_some() {
                master_chain.set_vst_preset(*id, effect.editor.project_preset()?);
            }
        }
        for track in &mut tracks {
            track.midi_enabled = false;
            for (id, effect) in &self.vst_effects.instances {
                if track.chain.find(*id).is_some() {
                    track
                        .chain
                        .set_vst_preset(*id, effect.editor.project_preset()?);
                }
            }
        }
        let mut midi: Vec<_> = self
            .timeline
            .midi_clip
            .iter()
            .chain(&self.timeline.other_midi_clips)
            .cloned()
            .collect();
        midi.sort_by_key(|c| c.id.value());
        let mut audio = self
            .timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
            .map(|c| c.project_metadata(self.tracks.get(c.track).ok_or("Audio sin pista")?.id))
            .collect::<Result<Vec<_>, String>>()?;
        audio.sort_by_key(|c| c.id.value());
        let mut pianos = Vec::new();
        for id in tracks.iter().filter_map(|t| t.instrument) {
            if pianos
                .iter()
                .any(|p: &elevate::project::ProjectPiano| p.id == id)
            {
                continue;
            }
            if id == self.instrument.id() {
                pianos.push(self.instrument.project_state());
            } else if let Some(piano) = self.piano_instances.get(&id) {
                pianos.push(piano.project_state());
            }
        }
        pianos.sort_by_key(|piano| piano.id.0);
        let mut samplers = Vec::new();
        for id in tracks
            .iter()
            .filter_map(|t| t.instrument)
            .filter(|id| self.is_sampler_id(*id))
        {
            if let Ok(snapshot) = self.sampler_snapshot(id) {
                samplers.push(elevate::project::ProjectSampler {
                    id,
                    source: snapshot
                        .asset
                        .source
                        .to_str()
                        .ok_or("Ruta de sample no UTF-8")?
                        .into(),
                    params: snapshot.params,
                });
            }
        }
        samplers.sort_by_key(|sampler| sampler.id.0);
        samplers.dedup_by_key(|sampler| sampler.id);
        let mut synths = Vec::new();
        for id in tracks
            .iter()
            .filter_map(|t| t.instrument)
            .filter(|id| self.is_synth_id(*id))
        {
            synths.push(elevate::project::ProjectSynth {
                id,
                params: self.synth_snapshot(id),
            });
        }
        synths.sort_by_key(|synth| synth.id.0);
        synths.dedup_by_key(|synth| synth.id);
        let mut vsts = Vec::new();
        for id in tracks
            .iter()
            .filter_map(|t| t.instrument)
            .filter(|id| self.is_vst_id(*id))
        {
            if vsts
                .iter()
                .any(|v: &elevate::project::ProjectVstInstrument| v.id == id)
            {
                continue;
            }
            let inst = if id == self.vst.id {
                Some(&self.vst)
            } else {
                self.vst_instances.get(&id)
            };
            if let Some(inst) = inst
                && inst.ready()
                && let Some(editor) = inst.editor()
            {
                let preset = editor.project_preset()?;
                let bundle = inst
                    .bundle
                    .as_ref()
                    .and_then(|p| p.to_str())
                    .unwrap_or_default()
                    .to_owned();
                vsts.push(elevate::project::ProjectVstInstrument {
                    id,
                    name: inst.name.clone(),
                    class_id: inst.class.clone().unwrap_or_default(),
                    bundle,
                    preset,
                });
            }
        }
        #[cfg(feature = "p2p")]
        if let Some(shared) = &self.project.shared {
            for preserved in shared.preserved_vsts() {
                let in_use = tracks.iter().any(|t| t.instrument == Some(preserved.id));
                if in_use && !vsts.iter().any(|v| v.id == preserved.id) {
                    vsts.push(preserved.clone());
                }
            }
        }
        vsts.sort_by_key(|v| v.id.0);
        let mut automation = self.timeline.automation.clone();
        let valid_ids: Vec<_> = tracks.iter().map(|t| t.id).collect();
        automation.retain_tracks(&valid_ids);
        Ok(Snapshot {
            tracks,
            midi,
            audio,
            pianos,
            samplers,
            synths,
            vsts,
            drums: self.pattern_editor.session.clone(),
            tempo: self.timeline.tempo,
            meter: self.timeline.meter,
            utility: self.utility.state(),
            master_chain,
            navigation: self.timeline.navigation.clone(),
            automation,
            midi_bindings: self.midi_mapper.bindings().to_vec(),
            levels: self.track_levels.clone(),
            pans: self.track_pans.clone(),
        })
    }

    fn session_signature(&self) -> Result<String, String> {
        Ok(self.session_snapshot()?.signature())
    }

    pub(super) fn refresh_project_dirty(&mut self) {
        if self
            .project
            .checked
            .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
        {
            return;
        }
        self.project.checked = Some(Instant::now());
        let signature = self.session_signature();
        if self.project.saved.is_none() {
            self.project.saved = signature.as_ref().ok().cloned();
        }
        self.project.dirty = signature.is_err()
            || signature.ok().as_ref() != self.project.saved.as_ref()
            || self
                .session_snapshot()
                .is_ok_and(|s| s.validate_support().is_err());
    }

    fn project_busy(&self) -> bool {
        self.project.task.is_some()
            || self.timeline.sample_editor.job.is_some()
            || self.import_task.is_some()
            || self.collect_task.is_some()
            || self.script_task.is_some()
            || self.vst_task.is_some()
            || self.sampler_ui.busy
            || self.vst_effects.task.is_some()
            || self.is_any_recording()
    }

    fn save_project_to(&mut self, path: PathBuf, from_picker: bool, cx: &mut Context<Self>) {
        if self.project_busy() {
            self.status_message = "Espera a que termine la operación en curso".into();
            return;
        }
        let result = (|| {
            let snapshot = self.session_snapshot()?;
            snapshot.validate_support()?;
            let signature = self.session_signature()?;
            Ok::<_, String>((snapshot, signature))
        })();
        let (snapshot, signature) = match result {
            Ok(s) => s,
            Err(e) => {
                self.status_message = e;
                cx.notify();
                return;
            }
        };
        #[cfg(feature = "p2p")]
        let shared = match self.shared_document_for_save(&snapshot) {
            Ok(shared) => shared,
            Err(error) => {
                self.status_message = error;
                cx.notify();
                return;
            }
        };
        #[cfg(not(feature = "p2p"))]
        let shared = None;
        let previous = shared
            .or_else(|| self.history_doc_bytes())
            .or_else(|| self.project.document.clone());
        self.status_message = "Guardando proyecto…".into();
        let job = cx.background_spawn(async move {
            let mut snapshot = snapshot;
            let file = if from_picker {
                location::project_file_path(&path)?
            } else {
                path
            };
            snapshot.resolve_piano_sources()?;
            let name = file.file_stem().unwrap_or_default().to_string_lossy();
            let root = file.parent().ok_or("Proyecto sin carpeta")?;
            snapshot.relativize_sources(root)?;
            let doc = snapshot.document_from(&name, previous.as_deref())?;
            doc.commit_change("Proyecto guardado", crate::ui::history::unix_now_secs());
            let bytes = doc.snapshot()?;
            doc.save(&file)
                .map_err(|error| format!("{}: {error}", file.display()))?;
            Ok::<_, String>((file, bytes))
        });
        self.project.task = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.project.task = None;
                match result {
                    Ok((path, document)) => {
                        this.status_message = format!("Proyecto guardado en {}", path.display());
                        this.project.recent = recent::record(&path);
                        this.project.path = Some(path);
                        this.sync_loro_history_bytes(&document, "Proyecto guardado");
                        this.project.document = Some(document);
                        this.project.saved = Some(signature);
                        this.project.checked = None;
                        this.refresh_project_dirty();
                        autosave::clear_autosave_file();
                    }
                    Err(e) => this.status_message = format!("No se pudo guardar: {e}"),
                }
                cx.notify();
            });
        }));
    }

    pub(in crate::ui) fn open_project_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.project_busy() {
            self.status_message = "Espera a que termine la operación en curso".into();
            return;
        }
        let before = self.session_signature();
        let target = path.clone();
        self.status_message = "Preparando proyecto…".into();
        let config = self.settings.audio.saved.clone();
        let registry = self.lua_components.registry.clone();
        let rate = config.sample_rate.unwrap_or(48_000);
        let job = cx.background_spawn(async move {
            let (snapshot, audio, document) = Snapshot::load(&target)?;
            let effects =
                effects::prepare(&snapshot.tracks, &[], &snapshot.master_chain, None, &config)?;
            let components = effects::prepare_components(
                &snapshot.tracks,
                &snapshot.master_chain,
                &registry,
                rate,
            )?;
            let samplers = snapshot.prepare_samplers(&[])?;
            let vsts = snapshot.prepare_vsts(&config)?;
            Ok::<_, String>((
                snapshot, audio, document, effects, components, samplers, vsts,
            ))
        });
        self.project.task = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.project.task = None;
                if this.session_signature() != before || this.project_busy() {
                    this.status_message =
                        "Apertura cancelada: la sesión cambió durante la carga".into();
                } else {
                    this.status_message = match result {
                        Ok((snapshot, audio, document, effects, components, samplers, vsts)) => {
                            this.install_project(
                                snapshot, audio, path, document, effects, components, samplers,
                                vsts,
                            )
                            .map(|()| "Proyecto abierto · Arrangement restaurado".into())
                            .unwrap_or_else(|e| e)
                        }
                        Err(e) => {
                            format!("No se abrió el proyecto: {e}. La sesión actual se conserva")
                        }
                    };
                }
                cx.notify();
            });
        }));
    }
}
