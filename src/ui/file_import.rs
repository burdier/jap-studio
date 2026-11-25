//! Importación común a drop, explorador y comandos; lectura/decodificación fuera de la UI.
use super::DawApp;
use crate::audio::{AudioClip, AudioFile};
use elevate::{
    explorer::EntryKind,
    musical::{
        clip::{ClipId, MidiClip},
        smf::parse_smf_to_clip_with_meter,
        track::{Track, TrackId, TrackKind, UNITY_LEVEL, assign_instrument},
    },
};
use gpui::{AppContext, Context};
use std::{path::PathBuf, sync::Arc};

enum ImportedFile {
    Midi(MidiClip),
    Audio(AudioFile),
}

impl DawApp {
    /// La cabecera usa el marcador de edición. En la rejilla manda el punto de caída.
    pub(super) fn drop_arrangement_files(
        &mut self,
        paths: &[PathBuf],
        header: Option<usize>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.timeline.import_preview = None;
        if self.tracks.is_empty()
            && !paths.is_empty()
            && paths
                .iter()
                .all(|path| EntryKind::for_file(path) == EntryKind::Audio)
        {
            self.import_audio_stack(paths, None, self.timeline.edit_beat(), cx);
            cx.notify();
            return;
        }
        if paths.len() > 1 {
            if paths
                .iter()
                .any(|path| EntryKind::for_file(path) != EntryKind::Audio)
            {
                self.status_message =
                    "Para importar en varias pistas, suelta solo archivos de audio".into();
            } else if let Some(beat) = header.map(|_| self.timeline.edit_beat()).or_else(|| {
                self.timeline.beat_at_position(
                    window.mouse_position(),
                    super::timeline::TimelineArea::Lanes,
                )
            }) {
                let anchor = header.or_else(|| {
                    self.timeline.place_cursor(
                        window.mouse_position(),
                        super::timeline::TimelineArea::Lanes,
                        self.ds.track_header_height,
                        self.tracks.len(),
                    )
                });
                let anchor = anchor
                    .and_then(|index| self.tracks.get(index))
                    .map(|track| track.id);
                self.import_audio_stack(paths, anchor, beat, cx);
            }
            cx.notify();
            return;
        }
        let [path] = paths else { return };
        if !EntryKind::for_file(path).importable() {
            self.status_message = "Elige un archivo de audio o MIDI compatible".into();
            cx.notify();
            return;
        }
        if EntryKind::for_file(path) == EntryKind::Hydrogen {
            self.import_hydrogen(path.clone(), None, cx);
            return;
        }
        let track = header.or_else(|| {
            self.timeline.place_cursor(
                window.mouse_position(),
                super::timeline::TimelineArea::Lanes,
                self.ds.track_header_height,
                self.tracks.len(),
            )
        });
        if let Some(track) = track {
            self.import_audio(path.clone(), track, self.timeline.edit_beat(), cx);
        }
        cx.notify();
    }

    fn import_audio_stack(
        &mut self,
        paths: &[PathBuf],
        anchor: Option<TrackId>,
        beat: f64,
        cx: &mut Context<Self>,
    ) {
        if self.import_task.is_some() {
            self.status_message = "Hay una importación en curso; espera a que termine".into();
            return;
        }
        if self.tracks.len() + paths.len() > 256 {
            self.status_message = "Arrangement admite hasta 256 pistas".into();
            return;
        }
        let paths = paths.to_vec();
        self.status_message = format!("Importando {} audios…", paths.len());
        let decode = cx.background_spawn(async move {
            paths
                .iter()
                .map(|path| {
                    AudioFile::decode(path).map_err(|error| format!("{} · {error}", path.display()))
                })
                .collect::<Result<Vec<_>, _>>()
        });
        self.import_task = Some(cx.spawn(async move |this, cx| {
            let files = decode.await;
            let _ = this.update(cx, |this, cx| {
                this.import_task = None;
                this.status_message =
                    match files.and_then(|files| this.install_audio_stack(files, anchor, beat)) {
                        Ok(1) => "Audio importado en una pista nueva".into(),
                        Ok(count) => format!("{count} audios importados · una pista por archivo"),
                        Err(error) => format!("No se pudo importar: {error}"),
                    };
                cx.notify();
            });
        }));
    }

