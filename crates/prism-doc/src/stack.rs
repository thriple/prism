//! Live features on a body (Inspector & Features, part 2): the geometry the
//! body is drawn from (`base`), and the features applied on top of it in
//! order. The definition's own model is always the result, so every other
//! part of the program reads it as before. A direct edit is applied to the
//! base and the features run again; a feature that cannot find its edges
//! any more is marked broken and skipped, never silently dropped.

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_topo::{EdgeId, Model};
use serde::{Deserialize, Serialize};

/// An edge a feature points at: its id, and where its ends were, so it can
/// be found again by place if the id is gone.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeRef {
    pub id: EdgeId,
    pub a: DVec3,
    pub b: DVec3,
}

impl EdgeRef {
    pub fn of(m: &Model, id: EdgeId) -> Option<EdgeRef> {
        let (a, b) = ends(m, id)?;
        Some(EdgeRef { id, a, b })
    }
}

fn ends(m: &Model, id: EdgeId) -> Option<(DVec3, DVec3)> {
    let e = m.edge(id)?;
    Some((m.vertex(e.a), m.vertex(e.b)))
}

fn same_place(m: &Model, id: EdgeId, r: &EdgeRef) -> bool {
    let tol = m.tol().dist;
    ends(m, id).is_some_and(|(a, b)| {
        (a.distance(r.a) <= tol && b.distance(r.b) <= tol)
            || (a.distance(r.b) <= tol && b.distance(r.a) <= tol)
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Feature {
    /// A fillet (a radius) or chamfer (how far into each face) on edges.
    Round {
        edges: Vec<EdgeRef>,
        size: f64,
        fillet: bool,
        /// Set by the last run: the edges are gone, or the kernel refused.
        #[serde(default)]
        broken: bool,
    },
}

impl Feature {
    pub fn name(&self) -> &'static str {
        match self {
            Feature::Round { fillet: true, .. } => "Fillet",
            Feature::Round { .. } => "Chamfer",
        }
    }

    pub fn is_broken(&self) -> bool {
        match self {
            Feature::Round { broken, .. } => *broken,
        }
    }

    /// Run it on `m`. `on_base`: the model is the base itself, so its ids
    /// are the ones the feature was made with; later features find their
    /// edges by place first, because the ids a feature makes can shift.
    fn run(&mut self, m: &mut Model, on_base: bool) {
        match self {
            Feature::Round {
                edges,
                size,
                fillet,
                broken,
            } => {
                let Some(found) = resolve(m, edges, on_base) else {
                    *broken = true;
                    return;
                };
                let mut t = m.clone();
                match prism_ops::rounds::round_edges(&mut t, &found, *size, *fillet) {
                    Ok(_) => {
                        *m = t;
                        *broken = false;
                    }
                    Err(_) => *broken = true,
                }
            }
        }
    }
}

/// The edges a feature points at in `m`: by id, then by place. They are
/// brought up to date, so a moved edge is followed.
fn resolve(m: &Model, refs: &mut [EdgeRef], on_base: bool) -> Option<BTreeSet<EdgeId>> {
    let mut out = BTreeSet::new();
    for r in refs.iter_mut() {
        let id = if m.edge(r.id).is_some() && (on_base || same_place(m, r.id, r)) {
            Some(r.id)
        } else {
            m.edges().map(|(e, _)| e).find(|e| same_place(m, *e, r))
        }?;
        if let Some(now) = EdgeRef::of(m, id) {
            *r = now;
        }
        out.insert(id);
    }
    (!out.is_empty()).then_some(out)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub base: Model,
    pub features: Vec<Feature>,
}

impl Stack {
    pub fn new(base: Model) -> Self {
        Stack {
            base,
            features: Vec::new(),
        }
    }

    /// The base with every feature applied in order.
    pub fn evaluate(&mut self) -> Model {
        let mut m = self.base.clone();
        for (i, f) in self.features.iter_mut().enumerate() {
            f.run(&mut m, i == 0);
        }
        m
    }
}
