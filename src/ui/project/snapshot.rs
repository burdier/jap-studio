//! Snapshot editable y preparación de apertura fuera del hilo de UI.
use crate::audio::{AudioClip, AudioFile};
use elevate::{
    musical::{
        clip::MidiClip,
        drums::DrumSession,
        time::{Meter, Tempo},
        track::Track,
    },
    project::{ProjectAudioClip, ProjectDoc, ProjectPiano},
};
use std::{collections::HashMap, path::Path, sync::Arc};

#[derive(Clone, Debug)]
pub(super) struct Snapshot {
    pub tracks: Vec<Track>,
    pub midi: Vec<MidiClip>,
    pub audio: Vec<ProjectAudioClip>,
    pub pianos: Vec<ProjectPiano>,
    pub drums: DrumSession,
    pub tempo: Tempo,
    pub meter: Meter,
    pub levels: Vec<i32>,
    pub pans: Vec<i32>,
}

impl Snapshot {
    pub fn signature(&self) -> String {
        format!("{self:?}")
    }

    pub fn resolve_piano_sources(&mut self) -> Result<(), String> {
        for piano in &mut self.pianos {
            if piano.source.is_empty() {
                piano.source = crate::audio::piano::discover_soundfonts()
                    .first()
                    .ok_or("No hay SoundFont para guardar el piano")?
                    .path
                    .to_str()
                    .ok_or("Ruta no UTF-8")?
                    .into();
            }
            piano.source = std::fs::canonicalize(&piano.source)
                .map_err(|e| format!("SoundFont {}: {e}", piano.source))?
                .to_str()
                .ok_or("Ruta no UTF-8")?
                .into();
            let font = crate::audio::piano::load_soundfont(Path::new(&piano.source))?;
            let count = font
                .get_presets()
                .iter()
                .filter(|p| {
                    u8::try_from(p.get_bank_number()).is_ok()
                        && u8::try_from(p.get_patch_number()).is_ok()
                })
                .count();
            if piano.preset as usize >= count {
                return Err("Preset de piano inexistente".into());
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn document(&self, name: &str) -> Result<ProjectDoc, String> {
        self.document_from(name, None)
    }

    pub fn document_from(&self, name: &str, previous: Option<&[u8]>) -> Result<ProjectDoc, String> {
        let doc = if let Some(bytes) = previous {
            let doc = ProjectDoc::import(bytes)?;
            // Remove old clips before track reconciliation; their tracks may have been removed.
            doc.reconcile_audio_clips(&[])?;
            doc.reconcile_midi_clips(&[])?;
            doc.reconcile_tracks(&self.tracks)?;
            doc.rename(name)?;
            doc.reconcile_midi_clips(&self.midi)?;
            doc.set_timing(self.tempo, self.meter)?;
            for (index, track) in self.tracks.iter().enumerate() {
                doc.set_track_mix(
                    track.id,
                    elevate::musical::track::TrackMix {
                        level: self.levels[index],
                        pan: self.pans[index],
                    },
                )?;
            }
            doc
        } else {
            ProjectDoc::from_session_state(
                name,
                &self.tracks,
                &self.midi,
                self.tempo,
                self.meter,
                &self.levels,
                &self.pans,
            )?
        };
        doc.set_drum_session(&self.drums)?;
        doc.reconcile_audio_clips(&self.audio)?;
        doc.set_pianos(&self.pianos)?;
        Ok(doc)
    }

    pub fn load(path: &Path) -> Result<(Self, Vec<AudioClip>, Vec<u8>), String> {
        let doc = ProjectDoc::load(path)?;
        let (tracks, midi) = doc.materialize_midi_session()?;
        let (tempo, meter) = doc.timing()?;
        let mix = tracks
            .iter()
            .map(|t| doc.track_mix(t.id))
            .collect::<Result<Vec<_>, _>>()?;
        let mut snapshot = Self {
            tracks,
            midi,
            audio: doc.audio_clips()?,
            pianos: doc.pianos()?,
            drums: doc.drum_session()?.unwrap_or_default(),
            tempo,
            meter,
            levels: mix.iter().map(|m| m.level).collect(),
            pans: mix.iter().map(|m| m.pan).collect(),
        };
        snapshot.validate_support()?;
        let root = path.parent().ok_or("Proyecto sin carpeta")?;
        for piano in &mut snapshot.pianos {
            if !piano.source.is_empty() {
                piano.source = root
                    .join(&piano.source)
                    .to_str()
                    .ok_or("Ruta no UTF-8")?
                    .into();
            }
            crate::instruments::Piano::restore_project(piano)?;
        }
        snapshot.resolve_piano_sources()?;
        for piece in &mut snapshot.drums.kit.pieces {
            for layer in &mut piece.layers {
                layer.source = root
                    .join(&layer.source)
                    .to_str()
                    .ok_or("Ruta no UTF-8")?
                    .into();
            }
        }
        let mut files = HashMap::new();
        let mut audio = Vec::new();
        for clip in &mut snapshot.audio {
            let source = root.join(&clip.source);
            let source = std::fs::canonicalize(&source)
                .map_err(|e| format!("Falta audio {}: {e}", source.display()))?;
            let file = if let Some(file) = files.get(&source) {
                Arc::clone(file)
            } else {
                let file = Arc::new(
                    AudioFile::decode(&source).map_err(|e| format!("{}: {e}", source.display()))?,
                );
                files.insert(source.clone(), file.clone());
                file
            };
            clip.source = source.to_str().ok_or("Ruta no UTF-8")?.into();
            audio.push(AudioClip {
                id: clip.id,
                name: clip.name.clone(),
                file,
                source_path_override: None,
                track: snapshot
                    .tracks
                    .iter()
                    .position(|t| t.id == clip.track)
                    .ok_or("Audio sin pista")?,
                start_beat: clip.start_beat,
                props: clip.props,
                trim_start: clip.trim_start,
                trim_len: clip.trim_len,
            });
        }
        Ok((snapshot, audio, doc.snapshot()?))
    }

    pub fn validate_support(&self) -> Result<(), String> {
        for track in &self.tracks {
            if let Some(id) = track.instrument {
                if !self.pianos.iter().any(|p| p.id == id) {
                    return Err(format!(
                        "{}: el guardado recuperable de Sampler/VST3 o instrumentos sin estado aún no está disponible",
                        track.name
                    ));
                }
            }
            if crate::audio::effects::needs_rack(&track.chain) {
                return Err(format!(
                    "{}: esta cadena FX necesita estado todavía no recuperable; solo Utility está soportado",
                    track.name
                ));
            }
            elevate::devices::fx_gain::compile(&track.chain)?;
        }
        Ok(())
    }
}
