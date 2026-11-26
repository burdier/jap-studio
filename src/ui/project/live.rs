//! Comparte el mismo estado editable que Abrir y Guardar.
mod apply;
mod invite;
mod presence;
pub(super) use invite::ShareInvite;
pub(super) use presence::IdentityDialog;

use super::{DawApp, Snapshot};
use crate::{
    audio::AudioClip,
    collaboration::live::{
        Event, MAX_ASSET_BYTES, Packet, Presence, Room, SharedAsset, Start,
    },
};
use elevate::project::ProjectDoc;
use gpui::{AppContext, Context, Task, Window};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    time::Duration,
};

const FLUSH_INTERVAL: Duration = Duration::from_millis(100);
const MAX_PENDING: usize = 32;
const CONFLICT: &str = "Ediciones simultáneas · Sesión cerrada · Guarda tus cambios en esta copia";

struct Incoming {
    base: Option<Vec<u8>>,
    bytes: Vec<u8>,
    force_sync: bool,
}

struct Prepared {
    effects: super::effects::PreparedEffects,
    components: std::collections::BTreeMap<
        elevate::musical::fx::FxId,
        std::sync::Arc<crate::audio::effects::ComponentEffect>,
    >,
    initial: bool,
    force_sync: bool,
    document: ProjectDoc,
    snapshot: Snapshot,
    audio: Vec<AudioClip>,
    samplers: Vec<super::snapshot::PreparedSampler>,
    vsts: Vec<super::snapshot::PreparedVst>,
    missing_vsts: Vec<elevate::project::ProjectVstInstrument>,
    warnings: Vec<String>,
}

pub(super) struct SharedProject {
    document: ProjectDoc,
    origin: u64,
    room: Room,
    host: bool,
    connected: bool,
    initialized: bool,
    invite: Option<String>,
    last_drum_step: Option<(
        std::time::Instant,
        crate::collaboration::live::DrumStepPresence,
    )>,
    sent_presence: Option<Presence>,
    peer: Option<Presence>,
    clear_cut_after_updates: bool,
    root: PathBuf,
    last: Snapshot,
    asset_paths: HashMap<String, PathBuf>,
    path_to_asset: HashMap<PathBuf, String>,
    sent_assets: HashSet<String>,
    preserved_vsts: Vec<elevate::project::ProjectVstInstrument>,
    inbox: VecDeque<Incoming>,
    prepare: Option<Task<()>>,
    prepared: Option<(Incoming, Result<Prepared, String>)>,
    _events: Task<()>,
    _flush: Task<()>,
}

impl SharedProject {
    pub(super) fn preserved_vsts(&self) -> &[elevate::project::ProjectVstInstrument] {
        &self.preserved_vsts
    }
}

impl DawApp {
    pub(in crate::ui) fn shared_project_label(&self) -> Option<String> {
        let shared = self.project.shared.as_ref()?;
        Some(if shared.connected && shared.initialized {
            shared.peer.as_ref().map_or_else(
                || "En sesión".into(),
                |p| {
                    if p.activity.is_empty() {
                        format!("En sesión · {}", p.alias)
                    } else {
                        format!("{} · {}", p.alias, p.activity)
                    }
                },
            )
        } else if shared.invite.is_some() {
            "Esperando invitado".into()
        } else {
            "Conectando".into()
        })
    }

    pub(super) fn share_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.project.dialog.is_some() {
            return;
        }
        self.project.share_invite = Some(ShareInvite::default());
        window.focus(&self.focus_handle, cx);
        if let Some(shared) = &self.project.shared {
            if shared.connected || !shared.host {
                self.show_share_error("Ya estás en una sesión compartida".into());
            }
            return;
        }
        let result = (|| {
            if self.project_busy() {
                return Err("Espera a que termine la operación en curso".into());
            }
            let root = self
                .project
                .path
                .as_deref()
                .and_then(Path::parent)
                .map(ToOwned::to_owned)
                .unwrap_or_else(collab_work_root);
            let name = self
                .project
                .path
                .as_deref()
                .and_then(Path::file_stem)
                .map(|s| s.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Sesión compartida".into());
            let mut persisted = self.session_snapshot()?;
            persisted.validate_support()?;
            persisted.resolve_piano_sources()?;
            let mut asset_paths = HashMap::new();
            let mut path_to_asset = HashMap::new();
            let mut sent_assets = HashSet::new();
            let assets = canonicalize_snapshot_assets(
                &mut persisted,
                &root,
                &mut asset_paths,
                &mut path_to_asset,
                &mut sent_assets,
                true,
            )?;
            let document = persisted.document_from(&name, self.project.document.as_deref())?;
            let origin = document.collaboration_origin()?;
            let updates = document.all_updates()?;
            self.start_shared_project(
                document,
                Some(origin),
                Start::Share { updates, assets },
                root,
                asset_paths,
                path_to_asset,
                sent_assets,
                cx,
            )
        })();
        if let Err(error) = result {
            self.show_share_error(error);
        }
    }

