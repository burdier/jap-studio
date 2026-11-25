//! Intercambio de dos copias de una sesión fuera de la UI y del motor.
pub(crate) mod live;
mod probe;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use elevate::project::ProjectDoc;
use iroh::{
    Endpoint, EndpointAddr, RelayMode, RelayUrl, SecretKey,
    endpoint::{Connection, RecvStream, SendStream, presets},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs,
    io::{self, Write},
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const PROTOCOL: &[u8] = b"hay/collab/1";
const MAX_UPDATES: usize = 16 * 1024 * 1024;
const MAX_INVITE: usize = 4096;
const SAVED: [u8; 1] = [1];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Route {
    Auto,
    Direct,
    Relay,
}

impl Route {
    fn parse(arg: &OsString) -> Result<Self> {
        match arg.to_str() {
            Some("auto") => Ok(Self::Auto),
            Some("direct") => Ok(Self::Direct),
            Some("relay") => Ok(Self::Relay),
            _ => Err("Usa auto, direct o relay".into()),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct Invitation {
    addr: EndpointAddr,
    route: Route,
    origin: u64,
    token: [u8; 32],
}

impl Invitation {
    fn decode(text: &str) -> Result<Self> {
        if text.len() > MAX_INVITE {
            return Err("Invitación demasiado larga".into());
        }
        let encoded = text
            .strip_prefix("hay1-")
            .ok_or("Invitación de Jap Studio inválida")?;
        Ok(serde_json::from_slice(&URL_SAFE_NO_PAD.decode(encoded)?)?)
    }

    fn encode(&self) -> Result<String> {
        Ok(format!(
            "hay1-{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(self)?)
        ))
    }
}

pub(super) fn run(args: Vec<OsString>) -> Result<()> {
    let usage = "Uso: --collab share <copia.loro> [auto|direct|relay] [URL] | join <copia.loro> <invitación> | probe <carpeta> <direct|relay> [URL]";
    let [command, path, rest @ ..] = args.as_slice() else {
        return Err(usage.into());
    };
    let path = Path::new(path);
    if command == "probe" {
        let (route, url) = route_args(rest)?;
        return probe::run(path, route, url);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let session = Session::load(path)?;

    runtime.block_on(async move {
        tokio::time::timeout(Duration::from_secs(120), async move {
            if command == "share" {
                let (route, url) = route_args(rest)?;
                share(session, route, url).await
            } else if command == "join" {
                let [invitation] = rest else {
                    return Err(usage.into());
                };
                join(
                    session,
                    Invitation::decode(invitation.to_str().ok_or(usage)?)?,
                )
                .await
            } else {
                Err(usage.into())
            }
        })
        .await
        .map_err(|_| "Se agotó el tiempo de conexión de Jap Studio")?
    })
}

fn route_args(args: &[OsString]) -> Result<(Route, Option<RelayUrl>)> {
    match args {
        [] => Ok((Route::Auto, None)),
        [route] => Ok((Route::parse(route)?, None)),
        [route, url] => {
            let route = Route::parse(route)?;
            if route == Route::Direct {
                return Err("Una conexión directa no usa URL de relay".into());
            }
            Ok((
                route,
                Some(url.to_str().ok_or("URL de relay inválida")?.parse()?),
            ))
        }
        _ => Err("Indica el modo de conexión y, opcionalmente, una URL de relay".into()),
    }
}

async fn endpoint(route: Route, relay: Option<RelayUrl>, share: bool) -> Result<Endpoint> {
    bind_endpoint(route, relay, share, PROTOCOL).await
}

async fn bind_endpoint(
    route: Route,
    relay: Option<RelayUrl>,
    share: bool,
    protocol: &[u8],
) -> Result<Endpoint> {
    let relay_mode = match (route, relay) {
        (Route::Direct, _) => RelayMode::Disabled,
        (_, Some(url)) => RelayMode::custom([url]),
        (_, None) => RelayMode::Default,
    };
    let mut builder = Endpoint::builder(presets::Minimal)
        .alpns(vec![protocol.to_vec()])
        .relay_mode(relay_mode);
    if route == Route::Relay {
        builder = builder.clear_ip_transports();
    }
    let endpoint = builder.bind().await?;
    if route != Route::Direct && share {
        endpoint.online().await;
    }
    Ok(endpoint)
}

async fn share(mut session: Session, route: Route, relay: Option<RelayUrl>) -> Result<()> {
    let endpoint = endpoint(route, relay, true).await?;
    let mut addr = endpoint.addr();
    if route == Route::Direct {
        // La dirección local también sirve cuando no hay una interfaz externa activa.
        for bound in endpoint
            .bound_sockets()
            .into_iter()
            .filter(SocketAddr::is_ipv4)
        {
            addr = addr.with_ip_addr((Ipv4Addr::LOCALHOST, bound.port()).into());
        }
    }
    let token = SecretKey::generate().to_bytes();
    let invitation = Invitation {
        addr,
        route,
        origin: session.origin,
        token,
    };
    println!("HAY_INVITE {}", invitation.encode()?);
    io::stdout().flush()?;

    let connection = endpoint
        .accept()
        .await
        .ok_or("Se cerró el endpoint")?
        .await?;
    let (mut send, mut recv) = connection.accept_bi().await?;
    let mut supplied = [0; 32];
    recv.read_exact(&mut supplied).await?;
    if supplied != token {
        return Err("La invitación no autoriza esta conexión".into());
    }
    let original_updates = session.updates()?;
    session.merge(&receive_updates(&mut recv).await?)?;
    send_updates(&mut send, &original_updates).await?;
    read_saved(&mut recv).await?;
    recv.read_to_end(0).await?;
    session.save()?;
    show_path(&connection, route).await?;
    send.write_all(&SAVED).await?;
    send.finish()?;
    if send.stopped().await?.is_some() {
        return Err("El invitado interrumpió la confirmación del guardado".into());
    }
    endpoint.close().await;
    Ok(())
}

async fn join(mut session: Session, invitation: Invitation) -> Result<()> {
    if session.origin != invitation.origin {
        return Err("Abre una copia del mismo proyecto para unirte".into());
    }
    let route = invitation.route;
    let endpoint = endpoint(route, invitation.addr.relay_urls().next().cloned(), false).await?;
    let connection = endpoint.connect(invitation.addr, PROTOCOL).await?;
    let (mut send, mut recv) = connection.open_bi().await?;
    send.write_all(&invitation.token).await?;
    send_updates(&mut send, &session.updates()?).await?;
    session.merge(&receive_updates(&mut recv).await?)?;
    session.save()?;
    send.write_all(&SAVED).await?;
    send.finish()?;
    read_saved(&mut recv).await?;
    recv.read_to_end(0).await?;
    show_path(&connection, route).await?;
    connection.closed().await;
    endpoint.close().await;
    Ok(())
}

async fn send_updates(send: &mut SendStream, updates: &[u8]) -> Result<()> {
    send.write_all(&(updates.len() as u32).to_be_bytes())
        .await?;
    send.write_all(updates).await?;
    Ok(())
}

async fn receive_updates(recv: &mut RecvStream) -> Result<Vec<u8>> {
    let mut size = [0; 4];
    recv.read_exact(&mut size).await?;
    let size = u32::from_be_bytes(size) as usize;
    if size == 0 || size > MAX_UPDATES {
        return Err("Tamaño de actualización fuera del límite de colaboración".into());
    }
    let mut updates = vec![0; size];
    recv.read_exact(&mut updates).await?;
    Ok(updates)
}

async fn read_saved(recv: &mut RecvStream) -> Result<()> {
    let mut status = [0; 1];
    recv.read_exact(&mut status).await?;
    if status != SAVED {
        return Err("El otro equipo no confirmó el guardado".into());
    }
    Ok(())
}

async fn show_path(connection: &Connection, expected: Route) -> Result<()> {
    let mut paths = connection.paths_stream();
    let path = paths
        .wait_any(|path| path.is_selected())
        .await
        .ok_or("Iroh no informó la ruta activa")?;
    let route = if path.is_ip() {
        Route::Direct
    } else if path.is_relay() {
        Route::Relay
    } else {
        return Err("Iroh informó una ruta desconocida".into());
    };
    if expected != Route::Auto && route != expected {
        return Err(format!("Se esperaba {expected:?} y se obtuvo {route:?}").into());
    }
    println!("HAY_SYNC {:?} {}", route, connection.remote_id());
    Ok(())
}

struct Session {
    path: PathBuf,
    original: Vec<u8>,
    origin: u64,
    document: ProjectDoc,
}

impl Session {
    fn load(path: &Path) -> Result<Self> {
        let original = fs::read(path)?;
        let document = ProjectDoc::import(&original)?;
        let origin = document.collaboration_origin()?;
        Ok(Self {
            path: path.to_owned(),
            original,
            origin,
            document,
        })
    }

    fn updates(&self) -> Result<Vec<u8>> {
        let updates = self.document.all_updates()?;
        if updates.len() > MAX_UPDATES {
            return Err("El historial del proyecto supera el límite de 16 MiB".into());
        }
        Ok(updates)
    }

    fn merge(&mut self, updates: &[u8]) -> Result<()> {
        if ProjectDoc::import(updates)?.collaboration_origin()? != self.origin {
            return Err("La actualización pertenece a otro proyecto".into());
        }
        self.document.merge(updates)?;
        Ok(())
    }

    fn save(&self) -> Result<()> {
        if fs::read(&self.path)? != self.original {
            return Err(
                "La copia local cambió en disco durante el intercambio. Repite la sincronización"
                    .into(),
            );
        }
        Ok(self.document.save(&self.path)?)
    }
}
