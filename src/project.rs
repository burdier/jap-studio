//! Documento CRDT del proyecto. Solo datos editables; audio decodificado y vistas viven fuera.
use crate::musical::track::{HiloMode, Track, TrackId, TrackKind, TrackSend};
use crate::musical::weave::WeaveRule;
use loro::{CommitOptions, ExportMode, Frontiers, ID, LoroDoc};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    ops::ControlFlow,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

mod audio;
mod collect;
mod drums;
pub use drums::{load_kit_preset, save_kit_preset};
mod instruments;
pub use instruments::{ProjectPiano, ProjectSampler, ProjectVstInstrument};
mod fx;
mod midi;
mod midi_controls;
mod pattern;
mod session_state;
pub use session_state::ArrangementNavigation;
mod track;
pub use audio::ProjectAudioClip;
pub use collect::CollectedAudio;
use track::{
    kind_code, kind_from_code, order_id, read_bool, read_i64, read_optional_id, read_string,
    validate_tracks,
};

const SCHEMA: i64 = 4;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTrack {
    pub id: TrackId,
    pub name: String,
    pub kind: TrackKind,
    pub parent: Option<TrackId>,
    pub instrument: Option<u32>,
    pub hilo: Option<HiloMode>,
    pub weave_rule: Option<WeaveRule>,
    pub muted: bool,
    pub solo: bool,
    pub sends: Vec<TrackSend>,
}

impl From<&Track> for ProjectTrack {
    fn from(track: &Track) -> Self {
        Self {
            id: track.id,
            name: track.name.to_string(),
            kind: track.kind,
            parent: track.parent,
            instrument: track.instrument.map(|id| id.0),
            hilo: track.hilo,
            weave_rule: track.weave_rule.clone(),
            muted: track.muted,
            solo: track.solo,
            sends: track.sends.clone(),
        }
    }
}

pub struct ProjectDoc {
    doc: LoroDoc,
}

/// Entrada persistente del historial CRDT almacenada en el `OpLog` de Loro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectCommit {
    /// Frontera Loro codificada al final de este commit (para `checkout_commit`).
    pub frontiers: Vec<u8>,
    pub message: String,
    /// Marca de tiempo Unix (segundos) guardada dentro del commit Loro.
    pub timestamp: i64,
    /// Número de operaciones CRDT contenidas en este commit.
    pub ops: usize,
}

impl ProjectDoc {
    #[cfg(feature = "p2p")]
    pub fn append_edit_credit(&self, message: &str) -> Result<(), String> {
        self.doc
            .get_list("edit_credits")
            .push(message)
            .map_err(|e| e.to_string())
    }

    #[cfg(feature = "p2p")]
    pub fn recent_edit_credits(&self, limit: usize) -> Vec<String> {
        use loro::ToJson;
        let list = self.doc.get_list("edit_credits");
        (list.len().saturating_sub(limit)..list.len())
            .rev()
            .filter_map(|i| {
                list.get(i)?
                    .get_deep_value()
                    .to_json_value()
                    .as_str()
                    .map(|s| s.chars().take(160).collect())
            })
            .collect()
    }

    pub fn new(name: &str) -> Result<Self, String> {
        let project = Self {
            doc: LoroDoc::new(),
        };
        let meta = project.doc.get_map("project");
        meta.insert("schema", SCHEMA).map_err(|e| e.to_string())?;
        meta.insert("name", name).map_err(|e| e.to_string())?;
        meta.insert(
            "next_clip_id",
            i64::try_from(crate::musical::clip::ClipId::next_id())
                .map_err(|_| "ID de clip agotado")?,
        )
        .map_err(|e| e.to_string())?;
        Ok(project)
    }

    pub fn name(&self) -> Result<String, String> {
        read_string(&self.doc.get_map("project"), "name")
    }

    /// El autor del campo de esquema identifica el origen de las copias de esta versión.
    #[cfg(feature = "p2p")]
    pub fn collaboration_origin(&self) -> Result<u64, String> {
        self.doc
            .get_map("project")
            .get_last_editor("schema")
            .ok_or_else(|| "El proyecto no tiene un origen de colaboración".into())
    }

    #[cfg(feature = "p2p")]
    pub fn collaboration_version(&self) -> loro::VersionVector {
        self.doc.commit();
        self.doc.oplog_vv()
    }

    #[cfg(feature = "p2p")]
    pub fn collaboration_updates(&self, from: &loro::VersionVector) -> Result<Vec<u8>, String> {
        self.validate()?;
        self.doc
            .export(ExportMode::updates(from))
            .map_err(|error| error.to_string())
    }