    pub(super) fn force_sync_shared_project(&mut self, cx: &mut Context<Self>) {
        if self.project_busy() {
            self.status_message = "Espera a que termine la operación en curso".into();
            return;
        }
        let Some(mut shared) = self.project.shared.take() else {
            self.status_message = "No hay una sesión compartida activa".into();
            return;
        };
        let result = (|| -> Result<usize, String> {
            if !shared.host {
                return Err("Solo el dueño de la sesión puede sincronizar".into());
            }
            if !shared.connected {
                return Err("Esperando a que se conecte tu invitado…".into());
            }
            let local = self.session_snapshot()?;
            let mut persisted = local.clone();
            persisted.validate_support()?;
            persisted.resolve_piano_sources()?;
            let assets = canonicalize_snapshot_assets(
                &mut persisted,
                &shared.root,
                &mut shared.asset_paths,
                &mut shared.path_to_asset,
                &mut shared.sent_assets,
                true,
            )?;
            let asset_count = assets.len();
            let candidate = shared.document.collaboration_fork();
            persisted.write_document(&candidate)?;
            let bytes = candidate.all_updates()?;
            for asset in assets {
                shared.room.send(Packet::Asset(asset))?;
            }
            shared.room.send(Packet::ForceSync(bytes))?;
            shared.document = candidate;
            shared.last = local;
            shared.inbox.clear();
            shared.prepared = None;
            Ok(asset_count)
        })();
        self.project.shared = Some(shared);
        match result {
            Ok(0) => {
                self.status_message = "Sesión sincronizada con el invitado".into();
            }
            Ok(n) => {
                self.status_message =
                    format!("Sesión sincronizada ({n} archivos de audio enviados)");
            }
            Err(error) => {
                self.status_message = error;
            }
        }
        cx.notify();
    }

    pub(super) fn join_shared_project(&mut self, cx: &mut Context<Self>) {
        if self.project.shared.is_some() || self.project.dialog.is_some() {
            return;
        }
        if self.project_busy() {
            self.status_message = "Espera a que termine la operación en curso".into();
            return;
        }
        let read = cx.read_from_clipboard_async();
        self.project.dialog = Some(cx.spawn(async move |this, cx| {
            let contents = read.await;
            let _ = this.update(cx, |this, cx| {
                this.project.dialog = None;
                let result = (|| {
                    let invite = contents
                        .ok()
                        .flatten()
                        .and_then(|item| item.text())
                        .filter(|text| text.trim_start().starts_with("hay-live:"))
                        .ok_or("Copia una invitación de Jap Studio y vuelve a pulsar Unirse")?;
                    // No hace falta tener el mismo proyecto guardado antes: el ID viaja en el token.
                    let document = ProjectDoc::new("Sesión compartida")?;
                    let root = this
                        .project
                        .path
                        .as_deref()
                        .and_then(Path::parent)
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(collab_work_root);
                    this.start_shared_project(
                        document,
                        None,
                        Start::Join(invite),
                        root,
                        HashMap::new(),
                        HashMap::new(),
                        HashSet::new(),
                        cx,
                    )
                })();
                if let Err(error) = result {
                    this.status_message = error;
                }
                cx.notify();
            });
        }));
    }

