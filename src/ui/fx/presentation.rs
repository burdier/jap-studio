//! Proyección de filas y memoria visual por pista; no publica cambios de audio.
use super::{FxTarget, FxTree, Row};
use elevate::musical::{
    fx::{FxChain, FxId, FxKind, FxNode},
    track::TrackId,
};
use gpui::Context;
use std::collections::HashSet;
impl FxTree {
    pub(super) fn target(&self) -> Option<FxTarget> {
        let track = self.track?;
        Some(FxTarget {
            track,
            node: self.views.get(&track)?.selected,
        })
    }
    pub(super) fn rebuild(&mut self) {
        self.rows.clear();
        let Some(track) = self.track else {
            return;
        };
        let view = self.views.entry(track).or_default();
        view.collapsed.retain(|id| self.chain.find(*id).is_some());
        if view
            .selected
            .is_some_and(|id| self.chain.find(id).is_none())
        {
            view.selected = None;
        }
        self.rows.push(Row {
            id: None,
            depth: 0,
            name: format!("Track: {}", self.name),
            container: true,
            enabled: true,
            parallel: false,
        });
        if !view.root_closed {
            flatten(self.chain.nodes(), 1, &view.collapsed, &mut self.rows);
        }
    }
    pub(super) fn sync(
        &mut self,
        track: TrackId,
        name: &str,
        chain: &FxChain,
        cx: &mut Context<Self>,
    ) {
        let changed = self.track != Some(track);
        if !changed && self.name == name && self.chain.revision() == chain.revision() {
            return;
        }
        let previous = self.views.get(&track).and_then(|view| view.selected);
        let previous_index = self
            .rows
            .iter()
            .position(|row| row.id == previous)
            .unwrap_or(0);
        let removed = !changed && previous.is_some_and(|id| chain.find(id).is_none());
        let structural = changed || !same_topology(self.chain.nodes(), chain.nodes());
        self.track = Some(track);
        self.name = name.into();
        self.chain = chain.clone();
        if structural {
            self.rebuild();
            if removed {
                let index = previous_index.min(self.rows.len().saturating_sub(1));
                self.views.get_mut(&track).unwrap().selected =
                    self.rows.get(index).and_then(|row| row.id);
            }
        } else {
            // Renombrar, parámetros o bypass no reconstruyen las filas ni reinician el scroll.
            for row in &mut self.rows {
                if let Some(node) = row.id.and_then(|id| chain.find(id)) {
                    row.name.clone_from(&node.name);
                    row.enabled = node.enabled;
                } else if row.id.is_none() {
                    row.name = format!("Track: {name}");
                }
            }
        }
        cx.notify();
    }
}
fn flatten(nodes: &[FxNode], depth: usize, collapsed: &HashSet<FxId>, rows: &mut Vec<Row>) {
    for node in nodes {
        rows.push(Row {
            id: Some(node.id),
            depth,
            name: node.name.clone(),
            container: node.children().is_some(),
            enabled: node.enabled,
            parallel: matches!(node.kind, FxKind::ParallelChain { .. }),
        });
        if !collapsed.contains(&node.id) {
            if let Some(children) = node.children() {
                flatten(children, depth + 1, collapsed, rows);
            }
        }
    }
}
fn same_topology(a: &[FxNode], b: &[FxNode]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.id == b.id
                && match (a.children(), b.children()) {
                    (Some(a), Some(b)) => same_topology(a, b),
                    (None, None) => true,
                    _ => false,
                }
        })
}
