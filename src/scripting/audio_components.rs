//! Descriptores Lua de efectos estéreo; lectura y validación siempre fuera de audio.
use crate::scripting::sandbox;
use mlua::{Table, Value, chunk::ChunkMode};
use std::collections::{HashMap, HashSet};
mod fields;
mod graph;
use fields::*;
use graph::*;

mod native;
use crate::ParameterId;
pub use native::{AudioNode, NativeKind};

#[derive(Debug, Clone)]
pub struct AudioComponent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nodes: Vec<AudioNode>,
    pub edges: Vec<(usize, usize)>,
    pub order: Vec<usize>,
    pub controls: Vec<AudioControl>,
    pub response_node: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct AudioControl {
    pub id: u32,
    pub key: String,
    pub label: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub step: f32,
    pub unit: String,
    pub targets: Vec<(usize, ParameterId)>,
    pub logarithmic: bool,
    pub choices: Vec<String>,
}

impl AudioControl {
    pub fn accepts(&self, value: f32) -> bool {
        value.is_finite()
            && value >= self.min
            && value <= self.max
            && (self.choices.is_empty() || value.fract() == 0.)
    }
    pub fn from_fraction(&self, fraction: f32) -> f32 {
        let f = fraction.clamp(0., 1.);
        let value = if self.logarithmic {
            self.min * (self.max / self.min).powf(f)
        } else {
            self.min + f * (self.max - self.min)
        };
        if self.choices.is_empty() {
            value.clamp(self.min, self.max)
        } else {
            value.round()
        }
    }
    pub fn fraction(&self, value: f32) -> f32 {
        if self.logarithmic {
            (value / self.min).ln() / (self.max / self.min).ln()
        } else {
            (value - self.min) / (self.max - self.min)
        }
        .clamp(0., 1.)
    }
}

