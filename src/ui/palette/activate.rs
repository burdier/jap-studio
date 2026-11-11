//! Dispatcher compartido por Enter y clic para filas nativas, VST3 y Lua.
use super::{DawApp, entry::PaletteEntry};
use elevate::catalog::CatalogAction;
use gpui::{Context, Window};

impl DawApp {
    pub(in crate::ui) fn activate_palette_entry(
        &mut self,
        entry: PaletteEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(builtin) = entry.builtin {
            self.activate_catalog_entry(builtin, window, cx);
        } else if entry.is_tool {
            self.palette.close();
            if let CatalogAction::Invoke(command) = entry.action {
                self.execute_command_with_context(&command, window, cx);
            }
        } else if let Some(plugin) = entry.plugin.as_ref().filter(|p| !p.instrument) {
            let target = self.palette.fx_target.or_else(|| self.selected_fx_target());
            self.palette.close();
            if let Some(target) = target {
                self.load_vst_effect(plugin.clone(), target, cx);
            } else {
                self.status_message = "Selecciona una pista para insertar el efecto".into();
            }
        } else if let Some(bundle) = entry.bundle {
            let target = self
                .palette
                .target_track()
                .and_then(|index| self.tracks.get(index))
                .map(|track| track.id);
            self.palette.close();
            self.load_vst_class(bundle, target, entry.plugin.map(|p| p.class_id), cx);
        } else if let CatalogAction::Unavailable(reason) = entry.action {
            self.palette.close();
            self.status_message = format!("{} · {reason}", entry.name);
            cx.notify();
        }
    }
}
