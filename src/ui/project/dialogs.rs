use super::*;
use gpui::{PathPromptOptions, PromptLevel, Window};

impl DawApp {
    pub(in crate::ui) fn project_command(
        &mut self,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match command.trim() {
            "project.save" => self.choose_project_save(false, cx),
            "project.save_as" => self.choose_project_save(true, cx),
            "project.reveal" => {
                if let Some(path) = &self.project.path {
                    cx.reveal_path(path);
                    self.status_message = path.display().to_string();
                } else {
                    self.status_message = "Guarda el proyecto para mostrar su ubicación".into();
                }
            }
            "project.open" => self.confirm_project_open(window, cx),
            #[cfg(feature = "p2p")]
            "project.share" => self.share_project(window, cx),
            #[cfg(feature = "p2p")]
            "project.sync" => self.force_sync_shared_project(cx),
            #[cfg(feature = "p2p")]
            "project.join" => self.join_shared_project(cx),
            #[cfg(feature = "p2p")]
            "project.leave" => self.leave_shared_project(),
            #[cfg(feature = "p2p")]
            "project.identity" => self.open_identity_dialog(window, cx),
            _ => return false,
        }
        cx.notify();
        true
    }

    fn choose_project_save(&mut self, save_as: bool, cx: &mut Context<Self>) {
        if self.project.dialog.is_some() || self.project_busy() {
            return;
        }
        if !save_as {
            if let Some(path) = self.project.path.clone() {
                self.save_project_to(path, false, cx);
                return;
            }
        }
        let directory = self
            .project
            .path
            .as_ref()
            .and_then(|path| path.parent())
            .map(ToOwned::to_owned)
            .or_else(dirs::document_dir)
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let prompt = cx.prompt_for_new_path(&directory, Some("Mi proyecto.loro"));
        self.project.dialog = Some(cx.spawn(async move |this, cx| {
            let result = prompt.await;
            let _ = this.update(cx, |this, cx| {
                this.project.dialog = None;
                match result {
                    Ok(Ok(Some(path))) => {
                        this.save_project_to(path, true, cx);
                    }
                    Ok(Ok(None)) => {}
                    _ => this.status_message = "No se pudo abrir el selector de guardado".into(),
                }
                cx.notify();
            });
        }));
    }

    fn confirm_project_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.project.dialog.is_some() || self.project_busy() {
            return;
        }
        self.project.checked = None;
        self.refresh_project_dirty();
        if !self.project.dirty {
            self.choose_project_open(cx);
            return;
        }
        let prompt = window.prompt(PromptLevel::Warning, "Hay cambios sin guardar", Some("Guarda primero para conservarlos. Abrir descartará los cambios cuando el proyecto esté listo."), &["Cancelar", "Guardar…", "Abrir y descartar"], cx);
        self.project.dialog = Some(cx.spawn(async move |this, cx| {
            let answer = prompt.await;
            let _ = this.update(cx, |this, cx| {
                this.project.dialog = None;
                match answer {
                    Ok(1) => this.choose_project_save(false, cx),
                    Ok(2) => this.choose_project_open(cx),
                    _ => {}
                }
                cx.notify();
            });
        }));
    }

    fn choose_project_open(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Abrir proyecto .loro".into()),
        });
        self.project.dialog = Some(cx.spawn(async move |this, cx| {
            let result = prompt.await;
            let _ = this.update(cx, |this, cx| {
                this.project.dialog = None;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            this.open_project_path(path.clone(), cx);
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => this.status_message = "No se pudo abrir el selector de proyectos".into(),
                }
                cx.notify();
            });
        }));
    }

    pub(in crate::ui) fn guard_project_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.project_busy() || self.project.dialog.is_some() {
            return false;
        }
        self.project.checked = None;
        self.refresh_project_dirty();
        if !self.project.dirty {
            autosave::clear_autosave_file();
            return true;
        }
        let prompt = window.prompt(
            PromptLevel::Warning,
            "Cerrar con cambios sin guardar",
            None,
            &["Cancelar", "Guardar…", "Cerrar y descartar"],
            cx,
        );
        let handle = window.window_handle();
        self.project.dialog = Some(cx.spawn(async move |this, cx| {
            let answer = prompt.await;
            let _ = this.update(cx, |this, cx| {
                this.project.dialog = None;
                if answer == Ok(1) {
                    this.choose_project_save(false, cx);
                }
            });
            if answer == Ok(2) {
                autosave::clear_autosave_file();
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
        }));
        false
    }
}
