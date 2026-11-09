//! Los menús, atajos, catálogo y comandos convergen en las mismas operaciones.
use super::FxTarget;
use crate::ui::DawApp;
use elevate::{
    catalog,
    musical::fx::{FxEdit, FxKind, FxNode},
};
use gpui::{Context, Window};
impl DawApp {
    pub(in crate::ui) fn handle_fx_command(&mut self, command: &str) -> bool {
        if !command
            .split_whitespace()
            .next()
            .is_some_and(|c| c.starts_with("fx."))
        {
            return false;
        }
        if command == "fx.show" {
            self.show_fx();
            return true;
        }
        let result = self
            .selected_fx_target()
            .ok_or_else(|| "Selecciona una pista".to_string())
            .and_then(|target| self.execute_fx_at(target, command));
        if let Err(error) = result {
            self.status_message = error;
        }
        true
    }
    pub(in crate::ui) fn execute_fx_at(
        &mut self,
        target: FxTarget,
        command: &str,
    ) -> Result<(), String> {
        let parts = elevate::actions::tokenize(command)?;
        let Some(command) = parts.first().map(String::as_str) else {
            return Err("Acción FX vacía".into());
        };
        let selected = || {
            target
                .node
                .ok_or_else(|| "Selecciona un efecto o contenedor".to_string())
        };
        let edit = match command {
            "fx.add" => {
                self.open_fx_palette(target);
                return Ok(());
            }
            "fx.rename" | "fx.parameters" if parts.len() == 1 => {
                let command = if command == "fx.rename" {
                    "fx.rename"
                } else {
                    "fx.parameters"
                };
                self.request_fx_action(target, command);
                return Ok(());
            }
            "fx.serial" | "fx.parallel" => {
                let track = self
                    .tracks
                    .iter()
                    .find(|t| t.id == target.track)
                    .ok_or("La pista ya no existe")?;
                if target.node.is_some_and(|id| track.chain.find(id).is_none()) {
                    return Err("El destino ya no existe".into());
                }
                let kind = if command == "fx.serial" {
                    FxKind::SerialChain {
                        children: Vec::new(),
                    }
                } else {
                    FxKind::ParallelChain {
                        branches: Vec::new(),
                    }
                };
                FxEdit::Insert(
                    FxNode::new(
                        if command == "fx.serial" {
                            "Serie"
                        } else {
                            "Paralelo"
                        },
                        kind,
                    ),
                    track.chain.insertion_after(target.node),
                )
            }
            "fx.rename" => {
                FxEdit::Rename(selected()?, parts.get(1).ok_or("Falta el nombre")?.clone())
            }
            "fx.parameters" => {
                let node = self.fx_node(target).ok_or("El efecto ya no existe")?;
                if node.children().is_some() {
                    return Err("Selecciona un efecto para editar parámetros".into());
                }
                let value: f32 = parts
                    .get(1)
                    .ok_or("Falta la ganancia")?
                    .replace(',', ".")
                    .parse()
                    .map_err(|_| "Ganancia inválida")?;
                if !value.is_finite() || !(0.0..=2.0).contains(&value) {
                    return Err("Ganancia entre 0× y 2×".into());
                }
                FxEdit::Parameter(node.id, 0, value)
            }
            "fx.duplicate" => FxEdit::Duplicate(selected()?),
            "fx.bypass" => FxEdit::Toggle(selected()?),
            "fx.remove" => FxEdit::Remove(selected()?),
            _ => return Err("Acción FX desconocida".into()),
        };
        self.edit_fx(target.track, edit)
    }
    pub(in crate::ui) fn open_fx_form(
        &mut self,
        target: FxTarget,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(node) = self.fx_node(target) else {
            self.status_message = "Selecciona un efecto o contenedor".into();
            return;
        };
        if command == "fx.parameters" && node.children().is_some() {
            self.status_message = "Selecciona un efecto para editar parámetros".into();
            return;
        }
        let id = if command == "fx.rename" {
            "action.fx.rename"
        } else {
            "action.fx.parameters"
        };
        if let Some(entry) = catalog::find(id) {
            if let Some(spec) = entry.action.parameters() {
                self.fx.form_target = Some(target);
                self.open_action_form(entry, spec, window, cx);
            }
        }
    }
}