    #[allow(clippy::too_many_arguments)]
    fn start_shared_project(
        &mut self,
        document: ProjectDoc,
        origin: Option<u64>,
        start: Start,
        root: PathBuf,
        asset_paths: HashMap<String, PathBuf>,
        path_to_asset: HashMap<PathBuf, String>,
        sent_assets: HashSet<String>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let host = matches!(&start, Start::Share { .. });
        let initial_origin = origin.unwrap_or(0);
        let last = self.session_snapshot()?;
        let (room, mut events) = Room::start(origin, start).map_err(|e| e.to_string())?;
        let task = cx.spawn(async move |this, cx| {
            while let Some(event) = events.recv().await {
                let done = matches!(event, Event::Ended(_));
                if this
                    .update(cx, |this, cx| {
                        this.on_shared_event(event);
                        cx.notify();
                    })
                    .is_err()
                    || done
                {
                    break;
                }
            }
        });
        let flush = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FLUSH_INTERVAL).await;
                if this
                    .update(cx, |this, cx| this.flush_shared_project(cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        self.project.shared = Some(SharedProject {
            document,
            origin: initial_origin,
            room,
            host,
            connected: false,
            initialized: host,
            last_drum_step: None,
            sent_presence: None,
            peer: None,
            clear_cut_after_updates: false,
            invite: None,
            root,
            last,
            asset_paths,
            path_to_asset,
            sent_assets,
            preserved_vsts: Vec::new(),
            inbox: VecDeque::new(),
            prepare: None,
            prepared: None,
            _events: task,
            _flush: flush,
        });
        self.status_message = "Conectando sesión compartida…".into();
        Ok(())
    }

    pub(super) fn leave_shared_project(&mut self) {
        self.project.share_invite = None;
        self.timeline.peer_clip = None;
        if let Some(shared) = self.project.shared.take() {
            if let Ok(bytes) = shared.document.snapshot() {
                self.project.document = Some(bytes);
            }
            self.status_message = "Sesión compartida cerrada".into();
        }
    }

    pub(super) fn shared_document_for_save(
        &mut self,
        snapshot: &Snapshot,
    ) -> Result<Option<Vec<u8>>, String> {
        if let Some(shared) = &self.project.shared
            && shared.initialized
            && shared.connected
            && shared.last != *snapshot
        {
            let candidate = shared.document.collaboration_fork();
            self.credit_shared_edits(&candidate, &shared.last, snapshot)?;
            return candidate.snapshot().map(Some);
        }
        self.shared_document()
    }

    pub(super) fn shared_document(&self) -> Result<Option<Vec<u8>>, String> {
        self.project
            .shared
            .as_ref()
            .map(|shared| shared.document.snapshot())
            .transpose()
    }

    fn on_shared_event(&mut self, event: Event) {
        let Some(shared) = &mut self.project.shared else {
            return;
        };
        match event {
            Event::Invitation(invite) => {
                shared.invite = Some(invite);
                self.status_message = "Invitación lista".into();
            }
            Event::Origin(origin) => {
                shared.origin = origin;
            }
            Event::Connected => {
                shared.connected = true;
                if shared.initialized {
                    self.shared_project_connected();
                }
            }
            Event::Asset(asset) | Event::Packet(Packet::Asset(asset)) => {
                match store_received_asset(
                    shared.origin,
                    &asset,
                    &mut shared.asset_paths,
                    &mut shared.path_to_asset,
                ) {
                    Ok((label, _local_path)) => {
                        shared.sent_assets.insert(asset.key);
                        self.status_message = format!("Audio sincronizado: {label}");
                    }
                    Err(error) => {
                        self.status_message = format!("Audio P2P: {error}");
                    }
                }
            }
            Event::Initial(bytes) => self.enqueue_shared_update(Incoming {
                base: None,
                bytes,
                force_sync: false,
            }),
            Event::Packet(Packet::ForceSync(bytes)) => {
                shared.inbox.clear();
                shared.prepared = None;
                self.enqueue_shared_update(Incoming {
                    base: None,
                    bytes,
                    force_sync: true,
                });
            }
            Event::Packet(Packet::Presence(mut p)) => {
                // La recepción es ordenada, pero materializar un cambio Loro es asíncrono.
                shared.clear_cut_after_updates = p.band_cut.is_none()
                    && (!shared.inbox.is_empty()
                        || shared.prepare.is_some()
                        || shared.prepared.is_some());
                if shared.clear_cut_after_updates {
                    p.band_cut = shared.peer.as_ref().and_then(|peer| peer.band_cut);
                }
                self.timeline.peer_clip = p.clip.map(|id| (id, p.alias.clone()));
                shared.peer = Some(p);
            }
            Event::Packet(Packet::Changes { base, bytes }) => {
                self.enqueue_shared_update(Incoming {
                    base: Some(base),
                    bytes,
                    force_sync: false,
                });
            }
            Event::Ended(message) => {
                let show_error = self.project.share_invite.is_some();
                self.leave_shared_project();
                if show_error {
                    self.project.share_invite = Some(ShareInvite::default());
                }
                self.show_share_error(message);
            }
        }
    }

    fn shared_project_connected(&mut self) {
        self.project.share_invite = None;
        self.status_message = "Sesión conectada · Proyecto sincronizado".into();
    }

    fn enqueue_shared_update(&mut self, update: Incoming) {
        if let Some(shared) = &mut self.project.shared {
            if shared.inbox.len() < MAX_PENDING {
                shared.inbox.push_back(update);
            } else {
                self.leave_shared_project();
                self.status_message = "Demasiados cambios pendientes · Guarda tu copia".into();
            }
        }
    }

    fn flush_shared_project(&mut self, cx: &mut Context<Self>) {
        self.publish_presence();
        if self.project_busy()
            || self.timeline.editing_project_clips()
            || self.piano_roll.drag.is_some()
            || self.piano_roll.pitch_drag.is_some()
            || self.piano_roll.control_drag
            || self.piano_roll.paint_stroke.is_some()
            || self.pattern_editor.editing_project_drums()
            || self.track_control_drag.is_some()
            || self.track_reorder_drag.is_some()
        {
            return;
        }
        let Some(mut shared) = self.project.shared.take() else {
            return;
        };
        let result = self.sync_shared_project(&mut shared, cx);
        self.project.shared = Some(shared);
        if let Err(error) = result {
            self.leave_shared_project();
            self.show_share_error(error);
            cx.notify();
        }
    }

    fn sync_shared_project(
        &mut self,
        shared: &mut SharedProject,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if !shared.connected || shared.prepare.is_some() {
            return Ok(());
        }
        let local = self.session_snapshot()?;
        let force_incoming = shared
            .prepared
            .as_ref()
            .is_some_and(|(inc, _)| inc.force_sync);
        if !force_incoming && shared.initialized && local != shared.last {
            self.publish_shared_snapshot(shared, local)?;
            if let Some((update, _)) = shared.prepared.take() {
                // Hay una edición nueva desde que empezó la preparación.
                shared.inbox.push_front(update);
            }
            cx.notify();
        } else if let Some((_, result)) = shared.prepared.take() {
            let prepared = result?;
            self.install_shared_snapshot(
                &local,
                prepared.snapshot,
                prepared.audio,
                prepared.effects,
                prepared.components,
                prepared.samplers,
                prepared.vsts,
            )?;
            shared.preserved_vsts = prepared.missing_vsts;
            shared.last = self.session_snapshot()?;
            shared.document = prepared.document;
            if shared.clear_cut_after_updates && shared.inbox.is_empty() {
                if let Some(peer) = &mut shared.peer {
                    peer.band_cut = None;
                }
                shared.clear_cut_after_updates = false;
            }
            if prepared.initial {
                shared.initialized = true;
                self.shared_project_connected();
            } else if prepared.force_sync {
                self.status_message = "Sesión sincronizada por el anfitrión".into();
            }
            if !prepared.warnings.is_empty() {
                self.status_message = format!("Aviso: {}", prepared.warnings.join(" · "));
            }
            cx.notify();
        } else if let Some(update) = shared.inbox.pop_front() {
            self.prepare_shared_update(shared, update, cx)?;
        }
        Ok(())
    }

    fn publish_shared_snapshot(
        &mut self,
        shared: &mut SharedProject,
        local: Snapshot,
    ) -> Result<(), String> {
        let mut persisted = local.clone();
        persisted.validate_support()?;
        if persisted.pianos != shared.last.pianos {
            persisted.resolve_piano_sources()?;
        }
        let new_assets = canonicalize_snapshot_assets(
            &mut persisted,
            &shared.root,
            &mut shared.asset_paths,
            &mut shared.path_to_asset,
            &mut shared.sent_assets,
            false,
        )?;
        let candidate = shared.document.collaboration_fork();
        let version = candidate.collaboration_version();
        persisted.write_document(&candidate)?;
        self.credit_shared_edits(&candidate, &shared.last, &local)?;
        if candidate.collaboration_version() != version {
            for asset in new_assets {
                shared.room.send(Packet::Asset(asset))?;
            }
            let bytes = candidate.collaboration_updates(&version)?;
            let base = shared.document.collaboration_frontiers().encode();
            shared.room.send(Packet::Changes { base, bytes })?;
            shared.document = candidate;
        }
        shared.last = local;
        self.project.checked = None;
        Ok(())
    }

    fn prepare_shared_update(
        &self,
        shared: &mut SharedProject,
        update: Incoming,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let document = shared.document.collaboration_fork();
        let expected_origin = shared.origin;
        let root = shared.root.clone();
        let asset_paths = shared.asset_paths.clone();
        let pianos = shared.last.pianos.clone();
        let previous_samplers = shared.last.samplers.clone();
        let previous_vsts = shared.last.vsts.clone();
        let previous_tracks = shared.last.tracks.clone();
        let config = self.settings.audio.saved.clone();
        let registry = self.lua_components.registry.clone();
        let rate = config.sample_rate.unwrap_or(48_000);
        let previous: Vec<_> = self
            .timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
            .cloned()
            .collect();
        let files = self
            .timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
            .filter_map(|clip| {
                let source = clip
                    .source_path_override
                    .as_ref()
                    .or(clip.file.source_path.as_ref())?;
                Some((source.clone(), clip.original_file().clone()))
            })
            .collect();
        let job = cx.background_spawn(async move {
            let result = (|| {
                let initial = update.base.is_none() && !update.force_sync;
                let force_sync = update.force_sync;
                let document = if let Some(base) = &update.base {
                    merge_shared_update(document, base, &update.bytes)?
                } else {
                    ProjectDoc::import(&update.bytes)?
                };
                if expected_origin != 0 && document.collaboration_origin()? != expected_origin {
                    return Err("El origen del proyecto compartido cambió".into());
                }
                let mut snapshot = Snapshot::from_document(&document)?;
                let (audio, mut warnings) = snapshot.prepare_resources_with_assets(
                    &root,
                    files,
                    &pianos,
                    &previous,
                    &asset_paths,
                    true,
                )?;
                let (effects, fx_warnings) = super::effects::prepare_tolerant(
                    &snapshot.tracks,
                    &previous_tracks,
                    &snapshot.master_chain,
                    None,
                    &config,
                )?;
                warnings.extend(fx_warnings);
                let components = super::effects::prepare_components(
                    &snapshot.tracks,
                    &snapshot.master_chain,
                    &registry,
                    rate,
                )?;
                let samplers = snapshot.prepare_samplers(&previous_samplers)?;
                let (vsts, missing_vsts, vst_warnings) =
                    snapshot.prepare_vsts_tolerant(&previous_vsts, &config);
                warnings.extend(vst_warnings);
                Ok(Prepared {
                    effects,
                    components,
                    samplers,
                    vsts,
                    missing_vsts,
                    warnings,
                    initial,
                    force_sync,
                    document,
                    snapshot,
                    audio,
                })
            })();
            (update, result)
        });
        shared.prepare = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(shared) = &mut this.project.shared {
                    shared.prepare = None;
                    shared.prepared = Some(result);
                    this.flush_shared_project(cx);
                }
            });
        }));
        Ok(())
    }
}

