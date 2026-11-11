//! Preparación del lanzador antes de crear hilos. También cubre cargo run y el binario directo.
use std::{io::Write, os::unix::fs::OpenOptionsExt, path::PathBuf};

/// Llamar únicamente desde el arranque, antes de GPUI/CPAL o cualquier worker.
pub(crate) fn configure() -> Result<(), String> {
    if std::env::var_os("WINELOADER").is_some()
        || on_path("wine")
        || !on_path("distrobox-host-exec")
        || std::fs::symlink_metadata("/run/host/usr/bin/wine").is_err()
    {
        return Ok(());
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .ok_or("No se encontró la carpeta de caché para el lanzador Wine")?
        .join("hay-daw/runtime");
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let path = cache.join("wine-host.sh");
    let temporary = cache.join(format!("wine-host-{}.tmp", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = file
        .write_all(include_bytes!("../../../scripts/wine-host.sh"))
        .and_then(|_| std::fs::rename(&temporary, &path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(|e| e.to_string())?;
    // Este punto precede a application(), que crea los hilos de GPUI.
    unsafe {
        std::env::set_var("WINELOADER", path);
        std::env::set_var("YABRIDGE_NO_WATCHDOG", "1");
    }
    Ok(())
}
fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
}
