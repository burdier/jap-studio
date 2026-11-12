//! Referencias, límites de texto y dispositivos FX del lote.
use super::{TrackBatchError, TrackRef};
use crate::{
    devices,
    musical::{
        fx::{FxEdit, FxKind},
        track::{TrackDevice, TrackId, TrackSnapshotEntry},
    },
};
use std::collections::BTreeMap;

pub(super) fn error(code: &'static str, message: impl Into<String>) -> TrackBatchError {
    TrackBatchError {
        code,
        operation: None,
        action: None,
        message: message.into(),
    }
}

pub(super) fn reference(
    reference: &TrackRef,
    handles: &BTreeMap<String, TrackId>,
) -> Result<TrackId, TrackBatchError> {
    match reference {
        TrackRef::Id(id) => Ok(*id),
        TrackRef::Handle(handle) => handles.get(handle).copied().ok_or_else(|| {
            error(
                "referencia_invalida",
                format!("Handle desconocido: {handle}"),
            )
        }),
    }
}

pub(super) fn index(entries: &[TrackSnapshotEntry], id: TrackId) -> Result<usize, TrackBatchError> {
    entries
        .iter()
        .position(|entry| entry.track.id == id)
        .ok_or_else(|| {
            error(
                "referencia_invalida",
                format!("Pista inexistente: {}", id.0),
            )
        })
}

pub(super) fn name(value: &str) -> Result<(), TrackBatchError> {
    if value.trim().is_empty() || value.len() > 128 || value.contains(['\n', '\r', '\0']) {
        Err(error(
            "operacion_invalida",
            "El nombre debe tener 1–128 bytes y una sola línea",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn new_handle(
    handle: &str,
    handles: &BTreeMap<String, TrackId>,
) -> Result<(), TrackBatchError> {
    if handle.is_empty()
        || handle.len() > 64
        || !handle
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err(error(
            "operacion_invalida",
            "Handle inválido (1–64 caracteres ASCII alfanuméricos o _)",
        ));
    }
    if handles.contains_key(handle) {
        return Err(error(
            "referencia_invalida",
            format!("Handle duplicado: {handle}"),
        ));
    }
    Ok(())
}

pub(super) fn validate_fx(
    edit: &FxEdit,
    entry: &TrackSnapshotEntry,
) -> Result<(), TrackBatchError> {
    match edit {
        FxEdit::Insert(node, _) => {
            if node.children().is_some_and(|children| !children.is_empty()) {
                return Err(error(
                    "operacion_invalida",
                    "Inserta contenedores FX vacíos y añade después sus hijos",
                ));
            }
            if let FxKind::Effect {
                plugin_id,
                parameters,
            } = &node.kind
            {
                let TrackDevice::Hay { catalog_id } = plugin_id else {
                    return Err(error(
                        "recurso_no_disponible",
                        "La inserción VST3 requiere preparar una instancia en el host",
                    ));
                };
                let descriptor = devices::find(catalog_id).ok_or_else(|| {
                    error(
                        "recurso_no_disponible",
                        format!("Dispositivo Hay no disponible: {catalog_id}"),
                    )
                })?;
                for (&id, &value) in parameters {
                    let spec = descriptor
                        .parameters
                        .iter()
                        .find(|spec| spec.id.0 == id)
                        .ok_or_else(|| {
                            error(
                                "operacion_invalida",
                                format!("Parámetro FX desconocido: {id}"),
                            )
                        })?;
                    spec.validate(value)
                        .map_err(|e| error("operacion_invalida", e.to_string()))?;
                }
            }
        }
        FxEdit::Parameter(id, parameter, value) => {
            let node = entry
                .track
                .chain
                .find(*id)
                .ok_or_else(|| error("referencia_invalida", "FX inexistente"))?;
            let FxKind::Effect {
                plugin_id: TrackDevice::Hay { catalog_id },
                ..
            } = &node.kind
            else {
                return Err(error(
                    "recurso_no_disponible",
                    "Solo se editan parámetros Hay por lote",
                ));
            };
            let descriptor = devices::find(catalog_id)
                .ok_or_else(|| error("recurso_no_disponible", "Dispositivo Hay no disponible"))?;
            let spec = descriptor
                .parameters
                .iter()
                .find(|spec| spec.id.0 == *parameter)
                .ok_or_else(|| error("operacion_invalida", "Parámetro FX desconocido"))?;
            spec.validate(*value)
                .map_err(|e| error("operacion_invalida", e.to_string()))?;
        }
        _ => {}
    }
    Ok(())
}
