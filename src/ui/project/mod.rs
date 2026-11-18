//! Gestión local de proyectos: guardar, preparar candidato y publicar sin perder la sesión actual.
mod dialogs;
mod install;
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
}

impl DawApp {
    fn session_snapshot(&self) -> Result<Snapshot, String> {
        let mut tracks = self.tracks.clone();
        for track in &mut tracks {
            track.midi_enabled = false;
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
        Ok(Snapshot {
            tracks,
            midi,
            audio,
            pianos,
            tempo: self.timeline.tempo,
            meter: self.timeline.meter,
            levels: self.track_levels.clone(),
            pans: self.track_pans.clone(),
        })
    }

    fn session_signature(&self) -> Result<String, String> {
        // Incluye el estado excluido para advertir antes de abandonar ediciones.
        Ok(format!(
            "{}|{:?}|{:?}",
            self.session_snapshot()?.signature(),
            self.pattern_editor.pattern,
            (self.utility.state(), self.midi_mapper.bindings())
        ))
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
            || self.import_task.is_some()
            || self.collect_task.is_some()
            || self.script_task.is_some()
            || self.vst_task.is_some()
            || self.sampler_ui.busy
            || self.vst_effects.task.is_some()
            || self.midi_capture.is_recording()
    }

    fn save_project_to(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.project_busy() {
            self.status_message = "Espera a que termine la operación en curso".into();
            return;
        }
        let result = (|| {
            if self.pattern_editor.pattern != super::pattern_editor::PatternEditor::new().pattern
                || self.utility.state() != Default::default()
                || self.midi_mapper.bindings()
                    != elevate::musical::midi_map::MidiMapper::new().bindings()
            {
                return Err("No se puede guardar aún el estado de Pattern, Utility de salida o MIDI Learn modificado. El archivo anterior se conserva".into());
            }
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
        let target = path.clone();
        let previous = self.project.document.clone();
        self.status_message = "Guardando proyecto…".into();
        let job = cx.background_spawn(async move {
            let name = target.file_stem().unwrap_or_default().to_string_lossy();
            let mut snapshot = snapshot;
            snapshot.resolve_piano_sources()?;
            let root = target.parent().ok_or("Proyecto sin carpeta")?;
            for clip in &mut snapshot.audio {
                if let Ok(relative) = std::path::Path::new(&clip.source).strip_prefix(root) {
                    clip.source = relative.to_str().ok_or("Ruta no UTF-8")?.into();
                }
            }
            let doc = snapshot.document_from(&name, previous.as_deref())?;
            doc.save(&target)?;
            doc.snapshot()
        });
        self.project.task = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.project.task = None;
                match result {
                    Ok(document) => {
                        this.project.document = Some(document);
                        this.project.path = Some(path);
                        this.project.saved = Some(signature);
                        this.project.checked = None;
                        this.refresh_project_dirty();
                        this.status_message = "Proyecto guardado".into();
                    }
                    Err(e) => this.status_message = format!("No se pudo guardar: {e}"),
                }
                cx.notify();
            });
        }));
    }

    fn open_project_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.project_busy() {
            self.status_message = "Espera a que termine la operación en curso".into();
            return;
        }
        let before = self.session_signature();
        let target = path.clone();
        self.status_message = "Preparando proyecto…".into();
        let job = cx.background_spawn(async move { Snapshot::load(&target) });
        self.project.task = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.project.task = None;
                if this.session_signature() != before || this.project_busy() {
                    this.status_message =
                        "Apertura cancelada: la sesión cambió durante la carga".into();
                } else {
                    this.status_message = match result {
                        Ok((snapshot, audio, document)) => this
                            .install_project(snapshot, audio, path, document)
                            .map(|()| "Proyecto abierto · Arrangement restaurado".into())
                            .unwrap_or_else(|e| e),
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
