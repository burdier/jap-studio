//! Registro v1 headless: carga explícita y publicación completa fuera de UI/audio.
mod command;
mod descriptor;
mod error;
mod execute;
#[cfg(test)]
mod execute_tests;
mod fields;
mod invocation;
mod parameters;
mod scan;
pub use command::ToolRequest;
pub use invocation::{PackageForm, ToolInvocation};

use crate::actions::ActionParameter;
pub use error::{PackageError, PackagePhase};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

/// Rutas proporcionadas por el host; este módulo no consulta configuración personal.
#[derive(Clone, Debug)]
pub struct PackageDirectories {
    pub bundled: PathBuf,
    pub user: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PackageDescriptor {
    pub api_version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    /// El mismo contrato/validador que las acciones nativas, con cadenas propias.
    pub parameters: Vec<ActionParameter<String>>,
}

/// Snapshot inmutable: no conserva VM ni función Lua y no vuelve a consultar el disco.
#[derive(Debug)]
pub struct Package {
    path: PathBuf,
    source: String,
    descriptor: PackageDescriptor,
}

impl Package {
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn descriptor(&self) -> &PackageDescriptor {
        &self.descriptor
    }
}

/// La exclusión de `&mut self` serializa recargas; el host debe llamarlas en un worker.
/// Clonar conserva snapshots para trabajos ya enviados sin retener una VM.
#[derive(Clone, Debug, Default)]
pub struct PackageRegistry {
    generation: u64,
    packages: BTreeMap<String, Arc<Package>>,
}

impl PackageRegistry {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn get(&self, id: &str) -> Option<&Arc<Package>> {
        self.packages.get(id)
    }

    pub fn packages(&self) -> impl ExactSizeIterator<Item = &Arc<Package>> {
        self.packages.values()
    }

    /// Un fallo no cambia fuentes, metadatos ni generación. Una recarga vacía
    /// válida retira todos los paquetes. No registra acciones nativas ni comandos.
    pub fn reload(&mut self, directories: &PackageDirectories) -> Result<(), PackageError> {
        let packages = scan::load(directories)?;
        let generation = self.generation.checked_add(1).ok_or_else(|| {
            PackageError::load(
                &directories.bundled,
                "Se agotaron las generaciones del registro",
            )
        })?;
        *self = Self {
            generation,
            packages,
        };
        Ok(())
    }
}
