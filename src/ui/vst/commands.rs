use crate::ui::DawApp;
use gpui::Context;
impl DawApp {
    pub(in crate::ui) fn execute_vst_command(
        &mut self,
        command: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if !command.starts_with("instrument.vst3.") && !command.starts_with("fx.vst3.") {
            return false;
        }
        let result = elevate::actions::tokenize(command);
        match result.as_deref() {
            Ok([name, path]) if name == "fx.vst3.load" => {
                if let Some(target) = self.selected_fx_target() {
                    self.load_vst_effect_path(path.into(), target, cx);
                } else {
                    self.status_message = "Selecciona una pista".into();
                }
            }
            Ok([name]) if name == "fx.vst3.editor" => {
                if let Some(effect) = self
                    .selected_fx_target()
                    .and_then(|t| t.node)
                    .and_then(|id| self.vst_effects.instances.get(&id))
                {
                    effect.editor.open();
                } else {
                    self.status_message = "Selecciona un efecto VST3 en el árbol".into();
                }
            }

            Ok([name, path]) if name == "instrument.vst3.load" => {
                let target = self.tracks.get(self.cursor_row).map(|track| track.id);
                self.load_vst(path.into(), target, cx);
            }
            Ok([name]) if name == "instrument.vst3.remove" && self.vst_selected() => {
                self.remove_vst()
            }
            Ok([name]) if name == "instrument.vst3.editor" && self.vst_selected() => {
                self.vst.open_editor()
            }
            Ok([name]) if name == "instrument.vst3.editor.close" && self.vst_selected() => {
                self.vst.close_editor()
            }
            Ok([name]) if name == "instrument.vst3.remove" => {
                self.status_message = "Selecciona la pista con el VST3".into()
            }
            Err(error) => self.status_message = error.clone(),
            _ => {
                self.status_message =
                    "Usa instrument.vst3.load 'ruta/Plugin.vst3' o instrument.vst3.remove".into()
            }
        }
        true
    }
}
