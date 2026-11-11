use std::{
    fmt,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackagePhase {
    Load,
    Descriptor,
}

/// Datos conservados para que cualquier host pueda presentar el mismo diagnóstico.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageError {
    pub phase: PackagePhase,
    pub path: PathBuf,
    pub id: Option<String>,
    pub field: Option<String>,
    pub message: String,
}

impl PackageError {
    pub(super) fn load(path: &Path, message: impl ToString) -> Self {
        Self {
            phase: PackagePhase::Load,
            path: path.to_owned(),
            id: None,
            field: None,
            message: message.to_string(),
        }
    }
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let phase = match self.phase {
            PackagePhase::Load => "load",
            PackagePhase::Descriptor => "descriptor",
        };
        write!(f, "{phase} · {}", self.path.display())?;
        if let Some(id) = &self.id {
            write!(f, " · {id}")?;
        }
        if let Some(field) = &self.field {
            write!(f, " · {field}")?;
        }
        write!(f, ": {}", self.message)
    }
}

impl std::error::Error for PackageError {}
