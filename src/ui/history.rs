//! Historial global persistente respaldado por Loro (`ProjectDoc` + `CommitOptions` + `Frontiers`).
//! Cada edición asentada se registra como un commit CRDT real que viaja dentro del archivo `.loro`
//! y permite Ctrl+Z / Ctrl+Y y *time-travel* instantáneo desde el panel `🕒 Historial`.
use super::DawApp;
use crate::audio::{AudioClip, AudioFile};
use elevate::{
    musical::{
        clip::{ClipId, MidiClip},
        drums::DrumSession,
        time::{Meter, Tempo},
        track::{Track, TrackSnapshot},
    },
    project::{ArrangementNavigation, ProjectCommit, ProjectDoc, ProjectPiano, ProjectSampler},
};
use gpui::{Context, MouseButton, div, prelude::*, px, rgb, rgba};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_COMMITS_SHOWN: usize = 128;
const CHECK_INTERVAL: Duration = Duration::from_millis(220);
const SETTLE_DELAY: Duration = Duration::from_millis(280);

/// Resumen ligero del estado actual usado solo para detectar cambios y redactar
/// descripciones legibles del commit Loro sin duplicar la historia en RAM.
#[derive(Clone)]
struct HistorySummary {
    tracks: Vec<Track>,
    track_levels: Vec<i32>,
    track_pans: Vec<i32>,
    audio_count: usize,
    midi_count: usize,
    midi_clips: Vec<MidiClip>,
    drums: DrumSession,
    tempo: Tempo,
    meter: Meter,
    navigation: ArrangementNavigation,
    automation: elevate::musical::track_automation::ArrangementAutomation,
    master_chain: elevate::musical::fx::FxChain,
}

#[derive(Default)]
pub(in crate::ui) struct ProjectHistory {
    /// Documento CRDT Loro vivo que acumula los deltas y commits del proyecto.
    pub doc: Option<ProjectDoc>,
    /// Lista cronológica de commits leídos del `OpLog` de Loro.
    pub commits: Vec<ProjectCommit>,
    /// Índice del commit activo en `commits`.
    pub cursor: usize,
    pub open: bool,
    /// Firma del estado que corresponde al commit en `cursor`.
    current_signature: u64,
    last_summary: Option<HistorySummary>,
    last_check: Option<Instant>,
    pending: Option<(u64, Instant)>,
    /// Caché O(1) de archivos de audio decodificados (`Arc<AudioFile>`) para time-travel sin E/S.
    audio_files: HashMap<PathBuf, Arc<AudioFile>>,
    /// Última versión conocida de cada clip de audio (para reutilizar `Processing` si el PCM no cambió).
    audio_clips: HashMap<ClipId, AudioClip>,
}

impl ProjectHistory {
    pub fn can_undo(&self) -> bool {
        self.cursor > 0 && !self.commits.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        self.cursor + 1 < self.commits.len()
    }

    pub fn len(&self) -> usize {
        self.commits.len()
    }
}

pub(in crate::ui) fn unix_now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub(in crate::ui) fn format_commit_time(unix_secs: i64) -> String {
    if unix_secs <= 0 {
        return "—".into();
    }
    let now = unix_now_secs() as libc::time_t;
    let t = unix_secs as libc::time_t;
    let mut tm_now: libc::tm = unsafe { std::mem::zeroed() };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        libc::localtime_r(&now, &mut tm_now);
        libc::localtime_r(&t, &mut tm);
    }
    if tm.tm_year == tm_now.tm_year && tm.tm_yday == tm_now.tm_yday {
        format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
    } else {
        format!(
            "{:02}/{:02} {:02}:{:02}",
            tm.tm_mday,
            tm.tm_mon + 1,
            tm.tm_hour,
            tm.tm_min
        )
    }
}

pub(in crate::ui) fn local_hhmm() -> String {
    let t = unix_now_secs() as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        libc::localtime_r(&t, &mut tm);
    }
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}

