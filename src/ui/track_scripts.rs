//! Scripts de pistas en worker; la sesión confirma únicamente un lote validado.
use super::DawApp;
use elevate::{musical::track::TrackBatch, scripting::tracks::build_track_batch};
use gpui::{AppContext, Context};
use std::{io::Read, path::PathBuf};

const TEMPLATE: &str = include_str!("../../assets/scripts/tracks/two_tracks.lua");
enum TrackScriptSource {
    File(PathBuf),
    Template,
}

impl DawApp {
    pub(in crate::ui) fn execute_track_script_command(
        &mut self,
        command: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let first = command.split_whitespace().next().unwrap_or("");
        if !matches!(first, "track.script" | "track.template.two") {
            return false;
        }
        let parts = match elevate::actions::tokenize(command) {
            Ok(parts) => parts,
            Err(error) => {
                self.status_message = format!("Lua pistas: {error}");
                return true;
            }
        };
        let source = match parts.as_slice() {
            [name] if name == "track.template.two" => TrackScriptSource::Template,
            [name, path] if name == "track.script" && !path.is_empty() => {
                TrackScriptSource::File(PathBuf::from(path))
            }
            _ => {
                self.status_message =
                    "Uso: track.script '/ruta/script.lua' o track.template.two".into();
                return true;
            }
        };
        self.start_track_script(source, cx);
        true
    }

    fn start_track_script(&mut self, source: TrackScriptSource, cx: &mut Context<Self>) {
        if self.script_task.is_some() {
            self.status_message = "Ya hay un script Lua en curso".into();
            return;
        }
        let snapshot = match self.track_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.status_message = format!("Lua pistas: {error}");
                return;
            }
        };
        self.status_message = "Ejecutando lote Lua de pistas…".into();
        let work = cx.background_spawn(async move {
            let source = match source {
                TrackScriptSource::Template => TEMPLATE.to_owned(),
                TrackScriptSource::File(path) => {
                    let mut source = String::new();
                    std::fs::File::open(path)
                        .map_err(|e| e.to_string())?
                        .take(256 * 1024 + 1)
                        .read_to_string(&mut source)
                        .map_err(|e| e.to_string())?;
                    if source.len() > 256 * 1024 {
                        return Err("El script Lua supera 256 KiB".into());
                    }
                    source
                }
            };
            build_track_batch(&source, &snapshot)
        });
        self.script_task = Some(cx.spawn(async move |this, cx| {
            let result: Result<TrackBatch, String> = work.await;
            let _ = this.update(cx, |this, cx| {
                this.script_task = None;
                this.status_message = match result {
                    Ok(batch) => match this.apply_track_script_batch(batch) {
                        Ok(handles) => {
                            let ids = handles
                                .iter()
                                .map(|(handle, id)| format!("{handle}={}", id.0))
                                .collect::<Vec<_>>()
                                .join(", ");
                            if ids.is_empty() {
                                "Lua pistas · lote aplicado".into()
                            } else {
                                format!("Lua pistas · lote aplicado · {ids}")
                            }
                        }
                        Err(error) => format!("Lua pistas: {error}"),
                    },
                    Err(error) => format!("Lua pistas: {error}"),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }
}
