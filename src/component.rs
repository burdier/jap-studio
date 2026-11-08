use std::fmt;

use crate::{ParameterId, Primitive, PrimitiveError, ProcessSpec, StereoFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlEvent {
    SetParameter {
        node: NodeId,
        parameter: ParameterId,
        value: f32,
    },
    SetBypassed {
        node: NodeId,
        bypassed: bool,
    },
}

impl ControlEvent {
    pub fn target(self) -> NodeId {
        match self {
            Self::SetParameter { node, .. } | Self::SetBypassed { node, .. } => node,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ComponentError {
    UnknownNode(NodeId),
    NodeIdsExhausted,
    Primitive(PrimitiveError),
}

impl fmt::Display for ComponentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownNode(node) => write!(formatter, "Nodo desconocido: {}", node.0),
            Self::NodeIdsExhausted => write!(formatter, "No quedan identificadores de nodo"),
            Self::Primitive(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ComponentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Primitive(error) => Some(error),
            _ => None,
        }
    }
}

impl From<PrimitiveError> for ComponentError {
    fn from(error: PrimitiveError) -> Self {
        Self::Primitive(error)
    }
}

struct Node {
    id: NodeId,
    primitive: Box<dyn Primitive>,
}

pub struct Component {
    name: String,
    specification: ProcessSpec,
    nodes: Vec<Node>,
    next_id: u64,
}

impl Component {
    pub fn new(name: impl Into<String>, specification: ProcessSpec) -> Self {
        Self {
            name: name.into(),
            specification,
            nodes: Vec::new(),
            next_id: 1,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add(&mut self, mut primitive: Box<dyn Primitive>) -> Result<NodeId, ComponentError> {
        let next_id = self
            .next_id
            .checked_add(1)
            .ok_or(ComponentError::NodeIdsExhausted)?;
        let id = NodeId(self.next_id);
        primitive.prepare(self.specification);
        self.nodes.push(Node { id, primitive });
        self.next_id = next_id;
        Ok(id)
    }

    pub fn remove(&mut self, id: NodeId) -> Option<Box<dyn Primitive>> {
        let index = self.nodes.iter().position(|node| node.id == id)?;
        Some(self.nodes.remove(index).primitive)
    }

    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &dyn Primitive)> {
        self.nodes
            .iter()
            .map(|node| (node.id, node.primitive.as_ref()))
    }

    pub fn apply(&mut self, event: ControlEvent) -> Result<(), ComponentError> {
        let node = self
            .nodes
            .iter_mut()
            .find(|node| node.id == event.target())
            .ok_or(ComponentError::UnknownNode(event.target()))?;
        match event {
            ControlEvent::SetParameter {
                parameter, value, ..
            } => {
                node.primitive.set_parameter(parameter, value)?;
            }
            ControlEvent::SetBypassed { bypassed, .. } => node.primitive.set_bypassed(bypassed),
        }
        Ok(())
    }

    pub fn process(&mut self, frames: &mut [StereoFrame]) {
        for node in &mut self.nodes {
            node.primitive.process(frames);
        }
    }

    pub fn reset(&mut self) {
        for node in &mut self.nodes {
            node.primitive.reset();
        }
    }
}
