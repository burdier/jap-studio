//! Entrada Lua VST3: valida snapshot y delega a controladores compartidos.
use crate::ui::{DawApp, fx::FxTarget};
use elevate::{
    musical::fx::FxEdit,
    scripting::vst3::{VstAction, VstPlan, build_action},
};
use gpui::{AppContext, Context};
use std::io::Read;

impl DawApp {
    pub(in crate::ui) fn execute_vst_script_command(
        &mut self,
        command: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if command.split_whitespace().next() != Some("vst3.script") {
            return false;
        }
        let parts = match elevate::actions::tokenize(command) {
            Ok(parts) => parts,
            Err(error) => {
                self.status_message = format!("Lua VST3: {error}");
                return true;
            }
        };
        let [_, path] = parts.as_slice() else {
            self.status_message = "Uso: vst3.script '/ruta/acciones.lua'".into();
            return true;
        };
        if self.script_task.is_some() {
            self.status_message = "Ya hay un script Lua en curso".into();
            return true;
        }
        let snapshot = match self.track_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.status_message = format!("Lua VST3: {error}");
                return true;
            }
        };
        let path = std::path::PathBuf::from(path);
        self.status_message = "Leyendo acción Lua VST3…".into();
        let work = cx.background_spawn(async move {
            let mut source = String::new();
            std::fs::File::open(path)
                .map_err(|e| e.to_string())?
                .take(256 * 1024 + 1)
                .read_to_string(&mut source)
                .map_err(|e| e.to_string())?;
            build_action(&source, &snapshot)
        });
        self.script_task = Some(cx.spawn(async move |this, cx| {
            let result: Result<VstPlan, String> = work.await;
            let _ = this.update(cx, |this, cx| {
                this.script_task = None;
                if let Err(error) = result.and_then(|plan| this.apply_vst_script_plan(plan, cx)) {
                    this.status_message = format!("Lua VST3: {error}");
                }
                cx.notify();
            });
        }));
        true
    }

    fn apply_vst_script_plan(
        &mut self,
        plan: VstPlan,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let current = self.track_snapshot()?;
        if current.revision != plan.origin.revision || current.entries != plan.origin.entries {
            return Err("La sesión cambió desde la captura; vuelve a ejecutar el script".into());
        }
        let track_id = match &plan.action {
            VstAction::InstrumentLoad { track, .. }
            | VstAction::EffectLoad { track, .. }
            | VstAction::EffectToggle { track, .. }
            | VstAction::EffectRemove { track, .. }
            | VstAction::EffectRename { track, .. }
            | VstAction::TrackLevel { track, .. }
            | VstAction::TrackPan { track, .. } => *track,
        };
        let index = self
            .tracks
            .iter()
            .position(|t| t.id == track_id)
            .ok_or("La pista ya no existe")?;
        match plan.action {
            VstAction::InstrumentLoad { path, class, .. } => {
                if self.vst_task.is_some() {
                    return Err("Ya se está cargando un instrumento VST3".into());
                }
                self.load_vst_class(path.into(), Some(track_id), class, cx);
            }
            VstAction::EffectLoad { path, after, .. } => {
                if self.vst_effects.task.is_some() {
                    return Err("Ya se está cargando un efecto VST3".into());
                }
                self.load_vst_effect_path(
                    path.into(),
                    FxTarget {
                        track: track_id,
                        node: after,
                    },
                    cx,
                );
            }
            VstAction::EffectToggle { fx, .. } => self.edit_fx(track_id, FxEdit::Toggle(fx))?,
            VstAction::EffectRemove { fx, .. } => self.edit_fx(track_id, FxEdit::Remove(fx))?,
            VstAction::EffectRename { fx, name, .. } => {
                self.edit_fx(track_id, FxEdit::Rename(fx, name))?
            }
            VstAction::TrackLevel { value, .. } => {
                self.set_track_level(index, value);
                self.status_message = "Lua VST3 · nivel de pista actualizado".into();
            }
            VstAction::TrackPan { value, .. } => {
                self.set_track_pan(index, value);
                self.status_message = "Lua VST3 · pan de pista actualizado".into();
            }
        }
        Ok(())
    }
}
