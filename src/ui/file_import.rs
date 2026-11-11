//! Importación común a drop, explorador y comandos; lectura/decodificación fuera de la UI.
use super::DawApp;
use crate::audio::{AudioClip, AudioFile};
use elevate::{
    explorer::EntryKind,
    musical::{clip::MidiClip, smf::parse_smf_to_clip_with_meter, track::assign_instrument},
};
use gpui::{AppContext, Context};
use std::{path::PathBuf, sync::Arc};

enum ImportedFile {
    Midi(MidiClip),
    Audio(AudioFile),
}

impl DawApp {
    pub(super) fn import_audio(
        &mut self,
        path: PathBuf,
        track: usize,
        start_beat: f64,
        cx: &mut Context<Self>,
    ) {
        if !self.track_accepts_clips(track) {
            self.status_message = "Selecciona una pista de audio/MIDI para importar".into();
            return;
        }
        if self.import_task.is_some() {
            self.status_message = "Hay una importación en curso; espera a que termine".into();
            return;
        }
        let target_id = self.tracks[track].id;
        let instrument_id = self.tracks[track]
            .instrument
            .unwrap_or(self.instrument.id());
        let meter = self.timeline.meter;
        self.stop_audio();
        self.stop_pattern();
        self.status_message = format!("Importando {}…", path.display());
        let decode = cx.background_spawn(async move {
            if EntryKind::for_file(&path) == EntryKind::Midi {
                let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
                let name = path
                    .file_stem()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
                    .into_owned();
                parse_smf_to_clip_with_meter(&bytes, name, track, start_beat, instrument_id, meter)
                    .map(ImportedFile::Midi)
                    .map_err(|error| error.to_string())
            } else {
                AudioFile::decode(&path).map(ImportedFile::Audio)
            }
        });
        self.import_task = Some(cx.spawn(async move |this, cx| {
            let result = decode.await;
            let _ = this.update(cx, |this, cx| {
                this.import_task = None;
                let Some(track) = this.tracks.iter().position(|track| track.id == target_id) else {
                    this.status_message =
                        "Importación descartada: la pista de destino fue eliminada".into();
                    cx.notify();
                    return;
                };
                match result {
                    Ok(file) => {
                        this.stop_audio();
                        this.stop_pattern();
                        match file {
                            ImportedFile::Midi(mut clip) => {
                                clip.track = track;
                                this.status_message = format!(
                                    "MIDI en {} · {} notas · {:.1} negras",
                                    this.tracks[track].name,
                                    clip.notes().len(),
                                    clip.duration_beats()
                                );
                                assign_instrument(&mut this.tracks, track, instrument_id);
                                this.timeline.insert_midi_clip(clip);
                            }
                            ImportedFile::Audio(file) => {
                                this.status_message = format!(
                                    "{} · {:.2}s · {} Hz",
                                    file.name,
                                    file.duration(),
                                    file.sample_rate
                                );
                                this.timeline.insert_audio_clip(AudioClip {
                                    name: file.name.clone(),
                                    file: Arc::new(file),
                                    track,
                                    start_beat,
                                    props: Default::default(),
                                    trim_start: 0.0,
                                    trim_len: None,
                                });
                            }
                        }
                        this.select_track(track, false, false);
                        this.timeline.set_edit_beat(start_beat);
                        this.sync_midi_focus();
                    }
                    Err(error) => this.status_message = format!("No se pudo importar: {error}"),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
