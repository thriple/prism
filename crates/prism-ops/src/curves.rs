//! Edit > Weld (brief 43, part 4a): edges joined end to end become one
//! curve, sharing one curve id, so they are picked, moved and erased as
//! one. A drawing that cuts into it later parts it again
//! (`sticky::break_curves`).

use std::collections::{BTreeMap, BTreeSet};

use prism_topo::{EdgeId, Model, VertexId};

pub const NOTHING: &str = "Select edges joined end to end, then Weld.";
pub const NOT_A_CHAIN: &str =
    "Weld joins edges end to end in one chain without branches, and these branch or do not join.";
pub const ALREADY: &str = "These edges are one curve already.";

/// Why `weld` would refuse, if it would.
pub fn weld_refusal(model: &Model, edges: &BTreeSet<EdgeId>) -> Option<&'static str> {
    let es: Vec<_> = edges.iter().filter_map(|e| model.edge(*e)).collect();
    if es.is_empty() {
        return Some(NOTHING);
    }
    // No branch: each point is the end of at most two of them.
    let mut ends: BTreeMap<VertexId, usize> = BTreeMap::new();
    for e in &es {
        for v in [e.a, e.b] {
            *ends.entry(v).or_default() += 1;
        }
    }
    if ends.values().any(|n| *n > 2) {
        return Some(NOT_A_CHAIN);
    }
    // One piece: every edge reached from the first through shared points.
    let mut seen: BTreeSet<EdgeId> = BTreeSet::new();
    let mut todo = vec![*edges.iter().find(|e| model.edge(**e).is_some())?];
    while let Some(e) = todo.pop() {
        if !seen.insert(e) {
            continue;
        }
        let Some(x) = model.edge(e) else { continue };
        todo.extend(
            edges
                .iter()
                .filter(|k| !seen.contains(k))
                .filter(|k| model.edge(**k).is_some_and(|y| y.has(x.a) || y.has(x.b))),
        );
    }
    if seen.len() != es.len() {
        return Some(NOT_A_CHAIN);
    }
    let ids: BTreeSet<_> = es.iter().map(|e| e.curve_id).collect();
    if es.len() > 1 && ids.len() == 1 && ids.iter().next().is_some_and(|c| c.is_some()) {
        return Some(ALREADY);
    }
    if es.len() == 1 {
        return Some(ALREADY);
    }
    None
}

/// Weld `edges` into one curve.
pub fn weld(model: &mut Model, edges: &BTreeSet<EdgeId>) -> Result<(), &'static str> {
    if let Some(why) = weld_refusal(model, edges) {
        return Err(why);
    }
    let id = model.new_curve_id();
    for e in edges {
        model.set_curve_id(*e, Some(id));
    }
    Ok(())
}
