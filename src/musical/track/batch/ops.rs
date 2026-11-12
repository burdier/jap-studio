//! Simulación en copia, validación ordenada y resultado para publicación.
use super::{TrackBatch, TrackBatchError, TrackBatchResult, validation::error};
use crate::musical::track::TrackSnapshot;
use std::collections::BTreeMap;

const MAX_OPERATIONS: usize = 128;
mod step;

pub fn apply_track_batch(
    current: &TrackSnapshot,
    batch: &TrackBatch,
) -> Result<TrackBatchResult, TrackBatchError> {
    if current.revision != batch.origin.revision || current.entries != batch.origin.entries {
        return Err(error("lote_obsoleto", "La sesión cambió desde la captura"));
    }
    if batch.edits.len() > MAX_OPERATIONS {
        return Err(error("limite", "Máximo 128 operaciones por lote"));
    }
    if batch.edits.is_empty() {
        return Err(error(
            "operacion_invalida",
            "El lote no contiene operaciones",
        ));
    }
    let next_revision = current
        .revision
        .checked_add(1)
        .ok_or_else(|| error("limite", "Se agotó la revisión de sesión"))?;
    let mut candidate = current.clone();
    let mut handles = BTreeMap::new();
    let mut created = 0;
    for (offset, edit) in batch.edits.iter().enumerate() {
        let result = step::apply_edit(&mut candidate, &mut handles, &mut created, edit);
        if let Err(mut err) = result {
            err.operation = Some(offset + 1);
            err.action = Some(edit.label());
            return Err(err);
        }
    }
    candidate.revision = next_revision;
    let sources = candidate
        .entries
        .iter()
        .map(|entry| {
            current
                .entries
                .iter()
                .position(|old| old.track.id == entry.track.id)
        })
        .collect();
    Ok(TrackBatchResult {
        snapshot: candidate,
        sources,
        handles,
    })
}
