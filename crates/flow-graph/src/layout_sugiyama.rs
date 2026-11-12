// @story 04.1
// @implements AC-04, AC-08
use std::collections::{BTreeMap, BTreeSet};

/// Deterministic Sugiyama-inspired topological layering coordinate calculator (`Auto-Layout`).
/// Assigns each node `(pos_x, pos_y)` on an `X = 340px`, `Y = 200px` grid ordered by longest path layer.
pub fn compute_sugiyama_positions(
    node_ids: &[String],
    edges: &[(String, String)],
) -> BTreeMap<String, (f64, f64)> {
    const START_X: f64 = 120.0;
    const START_Y: f64 = 120.0;
    const LAYER_STEP_X: f64 = 340.0;
    const ROW_STEP_Y: f64 = 200.0;

    let mut ordered_nodes: Vec<String> = node_ids.to_vec();
    ordered_nodes.sort();
    ordered_nodes.dedup();

    let mut incoming: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut outgoing: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for nid in &ordered_nodes {
        incoming.insert(nid.clone(), BTreeSet::new());
        outgoing.insert(nid.clone(), BTreeSet::new());
    }
    for (src, dst) in edges {
        if src != dst && incoming.contains_key(src) && incoming.contains_key(dst) {
            incoming.entry(dst.clone()).or_default().insert(src.clone());
            outgoing.entry(src.clone()).or_default().insert(dst.clone());
        }
    }

    let mut indegree: BTreeMap<String, usize> = ordered_nodes
        .iter()
        .map(|n| (n.clone(), incoming.get(n).map_or(0, BTreeSet::len)))
        .collect();
    let mut layer_of: BTreeMap<String, usize> =
        ordered_nodes.iter().map(|n| (n.clone(), 0)).collect();
    let mut ready: BTreeSet<String> = indegree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(n, _)| n.clone())
        .collect();

    while let Some(curr) = ready.pop_first() {
        let curr_layer = *layer_of.get(&curr).unwrap_or(&0);
        if let Some(children) = outgoing.get(&curr) {
            for child in children {
                let entry = layer_of.entry(child.clone()).or_insert(0);
                if *entry < curr_layer + 1 {
                    *entry = curr_layer + 1;
                }
                if let Some(deg) = indegree.get_mut(child) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        ready.insert(child.clone());
                    }
                }
            }
        }
    }

    let mut buckets: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for nid in ordered_nodes {
        let layer = *layer_of.get(&nid).unwrap_or(&0);
        buckets.entry(layer).or_default().push(nid);
    }

    let mut coords = BTreeMap::new();
    for (layer_idx, members) in buckets {
        for (row_idx, nid) in members.into_iter().enumerate() {
            let x = START_X + (layer_idx as f64) * LAYER_STEP_X;
            let y = START_Y + (row_idx as f64) * ROW_STEP_Y;
            coords.insert(nid, (x, y));
        }
    }
    coords
}