    /// Prepara una transacción local sin alterar el documento publicado.
    #[cfg(feature = "p2p")]
    pub fn collaboration_fork(&self) -> Self {
        Self {
            doc: self.doc.fork(),
        }
    }

    #[cfg(feature = "p2p")]
    pub fn collaboration_frontiers(&self) -> loro::Frontiers {
        self.doc.oplog_frontiers()
    }

    #[cfg(feature = "p2p")]
    pub fn collaboration_fork_at(&self, frontiers: &loro::Frontiers) -> Result<Self, String> {
        self.doc
            .fork_at(frontiers)
            .map(|doc| Self { doc })
            .map_err(|error| error.to_string())
    }

    pub fn rename(&self, name: &str) -> Result<(), String> {
        self.doc
            .get_map("project")
            .insert("name", name)
            .map_err(|e| e.to_string())
    }

    pub fn from_tracks(name: &str, tracks: &[Track]) -> Result<Self, String> {
        let project = Self::new(name)?;
        project.reconcile_tracks(tracks)?;
        Ok(project)
    }

    /// Captura pistas, FX y clips MIDI; los WAV se añaden con `reconcile_audio_clips`.
    pub fn from_session(
        name: &str,
        tracks: &[Track],
        clips: &[crate::musical::clip::MidiClip],
    ) -> Result<Self, String> {
        let project = Self::from_tracks(name, tracks)?;
        project.reconcile_midi_clips(clips)?;
        Ok(project)
    }

    /// Captura el estado musical y los controles de mezcla editables de la sesión.
    /// Esta captura aún no incluye clips WAV ni estado de instrumentos o plugins.
    pub fn from_session_state(
        name: &str,
        tracks: &[Track],
        clips: &[crate::musical::clip::MidiClip],
        tempo: crate::musical::time::Tempo,
        meter: crate::musical::time::Meter,
        levels: &[i32],
        pans: &[i32],
    ) -> Result<Self, String> {
        let snapshot = crate::musical::track::TrackSnapshot::capture(tracks, levels, pans, 0)?;
        let project = Self::from_session(name, tracks, clips)?;
        project.set_timing(tempo, meter)?;
        for entry in snapshot.entries {
            project.set_track_mix(entry.track.id, entry.mix)?;
        }
        Ok(project)
    }

