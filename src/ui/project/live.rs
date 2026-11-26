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
}

struct Prepared {
    effects: super::effects::PreparedEffects,
    initial: bool,
    document: ProjectDoc,
    snapshot: Snapshot,
    audio: Vec<AudioClip>,
    samplers: Vec<super::snapshot::PreparedSampler>,
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
    sent_assets: HashSet<String>,
    received_assets: HashMap<String, PathBuf>,
    inbox: VecDeque<Incoming>,
    prepare: Option<Task<()>>,
    prepared: Option<(Incoming, Result<Prepared, String>)>,
    _events: Task<()>,
    _flush: Task<()>,
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
        let result = self.shared_base().and_then(|document| {
            let root = self
                .project
                .path
                .as_deref()
                .and_then(Path::parent)
                .ok_or("Proyecto sin carpeta")?
                .to_owned();
            let updates = document.all_updates()?;
            let mut sent_assets = HashSet::new();
            let initial_snapshot = Snapshot::from_document(&document)?;
            let assets = collect_snapshot_assets(&initial_snapshot, &root, &mut sent_assets)?;
            self.start_shared_project(
                document,
                Start::Share { updates, assets },
                sent_assets,
                cx,
            )
        });
        if let Err(error) = result {
            self.show_share_error(error);
        }
    }

    pub(super) fn join_shared_project(&mut self, cx: &mut Context<Self>) {
        if self.project.shared.is_some() || self.project.dialog.is_some() {
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
                    let document = this.shared_base()?;
                    this.start_shared_project(document, Start::Join(invite), HashSet::new(), cx)
                })();
                if let Err(error) = result {
                    this.status_message = error;
                }
                cx.notify();
            });
        }));
    }

    fn shared_base(&mut self) -> Result<ProjectDoc, String> {
        if self.project_busy() {
            return Err("Espera a que termine la operación en curso".into());
        }
        self.project.checked = None;
        self.refresh_project_dirty();
        let bytes = self
            .project
            .document
            .as_deref()
            .filter(|_| !self.project.dirty)
            .ok_or("Guarda esta copia del proyecto antes de compartir o unirte")?;
        ProjectDoc::import(bytes)
    }

    fn start_shared_project(
        &mut self,
        document: ProjectDoc,
        start: Start,
        sent_assets: HashSet<String>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let host = matches!(&start, Start::Share { .. });
        let root = self
            .project
            .path
            .as_deref()
            .and_then(std::path::Path::parent)
            .ok_or("Proyecto sin carpeta")?
            .to_owned();
        let origin = document.collaboration_origin()?;
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
            origin,
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
            sent_assets,
            received_assets: HashMap::new(),
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
            Event::Connected => {
                shared.connected = true;
                if shared.initialized {
                    self.shared_project_connected();
                }
            }
            Event::Asset(asset) | Event::Packet(Packet::Asset(asset)) => {
                match store_received_asset(shared.origin, &asset, &mut shared.received_assets) {
                    Ok((label, local_path)) => {
                        if let Some(s) = local_path.to_str() {
                            shared.sent_assets.insert(s.to_owned());
                        }
                        self.status_message = format!("Audio sincronizado: {label}");
                    }
                    Err(error) => {
                        self.status_message = format!("Audio P2P: {error}");
                    }
                }
            }
            Event::Initial(bytes) => self.enqueue_shared_update(Incoming { base: None, bytes }),
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
        if local != shared.last {
            if !shared.initialized {
                return Err(CONFLICT.into());
            }
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
                prepared.samplers,
            )?;
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
        persisted.relativize_sources(&shared.root)?;
        let new_assets =
            collect_snapshot_assets(&persisted, &shared.root, &mut shared.sent_assets)?;
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
        let origin = shared.document.collaboration_origin()?;
        let root = shared.root.clone();
        let received_assets = shared.received_assets.clone();
        let pianos = shared.last.pianos.clone();
        let previous_samplers = shared.last.samplers.clone();
        let previous_tracks = shared.last.tracks.clone();
        let config = self.settings.audio.saved.clone();
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
                let initial = update.base.is_none();
                let document = if let Some(base) = &update.base {
                    merge_shared_update(document, base, &update.bytes)?
                } else {
                    ProjectDoc::import(&update.bytes)?
                };
                if document.collaboration_origin()? != origin {
                    return Err("El origen del proyecto compartido cambió".into());
                }
                let mut snapshot = Snapshot::from_document(&document)?;
                let audio = snapshot.prepare_resources_with_assets(
                    &root,
                    files,
                    &pianos,
                    &previous,
                    &received_assets,
                )?;
                let effects = super::effects::prepare(
                    &snapshot.tracks,
                    &previous_tracks,
                    &snapshot.master_chain,
                    None,
                    &config,
                )?;
                let samplers = snapshot.prepare_samplers(&previous_samplers)?;
                Ok(Prepared {
                    effects,
                    samplers,
                    initial,
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

fn collect_snapshot_assets(
    snapshot: &Snapshot,
    root: &Path,
    sent: &mut HashSet<String>,
) -> Result<Vec<SharedAsset>, String> {
    let mut assets = Vec::new();
    let sources = snapshot
        .audio
        .iter()
        .map(|clip| clip.source.as_str())
        .chain(snapshot.samplers.iter().map(|s| s.source.as_str()))
        .chain(
            snapshot
                .drums
                .kit
                .pieces
                .iter()
                .flat_map(|piece| piece.layers.iter().map(|layer| layer.source.as_str())),
        );

    for raw in sources {
        if raw.is_empty() || sent.contains(raw) {
            continue;
        }
        let path = root.join(raw);
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        if meta.len() as usize > MAX_ASSET_BYTES {
            return Err(format!(
                "El audio {} supera 32 MiB para envío P2P",
                path.file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
            ));
        }
        let bytes = std::fs::read(&path)
            .map_err(|e| format!("No se pudo leer audio {}: {e}", path.display()))?;
        sent.insert(raw.to_owned());
        assets.push(SharedAsset {
            key: raw.to_owned(),
            bytes,
        });
    }
    Ok(assets)
}

fn store_received_asset(
    origin: u64,
    asset: &SharedAsset,
    received: &mut HashMap<String, PathBuf>,
) -> Result<(String, PathBuf), String> {
    let raw_name = Path::new(&asset.key)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .unwrap_or("sample.wav");
    let clean_name: String = raw_name
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
    let clean_name = clean_name.trim_matches('.');
    let clean_name = if clean_name.is_empty() {
        "sample.wav"
    } else {
        clean_name
    };

    // Hash FNV-1a estable de 64 bits sobre la clave original para evitar colisiones de nombres.
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in asset.key.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }

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

    received.insert(asset.key.clone(), target.clone());
    received.insert(format!("name:{raw_name}"), target.clone());
    Ok((raw_name.to_owned(), target))
}
