//! Persistencia ligera de proyectos `.loro` recientes en `~/.config/hay-daw/recent-projects.txt`.
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_RECENT: usize = 10;

fn recent_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("hay-daw")
        .join("recent-projects.txt")
}

pub(in crate::ui) fn load() -> Vec<PathBuf> {
    let Ok(content) = fs::read_to_string(recent_file_path()) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let path = PathBuf::from(trimmed);
        if path.is_file() && !paths.contains(&path) {
            paths.push(path);
            if paths.len() >= MAX_RECENT {
                break;
            }
        }
    }
    paths
}

pub(in crate::ui) fn record(path: &Path) -> Vec<PathBuf> {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    // No contaminar recientes con el archivo interno de autosave.
    if canonical
        .file_name()
        .is_some_and(|name| name == "recovery.loro")
    {
        return load();
    }
    let mut paths = load();
    paths.retain(|existing| existing != &canonical);
    paths.insert(0, canonical);
    paths.truncate(MAX_RECENT);

    let dest = recent_file_path();
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let body = paths
        .iter()
        .filter_map(|p| p.to_str())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::write(&dest, body);
    paths
}
