//! Compilación de las cadenas disponibles hoy: Utility es una transformación lineal sin memoria.
//! Serie multiplica ganancias; paralelo suma ramas. No sustituye un host de plugins general.
use crate::musical::{
    fx::{FxChain, FxKind, FxNode},
    track::TrackDevice,
};

pub fn compile(chain: &FxChain) -> Result<f32, String> {
    serial(chain.nodes())
}
fn serial(nodes: &[FxNode]) -> Result<f32, String> {
    nodes
        .iter()
        .try_fold(1.0, |gain, node| checked(gain * coefficient(node)?))
}
fn coefficient(node: &FxNode) -> Result<f32, String> {
    if !node.enabled {
        return Ok(1.0);
    }
    match &node.kind {
        FxKind::Effect {
            plugin_id: TrackDevice::Hay { catalog_id },
            parameters,
        } if catalog_id == "hay.utility" => {
            let gain = parameters.get(&0).copied().unwrap_or(1.0);
            if !gain.is_finite() || !(0.0..=2.0).contains(&gain) {
                return Err("Utility: ganancia entre 0× y 2×".into());
            }
            Ok(gain)
        }
        // Render offline sin visualización: Spectrum conserva exactamente la señal.
        FxKind::Effect {
            plugin_id: TrackDevice::Hay { catalog_id },
            ..
        } if catalog_id == super::spectrum::ID => Ok(1.0),
        FxKind::Effect { .. } => Err(format!("{}: procesador no disponible", node.name)),
        FxKind::Crossover { branches, .. } => {
            let [low, high] = branches.as_slice() else {
                return Err("Cruce incompleto".into());
            };
            let low = coefficient(low)?;
            if low == coefficient(high)? {
                Ok(low)
            } else {
                Err("Ganancia distinta por banda".into())
            }
        }
        FxKind::SerialChain { children } => serial(children),
        FxKind::ParallelChain { branches } if branches.is_empty() => Ok(1.0),
        FxKind::ParallelChain { branches } => {
            let solo = branches.iter().any(|b| b.branch_mix.solo);
            branches.iter().try_fold(0., |sum, b| {
                checked(sum + coefficient(b)? * b.branch_mix.gain(solo))
            })
        }
    }
}
fn checked(gain: f32) -> Result<f32, String> {
    if gain.is_finite() && gain <= 1024.0 {
        Ok(gain)
    } else {
        Err("La cadena supera el límite de ganancia 1024×".into())
    }
}
