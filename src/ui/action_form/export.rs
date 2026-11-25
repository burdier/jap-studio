//! Estado del diálogo de exportación. El contrato del comando valida las mismas opciones.
use super::{ActionForm, DawApp, ParameterField};
use crate::audio::export::Report;
use elevate::{
    actions::{ParameterValue, ParameterValues},
    export::{ExportOptions, Format},
};
use gpui::{App, Context, PathPromptOptions, Window};
use std::collections::BTreeMap;

pub(in crate::ui) struct ExportForm {
    pub formats: Vec<Format>,
    pub advanced: bool,
    pub choosing_folder: bool,
    pub running: bool,
    pub completed: Option<Report>,
}

impl ExportForm {
    pub(super) fn add_formats(&self, values: &mut ParameterValues<String>) -> Result<(), String> {
        if self.formats.is_empty() {
            return Err("Elige al menos un formato de audio".into());
        }
        let Some(ParameterValue::Text(name)) = values.get_mut("name") else {
            return Err("Escribe el nombre del archivo".into());
        };
        let formats = self
            .formats
            .iter()
            .map(|format| format.extension())
            .collect::<Vec<_>>()
            .join(",");
        *name = format!("{name}.[{formats}]");
        Ok(())
    }
}

impl ActionForm {
    pub(in crate::ui) fn field_index(&self, key: &str) -> usize {
        self.spec
            .parameters
            .iter()
            .position(|field| field.key == key)
            .expect("campo del formulario")
    }

    pub(in crate::ui) fn text(&self, key: &str, cx: &App) -> String {
        match &self.fields[self.field_index(key)] {
            ParameterField::Input(input) => input.read(cx).value().to_string(),
            ParameterField::Bool { .. } => unreachable!("campo de texto"),
        }
    }

    pub(in crate::ui) fn checked(&self, key: &str) -> bool {
        matches!(
            &self.fields[self.field_index(key)],
            ParameterField::Bool { checked: true, .. }
        )
    }
}

impl DawApp {
    pub(super) fn init_export_form(&self, defaults: &mut BTreeMap<&str, String>) -> ExportForm {
        let saved = &self.settings.export.saved;
        let mut name = saved.name.clone();
        if name == ExportOptions::default().name {
            if let Some(stem) = self.project.path.as_ref().and_then(|path| path.file_stem()) {
                name = stem.to_string_lossy().into_owned();
            }
        }
        defaults.insert("name", name);
        ExportForm {
            formats: saved.formats.clone(),
            advanced: false,
            choosing_folder: false,
            running: false,
            completed: None,
        }
    }

    pub(in crate::ui) fn choose_export_folder(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = &mut self.action_form else {
            return;
        };
        let Some(export) = &mut form.export else {
            return;
        };
        if export.choosing_folder {
            return;
        }
        export.choosing_folder = true;
        let focus = form.focus.clone();
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Elegir carpeta".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = prompt.await;
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |this, cx| {
                    let Some(form) = this.action_form.as_mut().filter(|form| form.focus == focus)
                    else {
                        return;
                    };
                    let Some(export) = &mut form.export else {
                        return;
                    };
                    export.choosing_folder = false;
                    match result {
                        Ok(Ok(Some(paths))) => {
                            if let Some(path) = paths.first()
                                && let ParameterField::Input(input) =
                                    &form.fields[form.field_index("directory")]
                            {
                                input.update(cx, |input, cx| {
                                    input.replace_all(
                                        path.to_string_lossy().into_owned(),
                                        window,
                                        cx,
                                    );
                                });
                                form.error = None;
                            }
                        }
                        Ok(Ok(None)) => {}
                        _ => form.error = Some("No se pudo abrir el selector de carpetas".into()),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
        cx.notify();
    }

    pub(in crate::ui) fn finish_export_form(&mut self, result: Result<&Report, &str>) {
        if let Some(form) = &mut self.action_form
            && let Some(export) = &mut form.export
            && export.running
        {
            export.running = false;
            match result {
                Ok(report) => {
                    export.completed = Some(Report {
                        paths: report.paths.clone(),
                        overs: report.overs,
                    })
                }
                Err(error) => form.error = Some(error.to_owned()),
            }
        }
    }
}