impl DawApp {
    pub(in crate::ui) fn is_gesture_active(&self) -> bool {
        self.track_control_drag.is_some()
            || self.mixer_drag.is_some()
            || self.track_reorder_drag.is_some()
            || self.split_drag.is_some()
            || self.timeline.tempo_drag.is_some()
            || self.timeline.clip_moved()
            || self.timeline.automation_dragging()
            || self.sampler_ui.drag.is_some()
            || self.sampler_ui.wave_drag.is_some()
    }

    fn capture_summary(&self) -> (HistorySummary, u64) {
        let mut midi_clips: Vec<MidiClip> = self
            .timeline
            .midi_clip
            .iter()
            .chain(&self.timeline.other_midi_clips)
            .cloned()
            .collect();
        midi_clips.sort_by_key(|c| c.id.value());
        let audio_count = self.timeline.clip.iter().count() + self.timeline.other_audio_clips.len();
        let midi_count = midi_clips.len();

        let mut h = DefaultHasher::new();
        format!("{:?}", self.tracks).hash(&mut h);
        self.track_levels.hash(&mut h);
        self.track_pans.hash(&mut h);
        let mut audio_refs: Vec<&AudioClip> = self
            .timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
            .collect();
        audio_refs.sort_by_key(|c| c.id.value());
        for c in audio_refs {
            c.id.value().hash(&mut h);
            c.name.hash(&mut h);
            c.track.hash(&mut h);
            c.start_beat.to_bits().hash(&mut h);
            c.trim_start.to_bits().hash(&mut h);
            c.trim_len.map(f64::to_bits).hash(&mut h);
            format!("{:?}", c.props).hash(&mut h);
            format!("{:?}", c.processing.edit).hash(&mut h);
        }
        for c in &midi_clips {
            format!("{c:?}").hash(&mut h);
        }
        format!("{:?}", self.pattern_editor.session).hash(&mut h);
        format!("{:?}", self.timeline.tempo).hash(&mut h);
        format!("{:?}", self.timeline.meter).hash(&mut h);
        format!("{:?}", self.timeline.navigation).hash(&mut h);
        format!("{:?}", self.timeline.automation).hash(&mut h);
        format!("{:?}", self.master_chain.nodes()).hash(&mut h);

        let summary = HistorySummary {
            tracks: self.tracks.clone(),
            track_levels: self.track_levels.clone(),
            track_pans: self.track_pans.clone(),
            audio_count,
            midi_count,
            midi_clips,
            drums: self.pattern_editor.session.clone(),
            tempo: self.timeline.tempo,
            meter: self.timeline.meter,
            navigation: self.timeline.navigation.clone(),
            automation: self.timeline.automation.clone(),
            master_chain: self.master_chain.clone(),
        };
        (summary, h.finish())
    }

    pub(in crate::ui) fn current_project_hash(&self) -> u64 {
        let (_, hash) = self.capture_summary();
        hash
    }

    /// Exporta los bytes `.loro` del historial vivo para que `Ctrl+S` y el autoguardado
    /// conserven todos los commits y mensajes en el archivo guardado.
    pub(in crate::ui) fn history_doc_bytes(&self) -> Option<Vec<u8>> {
        self.history.doc.as_ref()?.snapshot().ok()
    }

    fn cache_current_audio(&mut self) {
        let clips: Vec<AudioClip> = self
            .timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
            .cloned()
            .collect();
        for clip in clips {
            let orig = clip.original_file().clone();
            if let Some(path) = orig.source_path.clone() {
                self.history.audio_files.insert(path, orig.clone());
            }
            if let Some(path) = clip.source_path_override.clone() {
                self.history.audio_files.insert(path, orig);
            }
            self.history.audio_clips.insert(clip.id, clip);
        }
    }

