//! Archivos temporales en destino; publicación sin sobrescribir y limpieza de lotes fallidos.
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) struct Batch {
    staged: Vec<(PathBuf, PathBuf)>,
    published: Vec<PathBuf>,
    committed: bool,
}

impl Batch {
    pub fn new() -> Self {
        Self {
            staged: Vec::new(),
            published: Vec::new(),
            committed: false,
        }
    }

    pub fn create(&mut self, path: &Path) -> Result<File, String> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        if path.try_exists().map_err(|e| e.to_string())? {
            return Err(format!(
                "Ya existe {}; elige otro nombre o carpeta",
                path.display()
            ));
        }
        let parent = path.parent().ok_or("Destino sin carpeta")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temp = parent.join(format!(
            ".hay-export-{}-{}.part",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let file = OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        self.staged.push((temp, path.into()));
        Ok(file)
    }

    pub fn commit(mut self) -> Result<Vec<PathBuf>, String> {
        for (temp, final_path) in &self.staged {
            // Mismo filesystem, sin ventana TOCTOU que pueda sobrescribir un archivo ajeno.
            fs::hard_link(temp, final_path)
                .map_err(|e| format!("{}: {e}", final_path.display()))?;
            self.published.push(final_path.clone());
        }
        self.committed = true;
        Ok(self.published.clone())
    }
}

impl Drop for Batch {
    fn drop(&mut self) {
        for (temp, _) in &self.staged {
            let _ = fs::remove_file(temp);
        }
        if !self.committed {
            for path in &self.published {
                let _ = fs::remove_file(path);
            }
        }
    }
}

pub(super) fn track_name(name: &str) -> String {
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        "track".into()
    } else {
        name
    }
}
