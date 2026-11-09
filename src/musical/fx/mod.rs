//! Topología editable por pista. Identidades estables; sin UI, DSP ni dispositivos abiertos.
mod edit;
use super::track::TrackDevice;
pub use edit::{FxEdit, FxPlacement};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FxId(pub u64);
impl FxId {
    fn fresh() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum FxKind {
    Effect {
        plugin_id: TrackDevice,
        parameters: BTreeMap<u32, f32>,
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
#[derive(Debug, Clone, PartialEq)]
pub struct FxNode {
    pub id: FxId,
    pub name: String,
    pub enabled: bool,
    pub kind: FxKind,
}
impl FxNode {
    pub fn new(name: impl Into<String>, kind: FxKind) -> Self {
        Self {
            id: FxId::fresh(),
            name: name.into(),
            enabled: true,
            kind,
        }
    }
    pub fn children(&self) -> Option<&[FxNode]> {
        match &self.kind {
            FxKind::Effect { .. } => None,
            FxKind::SerialChain { children } => Some(children),
            FxKind::ParallelChain { branches } => Some(branches),
        }
    }
    fn children_mut(&mut self) -> Option<&mut Vec<FxNode>> {
        match &mut self.kind {
            FxKind::Effect { .. } => None,
            FxKind::SerialChain { children } => Some(children),
            FxKind::ParallelChain { branches } => Some(branches),
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
    pub fn insertion_after(&self, id: Option<FxId>) -> FxPlacement {
        match id.and_then(|id| self.find(id)) {
            Some(node) if node.children().is_some() => FxPlacement::Inside(Some(node.id)),
            Some(node) => FxPlacement::After(node.id),
            None => FxPlacement::Inside(None),
        }
    }
}