    /// Sincroniza el estado vivo del DAW dentro de `doc` y crea un commit Loro etiquetado.
    fn write_live_state_to_doc(&self, doc: &ProjectDoc) -> Result<(), String> {
        let mut tracks = self.tracks.clone();
        for track in &mut tracks {
            track.midi_enabled = false;
            for (id, effect) in &self.vst_effects.instances {
                if track.chain.find(*id).is_some()
                    && let Ok(preset) = effect.editor.project_preset()
                {
                    track.chain.set_vst_preset(*id, preset);
                }
            }
        }
        let mut midi: Vec<_> = self
            .timeline
            .midi_clip
            .iter()
            .chain(&self.timeline.other_midi_clips)
            .cloned()
            .collect();
        midi.sort_by_key(|c| c.id.value());

        let mut audio = Vec::new();
        for clip in self
            .timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
        {
            if let Some(track) = self.tracks.get(clip.track)
                && let Ok(meta) = clip.project_metadata(track.id)
            {
                audio.push(meta);
            }
        }
        audio.sort_by_key(|c| c.id.value());

        let mut pianos = Vec::new();
        for id in tracks.iter().filter_map(|t| t.instrument) {
            if pianos.iter().any(|p: &ProjectPiano| p.id == id) {
                continue;
            }
            if id == self.instrument.id() {
                pianos.push(self.instrument.project_state());
            } else if let Some(piano) = self.piano_instances.get(&id) {
                pianos.push(piano.project_state());
            }
        }
        pianos.sort_by_key(|p| p.id.0);

        let mut samplers = Vec::new();
        for id in tracks
            .iter()
            .filter_map(|t| t.instrument)
            .filter(|id| self.is_sampler_id(*id))
        {
            if let Ok(snapshot) = self.sampler_snapshot(id)
                && let Some(source) = snapshot.asset.source.to_str()
                && !source.is_empty()
            {
                samplers.push(ProjectSampler {
                    id,
                    source: source.to_owned(),
                    params: snapshot.params,
                });
            }
        }
        samplers.sort_by_key(|s| s.id.0);
        samplers.dedup_by_key(|s| s.id);

        let track_snapshot =
            TrackSnapshot::capture(&tracks, &self.track_levels, &self.track_pans, 0)?;
        doc.reconcile_tracks(&tracks)?;
        doc.set_timing(self.timeline.tempo, self.timeline.meter)?;
        doc.set_arrangement_navigation(&self.timeline.navigation)?;
        doc.set_track_automation(&self.timeline.automation)?;
        let mut master_chain = self.master_chain.clone();
        for (id, effect) in &self.vst_effects.instances {
            if master_chain.find(*id).is_some()
                && let Ok(preset) = effect.editor.project_preset()
            {
                master_chain.set_vst_preset(*id, preset);
            }
        }
        doc.set_master_chain(&master_chain)?;
        for entry in track_snapshot.entries {
            doc.set_track_mix(entry.track.id, entry.mix)?;
        }
        doc.set_drum_session(&self.pattern_editor.session)?;
        doc.reconcile_session_clips(&midi, &audio)?;
        doc.set_pianos(&pianos)?;
        doc.set_samplers(&samplers)?;
        Ok(())
    }

