//! Coordinación UI → adaptadores de audio; decodificación en background.
use super::{CenterView, DawApp};
use crate::audio::{AudioClip, AudioFile, BEATS_PER_SECOND, DrumPlayer, Player};
use elevate::musical::{pitch::note_name, time::DEFAULT_BPM};
use gpui::{AppContext, Context};
use std::{path::PathBuf, sync::Arc};

impl DawApp {
    pub(super) fn note_on(&mut self, note: u8) {
        if note > 127 || self.instrument.held(note) {
            return;
        }
        if !self.instrument.is_running() {
            self.stop_audio();
            self.stop_pattern();
        }
        match self.instrument.note_on(note, 100, self.gain, self.bypass) {
            Ok(true) => {
                self.status_message = format!(
                    "Note On {} · {}",
                    note_name(note),
                    self.instrument.descriptor().name
                );
            }
            Ok(false) => {}
            Err(error) => {
                self.status_message = format!(
                    "No se pudo iniciar {}: {error}",
                    self.instrument.descriptor().name
                );
            }
        }
    }
    pub(super) fn note_off(&mut self, note: u8) {
        if !self.instrument.note_off(note) {
            return;
        }
        self.status_message = format!(
            "Note Off {} · {}",
            note_name(note),
            self.instrument.descriptor().name
        );
    }
    pub(super) fn release_mouse_note(&mut self) {
        if let Some(note) = self.instrument.take_mouse_note() {
            self.note_off(note);
        }
    }
    pub(super) fn all_notes_off(&mut self) {
        if self.instrument.all_notes_off() {
            self.status_message = "Todas las notas liberadas".into();
        }
    }
    pub(super) fn stop_instrument(&mut self) {
        self.instrument.stop();
    }
    /// Audición en loop del patrón. El plan se compila al pulsar Play.
    pub(super) fn play_pattern(&mut self) {
        self.stop_audio();
        self.stop_instrument();
        self.stop_pattern();
        match DrumPlayer::start(
            &self.pattern_editor.pattern,
            DEFAULT_BPM,
            self.gain,
            self.bypass,
        ) {
            Ok(player) => {
                let events = player.event_count();
                let rate = player.sample_rate;
                let loop_beats = player.loop_beats();
                self.pattern_player = Some(player);
                self.status_message = format!(
                    "Patrón en loop ({loop_beats:.2} negras) · {events} eventos · {rate} Hz · percusión sintetizada"
                );
            }
            Err(error) => self.status_message = format!("Salida de percusión: {error}"),
        }
    }
    pub(super) fn stop_pattern(&mut self) {
        self.pattern_player = None;
    }
    pub(super) fn play_context(&mut self) {
        if self.center_view == CenterView::Session {
            self.play_pattern();
        } else {
            self.play_audio();
        }
    }
    pub(super) fn stop_all(&mut self) {
        self.stop_audio();
        self.stop_pattern();
        self.stop_instrument();
    }
    pub(super) fn import_audio(
        &mut self,
        path: PathBuf,
        track: usize,
        start_beat: f64,
        cx: &mut Context<Self>,
    ) {
        self.stop_audio();
        self.stop_pattern();
        self.status_message = format!("Decodificando {}…", path.display());
        let decode = cx.background_spawn(async move { AudioFile::decode(&path) });
        self.import_task = Some(cx.spawn(async move |this, cx| {
            let result = decode.await;
            if let Err(error) = this.update(cx, |this, cx| {
                match result {
                    Ok(file) => {
                        this.stop_audio();
                        this.status_message = format!(
                            "{} · {:.2}s · {} Hz · Space: play · [/]: Volume",
                            file.name,
                            file.duration(),
                            file.sample_rate
                        );
                        this.timeline.clip = Some(AudioClip {
                            file: Arc::new(file),
                            track,
                            start_beat,
                        });
                        this.cursor_row = track;
                        this.timeline.set_edit_beat(start_beat);
                    }
                    Err(error) => this.status_message = format!("No se pudo importar: {error}"),
                }
                cx.notify();
            }) {
                eprintln!("Importación finalizada tras cerrar la vista: {error}");
            }
        }));
        cx.notify();
    }
    pub(super) fn play_audio(&mut self) {
        self.stop_instrument();
        self.stop_pattern();
        self.stop_audio();
        let Some(clip) = self.timeline.clip.as_ref() else {
            self.status_message = "Arrastra un archivo de audio a un carril".into();
            return;
        };
        if self.timeline.edit_beat() >= clip.start_beat + clip.file.duration() * BEATS_PER_SECOND {
            self.status_message = "Coloca el cursor antes del final del clip".into();
            return;
        }
        match Player::start(clip, self.timeline.edit_beat(), self.gain, self.bypass) {
            Ok(player) => {
                self.player = Some(player);
                self.status_message = "Reproduciendo · archivo → Volume → CPAL".into();
            }
            Err(error) => self.status_message = format!("Salida de audio: {error}"),
        }
    }
    pub(super) fn stop_audio(&mut self) {
        self.player = None;
        self.timeline.playhead = None;
    }
    pub(super) fn change_gain(&mut self, delta: f32) {
        self.gain = (self.gain + delta).clamp(0.0, 2.0);
        if let Some(player) = &self.player {
            player.set_gain(self.gain);
        }
        if let Some(player) = &self.pattern_player {
            player.set_gain(self.gain);
        }
        self.instrument.set_gain(self.gain);
        self.status_message = format!("Volume: {:.2}x", self.gain);
    }
    pub(super) fn toggle_bypass(&mut self) {
        self.bypass = !self.bypass;
        if let Some(player) = &self.player {
            player.set_bypass(self.bypass);
        }
        if let Some(player) = &self.pattern_player {
            player.set_bypass(self.bypass);
        }
        self.instrument.set_bypass(self.bypass);
    }
}
