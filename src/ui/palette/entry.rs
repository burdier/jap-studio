//! Filas con metadatos propios para el catálogo nativo, VST3 y herramientas Lua.
use elevate::catalog::{
    CatalogAction, CatalogEntry, EntryKind, plugins::InstalledInstrument, tools::ToolCatalogEntry,
};

#[derive(Clone)]
pub(in crate::ui) struct PaletteEntry {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub kind: EntryKind,
    pub action: CatalogAction<String>,
    pub builtin: Option<&'static CatalogEntry>,
    pub bundle: Option<std::path::PathBuf>,
    pub plugin: Option<InstalledInstrument>,
    pub is_tool: bool,
    pub has_parameters: bool,
}

impl From<&'static CatalogEntry> for PaletteEntry {
    fn from(entry: &'static CatalogEntry) -> Self {
        Self {
            id: entry.id.into(),
            name: entry.name.into(),
            summary: entry.summary.into(),
            kind: entry.kind,
            action: entry.action.to_owned(),
            builtin: Some(entry),
            bundle: None,
            plugin: None,
            is_tool: false,
            has_parameters: entry.action.parameters().is_some(),
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
            action: CatalogAction::Invoke("instrument.vst3.load".into()),
            builtin: None,
            bundle: Some(plugin.bundle.clone()),
            plugin: Some(plugin.clone()),
            is_tool: false,
            has_parameters: false,
        }
    }
}

impl From<&ToolCatalogEntry> for PaletteEntry {
    fn from(tool: &ToolCatalogEntry) -> Self {
        Self {
            id: tool.entry.id.clone(),
            name: tool.entry.name.clone(),
            summary: tool.entry.summary.clone(),
            kind: tool.entry.kind,
            action: tool.entry.action.clone(),
            builtin: None,
            bundle: None,
            plugin: None,
            is_tool: true,
            has_parameters: !tool.spec.parameters.is_empty(),
        }
    }
}
