//! Descriptores Lua de efectos estéreo; lectura y validación siempre fuera de audio.
use crate::scripting::sandbox;
use mlua::{Table, Value, chunk::ChunkMode};
use std::collections::{HashMap, HashSet, VecDeque};

const VOLUME: &str = "elevate.amplitude.volume";

#[derive(Debug, Clone)]
pub struct AudioComponent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nodes: Vec<AudioNode>,
    pub edges: Vec<(usize, usize)>,
    pub order: Vec<usize>,
    pub controls: Vec<AudioControl>,
}

#[derive(Debug, Clone)]
pub struct AudioNode {
    pub id: String,
    pub gain: f32,
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
    pub targets: Vec<usize>,
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
        ],
        "descriptor",
    )?;
    if number(&root, "api_version")? != 1.0 || string(&root, "kind")? != "audio_component" {
        return Err("Se requiere api_version=1 y kind=audio_component".into());
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
        if string(&port, "kind")? != "audio" || number(&port, "channels")? != 2.0 {
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
        if string(&row, "primitive")? != VOLUME {
            return Err(format!("{path}.primitive: solo se admite {VOLUME}"));
        }
        let params = table(&row, "parameters")?;
        fields(&params, &["gain"], &format!("{path}.parameters"))?;
        let gain = number(&params, "gain")?;
        if !(0.0..=2.0).contains(&gain) || gain > f32::MAX as f64 {
            return Err(format!("{path}.parameters.gain: fuera de 0..2"));
        }
        nodes.push(AudioNode {
            id: node_id,
            gain: gain as f32,
        });
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
            ],
            &path,
        )?;
        let id = number(&row, "id")?;
        let key = string(&row, "key")?;
        if id.fract() != 0.0
            || !(0.0..=65535.0).contains(&id)
            || !control_ids.insert(id as u32)
            || !identifier(&key)
            || !keys.insert(key.clone())
        {
            return Err(format!("{path}: ID o clave inválidos o duplicados"));
        }
        if string(&row, "kind")? != "number" {
            return Err(format!("{path}.kind: solo number"));
        }
        let unit = label(&row, "unit", 32)?;
        let label = label(&row, "label", 128)?;
        let min = number(&row, "min")?;
        let max = number(&row, "max")?;
        let default = number(&row, "default")?;
        let step = number(&row, "step")?;
        if min < 0.0 || max > 2.0 || min >= max || default < min || default > max || step <= 0.0 {
            return Err(format!("{path}: rango de ganancia inválido"));
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
            if string(&target, "parameter")? != "gain"
                || !destinations.insert(node_index)
                || (nodes[node_index].gain as f64 - default).abs() > 1e-6
            {
                return Err(format!(
                    "{path}: destino repetido, parámetro inválido o valor inicial distinto"
                ));
            }
            targets.push(node_index);
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
        });
    }
    Ok(AudioComponent {
        id,
        name,
        description,
        nodes,
        edges,
        order,
        controls,
    })
}

fn validate_graph(count: usize, edges: &[(usize, usize)]) -> Result<Vec<usize>, String> {
    let input = count;
    let output = count + 1;
    let mut indegree = vec![0; count + 2];
    let mut forward = vec![Vec::new(); count + 2];
    for &(from, to) in edges {
        indegree[to] += 1;
        forward[from].push(to);
    }
    let mut queue = VecDeque::from([input]);
    let mut order = Vec::new();
    while let Some(node) = queue.pop_front() {
        order.push(node);
        for &next in &forward[node] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                queue.push_back(next);
            }
        }
    }
    if order.len() != count + 2 || order.last() != Some(&output) {
        return Err("graph: ciclo o salida inaccesible".into());
    }
    let mut reaches_output = vec![false; count + 2];
    reaches_output[output] = true;
    for &node in order.iter().rev().skip(1) {
        reaches_output[node] = forward[node].iter().any(|&next| reaches_output[next]);
    }
    if (0..count).any(|node| !reaches_output[node]) {
        return Err("graph: nodo sin camino a output".into());
    }
    Ok(order)
}

