//! Plan privado de edición: ninguna operación modifica la sesión antes de validar todo.
use super::{TrackId, TrackSnapshot};
use crate::musical::fx::FxEdit;
use std::{collections::BTreeMap, fmt};

mod ops;
mod validation;
pub use ops::apply_track_batch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackRef {
    Id(TrackId),
    Handle(String),
}

#[derive(Debug, Clone)]
pub enum TrackBatchEdit {
    InsertAudio {
        handle: String,
        name: String,
        after: Option<TrackRef>,
        parent: Option<TrackRef>,
    },
    Group {
        handle: String,
        name: String,
        members: Vec<TrackRef>,
    },
    Rename {
        track: TrackRef,
        name: String,
    },
    Move {
        track: TrackRef,
        after: TrackRef,
    },
    Mute {
        track: TrackRef,
        value: bool,
    },
    Solo {
        track: TrackRef,
        value: bool,
    },
    Level {
        track: TrackRef,
        value: i32,
    },
    Pan {
        track: TrackRef,
        value: i32,
    },
    Fx {
        track: TrackRef,
        edit: FxEdit,
    },
}

impl TrackBatchEdit {
    fn label(&self) -> &'static str {
        match self {
            Self::InsertAudio { .. } => "insert_audio",
            Self::Group { .. } => "group",
            Self::Rename { .. } => "rename",
            Self::Move { .. } => "move",
            Self::Mute { .. } => "mute",
            Self::Solo { .. } => "solo",
            Self::Level { .. } => "level",
            Self::Pan { .. } => "pan",
            Self::Fx { .. } => "fx",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackBatch {
    pub origin: TrackSnapshot,
    pub edits: Vec<TrackBatchEdit>,
}

#[derive(Debug, Clone)]
pub struct TrackBatchResult {
    pub snapshot: TrackSnapshot,
    /// Por cada fila nueva, índice de su pista original; None para una pista creada.
    pub sources: Vec<Option<usize>>,
    pub handles: BTreeMap<String, TrackId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackBatchError {
    pub code: &'static str,
    pub operation: Option<usize>,
    pub action: Option<&'static str>,
    pub message: String,
}

impl fmt::Display for TrackBatchError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let (Some(index), Some(action)) = (self.operation, self.action) {
            write!(out, "Lote: operación {index} ({action}): {}", self.message)
        } else {
            write!(out, "Lote: {}", self.message)
        }
    }
}

impl std::error::Error for TrackBatchError {}