    fn install_audio_stack(
        &mut self,
        files: Vec<AudioFile>,
        anchor: Option<TrackId>,
        beat: f64,
    ) -> Result<usize, String> {
        let count = files.len();
        let existing = self.tracks.len();
        if existing + count > 256 {
            return Err("Arrangement admite hasta 256 pistas".into());
        }
        let insert_at = if let Some(id) = anchor {
            let index = self
                .tracks
                .iter()
                .position(|track| track.id == id)
                .ok_or("La pista de destino fue eliminada")?;
            let group = self.tracks[index].parent.unwrap_or(id);
            index
                + 1
                + self.tracks[index + 1..]
                    .iter()
                    .take_while(|track| track.parent == Some(group))
                    .count()
        } else {
            existing
        };
        let new_tracks = files.iter().map(|file| {
            let stem = std::path::Path::new(&file.name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&file.name);
            Track::new(stem, TrackKind::Audio)
        });
        let mut tracks = self.tracks.clone();
        tracks.splice(insert_at..insert_at, new_tracks);
        let mut sources: Vec<_> = (0..existing).map(Some).collect();
        sources.splice(insert_at..insert_at, std::iter::repeat_n(None, count));
        let mut levels = self.track_levels.clone();
        levels.splice(
            insert_at..insert_at,
            std::iter::repeat_n(UNITY_LEVEL, count),
        );
        let mut pans = self.track_pans.clone();
        pans.splice(insert_at..insert_at, std::iter::repeat_n(50, count));
        self.commit_track_plan(tracks, sources, levels, pans, Vec::new(), true, true, None);
        self.timeline.selected_midi.clear();
        for (index, file) in files.into_iter().enumerate().rev() {
            self.timeline
                .insert_audio_clip(imported_audio(file, insert_at + index, beat));
        }
        self.timeline.set_edit_beat(beat);
        self.publish_clip_edit();
        Ok(count)
    }

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
                        match file {
                            ImportedFile::Midi(mut clip) => {
                                clip.track = track;
                                this.status_message = format!(
                                    "MIDI en {} · {} notas · {:.1} negras",
                                    this.tracks[track].name,
                                    clip.notes().len(),
                                    clip.duration_beats()
                                );
                                if this.tracks[track].hilo.is_none()
                                    && this.tracks[track].instrument.is_none()
                                {
                                    if this.is_piano_id(instrument_id) {
                                        this.assign_piano_instance(track);
                                    } else {
                                        assign_instrument(&mut this.tracks, track, instrument_id);
                                    }
                                    this.track_revision = this.track_revision.wrapping_add(1);
                                }
                                this.timeline.insert_midi_clip(clip);
                            }
                            ImportedFile::Audio(file) => {
                                this.status_message = format!(
                                    "{} · {:.2}s · {} Hz",
                                    file.name,
                                    file.duration(),
                                    file.sample_rate
                                );
                                this.timeline.selected_midi.clear();
                                this.timeline
                                    .insert_audio_clip(imported_audio(file, track, start_beat));
                            }
                        }
                        this.select_track(track, false, false);
                        this.timeline.set_edit_beat(start_beat);
                        this.sync_midi_focus();
                        this.publish_clip_edit();
                    }
                    Err(error) => this.status_message = format!("No se pudo importar: {error}"),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}

fn imported_audio(file: AudioFile, track: usize, start_beat: f64) -> AudioClip {
    AudioClip {
        processing: Default::default(),
        id: ClipId::fresh(),
        name: file.name.clone(),
        file: Arc::new(file),
        source_path_override: None,
        track,
        start_beat,
        props: Default::default(),
        trim_start: 0.0,
        trim_len: None,
    }
}
