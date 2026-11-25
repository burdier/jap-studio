//! Snapshot editable y preparación de apertura fuera del hilo de UI.
use crate::audio::{AudioClip, AudioFile};
use elevate::{
    devices::utility_slot::UtilityState,
    musical::{
        clip::MidiClip,
        drums::DrumSession,
        time::{Meter, Tempo},
        track::Track,
    },
    project::{ProjectAudioClip, ProjectDoc, ProjectPiano, ProjectSampler, ProjectVstInstrument},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Snapshot {
    pub tracks: Vec<Track>,
    pub midi: Vec<MidiClip>,
    pub audio: Vec<ProjectAudioClip>,
    pub pianos: Vec<ProjectPiano>,
    pub samplers: Vec<ProjectSampler>,
    pub vsts: Vec<ProjectVstInstrument>,
    pub drums: DrumSession,
    pub tempo: Tempo,
    pub meter: Meter,
    pub utility: UtilityState,
    pub master_chain: elevate::musical::fx::FxChain,
    pub navigation: elevate::project::ArrangementNavigation,
    pub automation: elevate::musical::track_automation::ArrangementAutomation,
    pub midi_bindings: Vec<elevate::musical::midi_map::MidiBinding>,
    pub levels: Vec<i32>,
    pub pans: Vec<i32>,
}

impl Snapshot {
    pub fn signature(&self) -> String {
        format!("{self:?}")
    }

    pub fn resolve_piano_sources(&mut self) -> Result<(), String> {
        for piano in &mut self.pianos {
            Self::resolve_piano_source(piano)?;
        }
        Ok(())
    }

    fn resolve_piano_source(piano: &mut ProjectPiano) -> Result<(), String> {
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
        Ok(())
    }

    #[cfg(test)]
    pub fn document(&self, name: &str) -> Result<ProjectDoc, String> {
        self.document_from(name, None)
    }

    pub fn document_from(&self, name: &str, previous: Option<&[u8]>) -> Result<ProjectDoc, String> {
        let doc = if let Some(bytes) = previous {
            ProjectDoc::import(bytes)?
        } else {
            ProjectDoc::new(name)?
        };
        doc.rename(name)?;
        self.write_document(&doc)?;
        Ok(doc)
    }

    pub fn write_document(&self, doc: &ProjectDoc) -> Result<(), String> {
        let tracks = elevate::musical::track::TrackSnapshot::capture(
            &self.tracks,
            &self.levels,
            &self.pans,
            0,
        )?;
        doc.reconcile_tracks(&self.tracks)?;
        doc.set_timing(self.tempo, self.meter)?;
        doc.set_master_utility(self.utility)?;
        doc.set_master_chain(&self.master_chain)?;
        doc.set_arrangement_navigation(&self.navigation)?;
        doc.set_track_automation(&self.automation)?;
        doc.set_midi_bindings(&self.midi_bindings)?;
        for entry in tracks.entries {
            doc.set_track_mix(entry.track.id, entry.mix)?;
        }
        doc.set_drum_session(&self.drums)?;
        doc.reconcile_session_clips(&self.midi, &self.audio)?;
        doc.set_pianos(&self.pianos)?;
        doc.set_samplers(&self.samplers)?;
        doc.set_vst_instruments(&self.vsts)?;
        Ok(())
    }

    pub fn relativize_sources(&mut self, root: &Path) -> Result<(), String> {
        for source in self
            .audio
            .iter_mut()
            .map(|clip| &mut clip.source)
            .chain(
                self.drums
                    .kit
                    .pieces
                    .iter_mut()
                    .flat_map(|piece| piece.layers.iter_mut().map(|layer| &mut layer.source)),
            )
            .chain(self.pianos.iter_mut().map(|piano| &mut piano.source))
            .chain(self.samplers.iter_mut().map(|sampler| &mut sampler.source))
        {
            if let Ok(relative) = Path::new(source).strip_prefix(root) {
                *source = relative.to_str().ok_or("Ruta no UTF-8")?.into();
            }
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<(Self, Vec<AudioClip>, Vec<u8>), String> {
        let doc = ProjectDoc::load(path)?;
        let mut snapshot = Self::from_document(&doc)?;
        let root = path.parent().ok_or("Proyecto sin carpeta")?;
        let audio = snapshot.prepare_resources(root, HashMap::new(), &[], &[])?;
        Ok((snapshot, audio, doc.snapshot()?))
    }

    pub fn from_document(doc: &ProjectDoc) -> Result<Self, String> {
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
            samplers: doc.samplers()?,
            vsts: doc.vst_instruments()?,
            drums: doc.drum_session()?.unwrap_or_default(),
            tempo,
            meter,
            utility: doc.master_utility()?,
            master_chain: doc.master_chain()?,
            navigation: doc.arrangement_navigation()?,
            automation: doc.track_automation()?,
            midi_bindings: doc.midi_bindings()?.unwrap_or_else(|| {
                elevate::musical::midi_map::MidiMapper::new()
                    .bindings()
                    .to_vec()
            }),
            levels: mix.iter().map(|m| m.level).collect(),
            pans: mix.iter().map(|m| m.pan).collect(),
        };
        snapshot.validate_support()?;
        snapshot.midi.sort_by_key(|clip| clip.id.value());
        snapshot.audio.sort_by_key(|clip| clip.id.value());
        snapshot.pianos.sort_by_key(|piano| piano.id.0);
        Ok(snapshot)
    }

    pub fn prepare_resources(
        &mut self,
        root: &Path,
        mut files: HashMap<PathBuf, Arc<AudioFile>>,
        existing_pianos: &[ProjectPiano],
        previous: &[AudioClip],
    ) -> Result<Vec<AudioClip>, String> {
        for piano in &mut self.pianos {
            if !piano.source.is_empty() {
                piano.source = root
                    .join(&piano.source)
                    .to_str()
                    .ok_or("Ruta no UTF-8")?
                    .into();
            }
            if !existing_pianos.contains(piano) {
                crate::instruments::Piano::restore_project(piano)?;
                Self::resolve_piano_source(piano)?;
            }
        }
        for sampler in &mut self.samplers {
            sampler.source = root
                .join(&sampler.source)
                .to_str()
                .ok_or("Ruta de sample no UTF-8")?
                .into();
        }
        for piece in &mut self.drums.kit.pieces {
            for layer in &mut piece.layers {
                layer.source = root
                    .join(&layer.source)
                    .to_str()
                    .ok_or("Ruta no UTF-8")?
                    .into();
            }
        }
        let mut audio = Vec::new();
        for clip in &mut self.audio {
            let source = root.join(&clip.source);
            let (source, file) = match files.get(&source) {
                Some(file) => (source, Arc::clone(file)),
                None => {
                    let source = std::fs::canonicalize(&source)
                        .map_err(|e| format!("Falta audio {}: {e}", source.display()))?;
                    let file = if let Some(file) = files.get(&source) {
                        Arc::clone(file)
                    } else {
                        let file = Arc::new(
                            AudioFile::decode(&source)
                                .map_err(|e| format!("{}: {e}", source.display()))?,
                        );
                        files.insert(source.clone(), file.clone());
                        file
                    };
                    (source, file)
                }
            };
            clip.source = source.to_str().ok_or("Ruta no UTF-8")?.into();
            let (file, processing) = if let Some(old) = previous.iter().find(|old| {
                old.id == clip.id
                    && Arc::ptr_eq(old.original_file(), &file)
                    && old.props.pitch_semitones == clip.props.pitch_semitones
                    && old.processing.edit.same_pcm(&clip.edit)
            }) {
                let mut processing = old.processing.clone();
                processing.edit.fades = clip.edit.fades;
                processing.edit.crossfade = clip.edit.crossfade;
                processing.edit.crossfade_root = clip.edit.crossfade_root;
                (old.file.clone(), processing)
            } else {
                crate::audio::sample_edit::Processing::prepare_for_bpm(
                    file,
                    clip.edit.clone(),
                    clip.props.pitch_semitones,
                    self.tempo.bpm(),
                )?
            };
            audio.push(AudioClip {
                processing,
                id: clip.id,
                name: clip.name.clone(),
                source_path_override: (file.source_path.as_deref() != Some(source.as_path()))
                    .then_some(source),
                file,
                track: self
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
        Ok(audio)
    }

    pub fn validate_support(&self) -> Result<(), String> {
        let mut fx_ids = std::collections::HashSet::new();
        for track in &self.tracks {
            if !super::effects::unique_ids(track.chain.nodes(), &mut fx_ids) {
                return Err("ID de efecto duplicado entre pistas".into());
            }
            if let Some(id) = track.instrument {
                if !self.pianos.iter().any(|p| p.id == id)
                    && !self.samplers.iter().any(|s| s.id == id)
                    && !self.vsts.iter().any(|v| v.id == id)
                {
                    return Err(format!(
                        "{}: falta el estado recuperable del instrumento",
                        track.name
                    ));
                }
            }
            if crate::audio::effects::contains_vst(&track.chain)
                || (crate::audio::effects::needs_rack(&track.chain)
                    && super::effects::validate(track.chain.nodes()).is_ok())
            {
                super::effects::validate(track.chain.nodes())?;
                continue;
            }
            if !track.chain.bands().is_empty() {
                // Solo acepta cadenas completamente reconstruibles con el documento.
                crate::audio::effects::compile(
                    &track.chain,
                    &Default::default(),
                    &Default::default(),
                    &Default::default(),
                    48000,
                )?;
                continue;
            }
            if crate::audio::effects::needs_rack(&track.chain) {
                return Err(format!(
                    "{}: esta cadena FX necesita estado todavía no recuperable; solo Utility está soportado",
                    track.name
                ));
            }
            elevate::devices::fx_gain::compile(&track.chain)?;
        }
        if !super::effects::unique_ids(self.master_chain.nodes(), &mut fx_ids) {
            return Err("ID de efecto duplicado en Master FX".into());
        }
        if crate::audio::effects::contains_vst(&self.master_chain)
            || (crate::audio::effects::needs_rack(&self.master_chain)
                && super::effects::validate(self.master_chain.nodes()).is_ok())
        {
            super::effects::validate(self.master_chain.nodes())?;
        } else if !self.master_chain.bands().is_empty() {
            crate::audio::effects::compile(
                &self.master_chain,
                &Default::default(),
                &Default::default(),
                &Default::default(),
                48000,
            )?;
        } else if crate::audio::effects::needs_rack(&self.master_chain) {
            return Err(
                "Master: esta cadena FX necesita estado todavía no recuperable; solo Utility está soportado"
                    .into(),
            );
        } else {
            elevate::devices::fx_gain::compile(&self.master_chain)?;
        }
        Ok(())
    }

    #[cfg(feature = "p2p")]
    pub fn conflicts_with(&self, local: &Self, remote: &Self) -> bool {
        (competing(&self.tracks, &local.tracks, &remote.tracks)
            && !(routing_values_only(&self.tracks, &local.tracks)
                && routing_values_only(&self.tracks, &remote.tracks)
                && !super::effects::competing_presets(&self.tracks, &local.tracks, &remote.tracks)))
            || competing(&self.drums, &local.drums, &remote.drums)
            || competing(&self.tempo, &local.tempo, &remote.tempo)
            || competing(&self.meter, &local.meter, &remote.meter)
            || competing(&self.automation, &local.automation, &remote.automation)
            || competing_by_id(&self.midi, &local.midi, &remote.midi, |clip| clip.id)
            || competing_audio(&self.audio, &local.audio, &remote.audio)
            || competing_by_id(&self.pianos, &local.pianos, &remote.pianos, |piano| {
                piano.id
            })
            || competing_by_id(
                &self.samplers,
                &local.samplers,
                &remote.samplers,
                |sampler| sampler.id,
            )
            || conflicting_mix(self, local, remote)
    }
}

#[cfg(feature = "p2p")]
pub(super) fn same_track_settings(a: &Track, b: &Track) -> bool {
    *a == (Track {
        chain: a.chain.clone(),
        ..b.clone()
    })
}

// El corte, el nivel y el solo de cada rama tienen claves Loro independientes.
// Se pueden combinar si ambos mantienen la topología y los demás parámetros.
#[cfg(feature = "p2p")]
fn routing_values_only(base: &[Track], changed: &[Track]) -> bool {
    use elevate::musical::fx::{FxKind, FxNode};
    fn same_node(a: &FxNode, b: &FxNode) -> bool {
        if a.id != b.id || a.name != b.name || a.enabled != b.enabled {
            return false;
        }
        match (&a.kind, &b.kind) {
            (FxKind::Crossover { branches: x, .. }, FxKind::Crossover { branches: y, .. })
            | (FxKind::SerialChain { children: x }, FxKind::SerialChain { children: y })
            | (FxKind::ParallelChain { branches: x }, FxKind::ParallelChain { branches: y }) => {
                x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same_node(x, y))
            }
            (x, y) => x == y,
        }
    }
    base.len() == changed.len()
        && base.iter().zip(changed).all(|(a, b)| {
            if a == b {
                return true;
            }
            let x = a.chain.nodes();
            let y = b.chain.nodes();
            x.len() == y.len()
                && x.iter().zip(y).all(|(x, y)| same_node(x, y))
                && same_track_settings(a, b)
        })
}

#[cfg(feature = "p2p")]
fn competing_audio(
    base: &[ProjectAudioClip],
    local: &[ProjectAudioClip],
    remote: &[ProjectAudioClip],
) -> bool {
    let index = |clips: &[ProjectAudioClip]| {
        clips
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect::<HashMap<_, _>>()
    };
    let (l, r) = (index(local), index(remote));
    for b in base {
        let left = l.get(&b.id).map(|&i| &local[i]);
        let right = r.get(&b.id).map(|&i| &remote[i]);
        if !competing(&Some(b), &left, &right) {
            continue;
        }
        let (Some(x), Some(y)) = (left, right) else {
            return true;
        };
        // La posición, los procesos y cada control EQ tienen claves independientes en Loro.
        let placement =
            |c: &ProjectAudioClip| (c.track, c.start_beat, c.trim_start, c.trim_len, c.props);
        if competing(&placement(b), &placement(x), &placement(y))
            || competing(&b.name, &x.name, &y.name)
            || competing(&b.source, &x.source, &y.source)
            || (0..2).any(|i| competing(&b.edit.fades[i], &x.edit.fades[i], &y.edit.fades[i]))
            || competing(&b.edit.crossfade, &x.edit.crossfade, &y.edit.crossfade)
            || competing(&b.edit.ranges, &x.edit.ranges, &y.edit.ranges)
        {
            return true;
        }
    }
    let existing = index(base);
    local
        .iter()
        .any(|c| !existing.contains_key(&c.id) && r.get(&c.id).is_some_and(|&i| remote[i] != *c))
}

#[cfg(feature = "p2p")]
fn competing<T: PartialEq>(base: &T, local: &T, remote: &T) -> bool {
    local != base && remote != base && local != remote
}

#[cfg(feature = "p2p")]
fn competing_by_id<T: PartialEq, K: Eq + std::hash::Hash>(
    base: &[T],
    local: &[T],
    remote: &[T],
    id: impl Fn(&T) -> K,
) -> bool {
    let base: HashMap<_, _> = base.iter().map(|item| (id(item), item)).collect();
    let local: HashMap<_, _> = local.iter().map(|item| (id(item), item)).collect();
    let remote: HashMap<_, _> = remote.iter().map(|item| (id(item), item)).collect();
    base.keys()
        .chain(local.keys())
        .any(|key| competing(&base.get(key), &local.get(key), &remote.get(key)))
}

#[cfg(feature = "p2p")]
fn conflicting_mix(base: &Snapshot, local: &Snapshot, remote: &Snapshot) -> bool {
    fn controls(snapshot: &Snapshot) -> Vec<(elevate::musical::track::TrackId, i32)> {
        snapshot
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| (track.id, snapshot.levels[index]))
            .collect()
    }
    fn pans(snapshot: &Snapshot) -> Vec<(elevate::musical::track::TrackId, i32)> {
        snapshot
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| (track.id, snapshot.pans[index]))
            .collect()
    }
    competing_by_id(
        &controls(base),
        &controls(local),
        &controls(remote),
        |entry| entry.0,
    ) || competing_by_id(&pans(base), &pans(local), &pans(remote), |entry| entry.0)
}

// El worker decodifica el audio y prepara sfizz antes de publicar la sesión.
pub(super) type PreparedSampler = (
    elevate::musical::model::InstrumentId,
    elevate::devices::sampler::SamplerParams,
    Arc<crate::audio::sampler::SampleAsset>,
    crate::audio::sampler::SfizzEngine,
);

pub(super) type PreparedVst = (
    elevate::musical::model::InstrumentId,
    crate::audio::piano::PreparedPiano,
    String,
    PathBuf,
    Option<String>,
);

impl Snapshot {
    pub fn prepare_samplers(
        &self,
        previous: &[ProjectSampler],
    ) -> Result<Vec<PreparedSampler>, String> {
        use crate::instruments::sampler::{SampleAsset, Sampler};
        self.samplers
            .iter()
            .filter(|state| !previous.contains(state))
            .map(|state| {
                let asset = SampleAsset::load(Path::new(&state.source))
                    .map_err(|error| format!("Muestra {}: {error}", state.source))?;
                let engine = Sampler::prepare(asset.clone(), state.params.clone())?;
                Ok((state.id, state.params.clone(), asset, engine))
            })
            .collect()
    }

    pub fn prepare_vsts(
        &self,
        config: &crate::audio::device::AudioConfig,
    ) -> Result<Vec<PreparedVst>, String> {
        if self.vsts.is_empty() {
            return Ok(Vec::new());
        }
        let mut catalog = None;
        let mut ready = Vec::with_capacity(self.vsts.len());
        for state in &self.vsts {
            let mut bundle = PathBuf::from(&state.bundle);
            let mut class_id = state.class_id.clone();
            if !bundle.exists() || class_id.is_empty() {
                let plugins = catalog.get_or_insert_with(crate::audio::vst3::scan::discover);
                if let Some(found) = plugins
                    .iter()
                    .filter(|p| {
                        p.instrument
                            && ((!class_id.is_empty()
                                && p.class_id.eq_ignore_ascii_case(&class_id))
                                || (class_id.is_empty() && p.name == state.name))
                    })
                    .min_by_key(|p| p.bridged)
                {
                    bundle = found.bundle.clone();
                    class_id = found.class_id.clone();
                }
            }
            if !bundle.exists() {
                return Err(format!(
                    "Falta el instrumento VST3 {} en esta máquina",
                    state.name
                ));
            }
            let (prepared, name) = if !class_id.is_empty() {
                crate::audio::piano::PreparedPiano::load_vst_preset(
                    bundle.clone(),
                    class_id.clone(),
                    state.preset.clone(),
                    config,
                )?
            } else {
                crate::audio::piano::PreparedPiano::load_vst(&bundle, config, None)?
            };
            ready.push((
                state.id,
                prepared,
                name,
                bundle,
                (!class_id.is_empty()).then_some(class_id),
            ));
        }
        Ok(ready)
    }
}
