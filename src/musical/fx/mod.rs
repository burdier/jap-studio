//! Topología editable por pista. Identidades estables; sin UI, DSP ni dispositivos abiertos.
mod bands;
pub mod cabinet;
mod edit;
mod preset;
use super::track::TrackDevice;
pub use bands::Band;
pub use edit::{FxEdit, FxPlacement};
pub use preset::VstPreset;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FxId(pub u64);
static NEXT_FX_ID: AtomicU64 = AtomicU64::new(1);
impl FxId {
    fn fresh() -> Self {
        Self(NEXT_FX_ID.fetch_add(1, Ordering::Relaxed))
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum FxKind {
    Effect {
        plugin_id: TrackDevice,
        parameters: BTreeMap<u32, f32>,
    },
    Crossover {
        hz: u16,
        branches: Vec<FxNode>,
    },
    SerialChain {
        children: Vec<FxNode>,
    },
    /// Cada nodo es una rama: un efecto simple o una cadena, posiblemente anidada.
    /// Las ramas reciben la misma entrada y sus salidas se suman sin normalizar.
    ParallelChain {
        branches: Vec<FxNode>,
    },
}
/// Nivel posterior a los FX de una rama, en décimas de dB; el mínimo equivale a silencio.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BranchMix {
    pub db10: i16,
    pub solo: bool,
}
impl BranchMix {
    pub const MIN: i16 = -800;
    pub const MAX: i16 = 60;

    pub fn gain(self, any_solo: bool) -> f32 {
        if self.db10 == Self::MIN || (any_solo && !self.solo) {
            return 0.;
        }
        10f32.powf(f32::from(self.db10) / 200.)
    }
}
/// Fuente externa de detección (sidechain) para compresores nativos y efectos VST3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidechainSource {
    Track(super::track::TrackId),
    Drums,
}
#[derive(Debug, Clone, PartialEq)]
pub struct FxNode {
    pub id: FxId,
    pub name: String,
    pub enabled: bool,
    pub branch_mix: BranchMix,
    pub cabinet_ir: Option<cabinet::CabinetImpulse>,
    pub vst_preset: Option<VstPreset>,
    pub sidechain: Option<SidechainSource>,
    pub kind: FxKind,
}
impl FxNode {
    pub fn new(name: impl Into<String>, kind: FxKind) -> Self {
        Self {
            id: FxId::fresh(),
            name: name.into(),
            enabled: true,
            branch_mix: BranchMix::default(),
            cabinet_ir: None,
            vst_preset: None,
            sidechain: None,
            kind,
        }
    }
    pub fn children(&self) -> Option<&[FxNode]> {
        match &self.kind {
            FxKind::Effect { .. } => None,
            FxKind::SerialChain { children } => Some(children),
            FxKind::ParallelChain { branches } | FxKind::Crossover { branches, .. } => {
                Some(branches)
            }
        }
    }
    fn children_mut(&mut self) -> Option<&mut Vec<FxNode>> {
        match &mut self.kind {
            FxKind::Effect { .. } => None,
            FxKind::SerialChain { children } => Some(children),
            FxKind::ParallelChain { branches } | FxKind::Crossover { branches, .. } => {
                Some(branches)
            }
        }
    }
    pub fn duplicate(&self) -> Self {
        let mut copy = self.clone();
        copy.id = FxId::fresh();
        if let Some(children) = copy.children_mut() {
            for child in children {
                *child = child.duplicate();
            }
        }
        copy
    }
    pub fn find(&self, id: FxId) -> Option<&Self> {
        if self.id == id {
            return Some(self);
        }
        self.children()?.iter().find_map(|child| child.find(id))
    }
    fn find_mut(&mut self, id: FxId) -> Option<&mut Self> {
        if self.id == id {
            return Some(self);
        }
        self.children_mut()?
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct FxChain {
    nodes: Vec<FxNode>,
    revision: u64,
}
impl Default for FxChain {
    fn default() -> Self {
        Self::new()
    }
}
impl FxChain {
    /// Reconstruye una cadena persistida y reserva sus IDs para futuras inserciones.
    pub fn restore(nodes: Vec<FxNode>) -> Result<Self, String> {
        let mut ids = std::collections::HashSet::new();
        edit::validate(&nodes, 0, &mut ids)?;
        if let Some(max) = ids.iter().map(|id| id.0).max() {
            let next = max.checked_add(1).ok_or("Se agotaron los IDs de FX")?;
            NEXT_FX_ID.fetch_max(next, Ordering::Relaxed);
        }
        Ok(Self { nodes, revision: 0 })
    }
    pub const fn new() -> Self {
        Self {
            nodes: Vec::new(),
            revision: 0,
        }
    }
    pub fn nodes(&self) -> &[FxNode] {
        &self.nodes
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn find(&self, id: FxId) -> Option<&FxNode> {
        self.nodes.iter().find_map(|node| node.find(id))
    }
    pub fn duplicate(&self) -> Self {
        Self {
            nodes: self.nodes.iter().map(FxNode::duplicate).collect(),
            revision: 0,
        }
    }
    pub fn set_vst_preset(&mut self, id: FxId, preset: VstPreset) {
        if let Some(node) = self.nodes.iter_mut().find_map(|n| n.find_mut(id)) {
            node.vst_preset = Some(preset);
        }
    }
    pub fn insertion_after(&self, id: Option<FxId>) -> FxPlacement {
        match id.and_then(|id| self.find(id)) {
            Some(node) if node.children().is_some() => FxPlacement::Inside(Some(node.id)),
            Some(node) => FxPlacement::After(node.id),
            None => FxPlacement::Inside(None),
        }
    }
}
