//! Preescucha de una sola muestra. Una selección nueva cancela cualquier carga anterior.
use super::Explorer;
use crate::audio::{
    device::AudioConfig,
    sample_preview::{PreparedSamplePreview, SamplePreview},
};
use elevate::explorer::EntryKind;
use gpui::{Context, Task};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) struct Preview {
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
        self.preview.pending = None;
        self.preview.stream = None;
        self.preview.path = None;
    }

    pub(super) fn toggle_sample_preview(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.preview.path.as_ref() == Some(&path) {
            self.stop_preview();
            cx.notify();
        } else {
            self.play_preview(path, false, cx);
        }
    }

    pub(super) fn preview_selection(&mut self, audio: Option<PathBuf>, cx: &mut Context<Self>) {
        if !self.preview.automatic {
            return;
        }
        match audio {
            Some(path) if self.preview.path.as_ref() != Some(&path) => {
                self.play_preview(path, true, cx)
            }
            None => {
                self.stop_preview();
                cx.notify();
            }
            _ => {}
        }
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

    fn play_preview(&mut self, path: PathBuf, automatic: bool, cx: &mut Context<Self>) {
        self.stop_preview();
        self.status = None;
        self.preview.path = Some(path.clone());
        let audio = self.preview.audio.clone();
        let executor = cx.background_executor().clone();
        self.preview.pending = Some(cx.spawn(async move |this, cx| {
            if automatic {
                // Deja que las flechas recorran varias filas antes de decodificar.
                executor.timer(Duration::from_millis(90)).await;
            }
            let result = executor
                .spawn(async move { PreparedSamplePreview::load(&path, &audio) })
                .await;
            let running = this
                .update(cx, |this, cx| {
                    match result.and_then(|preview| preview.start(this.preview.level)) {
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