    /// Inicia un documento Loro limpio con un commit inicial.
    pub(in crate::ui) fn reset_history(&mut self, label: &str) {
        self.history.audio_files.clear();
        self.history.audio_clips.clear();
        self.cache_current_audio();
        let (summary, signature) = self.capture_summary();
        let name = self
            .project
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "session".into());
        if let Ok(doc) = ProjectDoc::new(&name) {
            let _ = self.write_live_state_to_doc(&doc);
            doc.commit_change(label, unix_now_secs());
            let commits = doc.history_commits(MAX_COMMITS_SHOWN);
            self.history.cursor = commits.len().saturating_sub(1);
            self.history.commits = commits;
            self.history.doc = Some(doc);
        } else {
            self.history.doc = None;
            self.history.commits.clear();
            self.history.cursor = 0;
        }
        self.history.current_signature = signature;
        self.history.last_summary = Some(summary);
        self.history.pending = None;
        self.history.last_check = Some(Instant::now());
    }

    /// Carga el historial CRDT persistido dentro de un archivo `.loro` abierto o recuperado,
    /// conservando todos los commits anteriores con sus mensajes y timestamps originales.
    pub(in crate::ui) fn load_loro_history(&mut self, bytes: &[u8], fallback_label: &str) {
        self.history.audio_files.clear();
        self.history.audio_clips.clear();
        self.sync_loro_history_bytes(bytes, fallback_label);
    }

    /// Actualiza el documento Loro vivo tras un `Ctrl+S` conservando la caché de audio en RAM.
    pub(in crate::ui) fn sync_loro_history_bytes(&mut self, bytes: &[u8], fallback_label: &str) {
        self.cache_current_audio();
        let (summary, signature) = self.capture_summary();
        match ProjectDoc::import(bytes) {
            Ok(doc) => {
                let mut commits = doc.history_commits(MAX_COMMITS_SHOWN);
                if commits.is_empty() {
                    let _ = self.write_live_state_to_doc(&doc);
                    doc.commit_change(fallback_label, unix_now_secs());
                    commits = doc.history_commits(MAX_COMMITS_SHOWN);
                }
                self.history.cursor = commits.len().saturating_sub(1);
                self.history.commits = commits;
                self.history.doc = Some(doc);
                self.history.current_signature = signature;
                self.history.last_summary = Some(summary);
                self.history.pending = None;
                self.history.last_check = Some(Instant::now());
            }
            Err(_) => self.reset_history(fallback_label),
        }
    }

    fn record_loro_commit(&mut self, summary: HistorySummary, signature: u64, label: String) {
        self.cache_current_audio();
        let Some(doc) = self.history.doc.as_ref() else {
            self.reset_history(&label);
            return;
        };

        // Si el usuario estaba viajando en el pasado (cursor < último commit) y hace una edición
        // nueva, primero consolidamos el punto de partida en el OpLog para que el próximo Ctrl+Z
        // regrese exactamente a ese estado y no salte a una rama descartada.
        if self.history.cursor + 1 < self.history.commits.len()
            && let Some(anchor) = self.history.commits.get(self.history.cursor).cloned()
            && let Ok(past) = doc.checkout_commit(&anchor.frontiers)
        {
            if let Ok((tracks, midi)) = past.materialize_midi_session()
                && let Ok((tempo, meter)) = past.timing()
                && let Ok(nav) = past.arrangement_navigation()
                && let Ok(auto) = past.track_automation()
                && let Ok(audio) = past.audio_clips()
                && let Ok(pianos) = past.pianos()
                && let Ok(samplers) = past.samplers()
            {
                let _ = doc.reconcile_tracks(&tracks);
                let _ = doc.set_timing(tempo, meter);
                let _ = doc.set_arrangement_navigation(&nav);
                let _ = doc.set_track_automation(&auto);
                if let Ok(mchain) = past.master_chain() {
                    let _ = doc.set_master_chain(&mchain);
                }
                for t in &tracks {
                    if let Ok(mix) = past.track_mix(t.id) {
                        let _ = doc.set_track_mix(t.id, mix);
                    }
                }
                if let Ok(Some(drums)) = past.drum_session() {
                    let _ = doc.set_drum_session(&drums);
                }
                let _ = doc.reconcile_session_clips(&midi, &audio);
                let _ = doc.set_pianos(&pianos);
                let _ = doc.set_samplers(&samplers);
                doc.commit_change(&format!("Volver a: {}", anchor.message), unix_now_secs());
            }
        }

        if self.write_live_state_to_doc(doc).is_ok() {
            doc.commit_change(&label, unix_now_secs());
            let commits = doc.history_commits(MAX_COMMITS_SHOWN);
            if !commits.is_empty() {
                self.history.cursor = commits.len() - 1;
                self.history.commits = commits;
            }
        }
        self.history.current_signature = signature;
        self.history.last_summary = Some(summary);
        self.history.pending = None;
    }

    pub(in crate::ui) fn tick_history(&mut self) {
        if self.history.doc.is_none() || self.history.commits.is_empty() {
            self.reset_history("Inicio de sesión");
            return;
        }
        if self.is_gesture_active() || self.project_busy_for_history() {
            return;
        }
        if self
            .history
            .last_check
            .is_some_and(|t| t.elapsed() < CHECK_INTERVAL)
        {
            return;
        }
        self.history.last_check = Some(Instant::now());
        let (summary, hash) = self.capture_summary();
        if hash == self.history.current_signature {
            self.history.pending = None;
            return;
        }
        let now = Instant::now();
        match self.history.pending {
            Some((pending_hash, since)) if pending_hash == hash => {
                if since.elapsed() < SETTLE_DELAY {
                    return;
                }
            }
            _ => {
                self.history.pending = Some((hash, now));
                return;
            }
        }
        let label = self
            .history
            .last_summary
            .as_ref()
            .map(|prev| describe_change(prev, &summary))
            .unwrap_or_else(|| "Edición".into());
        self.record_loro_commit(summary, hash, label);
    }

    pub(in crate::ui) fn undo_project(&mut self) {
        // Si hay un cambio reciente sin asentarse, commitéalo primero en Loro para no perder Redo.
        let (summary, hash) = self.capture_summary();
        if hash != self.history.current_signature {
            let label = self
                .history
                .last_summary
                .as_ref()
                .map(|prev| describe_change(prev, &summary))
                .unwrap_or_else(|| "Edición".into());
            self.record_loro_commit(summary, hash, label);
        }
        if !self.history.can_undo() {
            self.status_message = "Nada para deshacer".into();
            return;
        }
        let target = self.history.cursor - 1;
        let label = self.history.commits[self.history.cursor].message.clone();
        self.jump_history(target, Some(format!("↶ Deshecho (Loro): {label}")));
    }

    pub(in crate::ui) fn redo_project(&mut self) {
        if !self.history.can_redo() {
            self.status_message = "Nada para rehacer".into();
            return;
        }
        let target = self.history.cursor + 1;
        let label = self.history.commits[target].message.clone();
        self.jump_history(target, Some(format!("↷ Rehecho (Loro): {label}")));
    }

    /// Time-travel Loro real: reconstruye el estado exacto desde `doc.checkout_commit(frontiers)`.
    pub(in crate::ui) fn jump_history(&mut self, index: usize, message: Option<String>) {
        self.cache_current_audio();
        let Some(commit) = self.history.commits.get(index).cloned() else {
            return;
        };
        let Some(doc) = self.history.doc.as_ref() else {
            return;
        };
        let historical = match doc.checkout_commit(&commit.frontiers) {
            Ok(d) => d,
            Err(error) => {
                self.status_message = format!("Historial Loro: {error}");
                return;
            }
        };

        let (tracks, midi_clips) = match historical.materialize_midi_session() {
            Ok(pair) => pair,
            Err(error) => {
                self.status_message = format!("Historial Loro: {error}");
                return;
            }
        };
        let (tempo, meter) = match historical.timing() {
            Ok(t) => t,
            Err(error) => {
                self.status_message = format!("Historial Loro: {error}");
                return;
            }
        };
        let navigation = historical.arrangement_navigation().unwrap_or_default();
        let automation = historical.track_automation().unwrap_or_default();
        let master_chain = historical.master_chain().unwrap_or_default();
        let drums = historical.drum_session().ok().flatten().unwrap_or_default();
        let mut levels = Vec::with_capacity(tracks.len());
        let mut pans = Vec::with_capacity(tracks.len());
        for track in &tracks {
            let (level, pan) = historical
                .track_mix(track.id)
                .map_or((75, 50), |m| (m.level, m.pan));
            levels.push(level);
            pans.push(pan);
        }

        // Resolver clips de audio usando caché en memoria O(1) (y disco solo si es de sesión previa).
        let project_root = self
            .project
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf);
        let project_audio = historical.audio_clips().unwrap_or_default();
        let mut restored_audio = Vec::with_capacity(project_audio.len());
        for clip in project_audio {
            let Some(track_idx) = tracks.iter().position(|t| t.id == clip.track) else {
                continue;
            };
            let raw_path = PathBuf::from(&clip.source);
            let resolved_path = if raw_path.is_absolute() {
                raw_path
            } else if let Some(root) = &project_root {
                root.join(&raw_path)
            } else {
                raw_path
            };
            let file_opt = self
                .history
                .audio_files
                .get(&resolved_path)
                .cloned()
                .or_else(|| {
                    std::fs::canonicalize(&resolved_path)
                        .ok()
                        .and_then(|p| self.history.audio_files.get(&p).cloned())
                })
                .or_else(|| {
                    self.history
                        .audio_clips
                        .get(&clip.id)
                        .map(|c| c.original_file().clone())
                })
                .or_else(|| {
                    AudioFile::decode(&resolved_path).ok().map(|decoded| {
                        let arc = Arc::new(decoded);
                        self.history
                            .audio_files
                            .insert(resolved_path.clone(), arc.clone());
                        arc
                    })
                });
            let Some(orig_file) = file_opt else {
                continue;
            };
            let (file, processing) = if let Some(cached) = self.history.audio_clips.get(&clip.id)
                && Arc::ptr_eq(cached.original_file(), &orig_file)
                && cached.props.pitch_semitones == clip.props.pitch_semitones
                && cached.processing.edit.same_pcm(&clip.edit)
            {
                let mut proc = cached.processing.clone();
                proc.edit.fades = clip.edit.fades;
                proc.edit.crossfade = clip.edit.crossfade;
                proc.edit.crossfade_root = clip.edit.crossfade_root;
                (cached.file.clone(), proc)
            } else {
                match crate::audio::sample_edit::Processing::prepare_for_bpm(
                    orig_file,
                    clip.edit.clone(),
                    clip.props.pitch_semitones,
                    self.timeline.tempo.bpm(),
                ) {
                    Ok(pair) => pair,
                    Err(_) => continue,
                }
            };
            restored_audio.push(AudioClip {
                processing,
                id: clip.id,
                name: clip.name,
                source_path_override: (file.source_path.as_deref()
                    != Some(resolved_path.as_path()))
                .then_some(resolved_path),
                file,
                track: track_idx,
                start_beat: clip.start_beat,
                props: clip.props,
                trim_start: clip.trim_start,
                trim_len: clip.trim_len,
            });
        }

        self.timeline.end_clip_drag();
        self.stop_audio();

        self.tracks = tracks;
        self.track_levels = levels;
        self.track_pans = pans;
        self.track_revision = self.track_revision.wrapping_add(1);
        if self.tracks.is_empty() {
            self.cursor_row = 0;
        } else {
            self.cursor_row = self.cursor_row.min(self.tracks.len() - 1);
        }
        self.selected_tracks
            .retain(|id| self.tracks.iter().any(|t| t.id == *id));

        self.timeline.clip = None;
        self.timeline.other_audio_clips.clear();
        for clip in restored_audio {
            self.timeline.insert_audio_clip(clip);
        }
        self.timeline.midi_clip = None;
        self.timeline.other_midi_clips.clear();
        for clip in midi_clips {
            self.timeline.insert_midi_clip(clip);
        }
        self.timeline.selected_audio.retain(|id| {
            self.timeline
                .clip
                .iter()
                .chain(&self.timeline.other_audio_clips)
                .any(|c| c.id == *id)
        });
        self.timeline.selected_midi.retain(|id| {
            self.timeline
                .midi_clip
                .iter()
                .chain(&self.timeline.other_midi_clips)
                .any(|c| c.id == *id)
        });
        if self.timeline.focused_audio.is_some_and(|id| {
            !self
                .timeline
                .clip
                .iter()
                .chain(&self.timeline.other_audio_clips)
                .any(|c| c.id == id)
        }) {
            self.timeline.focused_audio = None;
        }

        self.pattern_editor.session = drums;
        self.timeline.tempo = tempo;
        self.timeline.meter = meter;
        self.timeline.navigation = navigation;
        self.timeline.automation = automation;
        self.master_chain = master_chain;

        if let Some(opened_id) = self.piano_roll.opened_clip.as_ref().map(|c| c.id) {
            if let Some(updated) = self
                .timeline
                .midi_clip
                .iter()
                .chain(&self.timeline.other_midi_clips)
                .find(|c| c.id == opened_id)
                .cloned()
            {
                self.piano_roll.opened_clip = Some(updated);
            } else {
                self.piano_roll.opened_clip = None;
                self.piano_roll.open = false;
            }
        }

        self.sync_vst_effects();
        self.publish_track_fx();
        self.publish_all_automation();
        self.publish_clip_edit();
        self.sync_midi_focus();

        let (summary, signature) = self.capture_summary();
        let total = self.history.commits.len();
        self.history.cursor = index;
        self.history.current_signature = signature;
        self.history.last_summary = Some(summary);
        self.history.pending = None;
        self.history.last_check = Some(Instant::now());
        self.status_message = message.unwrap_or_else(|| {
            format!(
                "Loro time-travel: {} ({}/{total})",
                commit.message,
                index + 1
            )
        });
    }

    pub(in crate::ui) fn render_history_panel(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.history.open {
            return None;
        }
        let ds = &self.ds;
        let total = self.history.commits.len();
        let cursor = self.history.cursor;
        let can_undo = self.history.can_undo();
        let can_redo = self.history.can_redo();

        let rows: Vec<_> = self
            .history
            .commits
            .iter()
            .enumerate()
            .rev()
            .map(|(idx, commit)| {
                let is_current = idx == cursor;
                let is_future = idx > cursor;
                let stamp = format_commit_time(commit.timestamp);
                div()
                    .id(("history-entry", idx))
                    .px_2()
                    .py(px(5.))
                    .rounded(px(4.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .cursor_pointer()
                    .bg(if is_current {
                        rgba((ds.cyan << 8) | 0x26)
                    } else {
                        rgba(0x00000000)
                    })
                    .border_1()
                    .border_color(if is_current {
                        rgba((ds.cyan << 8) | 0x88)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|row| row.bg(rgba((ds.accent << 8) | 0x20)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(9.))
                                    .text_color(rgb(if is_current {
                                        ds.cyan
                                    } else if is_future {
                                        ds.border
                                    } else {
                                        ds.muted
                                    }))
                                    .child(if is_current { "●" } else { "○" }),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.))
                                    .text_color(rgb(if is_future { ds.muted } else { ds.text }))
                                    .child(commit.message.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_size(px(9.))
                                    .text_color(rgb(ds.muted))
                                    .child(format!("Δ{}", commit.ops)),
                            )
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_size(px(9.5))
                                    .text_color(rgb(ds.muted))
                                    .child(stamp),
                            ),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.jump_history(idx, None);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            })
            .collect();

        Some(
            div()
                .id("history-panel")
                .absolute()
                .top(px(ds.title_bar_height + 6.0))
                .left(px(180.0))
                .w(px(336.0))
                .max_h(px(420.0))
                .flex()
                .flex_col()
                .bg(rgb(ds.panel_dark))
                .border_1()
                .border_color(rgb(ds.frame_border()))
                .rounded(px(ds.radius_md))
                .shadow_lg()
                .p_2()
                .gap_2()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_, _, _, cx| cx.stop_propagation()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pb_1()
                        .border_b_1()
                        .border_color(rgb(ds.frame_border()))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .px(px(5.))
                                        .py(px(1.))
                                        .rounded(px(3.))
                                        .bg(rgba((ds.cyan << 8) | 0x24))
                                        .text_size(px(9.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(rgb(ds.cyan))
                                        .child("LORO"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(rgb(ds.text))
                                        .child(format!(
                                            "Historial ({}/{total})",
                                            if total == 0 { 0 } else { cursor + 1 }
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .id("history-btn-undo")
                                        .px(px(6.))
                                        .py(px(2.))
                                        .rounded(px(3.))
                                        .bg(rgba(
                                            (ds.text << 8) | if can_undo { 0x16 } else { 0x06 },
                                        ))
                                        .text_size(px(10.5))
                                        .text_color(rgb(if can_undo { ds.text } else { ds.muted }))
                                        .cursor_pointer()
                                        .child("↶")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.undo_project();
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("history-btn-redo")
                                        .px(px(6.))
                                        .py(px(2.))
                                        .rounded(px(3.))
                                        .bg(rgba(
                                            (ds.text << 8) | if can_redo { 0x16 } else { 0x06 },
                                        ))
                                        .text_size(px(10.5))
                                        .text_color(rgb(if can_redo { ds.text } else { ds.muted }))
                                        .cursor_pointer()
                                        .child("↷")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.redo_project();
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("history-btn-close")
                                        .px(px(6.))
                                        .py(px(2.))
                                        .rounded(px(3.))
                                        .bg(rgba((ds.text << 8) | 0x10))
                                        .hover(|b| b.bg(rgba((ds.danger << 8) | 0x35)))
                                        .text_size(px(10.5))
                                        .text_color(rgb(ds.text))
                                        .cursor_pointer()
                                        .child("✕")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.history.open = false;
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                ),
                        ),
                )
                .child(
                    div()
                        .id("history-list")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .children(rows),
                )
                .child(
                    div()
                        .pt_1()
                        .border_t_1()
                        .border_color(rgb(ds.frame_border()))
                        .text_size(px(9.5))
                        .text_color(rgb(ds.muted))
                        .child("CRDT persistido en .loro · Ctrl+Z / Ctrl+Y · clic para viajar"),
                )
                .into_any_element(),
        )
    }
}

fn describe_change(prev: &HistorySummary, next: &HistorySummary) -> String {
    if next.tracks.len() > prev.tracks.len() {
        let added = next.tracks.len() - prev.tracks.len();
        if added == 1
            && let Some(t) = next.tracks.last()
        {
            return format!("Añadir pista «{}»", t.name);
        }
        return format!("Añadir {added} pistas");
    }
    if next.tracks.len() < prev.tracks.len() {
        return "Eliminar pista".into();
    }
    if prev.tempo != next.tempo {
        return format!("Tempo → {:.1} BPM", next.tempo.bpm());
    }
    if prev.meter != next.meter {
        return format!(
            "Compás → {}/{}",
            next.meter.numerator(),
            next.meter.denominator()
        );
    }
    if prev.navigation != next.navigation {
        return if prev.navigation.markers != next.navigation.markers {
            "Editar marcadores".into()
        } else {
            "Ajustar región de loop".into()
        };
    }
    if prev.track_levels != next.track_levels || prev.track_pans != next.track_pans {
        return "Mezcla (volumen / pan)".into();
    }
    if prev.automation != next.automation {
        return "Editar automatización".into();
    }
    if prev.master_chain.nodes() != next.master_chain.nodes() {
        return "Editar Master FX".into();
    }
    if next.audio_count > prev.audio_count {
        return "Añadir / dividir clip de audio".into();
    }
    if next.audio_count < prev.audio_count {
        return "Eliminar clip de audio".into();
    }
    if next.midi_count > prev.midi_count {
        return "Añadir / duplicar clip MIDI".into();
    }
    if next.midi_count < prev.midi_count {
        return "Eliminar clip MIDI".into();
    }
    if prev.drums != next.drums {
        return "Editar secuenciador de batería".into();
    }
    if prev.tracks != next.tracks {
        let fx_changed = prev
            .tracks
            .iter()
            .zip(&next.tracks)
            .any(|(a, b)| a.chain.revision() != b.chain.revision());
        if fx_changed {
            return "Editar cadena FX".into();
        }
        return "Editar pistas (mute / solo / orden)".into();
    }
    if prev.midi_clips != next.midi_clips {
        return "Editar notas / clips MIDI".into();
    }
    "Editar clips de audio".into()
}
