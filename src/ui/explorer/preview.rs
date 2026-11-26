//! Preescucha de una sola muestra. Una selección nueva cancela cualquier carga anterior.
use super::Explorer;
use crate::audio::{
    AudioFile,
    device::AudioConfig,
    fundamental,
    sample_analysis::{SampleProfile, analyze_path},
    sample_preview::{PreparedSamplePreview, SamplePreview},
};
use elevate::explorer::EntryKind;
use elevate::musical::{clip::MidiClip, track::Track};
use gpui::{Context, Task};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Copy)]
pub(super) struct Root {
    pub detected: Option<(u8, i32)>,
    pub chosen: (u8, i32),
}

pub(super) struct Preview {
    pub root: Option<(PathBuf, Root)>,
    pub profile: Option<(PathBuf, SampleProfile)>,
    pub profile_loading: bool,
    pub phrase: bool,
    pub analysing: Option<Task<()>>,
    profile_task: Option<Task<()>>,
    pub automatic: bool,
    pub level: u8,
    pub path: Option<PathBuf>,
    audio: AudioConfig,
    pending: Option<Task<()>>,
    stream: Option<SamplePreview>,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            root: None,
            profile: None,
            profile_loading: false,
            phrase: false,
            analysing: None,
            profile_task: None,
            automatic: false,
            level: 50,
            path: None,
            audio: AudioConfig::default(),
            pending: None,
            stream: None,
        }
    }
}

impl Explorer {
    pub(in crate::ui) fn preview_needs_sync(&self, audio: &AudioConfig, visible: bool) -> bool {
        &self.preview.audio != audio || (!visible && self.preview.path.is_some())
    }

    pub(in crate::ui) fn sync_preview(&mut self, audio: &AudioConfig, visible: bool) {
        if self.preview_needs_sync(audio, visible) {
            self.stop_preview();
            self.preview.audio = audio.clone();
        }
    }

    pub(super) fn stop_preview(&mut self) {
        self.preview.phrase = false;
        self.preview.pending = None;
        self.preview.stream = None;
        self.preview.path = None;
    }

    pub(super) fn toggle_sample_preview(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.preview.path.as_ref() == Some(&path) && !self.preview.phrase {
            self.stop_preview();
            cx.notify();
        } else {
            self.play_preview(path, false, None, cx);
        }
    }

    pub(super) fn restart_sample_preview(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.preview.path.as_ref() == Some(&path) && !self.preview.phrase {
            match &self.preview.stream {
                Some(stream) if stream.finished().is_ok() => {
                    stream.rewind();
                    return;
                }
                None => return, // La carga en curso ya arrancará desde el inicio.
                Some(_) => {}
            }
        }
        self.play_preview(path, false, None, cx);
    }

    pub(super) fn preview_selection(&mut self, audio: Option<PathBuf>, cx: &mut Context<Self>) {
        self.schedule_profile_analysis(audio.clone(), cx);
        if self.preview.phrase && self.preview.path != audio {
            self.stop_preview();
        }
        if !self.preview.automatic {
            return;
        }
        match audio {
            Some(path) if self.preview.path.as_ref() != Some(&path) => {
                self.play_preview(path, true, None, cx)
            }
            None => {
                self.stop_preview();
                cx.notify();
            }
            _ => {}
        }
    }

    pub(super) fn selected_profile(&self, path: &Path) -> Option<&SampleProfile> {
        self.preview
            .profile
            .as_ref()
            .filter(|(p, _)| p == path)
            .map(|(_, profile)| profile)
    }

