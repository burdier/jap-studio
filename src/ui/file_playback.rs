//! Preparación de mezcla cancelable; fuentes y remuestreo fuera del callback.
use super::DawApp;
use crate::audio::{AudioClip, PreparedPlayback, TrackInput};
use std::sync::mpsc::{self, Receiver, TryRecvError};
pub(super) struct PendingPlayback {
    result: Receiver<Result<PreparedPlayback, String>>,
    beat: f64,
    bpm: f64,
    clips: Vec<AudioClip>,
    revision: u64,
    audio: crate::audio::device::AudioConfig,
}
impl DawApp {
    fn audio_snapshot(&self) -> Vec<AudioClip> {
        self.timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
            .cloned()
            .collect()
    }
    pub(super) fn request_file_playback(&mut self, beat: f64) {
        self.sync_vst_effects();
        let clips = self.audio_snapshot();
        let bpm = self.timeline.tempo.bpm();
        let mut inputs = Vec::new();
        for (index, track) in self.tracks.iter().enumerate() {
            let sources: Vec<_> = clips
                .iter()
                .filter(|c| c.track == index && !c.props.muted && c.end_beat(bpm) > beat)
                .cloned()
                .collect();
            if sources.is_empty() {
                continue;
            }
            inputs.push(TrackInput {
                id: track.id,
                clips: sources,
                gain: self.track_gain(index),
                pan: self.track_pan(index),
                fx_gain: self.track_fx_gain(index),
                rack: self.effect_rack(index),
            });
        }
        if inputs.is_empty() {
            self.status_message = "No hay clips de audio después del cursor".into();
            return;
        }
        let (tx, rx) = mpsc::sync_channel(1);
        let audio = self.settings.audio.saved.clone();
        self.file_playback = Some(PendingPlayback {
            result: rx,
            beat,
            bpm,
            clips,
            revision: self.track_revision,
            audio: audio.clone(),
        });
        self.status_message = "Preparando mezcla de clips…".into();
        std::thread::spawn(move || {
            let _ = tx.send(PreparedPlayback::load(inputs, bpm, audio));
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
                Err("La preparación de la mezcla se interrumpió".into())
            }
        };
        let pending = self.file_playback.take().unwrap();
        if pending.revision != self.track_revision
            || pending.audio != self.settings.audio.saved
            || pending.bpm != self.timeline.tempo.bpm()
            || !same_sources(&pending.clips, &self.audio_snapshot())
        {
            self.status_message =
                "Preparación cancelada: cambió la mezcla o la salida; pulsa Play de nuevo".into();
            return;
        }
        match result
            .and_then(|prepared| prepared.start(pending.beat, pending.bpm, self.utility.clone()))
        {
            Ok(player) => {
                self.player = Some(player);
                self.status_message =
                    "Reproduciendo mezcla de audio · una salida compartida".into();
            }
            Err(error) => self.status_message = format!("Salida de audio: {error}"),
        }
    }
}
fn same_sources(a: &[AudioClip], b: &[AudioClip]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            std::sync::Arc::ptr_eq(&a.file, &b.file)
                && a.track == b.track
                && a.start_beat == b.start_beat
                && a.trim_start == b.trim_start
                && a.trim_len == b.trim_len
                && a.props == b.props
        })
}
