//! Preparación cancelable: remuestreo SoXR fuera de UI; Stop cancela el arranque.
use super::DawApp;
use crate::audio::{AudioClip, PreparedPlayback};
use elevate::devices::utility_slot::UtilitySlot;
use std::sync::mpsc::{self, Receiver, TryRecvError};
pub(super) struct PendingPlayback {
    result: Receiver<Result<PreparedPlayback, String>>,
    beat: f64,
    bpm: f64,
    clip: AudioClip,
    track: elevate::musical::track::TrackId,
    utility: UtilitySlot,
}
impl DawApp {
    pub(super) fn request_file_playback(&mut self, clip: AudioClip, beat: f64) {
        let (tx, rx) = mpsc::sync_channel(1);
        let audio = self.settings.audio.saved.clone();
        self.file_playback = Some(PendingPlayback {
            result: rx,
            beat,
            bpm: self.timeline.tempo.bpm(),
            track: self.tracks[clip.track].id,
            clip: clip.clone(),
            utility: self.utility.clone(),
        });
        self.status_message = "Preparando salida y audio…".into();
        std::thread::spawn(move || {
            let _ = tx.send(PreparedPlayback::load(clip, audio));
        });
    }
    pub(super) fn poll_file_playback(&mut self) {
        let Some(pending) = self.file_playback.as_ref() else {
            return;
        };
        let result = match pending.result.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                Err("La preparación del audio se interrumpió".into())
            }
        };
        let pending = self.file_playback.take().unwrap();
        // La fuente activa pudo cambiar mientras se remuestreaba. No iniciar una
        // selección antigua ni recuperar valores de mezcla anteriores a Mute/Solo.
        let Some(current) = self.timeline.clip.as_ref().filter(|current| {
            std::sync::Arc::ptr_eq(&current.file, &pending.clip.file)
                && current.start_beat == pending.clip.start_beat
                && current.trim_start == pending.clip.trim_start
                && current.trim_len == pending.clip.trim_len
                && self
                    .tracks
                    .get(current.track)
                    .is_some_and(|t| t.id == pending.track)
                && !current.props.muted
        }) else {
            self.status_message = "Preparación cancelada: el clip cambió".into();
            return;
        };
        let gain = (self.track_gain(current.track) * current.props.gain_linear()).clamp(0., 2.);
        let pan = self.track_pan(current.track);
        let fx = self.track_fx_gain(current.track);
        match result.and_then(|prepared| {
            prepared.with_rack(self.effect_rack(current.track)).start(
                pending.beat,
                pending.bpm,
                gain,
                pending.utility,
                pan,
                fx,
            )
        }) {
            Ok(player) => {
                self.player = Some(player);
                self.status_message = "Reproduciendo · salida de audio seleccionada".into();
            }
            Err(error) => self.status_message = format!("Salida de audio: {error}"),
        }
    }
}
