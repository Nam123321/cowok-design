// @story 04.1
// @implements AC-04, AC-07, AC-08
use std::collections::{BTreeMap, BTreeSet};

use crate::models::{FlowServiceError, MAX_COORD, MIN_COORD};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    White,
    Gray,
    Black,
}

/// Validates that `source_node_id != target_node_id` (rejecting self-loops with `400 VALIDATION_ERROR`).
pub fn ensure_not_self_loop(
    source_node_id: &str,
    target_node_id: &str,
) -> Result<(), FlowServiceError> {
    if source_node_id.trim() == target_node_id.trim() {
        return Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            format!("Self-loop transition is forbidden (sourceNodeId == targetNodeId == '{source_node_id}')"),
        ));
    }
    Ok(())
}

/// Validates finite world coordinates within `[-50_000, 50_000]` and normalizes `-0.0 -> 0.0` at `1/100 px`.
pub fn normalize_coord(val: f64, field_name: &str) -> Result<f64, FlowServiceError> {
    if !val.is_finite() || !(MIN_COORD..=MAX_COORD).contains(&val) {
        return Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            format!("{field_name} must be finite within [{MIN_COORD}, {MAX_COORD}], got {val}"),
        ));
    }
    let rounded = (val * 100.0).round() / 100.0;
    if rounded == 0.0 {
        Ok(0.0)
    } else {
        Ok(rounded)
    }
}

/// Runs deterministic 3-color DFS (`White` / `Gray` / `Black`) over sorted adjacency lists.
/// Returns `Err(422 DAG_CYCLE_DETECTED)` with a deterministic cycle path if any back-edge exists.
pub fn validate_acyclic_dag(
    node_ids: &[String],
    edges: &[(String, String)],
) -> Result<(), FlowServiceError> {
    let mut adj: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for nid in node_ids {
        adj.entry(nid.as_str()).or_default();
    }
    for (src, dst) in edges {
        ensure_not_self_loop(src, dst)?;
        adj.entry(src.as_str()).or_default().insert(dst.as_str());
        adj.entry(dst.as_str()).or_default();
    }

    let mut colors: BTreeMap<&str, Color> = adj.keys().map(|k| (*k, Color::White)).collect();
    let mut stack: Vec<&str> = Vec::new();
    let ordered_keys: Vec<&str> = adj.keys().copied().collect();

    for start in ordered_keys {
        if colors.get(start) == Some(&Color::White) {
            dfs_visit(start, &adj, &mut colors, &mut stack)?;
        }
    }
    Ok(())
}

fn dfs_visit<'a>(
    node: &'a str,
    adj: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    colors: &mut BTreeMap<&'a str, Color>,
    stack: &mut Vec<&'a str>,
) -> Result<(), FlowServiceError> {
    colors.insert(node, Color::Gray);
    stack.push(node);

    if let Some(neighbors) = adj.get(node) {
        for &next in neighbors {
            match colors.get(next).copied().unwrap_or(Color::White) {
                Color::Gray => {
                    let pos = stack.iter().position(|&n| n == next).unwrap_or(0);
                    let mut cycle_chain: Vec<&str> = stack[pos..].to_vec();
                    cycle_chain.push(next);
                    return Err(FlowServiceError::new(
                        422,
                        "DAG_CYCLE_DETECTED",
                        format!(
                            "Directed cycle detected in Flow DAG: {}",
                            cycle_chain.join(" -> ")
                        ),
                    ));
                }
                Color::White => dfs_visit(next, adj, colors, stack)?,
                Color::Black => {}
            }
        }
    }

    stack.pop();
    colors.insert(node, Color::Black);
    Ok(())
}
