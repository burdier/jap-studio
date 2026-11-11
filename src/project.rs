//! Documento CRDT del proyecto. Solo datos editables; audio decodificado y vistas viven fuera.
use crate::musical::track::{Track, TrackId, TrackKind};
use loro::{ExportMode, LoroDoc};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

mod fx;
mod midi;
mod midi_controls;
mod track;
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
    pub muted: bool,
    pub solo: bool,
}

impl From<&Track> for ProjectTrack {
    fn from(track: &Track) -> Self {
        Self {
            id: track.id,
            name: track.name.to_string(),
            kind: track.kind,
            parent: track.parent,
            instrument: track.instrument.map(|id| id.0),
            muted: track.muted,
            solo: track.solo,
        }
    }
}

pub struct ProjectDoc {
    doc: LoroDoc,
}

impl ProjectDoc {
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

    /// Captura pistas, FX y clips MIDI. El audio externo aún no pertenece al documento.
    pub fn from_session(
        name: &str,
        tracks: &[Track],
        clips: &[crate::musical::clip::MidiClip],
    ) -> Result<Self, String> {
        let project = Self::from_tracks(name, tracks)?;
        project.reconcile_midi_clips(clips)?;
        Ok(project)
    }

    /// Sincroniza la estructura de pistas existente sin recrear las entradas de la lista CRDT.
    /// Las instancias de audio todavía no forman parte de este esquema.
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
        if read_bool(&map, "muted").ok() != Some(track.muted) {
            map.insert("muted", track.muted)
                .map_err(|e| e.to_string())?;
        }
        if read_bool(&map, "solo").ok() != Some(track.solo) {
            map.insert("solo", track.solo).map_err(|e| e.to_string())?;
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
                muted: read_bool(&map, "muted")?,
                solo: read_bool(&map, "solo")?,
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
        midi::write_clips(&self.doc, &self.tracks()?, clips)?;
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

    pub fn midi_clips(&self) -> Result<Vec<crate::musical::clip::MidiClip>, String> {
        midi::read_clips(&self.doc, &self.tracks()?)
    }

    /// Reconstruye una sesión musical sin recursos de audio externo ni estado visual.
    pub fn materialize_midi_session(
        &self,
    ) -> Result<(Vec<Track>, Vec<crate::musical::clip::MidiClip>), String> {
        let tracks = self.tracks()?;
        let clips = midi::read_clips(&self.doc, &tracks)?;
        let restored = tracks
            .into_iter()
            .map(|entry| {
                Track::reserve_restored_id(entry.id)?;
                Ok(Track {
                    id: entry.id,
                    name: entry.name.into(),
                    kind: entry.kind,
                    instrument: entry.instrument.map(crate::musical::model::InstrumentId),
                    muted: entry.muted,
                    solo: entry.solo,
                    parent: entry.parent,
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
        candidate
            .doc
            .import(bytes)
            .map_err(|error| error.to_string())?;
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
        crate::musical::clip::ClipId::reserve_next_id(
            u64::try_from(read_i64(&self.doc.get_map("project"), "next_clip_id")?)
                .map_err(|_| "ID de clip inválido")?,
        )?;
        for track in self.tracks()? {
            fx::read_chain(&self.doc, track.id)?;
        }
        self.midi_clips()?;
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
}
