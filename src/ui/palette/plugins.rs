//! Adaptación de entradas instaladas al mismo buscador y sus interacciones.
use super::{DawApp, Palette};
use elevate::catalog::{CatalogAction, CatalogEntry, EntryKind, plugins::InstalledInstrument};
use gpui::{AppContext, Context, Window};

#[derive(Clone)]
pub(in crate::ui) struct PaletteEntry {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub kind: EntryKind,
    pub action: CatalogAction,
    pub builtin: Option<&'static CatalogEntry>,
    pub bundle: Option<std::path::PathBuf>,
    pub plugin: Option<InstalledInstrument>,
}

impl From<&'static CatalogEntry> for PaletteEntry {
    fn from(entry: &'static CatalogEntry) -> Self {
        Self {
            id: entry.id.into(),
            name: entry.name.into(),
            summary: entry.summary.into(),
            kind: entry.kind,
            action: entry.action,
            builtin: Some(entry),
            bundle: None,
            plugin: None,
        }
    }
}

impl From<&InstalledInstrument> for PaletteEntry {
    fn from(plugin: &InstalledInstrument) -> Self {
        Self {
            id: format!(
                "installed.vst3:{}:{}",
                plugin.bundle.display(),
                plugin.class_id
            ),
            name: plugin.name.clone(),
            summary: format!(
                "VST3 · {} · {}",
                if plugin.bridged {
                    "yabridge / Wine"
                } else {
                    "nativo"
                },
                if plugin.instrument {
                    "instrumento"
                } else {
                    "efecto"
                }
            ),
            kind: if plugin.instrument {
                EntryKind::Instrument
            } else {
                EntryKind::Component
            },
            action: CatalogAction::Invoke("instrument.vst3.load"),
            builtin: None,
            bundle: Some(plugin.bundle.clone()),
            plugin: Some(plugin.clone()),
        }
    }
}

impl Palette {
    pub(super) fn replace_plugins(&mut self, plugins: Vec<InstalledInstrument>) {
        let selected = self
            .results
            .get(self.selected)
            .map(|entry| entry.id.clone());
        self.plugins = plugins;
        self.refresh();
        if let Some(index) = self
            .results
            .iter()
            .position(|entry| Some(&entry.id) == selected.as_ref())
        {
            self.selected = index;
        }
    }
}

impl DawApp {
    pub(in crate::ui) fn scan_palette_plugins(&mut self, cx: &mut Context<Self>) {
        if !self.palette.is_open() || !self.palette.scan_needed || self.palette.scan_task.is_some()
        {
            return;
        }
        self.palette.scan_needed = false;
        let work = cx.background_spawn(async { crate::audio::vst3::scan::discover() });
        self.palette.scan_task = Some(cx.spawn(async move |this, cx| {
            let plugins = work.await;
            let _ = this.update(cx, |this, cx| {
                this.palette.scan_task = None;
                this.palette.replace_plugins(plugins);
                cx.notify();
            });
        }));
    }

    pub(in crate::ui) fn activate_palette_entry(
        &mut self,
        entry: PaletteEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(builtin) = entry.builtin {
            self.activate_catalog_entry(builtin, window, cx);
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
