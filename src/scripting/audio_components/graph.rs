//! Validación de topología estéreo acíclica.
use std::collections::{HashMap, VecDeque};
pub(super) fn validate_graph(count: usize, edges: &[(usize, usize)]) -> Result<Vec<usize>, String> {
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

pub(super) fn endpoint(
    name: &str,
    indices: &HashMap<String, usize>,
    count: usize,
) -> Result<usize, String> {
    match name {
        "input" => Ok(count),
        "output" => Ok(count + 1),
        _ => indices
            .get(name)
            .copied()
            .ok_or_else(|| format!("graph: referencia {name} desconocida")),
    }
}
