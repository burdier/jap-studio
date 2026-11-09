//! Lectura de un nivel de carpeta; el adaptador la ejecuta en su worker.
use super::{Entry, EntryKind};
use std::path::Path;

/// Un solo nivel. No sigue carpetas enlazadas: evita ciclos y recorridos recursivos accidentales.
pub fn read_directory(path: &Path) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for item in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let item = item.map_err(|e| e.to_string())?;
        let path = item.path();
        let kind = if item.file_type().map_err(|e| e.to_string())?.is_dir() {
            EntryKind::Directory
        } else {
            EntryKind::for_file(&path)
        };
        entries.push(Entry {
            path,
            name: item.file_name().to_string_lossy().into_owned(),
            kind,
            duration_seconds: None,
        });
    }
    entries.sort_by_cached_key(|e| {
        (
            e.kind != EntryKind::Directory,
            e.name.to_lowercase(),
            e.path.clone(),
        )
    });
    Ok(entries)
}
