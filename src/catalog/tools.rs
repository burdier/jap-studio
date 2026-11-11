//! Índice dinámico de acciones Lua: solo metadatos y la misma invocación del comando.
use super::{CatalogAction, EntryKind, OwnedCatalogEntry};
use crate::{actions::OwnedActionSpec, scripting::packages::PackageRegistry};

#[derive(Clone, Debug)]
pub struct ToolCatalogEntry {
    pub entry: OwnedCatalogEntry,
    pub spec: OwnedActionSpec,
}

pub fn entries(registry: &PackageRegistry) -> Vec<ToolCatalogEntry> {
    registry
        .packages()
        .map(|package| {
            let descriptor = package.descriptor();
            let spec = package.action_spec();
            ToolCatalogEntry {
                entry: OwnedCatalogEntry {
                    id: format!("lua.tool.{}", descriptor.id),
                    kind: EntryKind::Action,
                    name: descriptor.name.clone(),
                    summary: descriptor.description.clone(),
                    keywords: vec!["lua".into(), "midi".into(), descriptor.id.clone()],
                    action: CatalogAction::Invoke(spec.command.clone()),
                },
                spec,
            }
        })
        .collect()
}