fn endpoint(name: &str, indices: &HashMap<String, usize>, count: usize) -> Result<usize, String> {
    match name {
        "input" => Ok(count),
        "output" => Ok(count + 1),
        _ => indices
            .get(name)
            .copied()
            .ok_or_else(|| format!("graph: referencia {name} desconocida")),
    }
}
fn identifier(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
fn as_table(value: Value, path: &str) -> Result<Table, String> {
    match value {
        Value::Table(t) if t.metatable().is_none() => Ok(t),
        _ => Err(format!("{path}: se esperaba tabla sin metatabla")),
    }
}
fn table(parent: &Table, key: &str) -> Result<Table, String> {
    as_table(parent.raw_get(key).map_err(|e| e.to_string())?, key)
}
fn string(parent: &Table, key: &str) -> Result<String, String> {
    match parent.raw_get::<Value>(key).map_err(|e| e.to_string())? {
        Value::String(s) => s.to_str().map(|s| s.to_string()).map_err(|e| e.to_string()),
        _ => Err(format!("{key}: se esperaba texto")),
    }
}
fn number(parent: &Table, key: &str) -> Result<f64, String> {
    match parent.raw_get::<Value>(key).map_err(|e| e.to_string())? {
        Value::Integer(n) => Ok(n as f64),
        Value::Number(n) if n.is_finite() => Ok(n),
        _ => Err(format!("{key}: se esperaba número finito")),
    }
}
fn label(parent: &Table, key: &str, max: usize) -> Result<String, String> {
    let value = string(parent, key)?;
    if value.is_empty()
        || value.len() > max
        || value.contains(['\0', '\r', '\n'])
        || value.trim().is_empty()
    {
        Err(format!("{key}: texto inválido"))
    } else {
        Ok(value)
    }
}
fn fields(table: &Table, allowed: &[&str], path: &str) -> Result<(), String> {
    for pair in table.clone().pairs::<Value, Value>() {
        let (key, _) = pair.map_err(|e| e.to_string())?;
        let Value::String(key) = key else {
            return Err(format!("{path}: clave no textual"));
        };
        let key = key.to_str().map_err(|e| e.to_string())?;
        if !allowed.contains(&key.as_ref()) {
            return Err(format!("{path}.{key}: campo desconocido"));
        }
    }
    Ok(())
}
fn array(table: &Table, min: usize, max: usize, path: &str) -> Result<Vec<Table>, String> {
    let len = table.raw_len();
    if len < min || len > max {
        return Err(format!("{path}: tamaño fuera de {min}..{max}"));
    }
    let mut rows = Vec::with_capacity(len);
    for i in 1..=len {
        rows.push(as_table(
            table.raw_get(i).map_err(|e| e.to_string())?,
            &format!("{path}[{i}]"),
        )?);
    }
    let mut count = 0;
    for pair in table.clone().pairs::<Value, Value>() {
        let (key, _) = pair.map_err(|e| e.to_string())?;
        if !matches!(key, Value::Integer(i) if i >= 1 && i as usize <= len) {
            return Err(format!("{path}: arreglo disperso o con claves adicionales"));
        }
        count += 1;
    }
    if count != len {
        return Err(format!("{path}: arreglo disperso"));
    }
    Ok(rows)
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
        assert_eq!(component.controls[0].targets, vec![0, 1]);
    }

    #[test]
    fn graph_cycles_and_unknown_targets_reject_the_whole_descriptor() {
        let cycle = EXAMPLE.replace("to = \"output\"", "to = \"first\"");
        assert!(parse(&cycle).unwrap_err().contains("ciclo"));
        let unknown = EXAMPLE.replace("node = \"second\"", "node = \"missing\"");
        assert!(parse(&unknown).unwrap_err().contains("desconocido"));
    }
}