    /// Sincroniza la estructura de pistas existente sin recrear las entradas de la lista CRDT.
    /// La metadata de clips de audio se reconcilia por separado.
    pub fn reconcile_tracks(&self, tracks: &[Track]) -> Result<(), String> {
        let desired: Vec<_> = tracks.iter().map(ProjectTrack::from).collect();
        validate_tracks(&desired)?;
        let keep: HashSet<_> = desired.iter().map(|track| track.id).collect();
        for existing in self.tracks()? {
            if !keep.contains(&existing.id) {
                self.remove_track(existing.id)?;
            }
        }
        for track in &desired {
            self.upsert_track(track)?;
        }
        for track in tracks {
            fx::write_chain(&self.doc, track.id, &track.chain)?;
        }
        let order = self.doc.get_movable_list("track_order");
        for (target, track) in desired.iter().enumerate() {
            let source = (target..order.len())
                .find(|&index| order_id(&order, index) == Some(track.id))
                .ok_or("Falta una pista en el orden")?;
            if source != target {
                order
                    .mov(source, target)
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    /// Actualiza campos individuales: el orden y los demás campos conservan su identidad CRDT.
    pub fn upsert_track(&self, track: &ProjectTrack) -> Result<(), String> {
        let key = track.id.0.to_string();
        let order = self.doc.get_movable_list("track_order");
        if !(0..order.len()).any(|index| order_id(&order, index) == Some(track.id)) {
            order
                .insert(order.len(), i64::from(track.id.0))
                .map_err(|e| e.to_string())?;
        }
        let map = self.doc.get_map(format!("track:{key}"));
        if read_string(&map, "name").ok().as_deref() != Some(track.name.as_str()) {
            map.insert("name", track.name.as_str())
                .map_err(|e| e.to_string())?;
        }
        if read_i64(&map, "kind").ok() != Some(kind_code(track.kind)) {
            map.insert("kind", kind_code(track.kind))
                .map_err(|e| e.to_string())?;
        }
        if read_optional_id(&map, "parent").ok() != Some(track.parent.map(|id| id.0)) {
            map.insert("parent", track.parent.map_or(-1, |id| i64::from(id.0)))
                .map_err(|e| e.to_string())?;
        }
        if read_optional_id(&map, "instrument").ok() != Some(track.instrument) {
            map.insert("instrument", track.instrument.map_or(-1, i64::from))
                .map_err(|e| e.to_string())?;
        }
        let hilo_code = match track.hilo {
            None => -1,
            Some(HiloMode::Layer) => 0,
            Some(HiloMode::RoundRobin) => 1,
        };
        if read_i64(&map, "hilo_mode").ok() != Some(hilo_code) {
            map.insert("hilo_mode", hilo_code)
                .map_err(|e| e.to_string())?;
        }
        let weave_steps = track
            .weave_rule
            .as_ref()
            .map(|rule| {
                rule.steps()
                    .iter()
                    .map(|step| (usize::from(*step) + 1).to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        if read_string(&map, "weave_steps").ok().as_deref() != Some(weave_steps.as_str()) {
            map.insert("weave_steps", weave_steps.as_str())
                .map_err(|e| e.to_string())?;
        }
        if read_bool(&map, "muted").ok() != Some(track.muted) {
            map.insert("muted", track.muted)
                .map_err(|e| e.to_string())?;
        }
        if read_bool(&map, "solo").ok() != Some(track.solo) {
            map.insert("solo", track.solo).map_err(|e| e.to_string())?;
        }
        let sends_encoded = track
            .sends
            .iter()
            .filter(|send| send.amount > 0)
            .map(|send| format!("{}:{}", send.target.0, send.amount.min(100)))
            .collect::<Vec<_>>()
            .join(",");
        let existing_sends = map
            .get("sends")
            .and_then(|_| read_string(&map, "sends").ok())
            .unwrap_or_default();
        if existing_sends != sends_encoded {
            map.insert("sends", sends_encoded.as_str())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn move_track(&self, from: usize, to: usize) -> Result<(), String> {
        let order = self.doc.get_movable_list("track_order");
        if from >= order.len() || to >= order.len() {
            return Err("Pista fuera de rango".into());
        }
        order.mov(from, to).map_err(|e| e.to_string())
    }

    pub fn remove_track(&self, id: TrackId) -> Result<(), String> {
        let order = self.doc.get_movable_list("track_order");
        if let Some(index) = (0..order.len()).find(|&index| order_id(&order, index) == Some(id)) {
            order.delete(index, 1).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn tracks(&self) -> Result<Vec<ProjectTrack>, String> {
        let order = self.doc.get_movable_list("track_order");
        let mut tracks = Vec::with_capacity(order.len());
        for index in 0..order.len() {
            let id = order_id(&order, index).ok_or("ID de pista inválido")?;
            let map = self.doc.get_map(format!("track:{}", id.0));
            tracks.push(ProjectTrack {
                id,
                name: read_string(&map, "name")?,
                kind: kind_from_code(read_i64(&map, "kind")?)?,
                parent: read_optional_id(&map, "parent")?.map(TrackId),
                instrument: read_optional_id(&map, "instrument")?,
                hilo: match if map.get("hilo_mode").is_some() {
                    read_i64(&map, "hilo_mode")?
                } else {
                    -1
                } {
                    -1 => None,
                    0 => Some(HiloMode::Layer),
                    1 => Some(HiloMode::RoundRobin),
                    _ => return Err("Modo de Hilo inválido".into()),
                },
                weave_rule: if map.get("weave_steps").is_some() {
                    WeaveRule::parse_steps(&read_string(&map, "weave_steps")?)?
                } else {
                    None
                },
                muted: read_bool(&map, "muted")?,
                solo: read_bool(&map, "solo")?,
                sends: if map.get("sends").is_some() {
                    let raw = read_string(&map, "sends")?;
                    let mut parsed = Vec::new();
                    for part in raw.split(',').filter(|s| !s.is_empty()) {
                        let (target, amount) = part
                            .split_once(':')
                            .ok_or_else(|| "Envío auxiliar inválido".to_string())?;
                        let target = target
                            .parse::<u32>()
                            .map_err(|_| "ID de destino de envío inválido".to_string())?;
                        let amount = amount
                            .parse::<u8>()
                            .map_err(|_| "Nivel de envío inválido".to_string())?;
                        if amount > 0 {
                            parsed.push(TrackSend {
                                target: TrackId(target),
                                amount: amount.min(100),
                            });
                        }
                    }
                    parsed
                } else {
                    Vec::new()
                },
            });
        }
        validate_tracks(&tracks)?;
        Ok(tracks)
    }

    pub fn fx_chain(&self, track: TrackId) -> Result<crate::musical::fx::FxChain, String> {
        if !self.tracks()?.iter().any(|entry| entry.id == track) {
            return Err("Pista inexistente".into());
        }
        fx::read_chain(&self.doc, track)
    }

    /// Sustituye el conjunto visible de clips conservando el ID de cada clip y nota.
    /// Se llama después de reconciliar pistas para resolver carriles por TrackId.
    pub fn reconcile_midi_clips(
        &self,
        clips: &[crate::musical::clip::MidiClip],
    ) -> Result<(), String> {
        let audio_ids: HashSet<_> = self
            .audio_clips()?
            .iter()
            .map(|clip| clip.id.value())
            .collect();
        if clips
            .iter()
            .any(|clip| audio_ids.contains(&clip.id.value()))
        {
            return Err("ID compartido entre clips MIDI y audio".into());
        }
        midi::write_clips(&self.doc, &self.tracks()?, clips)?;
        self.persist_next_clip_id()
    }

    pub fn midi_clips(&self) -> Result<Vec<crate::musical::clip::MidiClip>, String> {
        self.read_midi_with_drum_links(&self.tracks()?)
    }

    fn read_midi_with_drum_links(
        &self,
        tracks: &[ProjectTrack],
    ) -> Result<Vec<crate::musical::clip::MidiClip>, String> {
        let mut clips = midi::read_clips(&self.doc, tracks)?;
        if clips.iter().any(|clip| clip.drum_target.is_some()) {
            let session = self
                .drum_session()?
                .ok_or("Falta el kit de los clips de batería")?;
            for clip in &mut clips {
                clip.refresh_drum_link(&session)?;
            }
        }
        Ok(clips)
    }

    /// Guarda referencias a archivos y edición de clips WAV, sin muestras decodificadas.
    pub fn reconcile_audio_clips(&self, clips: &[ProjectAudioClip]) -> Result<(), String> {
        let midi_ids: HashSet<_> = self
            .midi_clips()?
            .iter()
            .map(|clip| clip.id.value())
            .collect();
        if clips.iter().any(|clip| midi_ids.contains(&clip.id.value())) {
            return Err("ID compartido entre clips MIDI y audio".into());
        }
        audio::write_clips(&self.doc, &self.tracks()?, clips)?;
        self.persist_next_clip_id()
    }

    /// Reconcilia ambas colecciones tras un cambio de pistas o de kit.
    /// Los clips obsoletos pueden referir a pistas ya eliminadas.
    pub fn reconcile_session_clips(
        &self,
        midi: &[crate::musical::clip::MidiClip],
        audio: &[ProjectAudioClip],
    ) -> Result<(), String> {
        let midi_ids: HashSet<_> = midi.iter().map(|clip| clip.id).collect();
        if audio.iter().any(|clip| midi_ids.contains(&clip.id)) {
            return Err("ID compartido entre clips MIDI y audio".into());
        }
        let tracks = self.tracks()?;
        midi::write_clips(&self.doc, &tracks, midi)?;
        audio::write_clips(&self.doc, &tracks, audio)?;
        self.persist_next_clip_id()
    }

    fn persist_next_clip_id(&self) -> Result<(), String> {
        let meta = self.doc.get_map("project");
        let next = crate::musical::clip::ClipId::next_id().max(
            u64::try_from(read_i64(&meta, "next_clip_id")?).map_err(|_| "ID de clip inválido")?,
        );
        meta.insert(
            "next_clip_id",
            i64::try_from(next).map_err(|_| "ID de clip agotado")?,
        )
        .map_err(|error| error.to_string())
    }

    pub fn audio_clips(&self) -> Result<Vec<ProjectAudioClip>, String> {
        audio::read_clips(&self.doc, &self.tracks()?)
    }

    /// Reconstruye una sesión musical sin recursos de audio externo ni estado visual.
    pub fn materialize_midi_session(
        &self,
    ) -> Result<(Vec<Track>, Vec<crate::musical::clip::MidiClip>), String> {
        let tracks = self.tracks()?;
        let clips = self.read_midi_with_drum_links(&tracks)?;
        let restored = tracks
            .into_iter()
            .map(|entry| {
                Track::reserve_restored_id(entry.id)?;
                if let Some(id) = entry.instrument {
                    crate::musical::model::InstrumentId::reserve_restored(
                        crate::musical::model::InstrumentId(id),
                    )?;
                }
                Ok(Track {
                    id: entry.id,
                    name: entry.name.into(),
                    kind: entry.kind,
                    instrument: entry.instrument.map(crate::musical::model::InstrumentId),
                    hilo: entry.hilo,
                    weave_rule: entry.weave_rule,
                    midi_enabled: false,
                    muted: entry.muted,
                    solo: entry.solo,
                    parent: entry.parent,
                    sends: entry.sends,
                    chain: fx::read_chain(&self.doc, entry.id)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok((restored, clips))
    }

    pub fn snapshot(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        self.doc
            .export(ExportMode::Snapshot)
            .map_err(|e| e.to_string())
    }

    /// Actualizaciones Loro que otro documento puede importar. Aún no hay transporte de red.
    pub fn all_updates(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        self.doc
            .export(ExportMode::all_updates())
            .map_err(|e| e.to_string())
    }

    /// Importa en una copia y publica el resultado solo si el esquema sigue siendo válido.
    pub fn merge(&mut self, bytes: &[u8]) -> Result<(), String> {
        let candidate = Self::import(&self.snapshot()?)?;
        let imported = candidate
            .doc
            .import(bytes)
            .map_err(|error| error.to_string())?;
        if imported.pending.is_some() {
            return Err("Faltan operaciones anteriores del proyecto".into());
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn import(bytes: &[u8]) -> Result<Self, String> {
        let doc = LoroDoc::new();
        doc.import(bytes).map_err(|e| e.to_string())?;
        let project = Self { doc };
        project.validate()?;
        Ok(project)
    }

    fn validate(&self) -> Result<(), String> {
        if read_i64(&self.doc.get_map("project"), "schema")? != SCHEMA {
            return Err("Versión de proyecto no compatible".into());
        }
        self.name()?;
        self.timing()?;
        self.master_utility()?;
        self.pianos()?;
        self.samplers()?;
        self.vst_instruments()?;
        self.drum_session()?;
        crate::musical::clip::ClipId::reserve_next_id(
            u64::try_from(read_i64(&self.doc.get_map("project"), "next_clip_id")?)
                .map_err(|_| "ID de clip inválido")?,
        )?;
        for track in self.tracks()? {
            fx::read_chain(&self.doc, track.id)?;
            self.track_mix(track.id)?;
        }
        let midi_ids: HashSet<_> = self
            .midi_clips()?
            .iter()
            .map(|clip| clip.id.value())
            .collect();
        if self
            .audio_clips()?
            .iter()
            .any(|clip| midi_ids.contains(&clip.id.value()))
        {
            return Err("ID compartido entre clips MIDI y audio".into());
        }
        Ok(())
    }

    /// Escritura atómica: una exportación fallida nunca sustituye el archivo anterior.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let bytes = self.snapshot()?;
        let mut temporary = None;
        for _ in 0..8 {
            let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let candidate =
                path.with_extension(format!("loro-{}-{suffix}.tmp", std::process::id()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(file) => {
                    temporary = Some((candidate, file));
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.to_string()),
            }
        }
        let (temporary, mut file) = temporary.ok_or("No se pudo reservar archivo temporal")?;
        let result = file
            .write_all(&bytes)
            .and_then(|_| file.sync_all())
            .and_then(|_| fs::rename(&temporary, path));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|error| error.to_string())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        Self::import(&bytes)
    }

    /// Consolida las ediciones pendientes como un commit Loro etiquetado y persistente.
    /// Si no hubo cambios reales en el documento, Loro omite el commit sin ensuciar el historial.
    pub fn commit_change(&self, message: &str, timestamp_secs: i64) {
        self.doc.commit_with(
            CommitOptions::new()
                .commit_msg(message)
                .timestamp(timestamp_secs),
        );
    }

    /// Devuelve la frontera Loro actual codificada tras consolidar cambios pendientes.
    pub fn latest_frontiers(&self) -> Vec<u8> {
        self.doc.commit();
        self.doc.oplog_frontiers().encode()
    }

    /// Recorre el `OpLog` de Loro y devuelve el historial cronológico (del más antiguo al actual).
    /// Conserva mensajes y timestamps persistidos dentro del archivo `.loro`.
    pub fn history_commits(&self, limit: usize) -> Vec<ProjectCommit> {
        self.doc.commit();
        let tip = self.doc.oplog_frontiers();
        if tip.is_empty() {
            return Vec::new();
        }
        let mut raw = Vec::new();
        let tip_ids = tip.to_vec();
        let _ = self.doc.travel_change_ancestors(&tip_ids, &mut |meta| {
            if meta.len == 0 {
                return ControlFlow::Continue(());
            }
            let end_counter = meta.id.counter + (meta.len as i32) - 1;
            let end_id = ID::new(meta.id.peer, end_counter);
            let frontiers = Frontiers::from(end_id).encode();
            let message = meta
                .message
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            raw.push((frontiers, message, meta.timestamp, meta.len));
            if raw.len() >= limit.saturating_mul(2).max(64) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });

        // Filtrar solo commits etiquetados cuando el proyecto ya usa commits con mensaje;
        // si es un archivo `.loro` antiguo sin mensajes, exponer el último estado válido.
        let has_labeled = raw.iter().any(|(_, msg, _, _)| msg.is_some());
        let mut commits: Vec<ProjectCommit> = if has_labeled {
            raw.into_iter()
                .filter_map(|(frontiers, msg, timestamp, ops)| {
                    msg.map(|message| ProjectCommit {
                        frontiers,
                        message,
                        timestamp,
                        ops,
                    })
                })
                .take(limit)
                .collect()
        } else {
            raw.into_iter()
                .take(1)
                .map(|(frontiers, _, timestamp, ops)| ProjectCommit {
                    frontiers,
                    message: "Estado guardado".into(),
                    timestamp,
                    ops,
                })
                .collect()
        };
        // `travel_change_ancestors` recorre de más reciente a más antiguo; invertir a cronológico.
        commits.reverse();
        commits
    }

    /// Time-travel Loro: crea una vista aislada del documento en la frontera indicada
    /// sin alterar ni truncar el historial completo.
    pub fn checkout_commit(&self, frontiers: &[u8]) -> Result<Self, String> {
        let decoded =
            Frontiers::decode(frontiers).map_err(|e| format!("Frontera Loro inválida: {e}"))?;
        let forked = self
            .doc
            .fork_at(&decoded)
            .map_err(|e| format!("Loro checkout fallido: {e}"))?;
        let project = Self { doc: forked };
        project.validate()?;
        Ok(project)
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;
    use crate::musical::{
        time::{Meter, Tempo},
        track::{Track, TrackKind},
    };

    #[test]
    fn loro_history_commits_persist_and_time_travel_restores_exact_states() {
        let doc = ProjectDoc::new("Demo").unwrap();
        doc.set_timing(Tempo::new(120.0).unwrap(), Meter::default())
            .unwrap();
        doc.reconcile_tracks(&[]).unwrap();
        doc.commit_change("Inicio de sesión", 1_700_000_000);

        let t1 = Track::new("Piano", TrackKind::Synth);
        doc.reconcile_tracks(std::slice::from_ref(&t1)).unwrap();
        doc.commit_change("Añadir pista «Piano»", 1_700_000_010);

        doc.set_timing(Tempo::new(138.0).unwrap(), Meter::default())
            .unwrap();
        doc.commit_change("Tempo → 138.0 BPM", 1_700_000_020);

        // Simular cierre y reapertura desde bytes `.loro`
        let bytes = doc.snapshot().unwrap();
        let reopened = ProjectDoc::import(&bytes).unwrap();
        let commits = reopened.history_commits(64);
        assert_eq!(commits.len(), 3);
        assert_eq!(commits[0].message, "Inicio de sesión");
        assert_eq!(commits[1].message, "Añadir pista «Piano»");
        assert_eq!(commits[2].message, "Tempo → 138.0 BPM");
        assert_eq!(commits[2].timestamp, 1_700_000_020);

        // Time-travel a cada punto exacto del historial Loro
        let at0 = reopened.checkout_commit(&commits[0].frontiers).unwrap();
        assert!(at0.tracks().unwrap().is_empty());
        assert_eq!(at0.timing().unwrap().0.bpm(), 120.0);

        let at1 = reopened.checkout_commit(&commits[1].frontiers).unwrap();
        assert_eq!(at1.tracks().unwrap().len(), 1);
        assert_eq!(at1.tracks().unwrap()[0].name, "Piano");
        assert_eq!(at1.timing().unwrap().0.bpm(), 120.0);

        let at2 = reopened.checkout_commit(&commits[2].frontiers).unwrap();
        assert_eq!(at2.tracks().unwrap().len(), 1);
        assert_eq!(at2.timing().unwrap().0.bpm(), 138.0);
    }
}