pub fn parse(source: &str) -> Result<AudioComponent, String> {
    if source.len() > sandbox::MAX_SOURCE_BYTES {
        return Err("Componente Lua supera 256 KiB".into());
    }
    let lua = sandbox::new().map_err(|e| e.to_string())?;
    let value: Value = lua
        .load(source)
        .set_mode(ChunkMode::Text)
        .eval()
        .map_err(|e| e.to_string())?;
    let root = as_table(value, "descriptor")?;
    fields(
        &root,
        &[
            "api_version",
            "kind",
            "id",
            "name",
            "description",
            "ports",
            "graph",
            "controls",
            "response_node",
        ],
        "descriptor",
    )?;
    let version = integer(&root, "api_version")?;
    if ![1, 2].contains(&version) || string(&root, "kind")? != "audio_component" {
        return Err("Se requiere api_version=1 o 2 y kind=audio_component".into());
    }
    let id = string(&root, "id")?;
    if id.len() < 3 || id.len() > 64 || !id.contains('.') || !id.split('.').all(identifier) {
        return Err("id de componente inválido".into());
    }
    let name = label(&root, "name", 128)?;
    let description = label(&root, "description", 512)?;
    let ports = table(&root, "ports")?;
    fields(&ports, &["input", "output"], "ports")?;
    for key in ["input", "output"] {
        let port = table(&ports, key)?;
        fields(&port, &["kind", "channels"], key)?;
        if string(&port, "kind")? != "audio" || integer(&port, "channels")? != 2 {
            return Err(format!("ports.{key}: se requiere audio estéreo"));
        }
    }
    let graph = table(&root, "graph")?;
    fields(&graph, &["nodes", "edges"], "graph")?;
    let node_rows = array(&table(&graph, "nodes")?, 1, 32, "graph.nodes")?;
    let mut nodes = Vec::with_capacity(node_rows.len());
    let mut indices = HashMap::new();
    for (index, row) in node_rows.into_iter().enumerate() {
        let path = format!("graph.nodes[{}]", index + 1);
        fields(&row, &["id", "primitive", "parameters"], &path)?;
        let node_id = string(&row, "id")?;
        if node_id.len() > 64
            || !identifier(&node_id)
            || matches!(node_id.as_str(), "input" | "output")
            || indices.insert(node_id.clone(), index).is_some()
        {
            return Err(format!("{path}.id: ID inválido o duplicado"));
        }
        nodes.push(native::parse_node(&row, node_id, version, &path)?);
    }
    let edge_rows = array(&table(&graph, "edges")?, 2, 64, "graph.edges")?;
    let mut edges = Vec::with_capacity(edge_rows.len());
    let mut seen_edges = HashSet::new();
    for (index, row) in edge_rows.into_iter().enumerate() {
        let path = format!("graph.edges[{}]", index + 1);
        fields(&row, &["from", "to"], &path)?;
        let from = string(&row, "from")?;
        let to = string(&row, "to")?;
        let source = endpoint(&from, &indices, nodes.len())?;
        let destination = endpoint(&to, &indices, nodes.len())?;
        if source == nodes.len() + 1
            || destination == nodes.len()
            || source == destination
            || !seen_edges.insert((source, destination))
        {
            return Err(format!("{path}: conexión inválida o duplicada"));
        }
        edges.push((source, destination));
    }
    let order = validate_graph(nodes.len(), &edges)?;
    let control_rows = array(&table(&root, "controls")?, 1, 16, "controls")?;
    let mut controls = Vec::with_capacity(control_rows.len());
    let mut control_ids = HashSet::new();
    let mut keys = HashSet::new();
    let mut destinations = HashSet::new();
    for (index, row) in control_rows.into_iter().enumerate() {
        let path = format!("controls[{}]", index + 1);
        fields(
            &row,
            &[
                "id", "key", "label", "kind", "min", "max", "default", "step", "unit", "targets",
                "scale", "choices",
            ],
            &path,
        )?;
        let id = integer(&row, "id")?;
        let key = string(&row, "key")?;
        if !(0..=65535).contains(&id)
            || !control_ids.insert(id as u32)
            || !identifier(&key)
            || !keys.insert(key.clone())
        {
            return Err(format!("{path}: ID o clave inválidos o duplicados"));
        }
        let kind = string(&row, "kind")?;
        let choices = native::choices(&row, &kind, version)?;
        let logarithmic = match row.raw_get::<Value>("scale").map_err(|e| e.to_string())? {
            Value::Nil => false,
            Value::String(s) if version == 2 && s.to_str().is_ok_and(|s| s == "log") => true,
            _ => return Err(format!("{path}.scale: solo log en v2")),
        };
        let unit = label(&row, "unit", 32)?;
        let label = label(&row, "label", 128)?;
        let min = number(&row, "min")?;
        let max = number(&row, "max")?;
        let default = number(&row, "default")?;
        let step = number(&row, "step")?;
        if min < 0.0
            || max > f32::MAX as f64
            || (logarithmic && (min as f32 <= 0. || !choices.is_empty()))
            || (!choices.is_empty()
                && (min != 0.
                    || max != (choices.len() - 1) as f64
                    || default.fract() != 0.
                    || step != 1.))
            || min as f32 >= max as f32
            || default < min
            || default > max
            || step as f32 <= 0.0
            || step > f32::MAX as f64
        {
            return Err(format!("{path}: rango de control inválido"));
        }
        let target_rows = array(&table(&row, "targets")?, 1, 32, &format!("{path}.targets"))?;
        let mut targets = Vec::with_capacity(target_rows.len());
        for (target_index, target) in target_rows.into_iter().enumerate() {
            fields(
                &target,
                &["node", "parameter"],
                &format!("{path}.targets[{}]", target_index + 1),
            )?;
            let node = string(&target, "node")?;
            let Some(&node_index) = indices.get(&node) else {
                return Err(format!("{path}: nodo destino desconocido"));
            };
            let parameter = string(&target, "parameter")?;
            let spec = nodes[node_index]
                .info()
                .parameters
                .iter()
                .find(|s| s.key == parameter)
                .ok_or_else(|| format!("{path}: parámetro desconocido"))?;
            if min < spec.min as f64
                || max > spec.max as f64
                || !destinations.insert((node_index, spec.id))
                || nodes[node_index].values[spec.id.0 as usize] != default as f32
                || (nodes[node_index].kind == NativeKind::Filter
                    && spec.id.0 == 2
                    && choices.len() != 3)
            {
                return Err(format!(
                    "{path}: destino repetido, rango inválido o valor inicial distinto"
                ));
            }
            targets.push((node_index, spec.id));
        }
        controls.push(AudioControl {
            id: id as u32,
            key,
            label,
            min: min as f32,
            max: max as f32,
            default: default as f32,
            step: step as f32,
            unit,
            targets,
            logarithmic,
            choices,
        });
    }
    let response_node = match root
        .raw_get::<Value>("response_node")
        .map_err(|e| e.to_string())?
    {
        Value::Nil => None,
        Value::String(s) if version == 2 => {
            let id = s.to_str().map_err(|e| e.to_string())?;
            let index = *indices
                .get(id.as_ref())
                .ok_or("response_node desconocido")?;
            if nodes[index].kind != NativeKind::Filter {
                return Err("response_node requiere filtro".into());
            }
            Some(index)
        }
        _ => return Err("response_node requiere ID textual y v2".into()),
    };
    Ok(AudioComponent {
        id,
        name,
        description,
        nodes,
        edges,
        order,
        controls,
        response_node,
    })
}

#[cfg(test)]
mod tests {
    use super::parse;

    const EXAMPLE: &str = include_str!("../../assets/scripts/components/audio/double_volume.lua");

    #[test]
    fn bundled_component_has_two_ordered_stages_and_one_macro() {
        let component = parse(EXAMPLE).unwrap();
        assert_eq!(component.id, "hay.fx.double_volume");
        assert_eq!(component.order, vec![2, 0, 1, 3]);
        assert_eq!(
            component.controls[0].targets,
            vec![(0, crate::ParameterId(0)), (1, crate::ParameterId(0))]
        );
    }

    #[test]
    fn graph_cycles_and_unknown_targets_reject_the_whole_descriptor() {
        let cycle = EXAMPLE.replace("to = \"output\"", "to = \"first\"");
        assert!(parse(&cycle).unwrap_err().contains("ciclo"));
        let unknown = EXAMPLE.replace("node = \"second\"", "node = \"missing\"");
        assert!(parse(&unknown).unwrap_err().contains("desconocido"));
    }
}
