//! Árbol de bibliotecas independiente de la UI. E/S explícita, nunca durante el render.
pub mod persistence;
mod scan;
mod tree;
pub use scan::read_directory;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    Midi,
    Audio,
    Other,
}

impl EntryKind {
    pub fn for_file(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "mid" | "midi" => Self::Midi,
            "wav" | "wave" | "flac" | "mp3" | "ogg" | "oga" | "aac" | "m4a" | "mp4" => Self::Audio,
            _ => Self::Other,
        }
    }
    pub fn importable(self) -> bool {
        matches!(self, Self::Midi | Self::Audio)
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub kind: EntryKind,
    /// Duración en segundos para archivos de audio cuando el explorador pudo leerla.
    pub duration_seconds: Option<f64>,
}

impl Entry {
    fn directory(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        Self {
            path,
            name,
            kind: EntryKind::Directory,
            duration_seconds: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Row {
    pub entry: Entry,
    pub depth: usize,
    pub root: bool,
    pub expanded: bool,
    pub status: Option<String>,
}

pub enum Directory {
    Loading,
    Ready(Vec<Entry>),
    Failed(String),
}

#[derive(Default)]
pub struct LibraryTree {
    roots: Vec<PathBuf>,
    expanded: HashSet<PathBuf>,
    directories: HashMap<PathBuf, Directory>,
    rows: Vec<Row>,
}
