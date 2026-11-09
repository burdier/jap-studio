use super::Explorer;
use elevate::explorer::{persistence, read_directory};
use gpui::{AppContext, Context, PathPromptOptions, ScrollStrategy};
use std::path::PathBuf;

impl Explorer {
    pub(super) fn restore(&mut self, cx: &mut Context<Self>) {
        let Some(file) = self.config.clone() else {
            self.status = Some("No se encontró la carpeta de configuración".into());
            return;
        };
        self.status = Some("Cargando bibliotecas…".into());
        let load = cx.background_spawn(async move { persistence::load(&file) });
        self.restore_task = Some(cx.spawn(async move |this, cx| {
            let result = load.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(roots) => {
                        for path in roots {
                            this.tree.add(path.clone());
                            this.tree.toggle(&path); // Restaurar accesos no recorre el disco.
                        }
                        this.ready = true;
                        this.status = None;
                    }
                    Err(error) => {
                        this.status = Some(format!(
                            "No se pudieron cargar las bibliotecas: {error}. Pulsa Recargar."
                        ))
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn scan_pending(&mut self, cx: &mut Context<Self>) {
        for path in self.tree.pending() {
            let work_path = path.clone();
            let scan = cx.background_spawn(async move {
                read_directory(&work_path).map(|mut entries| {
                    for entry in &mut entries {
                        if entry.kind == elevate::explorer::EntryKind::Audio {
                            entry.duration_seconds = crate::audio::AudioFile::decode(&entry.path)
                                .ok()
                                .map(|audio| audio.duration());
                        }
                    }
                    entries
                })
            });
            let generation = self.generation;
            let key = path.clone();
            let task = cx.spawn(async move |this, cx| {
                let result = scan.await;
                let _ = this.update(cx, |this, cx| {
                    if this.generation != generation {
                        return;
                    }
                    this.scans.remove(&path);
                    this.tree.complete(path, result);
                    cx.notify();
                });
            });
            self.scans.insert(key, task);
        }
    }

    pub(super) fn add_library(&mut self, cx: &mut Context<Self>) {
        if !self.ready || self.picking {
            return;
        }
        self.picking = true;
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Añadir bibliotecas".into()),
        });
        self.picker_task = Some(cx.spawn(async move |this, cx| {
            let result = prompt.await;
            let paths = match result {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) => Vec::new(),
                error => {
                    let _ = this.update(cx, |this, cx| {
                        this.picking = false;
                        this.status = Some(format!("No se pudo abrir el selector: {error:?}"));
                        cx.notify();
                    });
                    return;
                }
            };
            let resolved = cx
                .background_spawn(async move {
                    paths
                        .into_iter()
                        .map(persistence::resolve_library)
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.picking = false;
                this.status = None;
                let mut changed = false;
                for result in resolved {
                    match result {
                        Ok(path) => {
                            this.selected = Some(path.clone());
                            changed |= this.tree.add(path);
                        }
                        Err(error) => this.status = Some(error),
                    }
                }
                if changed {
                    this.mappings_changed(cx);
                }
                this.scan_pending(cx);
                if let Some(index) = this
                    .tree
                    .rows()
                    .iter()
                    .position(|row| Some(&row.entry.path) == this.selected.as_ref())
                {
                    this.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn remove_library(&mut self, path: &PathBuf, cx: &mut Context<Self>) {
        self.tree.remove(path);
        self.scans
            .retain(|dir, _| this_root_contains(&self.tree, dir));
        if self
            .selected
            .as_ref()
            .is_some_and(|selected| selected.starts_with(path))
        {
            self.selected = None;
        }
        self.mappings_changed(cx);
        cx.notify();
    }

    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.ready {
            self.restore(cx);
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.scans.clear();
        self.tree.refresh();
        self.selected = None;
        self.status = None;
        self.scan_pending(cx);
        if self.save_error.is_some() {
            self.persist(cx);
        }
        cx.notify();
    }

    fn mappings_changed(&mut self, cx: &mut Context<Self>) {
        self.revision = self.revision.wrapping_add(1);
        self.persist(cx);
    }

    fn persist(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(file) = self.config.clone() else {
            return;
        };
        self.saving = true;
        let roots = self.tree.roots().to_vec();
        let revision = self.revision;
        let write = cx.background_spawn(async move { persistence::save(&file, &roots) });
        self.save_task = Some(cx.spawn(async move |this, cx| {
            let result = write.await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                this.save_error = result.err().map(|error| {
                    format!("Bibliotecas sin guardar: {error}. Pulsa Recargar para reintentar.")
                });
                // Serializa escrituras: nunca publica una lista antigua después de una nueva.
                if this.revision != revision {
                    this.persist(cx);
                }
                cx.notify();
            });
        }));
    }
}

fn this_root_contains(tree: &elevate::explorer::LibraryTree, path: &std::path::Path) -> bool {
    tree.roots().iter().any(|root| path.starts_with(root))
}