    pub(super) fn schedule_profile_analysis(
        &mut self,
        audio: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = audio else {
            self.preview.profile_task = None;
            self.preview.profile_loading = false;
            return;
        };
        if self.selected_profile(&path).is_some() {
            self.preview.profile_loading = false;
            return;
        }
        self.preview.profile_loading = true;
        let executor = cx.background_executor().clone();
        self.preview.profile_task = Some(cx.spawn(async move |this, cx| {
            // Pequeña espera para no decodificar filas intermedias al recorrer con flechas.
            executor.timer(Duration::from_millis(65)).await;
            let work_path = path.clone();
            let result = executor
                .spawn(async move { analyze_path(&work_path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.selected_file().as_ref() == Some(&path) {
                    this.preview.profile_loading = false;
                    if let Ok(profile) = result {
                        let detected = profile
                            .pitch_midi
                            .map(|m| (m, profile.pitch_cents.unwrap_or(0)));
                        this.set_detected_root(&path, detected);
                        this.preview.profile = Some((path, profile));
                    }
                    cx.notify();
                }
            });
        }));
    }

    pub(super) fn toggle_auto_preview(&mut self, cx: &mut Context<Self>) {
        self.preview.automatic = !self.preview.automatic;
        let audio = self
            .selected_file()
            .filter(|path| EntryKind::for_file(path) == EntryKind::Audio);
        self.preview_selection(audio, cx);
        cx.notify();
    }

    pub(super) fn change_preview_level(&mut self, direction: i16, cx: &mut Context<Self>) {
        self.preview.level = (i16::from(self.preview.level) + direction).clamp(0, 100) as u8;
        if let Some(stream) = &self.preview.stream {
            stream.set_level(self.preview.level);
        }
        cx.notify();
    }

    pub(super) fn selected_root(&self, path: &Path) -> Option<Root> {
        self.preview
            .root
            .as_ref()
            .filter(|(p, _)| p == path)
            .map(|(_, root)| *root)
    }

    fn set_detected_root(&mut self, path: &Path, detected: Option<(u8, i32)>) {
        let selected = self.selected_file().as_deref() == Some(path);
        if selected && self.selected_root(path).is_none() {
            self.preview.root = Some((
                path.to_owned(),
                Root {
                    detected,
                    chosen: detected.unwrap_or((60, 0)),
                },
            ));
        }
    }

    pub(super) fn detect_root(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.preview.root = None;
        self.status = None;
        let work_path = path.clone();
        let job = cx.background_executor().spawn(async move {
            AudioFile::decode(&work_path)
                .map(|file| fundamental::stable_root(&file.frames, file.sample_rate))
        });
        cx.notify();
        self.preview.analysing = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                if this.selected_file().as_ref() == Some(&path) {
                    match result {
                        Ok(root) => this.set_detected_root(&path, root),
                        Err(error) => this.status = Some(format!("Nota raíz: {error}")),
                    }
                }
                this.preview.analysing = None;
                cx.notify();
            });
        }));
    }

    pub(super) fn step_root(&mut self, path: PathBuf, step: i16, cx: &mut Context<Self>) {
        let mut root = self.selected_root(&path).unwrap_or(Root {
            detected: None,
            chosen: (60, 0),
        });
        root.chosen = ((i16::from(root.chosen.0) + step).clamp(0, 127) as u8, 0);
        self.preview.root = Some((path, root));
        self.stop_preview();
        cx.notify();
    }

    pub(in crate::ui) fn preview_phrase(
        &mut self,
        path: PathBuf,
        clip: MidiClip,
        track: Track,
        bpm: f64,
        tuning: (u8, i32),
        cx: &mut Context<Self>,
    ) {
        if self.preview.phrase && self.preview.path.as_ref() == Some(&path) {
            self.stop_preview();
            cx.notify();
        } else {
            self.play_preview(path, false, Some((clip, track, bpm, tuning)), cx);
        }
    }

    fn play_preview(
        &mut self,
        path: PathBuf,
        automatic: bool,
        phrase: Option<(MidiClip, Track, f64, (u8, i32))>,
        cx: &mut Context<Self>,
    ) {
        self.stop_preview();
        self.status = None;
        self.preview.path = Some(path.clone());
        self.preview.phrase = phrase.is_some();
        let audio = self.preview.audio.clone();
        let executor = cx.background_executor().clone();
        self.preview.pending = Some(cx.spawn(async move |this, cx| {
            if automatic {
                // Deja que las flechas recorran varias filas antes de decodificar.
                executor.timer(Duration::from_millis(90)).await;
            }
            let source = path.clone();
            let is_phrase = phrase.is_some();
            let result = executor
                .spawn(async move {
                    if let Some((clip, track, bpm, (root, cents))) = phrase {
                        PreparedSamplePreview::phrase(
                            &source, clip, track, bpm, root, cents, &audio,
                        )
                        .map(|prepared| (prepared, None))
                    } else {
                        PreparedSamplePreview::load(&source, &audio)
                    }
                })
                .await;
            let running = this
                .update(cx, |this, cx| {
                    match result.and_then(|(preview, root)| {
                        if !is_phrase {
                            this.set_detected_root(&path, root);
                        }
                        preview.start(this.preview.level)
                    }) {
                        Ok(stream) => {
                            this.preview.stream = Some(stream);
                            cx.notify();
                            true
                        }
                        Err(error) => {
                            this.stop_preview();
                            this.status = Some(format!("Preescucha: {error}"));
                            cx.notify();
                            false
                        }
                    }
                })
                .unwrap_or(false);
            if !running {
                return;
            }
            // El callback determina el final. Este sondeo solo actualiza el botón.
            loop {
                executor.timer(Duration::from_millis(40)).await;
                let active = this
                    .update(cx, |this, cx| {
                        let Some(stream) = &this.preview.stream else {
                            return false;
                        };
                        match stream.finished() {
                            Ok(false) => true,
                            result => {
                                if let Err(error) = result {
                                    this.status = Some(error);
                                }
                                this.stop_preview();
                                cx.notify();
                                false
                            }
                        }
                    })
                    .unwrap_or(false);
                if !active {
                    break;
                }
            }
        }));
        cx.notify();
    }

    pub(super) fn stop_preview_in(&mut self, folder: &Path) {
        if self
            .preview
            .path
            .as_ref()
            .is_some_and(|path| path.starts_with(folder))
        {
            self.stop_preview();
        }
    }
}
