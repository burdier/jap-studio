//! Colecta el audio importado fuera de la UI; el resultado es un snapshot de recursos.
use super::DawApp;
use crate::audio::AudioClip;
use elevate::project::ProjectDoc;
use gpui::{AppContext, Context};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

fn rebind_if_source_unchanged(clip: &mut AudioClip, original: &str, collected: &Path) -> bool {
    let current = clip
        .source_path_override
        .as_ref()
        .or(clip.file.source_path.as_ref())
        .and_then(|path| path.to_str());
    if current != Some(original) {
        return false;
    }
    clip.source_path_override = Some(collected.to_path_buf());
    true
}

impl DawApp {
    pub(super) fn start_audio_collect(&mut self, mut directory: PathBuf, cx: &mut Context<Self>) {
        if self.collect_task.is_some() {
            self.status_message = "Ya hay una colecta de audio en curso".into();
            return;
        }
        let result = (|| {
            if directory.as_os_str().is_empty() {
                return Err("Elige una carpeta nueva".to_string());
            }
            if let Ok(rest) = directory.strip_prefix("~") {
                directory = dirs::home_dir()
                    .ok_or("No se encontró la carpeta personal")?
                    .join(rest);
            }
            let source_root = std::env::current_dir().map_err(|error| error.to_string())?;
            if directory.is_relative() {
                directory = source_root.join(&directory);
            }
            let audio = self
                .timeline
                .clip
                .iter()
                .chain(&self.timeline.other_audio_clips)
                .map(|clip| {
                    let track = self
                        .tracks
                        .get(clip.track)
                        .ok_or("Clip de audio sin pista")?;
                    clip.project_metadata(track.id)
                })
                .collect::<Result<Vec<_>, String>>()?;
            if audio.is_empty() {
                return Err("No hay clips de audio para colectar".into());
            }
            let midi = self
                .timeline
                .midi_clip
                .iter()
                .chain(&self.timeline.other_midi_clips)
                .cloned()
                .collect::<Vec<_>>();
            Ok((source_root, audio, midi))
        })();
        let (source_root, audio, midi) = match result {
            Ok(value) => value,
            Err(error) => {
                self.status_message = error;
                cx.notify();
                return;
            }
        };
        let originals: HashMap<_, _> = audio
            .iter()
            .map(|clip| (clip.id, clip.source.clone()))
            .collect();
        let tracks = self.tracks.clone();
        let levels = self.track_levels.clone();
        let pans = self.track_pans.clone();
        let tempo = self.timeline.tempo;
        let meter = self.timeline.meter;
        let name = directory
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.status_message = format!("Colectando audio en {}…", directory.display());
        let work_directory = directory.clone();
        let collect = cx.background_spawn(async move {
            let project = ProjectDoc::from_session_state(
                &name, &tracks, &midi, tempo, meter, &levels, &pans,
            )?;
            project.reconcile_audio_clips(&audio)?;
            project.collect_audio_into(&work_directory, &source_root)
        });
        self.collect_task = Some(cx.spawn(async move |this, cx| {
            let result = collect.await;
            let _ = this.update(cx, |this, cx| {
                this.collect_task = None;
                this.status_message = match result {
                    Ok(collected) => {
                        let paths: HashMap<_, _> = collected
                            .clips
                            .into_iter()
                            .map(|clip| (clip.id, directory.join(clip.source)))
                            .collect();
                        let mut rebound = 0;
                        for clip in this
                            .timeline
                            .clip
                            .iter_mut()
                            .chain(&mut this.timeline.other_audio_clips)
                        {
                            let Some(original) = originals.get(&clip.id) else { continue };
                            let Some(path) = paths.get(&clip.id) else { continue };
                            if rebind_if_source_unchanged(clip, original, path) {
                                rebound += 1;
                            }
                        }
                        format!("{} fuentes colectadas en {} · {rebound} clips usan esa carpeta · abrir proyecto completo sigue pendiente", collected.files, directory.display())
                    }
                    Err(error) => format!("No se pudo colectar audio: {error}"),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::rebind_if_source_unchanged;
    use crate::audio::{AudioClip, AudioFile};
    use elevate::musical::{
        clip::{ClipId, ClipProps},
        track::TrackId,
    };
    use std::{
        path::{Path, PathBuf},
        sync::Arc,
    };

    #[test]
    fn collection_rebinds_only_the_source_captured_at_start() {
        let mut clip = AudioClip {
            processing: Default::default(),
            id: ClipId::fresh(),
            name: "Toma".into(),
            file: Arc::new(AudioFile {
                name: "toma.wav".into(),
                source_path: Some(PathBuf::from("/original/toma.wav")),
                sample_rate: 48_000,
                frames: vec![[0.0, 0.0]],
                peaks: vec![],
                max_peak: 0.0,
            }),
            source_path_override: None,
            track: 0,
            start_beat: 0.0,
            props: ClipProps::default(),
            trim_start: 0.0,
            trim_len: None,
        };
        assert!(rebind_if_source_unchanged(
            &mut clip,
            "/original/toma.wav",
            Path::new("/proyecto/Audio/toma.wav")
        ));
        assert!(!rebind_if_source_unchanged(
            &mut clip,
            "/original/toma.wav",
            Path::new("/otro/Audio/toma.wav")
        ));
        assert_eq!(
            clip.project_metadata(TrackId(1)).unwrap().source,
            "/proyecto/Audio/toma.wav"
        );
    }
}
