//! Ediciones transaccionales: rechazan ciclos y destinos obsoletos sin perder nodos.
use super::{FxChain, FxId, FxKind, FxNode};

#[derive(Clone, Copy, Debug)]
pub enum FxPlacement {
    Inside(Option<FxId>),
    Before(FxId),
    After(FxId),
}
#[derive(Clone, Debug)]
pub enum FxEdit {
    Insert(FxNode, FxPlacement),
    Remove(FxId),
    Duplicate(FxId),
    Rename(FxId, String),
    Toggle(FxId),
    Parameter(FxId, u32, f32),
    Move(FxId, FxPlacement),
}
impl FxChain {
    pub fn apply(&mut self, edit: FxEdit) -> Result<Option<FxId>, String> {
        let mut next = self.clone();
        let selected = match edit {
            FxEdit::Insert(node, place) => {
                let id = node.id;
                insert(&mut next.nodes, node, place)?;
                Some(id)
            }
            FxEdit::Remove(id) => {
                take(&mut next.nodes, id).ok_or("FX inexistente")?;
                None
            }
            FxEdit::Duplicate(id) => {
                let mut node = next.find(id).ok_or("FX inexistente")?.duplicate();
                node.name.push_str(" copia");
                let new_id = node.id;
                insert(&mut next.nodes, node, FxPlacement::After(id))?;
                Some(new_id)
            }
            FxEdit::Move(id, place) => {
                let target = match place {
                    FxPlacement::Inside(id) => id,
                    FxPlacement::Before(id) | FxPlacement::After(id) => Some(id),
                };
                let node = next.find(id).ok_or("FX inexistente")?;
                if target.is_some_and(|target| node.find(target).is_some()) {
                    return Err(
                        "No puedes mover un contenedor dentro de sí mismo o de sus hijos".into(),
                    );
                }
                let node = take(&mut next.nodes, id).ok_or("FX inexistente")?;
                insert(&mut next.nodes, node, place)?;
                Some(id)
            }
            FxEdit::Rename(id, name) => {
                let name = name.trim();
                if name.is_empty() || name.contains(['\n', '\r', '\0']) {
                    return Err("Escribe un nombre de una sola línea".into());
                }
                next.node_mut(id)?.name = name.into();
                Some(id)
            }
            FxEdit::Toggle(id) => {
                let node = next.node_mut(id)?;
                node.enabled = !node.enabled;
                Some(id)
            }
            FxEdit::Parameter(id, parameter, value) => {
                if !value.is_finite() {
                    return Err("El parámetro debe ser finito".into());
                }
                let FxKind::Effect { parameters, .. } = &mut next.node_mut(id)?.kind else {
                    return Err("Selecciona un efecto".into());
                };
                parameters.insert(parameter, value);
                Some(id)
            }
        };
        let mut ids = std::collections::HashSet::new();
        validate(&next.nodes, 0, &mut ids)?;
        next.revision = self.revision.wrapping_add(1);
        *self = next;
        Ok(selected)
    }
    fn node_mut(&mut self, id: FxId) -> Result<&mut FxNode, String> {
        self.nodes
            .iter_mut()
            .find_map(|node| node.find_mut(id))
            .ok_or_else(|| "FX inexistente".into())
    }
}
pub(super) fn validate(
    nodes: &[FxNode],
    depth: usize,
    ids: &mut std::collections::HashSet<FxId>,
) -> Result<(), String> {
    if depth > 32 {
        return Err("Máximo 32 niveles de FX".into());
    }
    for node in nodes {
        if !ids.insert(node.id) {
            return Err("Identidad FX duplicada".into());
        }
        if ids.len() > 256 {
            return Err("Máximo 256 nodos FX por pista".into());
        }
        if let Some(children) = node.children() {
            validate(children, depth + 1, ids)?;
        }
    }
    Ok(())
}
fn take(nodes: &mut Vec<FxNode>, id: FxId) -> Option<FxNode> {
    if let Some(index) = nodes.iter().position(|node| node.id == id) {
        return Some(nodes.remove(index));
    }
    nodes
        .iter_mut()
        .find_map(|node| take(node.children_mut()?, id))
}
fn insert(nodes: &mut Vec<FxNode>, node: FxNode, place: FxPlacement) -> Result<(), String> {
    match place {
        FxPlacement::Inside(None) => nodes.push(node),
        FxPlacement::Inside(Some(id)) => {
            let parent = nodes
                .iter_mut()
                .find_map(|node| node.find_mut(id))
                .ok_or("Contenedor inexistente")?;
            parent
                .children_mut()
                .ok_or("Un efecto no admite hijos")?
                .push(node);
        }
        FxPlacement::Before(id) | FxPlacement::After(id) => {
            let siblings = siblings(nodes, id).ok_or("Destino inexistente")?;
            let index = siblings.iter().position(|node| node.id == id).unwrap();
            siblings.insert(
                index + usize::from(matches!(place, FxPlacement::After(_))),
                node,
            );
        }
    }
    Ok(())
}
fn siblings(nodes: &mut Vec<FxNode>, id: FxId) -> Option<&mut Vec<FxNode>> {
    if nodes.iter().any(|node| node.id == id) {
        return Some(nodes);
    }
    nodes
        .iter_mut()
        .find_map(|node| siblings(node.children_mut()?, id))
}
