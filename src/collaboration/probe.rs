//! Prueba dirigida que arranca dos procesos del mismo ejecutable y verifica ambas copias.

use super::{Result, Route};
use elevate::{
    musical::{
        drums::DrumSession,
        time::Tick,
        track::{Track, TrackKind},
    },
    project::ProjectDoc,
};
use iroh::RelayUrl;
use std::{
    env, fs,
    io::{BufRead, BufReader, Read},
    path::Path,
    process::{Command, Stdio},
};

pub(super) fn run(folder: &Path, route: Route, url: Option<RelayUrl>) -> Result<()> {
    let mode = match route {
        Route::Direct => "direct",
        Route::Relay => "relay",
        Route::Auto => return Err("La prueba requiere direct o relay".into()),
    };
    fs::create_dir(folder)?;
    let host_path = folder.join("anfitrion.loro");
    let guest_path = folder.join("invitado.loro");

    let track = Track::new("Mezcla", TrackKind::Audio);
    let mut drums = DrumSession::default();
    let hat = drums.kit.add_sample("HiHat.wav".into())?;
    let base = ProjectDoc::from_tracks("Prueba P2P", std::slice::from_ref(&track))?;
    base.set_track_level(track.id, 75)?;
    base.set_drum_session(&drums)?;
    let bytes = base.snapshot()?;
    let host_doc = ProjectDoc::import(&bytes)?;
    let guest_doc = ProjectDoc::import(&bytes)?;

    drums
        .active_pattern_mut()
        .notes
        .add_note(hat, Tick(240), Tick(120), 36, 93)?;
    host_doc.set_drum_session(&drums)?;
    host_doc.save(&host_path)?;
    guest_doc.set_track_level(track.id, 53)?;
    guest_doc.save(&guest_path)?;

    let exe = env::current_exe()?;
    let mut share = Command::new(&exe);
    share.args(["--collab", "share"]).arg(&host_path).arg(mode);
    if let Some(url) = url {
        share.arg(url.to_string());
    }
    let mut host = share.stdout(Stdio::piped()).spawn()?;
    let exchanged = (|| -> Result<(String, String)> {
        let mut output = BufReader::new(host.stdout.take().ok_or("Falta la salida del anfitrión")?);
        let mut line = String::new();
        output.read_line(&mut line)?;
        let invite = line
            .trim()
            .strip_prefix("HAY_INVITE ")
            .ok_or("El anfitrión no generó una invitación")?;
        let guest = Command::new(&exe)
            .args(["--collab", "join"])
            .arg(&guest_path)
            .arg(invite)
            .output()?;
        if !guest.status.success() {
            return Err(format!(
                "El invitado falló: {}",
                String::from_utf8_lossy(&guest.stderr).trim()
            )
            .into());
        }
        let mut log = String::new();
        output.read_to_string(&mut log)?;
        if !host.wait()?.success() {
            return Err("La instancia anfitriona falló".into());
        }
        Ok((log, String::from_utf8(guest.stdout)?))
    })();
    if exchanged.is_err() {
        let _ = host.kill();
        let _ = host.wait();
    }
    let (host_log, guest_log) = exchanged?;

    let prefix = format!("HAY_SYNC {route:?} ");
    let host_peer = host_log.lines().find_map(|line| line.strip_prefix(&prefix));
    let guest_peer = guest_log
        .lines()
        .find_map(|line| line.strip_prefix(&prefix));
    if host_peer.is_none() || guest_peer.is_none() || host_peer == guest_peer {
        return Err(format!(
            "Las dos instancias no confirmaron la ruta {route:?} con identidades distintas"
        )
        .into());
    }

    for path in [&host_path, &guest_path] {
        let document = ProjectDoc::load(path)?;
        if document.drum_session()?.as_ref() != Some(&drums)
            || document.track_mix(track.id)?.level != 53
        {
            return Err(format!("Faltan cambios de colaboración en {}", path.display()).into());
        }
    }
    println!(
        "{route:?}: dos procesos del mismo binario recuperaron el hi-hat del anfitrión y el fader 53 del invitado."
    );
    Ok(())
}