fn merge_shared_update(
    mut document: ProjectDoc,
    base: &[u8],
    bytes: &[u8],
) -> Result<ProjectDoc, String> {
    let base = loro::Frontiers::decode(base).map_err(|e| e.to_string())?;
    if base != document.collaboration_frontiers() {
        let mut remote = document.collaboration_fork_at(&base)?;
        let ancestor = Snapshot::from_document(&remote)?;
        remote.merge(bytes)?;
        if ancestor.conflicts_with(
            &Snapshot::from_document(&document)?,
            &Snapshot::from_document(&remote)?,
        ) {
            return Err(CONFLICT.into());
        }
        document
            .merge(bytes)
            .map_err(|error| format!("{CONFLICT} · {error}"))?;
    } else {
        document.merge(bytes)?;
    }
    Ok(document)
}

fn collab_work_root() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("jap-studio/collab-work")
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn sanitize_asset_filename(raw: &str) -> String {
    let base = raw
        .rsplit(['/', '\\'])
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or("sample.wav");
    let clean: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .take(96)
        .collect();
    let clean = clean.trim_matches('.');
    if clean.is_empty() {
        "sample.wav".into()
    } else {
        clean.into()
    }
}

/// Reescribe todas las rutas de audio del snapshot a claves portátiles por contenido
/// (`p2p-asset://<hash>/<nombre>`) para eliminar problemas de rutas entre máquinas.
fn canonicalize_snapshot_assets(
    snapshot: &mut Snapshot,
    root: &Path,
    asset_paths: &mut HashMap<String, PathBuf>,
    path_to_asset: &mut HashMap<PathBuf, String>,
    sent: &mut HashSet<String>,
    force_send_all: bool,
) -> Result<Vec<SharedAsset>, String> {
    let mut assets = Vec::new();
    let mut queued = HashSet::new();
    let sources = snapshot
        .audio
        .iter_mut()
        .map(|clip| &mut clip.source)
        .chain(snapshot.samplers.iter_mut().map(|s| &mut s.source))
        .chain(
            snapshot
                .drums
                .kit
                .pieces
                .iter_mut()
                .flat_map(|piece| piece.layers.iter_mut().map(|layer| &mut layer.source)),
        );

    for source in sources {
        if source.is_empty() {
            continue;
        }
        let (key, local_path, cached_bytes) = if source.starts_with("p2p-asset://") {
            let key = source.clone();
            let Some(path) = asset_paths.get(&key).cloned() else {
                continue;
            };
            (key, path, None)
        } else {
            let path = root.join(source.as_str());
            let canonical = std::fs::canonicalize(&path).unwrap_or(path);
            if let Some(existing_key) = path_to_asset.get(&canonical).cloned() {
                *source = existing_key.clone();
                (existing_key, canonical, None)
            } else {
                let Ok(meta) = std::fs::metadata(&canonical) else {
                    continue;
                };
                if !meta.is_file() {
                    continue;
                }
                if meta.len() as usize > MAX_ASSET_BYTES {
                    return Err(format!(
                        "El audio {} supera 32 MiB para envío P2P",
                        canonical
                            .file_name()
                            .unwrap_or(canonical.as_os_str())
                            .to_string_lossy()
                    ));
                }
                let bytes = std::fs::read(&canonical)
                    .map_err(|e| format!("No se pudo leer audio {}: {e}", canonical.display()))?;
                let hash = fnv1a_64(&bytes);
                let clean = sanitize_asset_filename(source);
                let key = format!("p2p-asset://{hash:016x}/{clean}");
                asset_paths.insert(key.clone(), canonical.clone());
                asset_paths.insert(format!("name:{clean}"), canonical.clone());
                path_to_asset.insert(canonical.clone(), key.clone());
                *source = key.clone();
                (key, canonical, Some(bytes))
            }
        };

        let should_send = (force_send_all || !sent.contains(&key)) && queued.insert(key.clone());
        if should_send {
            let bytes = match cached_bytes {
                Some(b) => b,
                None => {
                    let meta = std::fs::metadata(&local_path).map_err(|e| {
                        format!("No se pudo leer audio {}: {e}", local_path.display())
                    })?;
                    if meta.len() as usize > MAX_ASSET_BYTES {
                        return Err("Un archivo de audio supera 32 MiB para envío P2P".into());
                    }
                    std::fs::read(&local_path).map_err(|e| {
                        format!("No se pudo leer audio {}: {e}", local_path.display())
                    })?
                }
            };
            sent.insert(key.clone());
            assets.push(SharedAsset { key, bytes });
        }
    }
    Ok(assets)
}

fn store_received_asset(
    origin: u64,
    asset: &SharedAsset,
    asset_paths: &mut HashMap<String, PathBuf>,
    path_to_asset: &mut HashMap<PathBuf, String>,
) -> Result<(String, PathBuf), String> {
    let clean_name = sanitize_asset_filename(&asset.key);
    let hash = fnv1a_64(&asset.bytes);

    let dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("jap-studio/collab/{origin:016x}"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(format!("{hash:016x}_{clean_name}"));

    let needs_write = std::fs::metadata(&target)
        .map(|m| m.len() as usize != asset.bytes.len())
        .unwrap_or(true);
    if needs_write {
        let tmp = target.with_extension(format!("tmp.{}", std::process::id()));
        std::fs::write(&tmp, &asset.bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    }

    let canonical = std::fs::canonicalize(&target).unwrap_or(target);
    asset_paths.insert(asset.key.clone(), canonical.clone());
    asset_paths.insert(format!("name:{clean_name}"), canonical.clone());
    path_to_asset.insert(canonical.clone(), asset.key.clone());
    Ok((clean_name, canonical))
}
