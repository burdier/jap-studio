use super::{Directory, Entry, EntryKind, LibraryTree, Row};
use std::path::{Path, PathBuf};

impl LibraryTree {
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn add(&mut self, path: PathBuf) -> bool {
        if self.roots.contains(&path) {
            return false;
        }
        self.roots.push(path.clone());
        self.expanded.insert(path);
        self.rebuild();
        true
    }

    pub fn remove(&mut self, path: &Path) {
        self.roots.retain(|root| root != path);
        self.directories
            .retain(|dir, _| self.roots.iter().any(|root| dir.starts_with(root)));
        self.expanded
            .retain(|dir| self.roots.iter().any(|root| dir.starts_with(root)));
        self.rebuild();
    }

    pub fn toggle(&mut self, path: &Path) {
        if !self.expanded.remove(path) {
            self.expanded.insert(path.to_owned());
        }
        // Un segundo intento vuelve a leer una carpeta que antes no estaba disponible.
        if matches!(self.directories.get(path), Some(Directory::Failed(_))) {
            self.directories.remove(path);
        }
        self.rebuild();
    }

    pub fn refresh(&mut self) {
        self.directories.clear();
        self.expanded = self.roots.iter().cloned().collect();
        self.rebuild();
    }

    /// Reserva las lecturas visibles que faltan. El adaptador decide cómo ejecutarlas.
    pub fn pending(&mut self) -> Vec<PathBuf> {
        let paths: std::collections::HashSet<_> = self
            .rows
            .iter()
            .filter(|row| row.expanded && !self.directories.contains_key(&row.entry.path))
            .map(|row| row.entry.path.clone())
            .collect();
        for path in &paths {
            self.directories.insert(path.clone(), Directory::Loading);
        }
        if !paths.is_empty() {
            self.rebuild();
        }
        paths.into_iter().collect()
    }

    pub fn complete(&mut self, path: PathBuf, result: Result<Vec<Entry>, String>) {
        if !self.roots.iter().any(|root| path.starts_with(root)) {
            return;
        }
        self.directories.insert(
            path,
            match result {
                Ok(entries) => Directory::Ready(entries),
                Err(error) => Directory::Failed(error),
            },
        );
        self.rebuild();
    }

    fn rebuild(&mut self) {
        let mut rows = Vec::new();
        // Pila explícita: el tamaño de una biblioteca no consume la pila del hilo UI.
        let mut pending: Vec<_> = self
            .roots
            .iter()
            .rev()
            .map(|root| (Entry::directory(root.clone()), 0))
            .collect();
        while let Some((entry, depth)) = pending.pop() {
            let expanded =
                entry.kind == EntryKind::Directory && self.expanded.contains(&entry.path);
            let status = if expanded {
                match self.directories.get(&entry.path) {
                    Some(Directory::Loading) => Some("Leyendo…".into()),
                    Some(Directory::Failed(error)) => Some(format!("No disponible: {error}")),
                    Some(Directory::Ready(entries)) => {
                        pending.extend(entries.iter().rev().cloned().map(|e| (e, depth + 1)));
                        entries.is_empty().then(|| "Vacía".into())
                    }
                    None => None,
                }
            } else {
                None
            };
            rows.push(Row {
                entry,
                depth,
                root: depth == 0,
                expanded,
                status,
            });
        }
        self.rows = rows;
    }
}
