//! Sesión Iroh de dos pares con operaciones Loro agrupadas por revisión.
use super::{Invitation, MAX_UPDATES, Result, Route, bind_endpoint, receive_updates, send_updates};
use iroh::{
    Endpoint, SecretKey,
    endpoint::{Connection, RecvStream, SendStream},
};
use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

const PROTOCOL: &[u8] = b"hay/collab/live/4";
const INVITE_PREFIX: &str = "hay-live:";
const QUEUE: usize = 256;
const MAX_FRONTIERS: usize = 4096;
const ASSET_FRAME: u16 = u16::MAX;
const MAX_ASSET_KEY: usize = 4096;
pub(crate) const MAX_ASSET_BYTES: usize = 32 * 1024 * 1024;
const MAX_INITIAL_ASSETS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Presence {
    pub alias: String,
    pub activity: String,
    pub clip: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<NotePresence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drum_step: Option<DrumStepPresence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub band_cut: Option<BandCutPresence>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct BandCutPresence {
    pub track: u32,
    pub crossover: u64,
    pub from_hz: u16,
    pub hz: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct DrumStepPresence {
    pub pattern: u32,
    pub piece: u32,
    pub tick: i64,
    pub pitch: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct NotePresence {
    pub id: u64,
    pub start: i64,
    pub duration: i64,
    pub pitch: u8,
}

pub(crate) fn display_text(value: &str, limit: usize) -> String {
    value
        .trim()
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(limit)
        .collect()
}

#[derive(Clone)]
pub(crate) struct SharedAsset {
    pub key: String,
    pub bytes: Vec<u8>,
}

pub(crate) enum Packet {
    Presence(Presence),
    Asset(SharedAsset),
    Changes { base: Vec<u8>, bytes: Vec<u8> },
}

pub(crate) enum Event {
    Invitation(String),
    Asset(SharedAsset),
    Initial(Vec<u8>),
    Connected,
    Packet(Packet),
    Ended(String),
}

pub(crate) enum Start {
    Share {
        updates: Vec<u8>,
        assets: Vec<SharedAsset>,
    },
    Join(String),
}

pub(crate) struct Room {
    outgoing: mpsc::Sender<Packet>,
    _stop: oneshot::Sender<()>,
}

impl Room {
    pub fn start(origin: u64, start: Start) -> Result<(Self, mpsc::Receiver<Event>)> {
        let (outgoing, inbox) = mpsc::channel(QUEUE);
        let (events, receiver) = mpsc::channel(QUEUE);
        let (stop, canceled) = oneshot::channel();
        std::thread::Builder::new()
            .name("hay-collaboration".into())
            .spawn(move || {
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map(|runtime| {
                        runtime.block_on(async {
                            tokio::select! {
                                result = session(origin, start, inbox, &events) => result,
                                _ = canceled => Ok(()),
                            }
                        })
                    });
                let message = match result {
                    Ok(Ok(())) => "Sesión compartida cerrada".into(),
                    Ok(Err(error)) => format!("Colaboración: {error}"),
                    Err(error) => format!("Colaboración: {error}"),
                };
                let _ = events.blocking_send(Event::Ended(message));
            })?;
        Ok((
            Self {
                outgoing,
                _stop: stop,
            },
            receiver,
        ))
    }

    pub fn send(&self, packet: Packet) -> std::result::Result<(), String> {
        if matches!(&packet, Packet::Changes { base, bytes }
            if base.len() > MAX_FRONTIERS || bytes.len() > MAX_UPDATES)
        {
            return Err("El cambio supera el tamaño permitido por la sesión".into());
        }
        if matches!(&packet, Packet::Asset(asset)
            if asset.key.is_empty()
                || asset.key.len() > MAX_ASSET_KEY
                || asset.bytes.len() > MAX_ASSET_BYTES)
        {
            return Err("El archivo de audio supera 32 MiB o tiene ruta inválida".into());
        }
        self.outgoing
            .try_send(packet)
            .map_err(|_| "Se interrumpió el envío de cambios".into())
    }
}

async fn session(
    origin: u64,
    start: Start,
    incoming: mpsc::Receiver<Packet>,
    events: &mpsc::Sender<Event>,
) -> Result<()> {
    let (endpoint, connection, mut send, mut recv) =
        tokio::time::timeout(Duration::from_secs(120), connect(origin, start, events))
            .await
            .map_err(|_| "Se agotó el tiempo de la invitación")??;
    let result = exchange(&mut send, &mut recv, incoming, events).await;
    let disconnected = connection.close_reason().is_some();
    endpoint.close().await;
    if disconnected {
        return Ok(());
    }
    result
}

async fn connect(
    origin: u64,
    start: Start,
    events: &mpsc::Sender<Event>,
) -> Result<(Endpoint, Connection, SendStream, RecvStream)> {
    let (endpoint, connection, send, recv) = match start {
        Start::Share { updates, assets } => {
            if updates.len() > MAX_UPDATES {
                return Err("El proyecto supera 16 MiB de operaciones Loro".into());
            }
            if assets.len() > MAX_INITIAL_ASSETS {
                return Err("El proyecto supera el máximo de archivos de audio iniciales".into());
            }
            let endpoint = bind_endpoint(Route::Auto, None, false, PROTOCOL).await?;
            // Publicar la ruta de relay cuando esté disponible y conservar acceso local sin Internet.
            let _ = tokio::time::timeout(Duration::from_secs(5), endpoint.online()).await;
            let mut addr = endpoint.addr();
            for bound in endpoint
                .bound_sockets()
                .into_iter()
                .filter(SocketAddr::is_ipv4)
            {
                addr = addr.with_ip_addr((Ipv4Addr::LOCALHOST, bound.port()).into());
            }
            let token = SecretKey::generate().to_bytes();
            let invite = Invitation {
                addr,
                route: Route::Auto,
                origin,
                token,
            };
            events
                .send(Event::Invitation(format!(
                    "{INVITE_PREFIX}{}",
                    invite.encode()?
                )))
                .await?;
            let connection = endpoint.accept().await.ok_or("Se cerró la sesión")?.await?;
            let (mut send, mut recv) = connection.accept_bi().await?;
            let mut supplied = [0; 32];
            recv.read_exact(&mut supplied).await?;
            if supplied != token {
                return Err("La invitación no autoriza esta conexión".into());
            }
            send.write_all(&(assets.len() as u32).to_be_bytes()).await?;
            for asset in &assets {
                send_asset(&mut send, asset).await?;
            }
            send_updates(&mut send, &updates).await?;
            (endpoint, connection, send, recv)
        }
        Start::Join(code) => {
            let code = code
                .trim()
                .strip_prefix(INVITE_PREFIX)
                .ok_or("Copia una invitación de sesión de Jap Studio")?;
            let invite = Invitation::decode(code)?;
            if invite.origin != origin {
                return Err("Abre una copia guardada del mismo proyecto para unirte".into());
            }
            let endpoint = bind_endpoint(
                Route::Auto,
                invite.addr.relay_urls().next().cloned(),
                false,
                PROTOCOL,
            )
            .await?;
            let connection = endpoint.connect(invite.addr, PROTOCOL).await?;
            let (mut send, mut recv) = connection.open_bi().await?;
            send.write_all(&invite.token).await?;
            let mut count_buf = [0; 4];
            recv.read_exact(&mut count_buf).await?;
            let count = u32::from_be_bytes(count_buf) as usize;
            if count > MAX_INITIAL_ASSETS {
                return Err("Demasiados archivos de audio iniciales".into());
            }
            for _ in 0..count {
                let asset = receive_asset(&mut recv).await?;
                events.send(Event::Asset(asset)).await?;
            }
            let bytes = receive_updates(&mut recv).await?;
            events.send(Event::Initial(bytes)).await?;
            (endpoint, connection, send, recv)
        }
    };
    // El mismo stream permanece abierto para cambios posteriores en ambas direcciones.
    events.send(Event::Connected).await?;
    Ok((endpoint, connection, send, recv))
}

async fn exchange(
    send: &mut SendStream,
    recv: &mut RecvStream,
    mut incoming: mpsc::Receiver<Packet>,
    events: &mpsc::Sender<Event>,
) -> Result<()> {
    // read_exact puede quedar a mitad de un mensaje. Mantener una sola lectura hasta terminarla.
    let reader = async {
        loop {
            let mut length = [0; 2];
            recv.read_exact(&mut length).await?;
            let length = usize::from(u16::from_be_bytes(length));
            if length == 0 {
                let mut n = [0; 2];
                recv.read_exact(&mut n).await?;
                let n = usize::from(u16::from_be_bytes(n));
                if n == 0 || n > 2048 {
                    return Err("Presencia inválida".into());
                }
                let mut data = vec![0; n];
                recv.read_exact(&mut data).await?;
                let mut presence: Presence = serde_json::from_slice(&data)?;
                presence.alias = display_text(&presence.alias, 36);
                presence.activity = display_text(&presence.activity, 96);
                if presence.alias.is_empty() {
                    return Err("Nombre de colaborador vacío".into());
                }
                events
                    .send(Event::Packet(Packet::Presence(presence)))
                    .await?;
                continue;
            }
            if length == usize::from(ASSET_FRAME) {
                let asset = receive_asset(recv).await?;
                events.send(Event::Packet(Packet::Asset(asset))).await?;
                continue;
            }
            if length > MAX_FRONTIERS {
                return Err("Versión de colaboración inválida".into());
            }
            let mut base = vec![0; length];
            recv.read_exact(&mut base).await?;
            let bytes = receive_updates(recv).await?;
            events
                .send(Event::Packet(Packet::Changes { base, bytes }))
                .await?;
        }
    };
    let writer = async {
        while let Some(packet) = incoming.recv().await {
            match packet {
                Packet::Presence(presence) => {
                    let data = serde_json::to_vec(&presence)?;
                    send.write_all(&[0, 0]).await?;
                    send.write_all(&(data.len() as u16).to_be_bytes()).await?;
                    send.write_all(&data).await?;
                }
                Packet::Asset(asset) => {
                    send.write_all(&ASSET_FRAME.to_be_bytes()).await?;
                    send_asset(send, &asset).await?;
                }
                Packet::Changes { base, bytes } => {
                    send.write_all(&(base.len() as u16).to_be_bytes()).await?;
                    send.write_all(&base).await?;
                    send_updates(send, &bytes).await?;
                }
            }
        }
        Ok(())
    };
    tokio::select! { result = reader => result, result = writer => result }
}

async fn send_asset(send: &mut SendStream, asset: &SharedAsset) -> Result<()> {
    if asset.key.is_empty()
        || asset.key.len() > MAX_ASSET_KEY
        || asset.bytes.len() > MAX_ASSET_BYTES
    {
        return Err("Archivo de audio inválido o demasiado grande para P2P".into());
    }
    let key_bytes = asset.key.as_bytes();
    send.write_all(&(key_bytes.len() as u16).to_be_bytes()).await?;
    send.write_all(key_bytes).await?;
    send.write_all(&(asset.bytes.len() as u32).to_be_bytes()).await?;
    send.write_all(&asset.bytes).await?;
    Ok(())
}

async fn receive_asset(recv: &mut RecvStream) -> Result<SharedAsset> {
    let mut klen = [0; 2];
    recv.read_exact(&mut klen).await?;
    let klen = usize::from(u16::from_be_bytes(klen));
    if klen == 0 || klen > MAX_ASSET_KEY {
        return Err("Clave de archivo de audio inválida".into());
    }
    let mut key_buf = vec![0; klen];
    recv.read_exact(&mut key_buf).await?;
    let key = String::from_utf8(key_buf)?;
    let mut blen = [0; 4];
    recv.read_exact(&mut blen).await?;
    let blen = u32::from_be_bytes(blen) as usize;
    if blen > MAX_ASSET_BYTES {
        return Err("Archivo de audio recibido supera 32 MiB".into());
    }
    let mut bytes = vec![0; blen];
    recv.read_exact(&mut bytes).await?;
    Ok(SharedAsset { key, bytes })
}
