//! Round many edges of a solid at once through the kernel (Fillet Joins
//! step B, brief 32): fillets or chamfers of one size, with clean corners
//! where they meet (mitres, ball corners). The solid goes to the kernel by
//! [`crate::bridge`] and comes back with its untouched faces, edges and
//! corners keeping their ids and paint; new faces are Default. The lines
//! where a round meets a face it is tangent to are soft and smooth.

use std::collections::BTreeSet;

use prism_kernel::solid::Solid;
use prism_math::DVec3;
use prism_topo::{EdgeId, FaceId, Model};

use crate::bridge::{put_back, region, solid_of};

/// Said when there is nothing to round.
pub const NOTHING: &str = "Refused: pick the edges of a solid to round.";
/// Said for a size that is not more than zero.
pub const NO_SIZE: &str = "Refused: the size must be more than zero.";
/// Said when the edges are not all on one closed solid.
pub const TWO_SOLIDS: &str = "Refused: the edges are not all on one closed solid.";

/// Round `edges` (all of one closed solid) by `size`: fillets, or chamfers
/// when `fillet` is false. Returns the solid's faces afterwards. A refusal
/// leaves the model as it was.
pub fn round_edges(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Result<Vec<FaceId>, &'static str> {
    let first = *edges.first().ok_or(NOTHING)?;
    if !(size.is_finite() && size > model.tol().dist) {
        return Err(NO_SIZE);
    }
    let edge_faces = model.edge_faces();
    let seed = edge_faces
        .get(&first)
        .and_then(|v| v.first())
        .copied()
        .ok_or(TWO_SOLIDS)?;
    let faces = solid_of(model, seed).map_err(|_| TWO_SOLIDS)?;
    for e in edges {
        let on = edge_faces
            .get(e)
            .is_some_and(|fs| fs.iter().filter(|f| faces.contains(f)).count() == 2);
        if !on {
            return Err(TWO_SOLIDS);
        }
    }
    let r = region(model, &faces)?;
    let mut s = Solid::build(&r, model.tol().dist)?;
    let keys: Vec<u64> = edges.iter().map(|e| u64::from(e.0)).collect();
    s.round(&keys, size, fillet)?;
    let back = s.region()?;
    let mut work = model.clone();
    let before: BTreeSet<EdgeId> = work.edges().map(|(id, _)| id).collect();
    let out = put_back(&mut work, &faces, &back)?;
    soften_tangent(&mut work, &before);
    if !crate::check::problems(&work).is_empty() {
        return Err(prism_kernel::solid::CANNOT_STORE);
    }
    *model = work;
    Ok(out)
}

/// Why rounding `edges` would be refused, if it would.
pub fn refusal(
    model: &Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<&'static str> {
    let mut m = model.clone();
    round_edges(&mut m, edges, size, fillet).err()
}

/// The new edges (not in `before`) between two faces that are tangent
/// there are soft and smooth: the line between a round and its faces.
fn soften_tangent(model: &mut Model, before: &BTreeSet<EdgeId>) {
    let edge_faces = model.edge_faces();
    let mut soft = Vec::new();
    for (id, e) in model.edges() {
        if before.contains(&id) {
            continue;
        }
        let Some([f, g]) = edge_faces.get(&id).map(Vec::as_slice) else {
            continue;
        };
        let mid = middle(model, e);
        let (Some(a), Some(b)) = (model.face(*f), model.face(*g)) else {
            continue;
        };
        let (na, nb) = (a.surface.normal_at(mid), b.surface.normal_at(mid));
        if na.dot(nb) > 1.0 - 1e-6 {
            soft.push(id);
        }
    }
    for id in soft {
        model.set_soft(id, true, true);
    }
}

/// A point halfway along an edge.
fn middle(model: &Model, e: &prism_topo::Edge) -> DVec3 {
    let (a, b) = (model.vertex(e.a), model.vertex(e.b));
    match &e.curve {
        prism_geom::Curve::Line => (a + b) * 0.5,
        prism_geom::Curve::Arc { centre, axis, .. } => {
            let (xa, xb) = (a - *centre, b - *centre);
            let mut sweep = axis.dot(xa.cross(xb)).atan2(xa.dot(xb));
            if sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
            *centre + prism_math::DQuat::from_axis_angle(*axis, sweep / 2.0) * xa
        }
        prism_geom::Curve::Spline(sp) => sp.point(sp.halfway()),
    }
}
