//! Shell / Thicken (brief 42, part 3a): faces become a solid of a set
//! thickness.
//!
//! The faces joined to the ones given by edges are taken whole: an open
//! sheet becomes a slab, and a closed body becomes hollow, less the faces
//! left open (a box becomes a tray). Each face moves along its own normal
//! by the thickness (out for more than zero, in for less), and a corner
//! goes where the moved faces meet, so the walls are the same thickness
//! everywhere. Where an edge has a face on one side only (a sheet's rim, or
//! the edge of an open face), a wall joins it to its moved copy: flat for a
//! line (two flat triangles if it cannot be), and for an arc a cylinder, a
//! cone or a flat ring.
//!
//! Curved faces (part 3c): cylinders, cones, spheres and tori move to the
//! same kind of surface (a cylinder of a radius the thickness more or
//! less), and their arcs to arcs; a point on an edge moves the shortest
//! way onto every moved face it is on. A freeform face on its own becomes
//! the slab Push/Pull makes of it (`thicken`). Corners of up to three
//! surfaces' directions are exact.
//!
//! Joined freeform faces (brief 49 part 1): a body of two or more
//! freeform faces, joined only where they meet smoothly (the same normal
//! along the shared edge, so each side's offset already meets the
//! other's there), offsets each face on its own and shares the fitted
//! offset of a join between its two faces.
//!
//! Creases (part 2): a body may also mix flat (and other analytic) faces
//! with freeform ones. Where two kept faces join at an angle, their
//! offsets no longer meet by themselves: the true mitre is found by
//! marching along the shared edge and, at each point, solving for where
//! both offsets meet exactly (Newton, in the plane of their two normals,
//! as `corner` does for an exact flat corner). An inside corner (the
//! mitre trims back within each face's own edge) is cut this way. An
//! outside corner (part 2b: the mitre would reach past a freeform face's
//! own edge, which does not extend) gets no single mitre; each face
//! keeps its own straight offset edge instead, and a small face bridges
//! the gap between the two. A fold too sharp to mitre at all (close to
//! doubling back on itself) is refused by name.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Cone, Curve, Nurbs, Surface};
use prism_math::tol::ANGLE;
use prism_math::{DVec2, DVec3, Plane, Tol, newell};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

pub const NOTHING: &str = "Refused: click a face to shell or thicken.";
pub const ZERO: &str = "Refused: a thickness of zero makes nothing.";
pub const ALL_OPEN: &str = "Refused: every face is left open, so there is nothing to thicken.";
pub const FREEFORM: &str = "Refused: Shell / Thicken takes a freeform face on its own, but not one joined to other faces, nor faces bounded by freeform curves, yet.";
pub const MIXED: &str =
    "Refused: these faces face different ways. Solid > Heal turns them the same way.";
pub const BRANCHES: &str = "Refused: more than two faces meet at an edge here.";
pub const CORNER: &str = "Refused: more than three faces meet at a corner on the rim here, or round a curved face, and their moved copies do not meet at one point. Not yet.";
pub const TOO_THICK: &str = "Refused: that thickness turns a face inside out. Try a smaller one.";
pub const RIM_STYLE: &str = "Refused: a rim style other than Square works on joined faces with a freeform one, not on this shape yet.";
pub const FLUSH_OFF: &str = "Refused: a flush rim's plane must pass through the edge it ends.";
pub const FLUSH_PARALLEL: &str = "Refused: that plane lies along the face at its rim, so a wall cannot end in it. Pick another plane or Square.";
/// The thinnest wall the Shell / Thicken tool and the Inspector will make
/// from what a person types or drags, in metres: 0.1 cm. The kernel itself
/// builds much thinner walls (down to 0.01 cm on a 15 m hull, NOTES "thin
/// walls"), and drawing and editing stay exact to 0.01 cm; this only keeps
/// a stray tiny size from making a skin nobody asked for.
pub const MIN_THICKNESS: f64 = 1e-3;

pub const TOO_THIN: &str =
    "Too thin: a wall must be at least 0.1 cm (1 mm). Type a larger thickness, or drag further.";

/// `TOO_THIN` for a thickness (signed) that is not zero and under the
/// least; zero is left to the kernel's own message (nothing sized yet).
pub fn thin_refusal(thickness: f64) -> Option<&'static str> {
    // A size under a nanometre is nothing sized yet (the pointer just
    // pressed), not a thin wall.
    (thickness.abs() > 1e-9 && thickness.abs() < MIN_THICKNESS * (1.0 - 1e-9)).then_some(TOO_THIN)
}

pub const FLUSH_REACH: &str = "Refused: to end flush in that plane the surface would have to be carried on a long way past its own edge. Use a plane closer to square, or Square.";
pub const SHARP: &str = "Refused: these faces fold back too sharply here to cut a mitre. A rounded corner there is not done yet.";

/// The faces joined to `seeds` by edges: their sheets or bodies.
pub fn body(model: &Model, seeds: &BTreeSet<FaceId>) -> BTreeSet<FaceId> {
    let ef = model.edge_faces();
    let mut out = BTreeSet::new();
    let mut todo: Vec<FaceId> = seeds
        .iter()
        .copied()
        .filter(|f| model.face(*f).is_some())
        .collect();
    while let Some(f) = todo.pop() {
        if !out.insert(f) {
            continue;
        }
        let Some(face) = model.face(f) else {
            continue;
        };
        for e in face.edge_ids() {
            todo.extend(
                ef.get(&e)
                    .into_iter()
                    .flatten()
                    .filter(|g| !out.contains(*g)),
            );
        }
    }
    out
}

/// How the wall at a rim ends (brief 49 part 3). A rim is an edge with a
/// kept face on one side only; the same style is given to every edge of an
/// open boundary.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RimStyle {
    /// Cut straight across the surface (the default).
    Square,
    /// The wall ends in a plane through the edge, so a part that meets
    /// there fits exactly: the offset surface is trimmed where it meets
    /// the plane, and the wall lies in the plane.
    Flush(Plane),
}

/// The style of each rim edge; an edge not listed is `Square`.
pub type RimStyles = BTreeMap<EdgeId, RimStyle>;

/// The open boundaries of the shell of `faces` leaving `open` open: the
/// rim edges (an edge with a kept face on one side only) joined end to end
/// through points where exactly two of them meet. Each boundary is the
/// edges in order along it; one style is chosen for the whole of it.
pub fn boundaries(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
) -> Vec<Vec<EdgeId>> {
    let body = body(model, faces);
    let mut kept_uses: BTreeMap<EdgeId, usize> = BTreeMap::new();
    let mut all_uses: BTreeMap<EdgeId, usize> = BTreeMap::new();
    for id in &body {
        let Some(f) = model.face(*id) else {
            continue;
        };
        for side in f.sides() {
            *all_uses.entry(side.edge).or_default() += 1;
            if !open.contains(id) {
                *kept_uses.entry(side.edge).or_default() += 1;
            }
        }
    }
    let rims: Vec<EdgeId> = kept_uses
        .iter()
        .filter(|(e, k)| **k == 1 && all_uses[e] <= 2)
        .map(|(e, _)| *e)
        .collect();
    let mut at: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for e in &rims {
        if let Some(edge_data) = model.edge(*e) {
            at.entry(edge_data.a).or_default().push(*e);
            at.entry(edge_data.b).or_default().push(*e);
        }
    }
    let mut seen: BTreeSet<EdgeId> = BTreeSet::new();
    let mut out: Vec<Vec<EdgeId>> = Vec::new();
    for start in &rims {
        if seen.contains(start) {
            continue;
        }
        // Grow both ways from the start through points with two rims.
        let mut chain: std::collections::VecDeque<EdgeId> = [*start].into();
        seen.insert(*start);
        for front in [true, false] {
            loop {
                let end = *(if front { chain.back() } else { chain.front() }).unwrap_or(start);
                let Some(edge_data) = model.edge(end) else {
                    break;
                };
                // The point this end of the chain leaves by.
                let shared = if chain.len() == 1 {
                    if front { edge_data.b } else { edge_data.a }
                } else {
                    let other = if front {
                        chain.get(chain.len() - 2)
                    } else {
                        chain.get(1)
                    }
                    .and_then(|o| model.edge(*o));
                    let Some(other) = other else { break };
                    if edge_data.a == other.a || edge_data.a == other.b {
                        edge_data.b
                    } else {
                        edge_data.a
                    }
                };
                let next = at
                    .get(&shared)
                    .filter(|v| v.len() == 2)
                    .and_then(|v| v.iter().copied().find(|x| *x != end && !seen.contains(x)));
                let Some(next) = next else { break };
                seen.insert(next);
                if front {
                    chain.push_back(next);
                } else {
                    chain.push_front(next);
                }
            }
        }
        out.push(chain.into_iter().collect());
    }
    // A loop of rims turns corners: split it at each sharp one.
    let mut runs: Vec<Vec<EdgeId>> = Vec::new();
    for chain in out {
        runs.extend(split_at_corners(model, &chain, &at));
    }
    runs
}

/// The vertex two edges share, if any.
fn shared_vertex(model: &Model, a: EdgeId, b: EdgeId) -> Option<VertexId> {
    let (ea, eb) = (model.edge(a)?, model.edge(b)?);
    [ea.a, ea.b].into_iter().find(|v| *v == eb.a || *v == eb.b)
}

/// Whether the end of one run meets the end of another at a point where
/// only those two rims meet.
fn touch(model: &Model, a: &[EdgeId], b: &[EdgeId], at: &BTreeMap<VertexId, Vec<EdgeId>>) -> bool {
    let ends = |r: &[EdgeId]| [r.first().copied(), r.last().copied()];
    ends(a).into_iter().flatten().any(|x| {
        ends(b).into_iter().flatten().any(|y| {
            shared_vertex(model, x, y).is_some_and(|v| at.get(&v).is_some_and(|l| l.len() == 2))
        })
    })
}

/// A chain of rim edges, cut where it turns more than 30 degrees (and
/// at its far end if it closes on itself).
fn split_at_corners(
    model: &Model,
    chain: &[EdgeId],
    at: &BTreeMap<VertexId, Vec<EdgeId>>,
) -> Vec<Vec<EdgeId>> {
    let n = chain.len();
    if n < 2 {
        return vec![chain.to_vec()];
    }
    // The direction an edge leaves a point by.
    let leaving = |e: EdgeId, v: VertexId| -> DVec3 {
        let Some(edge_data) = model.edge(e) else {
            return DVec3::ZERO;
        };
        let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
        let t = if edge_data.a == v { 1e-3 } else { 1.0 - 1e-3 };
        (edge_data.curve.point(pa, pb, t) - model.vertex(v)).normalize_or_zero()
    };
    let closed = shared_vertex(model, chain[n - 1], chain[0])
        .is_some_and(|v| at.get(&v).is_some_and(|l| l.len() == 2) && n > 2);
    let joints = if closed { n } else { n - 1 };
    // Whether the chain turns sharply after edge `i`.
    let sharp: Vec<bool> = (0..joints)
        .map(|i| {
            let (a, b) = (chain[i], chain[(i + 1) % n]);
            let Some(v) = shared_vertex(model, a, b) else {
                return true;
            };
            // Straight on means the second edge leaves opposite to how
            // the first does.
            -leaving(a, v).dot(leaving(b, v)) < 0.866
        })
        .collect();
    let Some(first) = sharp.iter().position(|s| *s) else {
        return vec![chain.to_vec()];
    };
    // Start just after a sharp turn (for a loop), or at the start.
    let from = if closed { (first + 1) % n } else { 0 };
    let mut runs: Vec<Vec<EdgeId>> = vec![Vec::new()];
    for k in 0..n {
        let i = (from + k) % n;
        if let Some(r) = runs.last_mut() {
            r.push(chain[i]);
        }
        if i < joints && sharp[i] && k + 1 < n {
            runs.push(Vec::new());
        }
    }
    runs.retain(|r| !r.is_empty());
    runs
}

/// The plane a boundary lies in, if it lies in one and is not a straight
/// line (a line lies in many planes: no way to choose), sampled along its
/// edges.
pub fn boundary_plane(model: &Model, edges: &[EdgeId], tol: &Tol) -> Option<Plane> {
    let mut pts: Vec<DVec3> = Vec::new();
    for e in edges {
        let edge_data = model.edge(*e)?;
        let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
        for i in 0..=8 {
            pts.push(edge_data.curve.point(pa, pb, f64::from(i) / 8.0));
        }
    }
    let p0 = *pts.first()?;
    let p1 = pts
        .iter()
        .copied()
        .max_by(|a, b| a.distance(p0).total_cmp(&b.distance(p0)))?;
    if tol.is_zero(p1.distance(p0)) {
        return None;
    }
    let p2 = pts
        .iter()
        .copied()
        .max_by(|a, b| distance_to_line(*a, p0, p1).total_cmp(&distance_to_line(*b, p0, p1)))?;
    if tol.is_zero(distance_to_line(p2, p0, p1)) {
        return None;
    }
    let plane = Plane::new(p0, (p1 - p0).cross(p2 - p0));
    pts.iter()
        .all(|p| tol.is_zero(plane.distance(*p)))
        .then_some(plane)
}

/// The two ends of a boundary that is one straight line (its edges all
/// along it), if it is.
pub fn boundary_line(model: &Model, edges: &[EdgeId], tol: &Tol) -> Option<(DVec3, DVec3)> {
    let mut pts: Vec<DVec3> = Vec::new();
    for e in edges {
        let edge_data = model.edge(*e)?;
        let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
        for i in 0..=8 {
            pts.push(edge_data.curve.point(pa, pb, f64::from(i) / 8.0));
        }
    }
    let p0 = *pts.first()?;
    let p1 = pts
        .iter()
        .copied()
        .max_by(|a, b| a.distance(p0).total_cmp(&b.distance(p0)))?;
    if tol.is_zero(p1.distance(p0)) {
        return None;
    }
    pts.iter()
        .all(|p| tol.is_zero(distance_to_line(*p, p0, p1)))
        .then_some((p0, p1))
}

/// Points along a boundary, in order, for drawing it.
pub fn boundary_points(model: &Model, edges: &[EdgeId]) -> Vec<DVec3> {
    let mut out: Vec<DVec3> = Vec::new();
    let mut at: Option<VertexId> = None;
    for e in edges {
        let Some(edge_data) = model.edge(*e) else {
            continue;
        };
        let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
        // Walk each edge away from the vertex the chain arrived by (the
        // first, away from the end it does not share with the next).
        let start = at.unwrap_or_else(|| match edges.get(1).and_then(|n| model.edge(*n)) {
            Some(n) if edge_data.a == n.a || edge_data.a == n.b => edge_data.b,
            _ => edge_data.a,
        });
        let forward = start == edge_data.a;
        for i in 0..=8 {
            let t = f64::from(i) / 8.0;
            out.push(
                edge_data
                    .curve
                    .point(pa, pb, if forward { t } else { 1.0 - t }),
            );
        }
        at = Some(if forward { edge_data.b } else { edge_data.a });
    }
    out
}

/// Whether a plane is one a part would plausibly meet there: a plane of
/// the world's axes, or the plane of a flat face of the model.
fn meaningful_plane(model: &Model, plane: &Plane, tol: &Tol) -> bool {
    let axis = [DVec3::X, DVec3::Y, DVec3::Z]
        .iter()
        .any(|a| tol.parallel(*a, plane.normal));
    axis || model.faces().any(|(_, f)| {
        f.plane().is_some_and(|q| {
            tol.parallel(q.normal, plane.normal) && tol.is_zero(q.distance(plane.origin))
        })
    })
}

/// The styles the program picks when nobody has: a boundary that lies in
/// a plane a part would meet there (an axis plane, or the plane of a flat
/// face) ends flush in it, wherever that can be built; every other
/// boundary, and the edge of an open face, is Square (brief 49 part 3). A
/// straight boundary lies in many planes, so it takes the plane it shares
/// with a neighbour (the hull's centre line turns a right angle in its
/// plane). Empty if the shell cannot be built at all.
pub fn default_rims(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
) -> RimStyles {
    default_rims_from(model, faces, open, d, RimStyles::new())
}

/// The style of every edge of the boundary each of `picks` is on (a style
/// chosen for a boundary is the style of its whole length).
pub fn styles_for_boundaries(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    picks: &[(EdgeId, RimStyle)],
) -> RimStyles {
    let boundaries = boundaries(model, faces, open);
    let mut styles = RimStyles::new();
    for (edge, style) in picks {
        if let Some(b) = boundaries.iter().find(|b| b.contains(edge)) {
            for e in b {
                styles.insert(*e, *style);
            }
        }
    }
    styles
}

/// [`default_rims`] leaving the styles in `start` (chosen by the user) as
/// they are, and choosing only for the boundaries not in them.
pub fn default_rims_from(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    start: RimStyles,
) -> RimStyles {
    let mut styles = start;
    if slab_with(model, faces, open, d, &styles).is_err() {
        return styles;
    }
    // A lone face has nothing it joins: its rims are left as chosen.
    if body(model, faces).len() < 2 {
        return styles;
    }
    let tol = model.tol();
    let runs = boundaries(model, faces, open);
    // Candidates: each run alone, then each pair that meet end to end.
    let mut groups: Vec<Vec<EdgeId>> = runs.clone();
    let mut at: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for e in runs.iter().flatten() {
        if let Some(edge_data) = model.edge(*e) {
            at.entry(edge_data.a).or_default().push(*e);
            at.entry(edge_data.b).or_default().push(*e);
        }
    }
    for i in 0..runs.len() {
        for j in i + 1..runs.len() {
            if touch(model, &runs[i], &runs[j], &at) {
                let mut both = runs[i].clone();
                both.extend(runs[j].iter().copied());
                groups.push(both);
            }
        }
    }
    // The wall at the edge of an open face stays square: that is a hole
    // in the shell, not a rim to fit to anything.
    let at_open: BTreeSet<EdgeId> = open
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.sides().map(|s| s.edge).collect::<Vec<_>>())
        .collect();
    for group in groups {
        if group
            .iter()
            .any(|e| styles.contains_key(e) || at_open.contains(e))
        {
            continue;
        }
        let Some(plane) = boundary_plane(model, &group, &tol) else {
            continue;
        };
        if !meaningful_plane(model, &plane, &tol) {
            continue;
        }
        let mut trial = styles.clone();
        for e in &group {
            trial.insert(*e, RimStyle::Flush(plane));
        }
        if slab_with(model, faces, open, d, &trial).is_ok() {
            styles = trial;
        }
    }
    styles
}

/// What the shell is made of.
struct Plan {
    body: BTreeSet<FaceId>,
    kept: Vec<FaceId>,
    /// Where each point of a kept face moves to.
    moved: BTreeMap<VertexId, DVec3>,
    /// Corners of four or more faces whose moved copies miss one point
    /// (part 3d): where each edge there ends, round the corner in order. A
    /// small flat face fills the gap between them.
    splits: BTreeMap<VertexId, Vec<(EdgeId, DVec3)>>,
    /// Each kept face's edges moved with it.
    curves: BTreeMap<EdgeId, Curve>,
    /// Edges with a kept face on one side only: the edge, the way that face
    /// walks it, and the face.
    rims: Vec<(EdgeId, VertexId, VertexId, FaceId)>,
}

/// Why `shell` would refuse, if it would.
pub fn refusal(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    thickness: f64,
) -> Option<&'static str> {
    slab(model, faces, open, thickness).err()
}

/// [`refusal`] with rim styles.
pub fn refusal_with(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    thickness: f64,
    styles: &RimStyles,
) -> Option<&'static str> {
    slab_with(model, faces, open, thickness, styles).err()
}

/// Shell or thicken the bodies of `faces` by `thickness`, leaving `open`
/// open, in one step.
pub fn shell(
    model: &mut Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    thickness: f64,
) -> Result<(), &'static str> {
    shell_with(model, faces, open, thickness, &RimStyles::new())
}

/// [`shell`] with rim styles.
pub fn shell_with(
    model: &mut Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    thickness: f64,
    styles: &RimStyles,
) -> Result<(), &'static str> {
    let (s, body) = slab_with(model, faces, open, thickness, styles)?;
    let tol = model.tol();
    crate::thicken::remove_faces(model, &body);
    crate::boolean::join(model, &s, &tol);
    Ok(())
}

/// The new solid on its own, and the faces it replaces.
pub fn slab(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
) -> Result<(Model, BTreeSet<FaceId>), &'static str> {
    slab_with(model, faces, open, d, &RimStyles::new())
}

/// [`slab`] with rim styles (brief 49 part 3). A style other than
/// `Square` needs a joined body with a freeform face.
pub fn slab_with(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    styles: &RimStyles,
) -> Result<(Model, BTreeSet<FaceId>), &'static str> {
    let styled = styles.values().any(|s| *s != RimStyle::Square);
    if styled {
        // A lone freeform face takes a chosen style too, by the same route
        // (the program does not pick one for it: `default_rims_from`).
        return freeform_joined(model, faces, open, d, styles).unwrap_or(Err(RIM_STYLE));
    }
    if let Some(r) = freeform_alone(model, faces, open, d) {
        return r;
    }
    if let Some(r) = freeform_joined(model, faces, open, d, styles) {
        return r;
    }
    let plan = plan(model, faces, open, d)?;
    let tol = model.tol();
    let mut s = Model::new();
    let mut base: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    let mut up: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for (v, q) in &plan.moved {
        base.insert(*v, s.add_vertex(model.vertex(*v)));
        up.insert(*v, s.add_vertex(*q));
    }
    // At a split corner, each edge ends at its own point (or one it
    // shares with the next).
    let mut ends: BTreeMap<(VertexId, EdgeId), VertexId> = BTreeMap::new();
    for (v, fan) in &plan.splits {
        base.insert(*v, s.add_vertex(model.vertex(*v)));
        let mut made: Vec<(DVec3, VertexId)> = Vec::new();
        for (e, q) in fan {
            let id = match made.iter().find(|(p, _)| tol.same_point(*p, *q)) {
                Some((_, id)) => *id,
                None => {
                    let id = s.add_vertex(*q);
                    made.push((*q, id));
                    id
                }
            };
            ends.insert((*v, *e), id);
        }
    }
    let end = |v: VertexId, e: EdgeId| up.get(&v).copied().unwrap_or_else(|| ends[&(v, e)]);
    // Each edge and its moved copy.
    let mut low_e: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut high_e: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut cids = BTreeMap::new();
    for (id, moved) in &plan.curves {
        let e = model.edge(*id).ok_or(NOTHING)?;
        let cid = e
            .curve_id
            .map(|c| *cids.entry(c).or_insert_with(|| s.new_curve_id()));
        let lo = edge(&mut s, base[&e.a], base[&e.b], e.curve.clone(), cid)?;
        let hi_cid = cid.map(|_| s.new_curve_id());
        let hi = edge(&mut s, end(e.a, *id), end(e.b, *id), moved.clone(), hi_cid)?;
        s.set_soft(lo, e.soft, e.smooth);
        s.set_soft(hi, e.soft, e.smooth);
        low_e.insert(*id, lo);
        high_e.insert(*id, hi);
    }
    let copy = |f: &Face,
                vmap: &BTreeMap<VertexId, VertexId>,
                emap: &BTreeMap<EdgeId, EdgeId>,
                surface: Surface| {
        let map = |l: &Vec<VertexId>| l.iter().map(|v| vmap[v]).collect::<Vec<_>>();
        let edges = (0..=f.holes.len())
            .map(|k| f.loop_edges(k).map(|x| emap[&x.edge]).collect())
            .collect();
        let mut c = Face::with_edges(
            map(&f.outer),
            f.holes.iter().map(map).collect(),
            surface,
            edges,
        );
        c.paint = f.paint;
        c
    };
    // The moved copy of a face: at a split corner it goes from where the
    // edge in ends to where the edge out ends, by a short edge.
    let mut shorts: BTreeMap<(VertexId, FaceId), Option<EdgeId>> = BTreeMap::new();
    let mut high_copy = |s: &mut Model, id: FaceId, f: &Face, surface: Surface| {
        let mut loops: Vec<Vec<VertexId>> = Vec::new();
        let mut edges: Vec<Vec<EdgeId>> = Vec::new();
        for k in 0..=f.holes.len() {
            let sides: Vec<_> = f.loop_edges(k).collect();
            let (mut l, mut es) = (Vec::new(), Vec::new());
            for (i, side) in sides.iter().enumerate() {
                let v = side.from;
                if let Some(u) = up.get(&v) {
                    l.push(*u);
                } else {
                    let prev = sides[(i + sides.len() - 1) % sides.len()].edge;
                    let (a, b) = (ends[&(v, prev)], ends[&(v, side.edge)]);
                    l.push(a);
                    let short = (a != b).then(|| s.add_edge(a, b)).flatten();
                    shorts.insert((v, id), short);
                    if let Some(x) = short {
                        es.push(x);
                        l.push(b);
                    }
                }
                es.push(high_e[&side.edge]);
            }
            loops.push(l);
            edges.push(es);
        }
        let outer = loops.remove(0);
        let mut c = Face::with_edges(outer, loops, surface, edges);
        c.paint = f.paint;
        c
    };
    for id in &plan.kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let moved = offset_surface(&f.surface, d, &tol).ok_or(TOO_THICK)?;
        let mut low = copy(f, &base, &low_e, f.surface.clone());
        let mut high = high_copy(&mut s, *id, f, moved);
        // The solid lies between the face and its copy: whichever is
        // behind the other turns round to face out of it.
        if d > 0.0 {
            low.reverse();
        } else {
            high.reverse();
        }
        s.add_face(low);
        s.add_face(high);
    }
    // The small face at each split corner, walked against the faces round
    // it (as a moved copy is, so it turns round with them).
    for (v, fan) in &plan.splits {
        let mut l: Vec<VertexId> = Vec::new();
        let mut es: Vec<EdgeId> = Vec::new();
        for j in (0..fan.len()).rev() {
            let (e_in, _) = fan[j];
            let (e_out, _) = fan[(j + 1) % fan.len()];
            let Some(f) = plan.kept.iter().copied().find(|f| {
                model.face(*f).is_some_and(|x| {
                    let sides: Vec<_> = x.sides().collect();
                    sides.iter().any(|a| a.to == *v && a.edge == e_in)
                        && sides.iter().any(|b| b.from == *v && b.edge == e_out)
                })
            }) else {
                return Err(CORNER);
            };
            if let Some(Some(x)) = shorts.get(&(*v, f)) {
                l.push(ends[&(*v, e_out)]);
                es.push(*x);
            }
        }
        if l.len() >= 3 {
            let pts: Vec<DVec3> = l.iter().map(|x| s.vertex(*x)).collect();
            let n = newell(&pts).normalize_or_zero();
            let mut cap =
                Face::with_edges(l, vec![], Surface::Plane(Plane::new(pts[0], n)), vec![es]);
            if d < 0.0 {
                cap.reverse();
            }
            s.add_face(cap);
        }
    }
    // A line up from each point of a rim.
    let mut rise: BTreeMap<VertexId, EdgeId> = BTreeMap::new();
    for (_, a, b, _) in &plan.rims {
        for v in [a, b] {
            if !rise.contains_key(v) {
                rise.insert(*v, edge(&mut s, base[v], up[v], Curve::Line, None)?);
            }
        }
    }
    for (id, from, to, face) in &plan.rims {
        let e = model.edge(*id).ok_or(NOTHING)?;
        let f = model.face(*face).ok_or(NOTHING)?;
        // The wall faces out of the solid: to the right of the edge, walked
        // the face's way, about the face's normal.
        let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
        let mid = e.curve.point(pa, pb, 0.5);
        let ahead = (e.curve.point(pa, pb, 0.51) - e.curve.point(pa, pb, 0.49))
            * if *from == e.a { 1.0 } else { -1.0 };
        let out = ahead.cross(f.surface.normal_at(mid));
        // Walked against the copy of the face beside it.
        let (p, q) = if d > 0.0 { (*from, *to) } else { (*to, *from) };
        let lo = low_e[id];
        let hi = high_e[id];
        match &e.curve {
            Curve::Line => {
                let pts = [base[&p], base[&q], up[&q], up[&p]];
                if flat_plane(&s, &pts, &tol).is_some() {
                    let plane = Plane::new(s.vertex(pts[0]), loop_normal(&s, &pts).normalize());
                    s.add_face(Face::with_edges(
                        pts.to_vec(),
                        vec![],
                        Surface::Plane(plane),
                        vec![vec![lo, rise[&q], hi, rise[&p]]],
                    ));
                } else {
                    let diag = edge(&mut s, pts[0], pts[2], Curve::Line, None)?;
                    for (l, es) in [
                        ([pts[0], pts[1], pts[2]], [lo, rise[&q], diag]),
                        ([pts[0], pts[2], pts[3]], [diag, hi, rise[&p]]),
                    ] {
                        let plane = Plane::new(s.vertex(l[0]), loop_normal(&s, &l).normalize());
                        s.add_face(Face::with_edges(
                            l.to_vec(),
                            vec![],
                            Surface::Plane(plane),
                            vec![es.to_vec()],
                        ));
                    }
                }
            }
            Curve::Arc {
                centre,
                axis,
                radius,
            } => {
                let Curve::Arc {
                    centre: c2,
                    radius: r2,
                    ..
                } = plan.curves[id]
                else {
                    return Err(FREEFORM);
                };
                let wall = arc_wall(*centre, *axis, *radius, c2, r2, mid, out, &tol)?;
                s.add_face(Face::with_edges(
                    vec![base[&p], base[&q], up[&q], up[&p]],
                    vec![],
                    wall,
                    vec![vec![lo, rise[&q], hi, rise[&p]]],
                ));
            }
            Curve::Spline(_) => return Err(FREEFORM),
        }
    }
    // A line up between walls that meet smoothly is soft.
    let ef = s.edge_faces();
    for e in rise.values() {
        let Some([f, g]) = ef.get(e).map(|x| x.as_slice()) else {
            continue;
        };
        let at = {
            let x = s.edge(*e).ok_or(TOO_THICK)?;
            (s.vertex(x.a) + s.vertex(x.b)) * 0.5
        };
        let (nf, ng) = (
            s.face(*f).map(|x| x.surface.normal_at(at)),
            s.face(*g).map(|x| x.surface.normal_at(at)),
        );
        if let (Some(a), Some(b)) = (nf, ng)
            && tol.parallel(a, b)
            && a.dot(b) > 0.0
        {
            s.set_soft(*e, true, true);
        }
    }
    if !crate::boolean::is_solid(&s) {
        return Err(TOO_THICK);
    }
    Ok((s, plan.body))
}

/// A freeform face on its own: the slab Push/Pull makes of it.
fn freeform_alone(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
) -> Option<Result<(Model, BTreeSet<FaceId>), &'static str>> {
    let body = body(model, faces);
    let [f] = body.iter().copied().collect::<Vec<_>>()[..] else {
        return None;
    };
    model.face(f)?.surface.patch()?;
    if open.contains(&f) {
        return Some(Err(ALL_OPEN));
    }
    if model.tol().is_zero(d) {
        return Some(Err(ZERO));
    }
    Some(crate::thicken::slab(model, f, d).map(|s| (s.model, body)))
}

/// A body of two or more faces, at least one of them freeform, joined
/// smoothly or at a crease (brief 49 parts 1 and 2): each face is offset
/// on its own (a freeform face as `thicken::offset_surface` fits it, a
/// flat or other analytic face exactly); where two kept faces share an
/// edge smoothly, their offsets already meet there; where they meet at an
/// angle, the mitre is found (`crease_curve`). A rim (an edge with one
/// kept face, at a free boundary or an open face) gets a ruled wall, cut
/// square across. A body with no freeform face at all is left to `plan`,
/// which already does flat and curved analytic faces exactly.
fn freeform_joined(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    styles: &RimStyles,
) -> Option<Result<(Model, BTreeSet<FaceId>), &'static str>> {
    let body = body(model, faces);
    let styled = styles.values().any(|s| *s != RimStyle::Square);
    if body.len() < 2 && !(styled && body.len() == 1) {
        return None;
    }
    if body.iter().any(|f| model.face(*f).is_none()) {
        return None;
    }
    if !body
        .iter()
        .any(|f| model.face(*f).is_some_and(|x| x.surface.patch().is_some()))
    {
        return None;
    }
    // A flush rim that leans away needs the offset surface carried on past
    // its own edge: tried as it is first, then with it grown a little more
    // each time (only when the one before said it was out of reach).
    let mut result = Err(FLUSH_REACH);
    let mut wanted = false;
    for reach in REACHES {
        result = freeform_slab(model, &body, open, d, styles, reach);
        wanted |= matches!(result, Err(FLUSH_REACH));
        if !styled || !matches!(result, Err(FLUSH_REACH)) {
            break;
        }
    }
    // A surface grown as far as it will go that then cannot be fitted is
    // still a plane out of reach, not a thickness gone wrong.
    if wanted && matches!(result, Err(TOO_THICK)) {
        result = Err(FLUSH_REACH);
    }
    Some(result)
}

/// How far each face's offset surface is carried on past the box of its
/// own outline, as a fraction of that box, in the tries a flush rim makes
/// to reach its plane.
const REACHES: [f64; 5] = [0.0, 0.02, 0.05, 0.1, 0.5];

/// Whether the two faces at an interior edge meet smoothly: their normals
/// agree along the edge, sampled at several points (not just the
/// middle), so each face's offset point there is the same.
fn smooth_join(model: &Model, e: EdgeId, fa: FaceId, fb: FaceId, tol: &Tol) -> bool {
    let (Some(edge), Some(a), Some(b)) = (model.edge(e), model.face(fa), model.face(fb)) else {
        return false;
    };
    let (pa, pb) = (model.vertex(edge.a), model.vertex(edge.b));
    (1..8).all(|i| {
        let p = edge.curve.point(pa, pb, i as f64 / 8.0);
        let (na, nb) = (a.surface.normal_at(p), b.surface.normal_at(p));
        tol.parallel(na, nb) && na.dot(nb) > 0.0
    })
}

/// The slab of a joined body with at least one freeform face: each kept
/// face offset on its own, sharing an offset edge with its neighbour
/// (smoothly, or mitred at a crease), walled at its rims.
fn freeform_slab(
    model: &Model,
    body: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    styles: &RimStyles,
    reach: f64,
) -> Result<(Model, BTreeSet<FaceId>), &'static str> {
    use prism_geom::{Patch, Spline};
    let tol = model.tol();
    if !d.is_finite() || tol.is_zero(d) {
        return Err(ZERO);
    }
    let kept: Vec<FaceId> = body.iter().copied().filter(|f| !open.contains(f)).collect();
    if kept.is_empty() {
        return Err(ALL_OPEN);
    }
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        if f.surface.patch().is_some()
            && crate::crossing::Laid::new(model, *id).is_none_or(|l| l.wraps())
        {
            return Err(crate::thicken::ROUND);
        }
    }
    // How the body's faces use each edge, as the analytic plan() does.
    let mut uses: BTreeMap<EdgeId, Vec<(bool, VertexId, VertexId, FaceId)>> = BTreeMap::new();
    for id in body {
        let f = model.face(*id).ok_or(NOTHING)?;
        let keep = !open.contains(id);
        for side in f.sides() {
            uses.entry(side.edge)
                .or_default()
                .push((keep, side.from, side.to, *id));
        }
    }
    let mut rims: Vec<(EdgeId, VertexId, VertexId, FaceId)> = Vec::new();
    let mut driver: BTreeMap<EdgeId, FaceId> = BTreeMap::new();
    let mut creases: Vec<(EdgeId, FaceId, FaceId)> = Vec::new();
    for (e, u) in &uses {
        if u.len() > 2 {
            return Err(BRANCHES);
        }
        let kept_uses: Vec<_> = u.iter().filter(|x| x.0).collect();
        match kept_uses.as_slice() {
            [one] => {
                rims.push((*e, one.1, one.2, one.3));
                driver.insert(*e, one.3);
            }
            [x, y] => {
                if x.1 == y.1 {
                    return Err(MIXED);
                }
                if smooth_join(model, *e, x.3, y.3, &tol) {
                    driver.insert(*e, x.3);
                } else {
                    creases.push((*e, x.3, y.3));
                }
            }
            _ => {}
        }
    }
    // Rims that end flush in a plane (part 3): the plane must contain the
    // edge, and the face must not lie along it.
    let mut flush: BTreeMap<EdgeId, Plane> = BTreeMap::new();
    let mut flush_at: BTreeMap<VertexId, Vec<Plane>> = BTreeMap::new();
    for (e, from, to, face) in &rims {
        let Some(RimStyle::Flush(plane)) = styles.get(e).copied() else {
            continue;
        };
        let edge_data = model.edge(*e).ok_or(NOTHING)?;
        let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
        let _ = (from, to);
        let f = model.face(*face).ok_or(NOTHING)?;
        for i in 0..=8 {
            let p = edge_data.curve.point(pa, pb, f64::from(i) / 8.0);
            if !tol.is_zero(plane.distance(p)) {
                return Err(FLUSH_OFF);
            }
            if tol.parallel(f.surface.normal_at(p), plane.normal) {
                return Err(FLUSH_PARALLEL);
            }
        }
        flush.insert(*e, plane);
        for v in [edge_data.a, edge_data.b] {
            let planes = flush_at.entry(v).or_default();
            if !planes.contains(&plane) {
                planes.push(plane);
            }
        }
    }
    let mut s = Model::new();
    let eps = tol.dist * 0.1;
    // Each kept face's own offset surface: a freeform one fitted (its
    // own edge still bounds it), a flat or other analytic one exact and
    // already unbounded.
    let mut offsets: BTreeMap<FaceId, Surface> = BTreeMap::new();
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let surface = if let Some(patch) = f.surface.patch() {
            let laid = crate::crossing::Laid::new(model, *id).ok_or(TOO_THICK)?;
            let top = if reach > 0.0 {
                // Grown only on the side of its own box a flush rim of it
                // lies along (the offset is carried on past that edge to
                // meet the plane), or all round where that is not clear.
                let (mut lo, mut hi) = laid.polys[0]
                    .iter()
                    .fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(a, b), p| {
                        (a.min(*p), b.max(*p))
                    });
                let size = hi - lo;
                let (box_lo, box_hi) = (lo, hi);
                let mut grown = false;
                for e in flush.keys().filter(|e| driver.get(*e) == Some(id)) {
                    let edge_data = model.edge(*e).ok_or(NOTHING)?;
                    let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
                    let uv: Vec<DVec2> = (0..=8)
                        .map(|i| {
                            let (u, v) =
                                patch.param(edge_data.curve.point(pa, pb, f64::from(i) / 8.0));
                            DVec2::new(u, v)
                        })
                        .collect();
                    let along =
                        |k: usize, at: f64| uv.iter().all(|q| (q[k] - at).abs() <= size[k] * 1e-6);
                    let mut side = false;
                    for k in 0..2 {
                        if along(k, box_lo[k]) {
                            lo[k] = box_lo[k] - size[k] * reach;
                            side = true;
                        } else if along(k, box_hi[k]) {
                            hi[k] = box_hi[k] + size[k] * reach;
                            side = true;
                        }
                    }
                    if !side {
                        lo = box_lo - size * reach;
                        hi = box_hi + size * reach;
                    }
                    grown = true;
                }
                if grown {
                    crate::thicken::offset_surface(patch, &[lo, hi], d, eps)
                } else {
                    crate::thicken::offset_surface(patch, &laid.polys[0], d, eps)
                }
            } else {
                crate::thicken::offset_surface(patch, &laid.polys[0], d, eps)
            }
            .ok_or(TOO_THICK)?;
            Surface::Nurbs(Patch {
                flipped: patch.flipped,
                ..Patch::whole(top)
            })
        } else {
            offset_surface(&f.surface, d, &tol).ok_or(TOO_THICK)?
        };
        offsets.insert(*id, surface);
    }
    // Where each point a kept face reaches moves to: the exact corner of
    // every kept face there (`plan`'s own move_point, which already works
    // on any surface, flat or freeform).
    let mut at_vertex: BTreeMap<VertexId, Vec<FaceId>> = BTreeMap::new();
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        for v in f.loops().flatten() {
            let fs = at_vertex.entry(*v).or_default();
            if !fs.contains(id) {
                fs.push(*id);
            }
        }
    }
    let mut base: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    let mut up: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    let mut moved: BTreeMap<VertexId, DVec3> = BTreeMap::new();
    let on_rim: BTreeSet<VertexId> = rims.iter().flat_map(|r| [r.1, r.2]).collect();
    // Corners away from any rim where the faces' moved copies miss one
    // point (four or more faces): placed below, once each crease is known
    // to be inside or outside, by the faces between outside creases.
    let mut torn: BTreeSet<VertexId> = BTreeSet::new();
    for (v, fs) in &at_vertex {
        let p = model.vertex(*v);
        base.insert(*v, s.add_vertex(p));
        let q = match flush_at.get(v) {
            Some(planes) => {
                // A square rim of the same face meeting here keeps its
                // wall square: the corner stays in the plane through
                // the edge and the face's normal.
                let mut walls: Vec<DVec3> = Vec::new();
                let mut square_rims: Vec<(EdgeId, FaceId)> = Vec::new();
                for (e, _, _, face) in &rims {
                    let Some(edge_data) = model.edge(*e) else {
                        continue;
                    };
                    if flush.contains_key(e) || (edge_data.a != *v && edge_data.b != *v) {
                        continue;
                    }
                    let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
                    let near = edge_data.curve.point(
                        pa,
                        pb,
                        if edge_data.a == *v { 1e-3 } else { 1.0 - 1e-3 },
                    );
                    let n = model.face(*face).ok_or(NOTHING)?.surface.normal_at(p);
                    let m = (near - p).cross(n).normalize_or_zero();
                    if m != DVec3::ZERO {
                        walls.push(m);
                        square_rims.push((*e, *face));
                    }
                }
                // One face, one flush plane and one square rim of that
                // face end here (a chain's end): the corner is where the
                // square rim's offset edge meets the plane, exactly.
                if let ([f], [plane], [(e, _)]) =
                    (fs.as_slice(), planes.as_slice(), square_rims.as_slice())
                {
                    flush_end(model, *f, *e, *v, &offsets[f], plane, d, &tol)?
                } else {
                    let x0 = move_point_flush(model, fs, p, d, planes, &walls, &tol)?;
                    refine_flush_corner(&offsets, fs, planes, x0, &tol)
                }
            }
            None => match move_point(model, fs, p, d, &tol) {
                Ok(q) => polish_corner(&offsets, fs, q, &tol),
                Err(CORNER) if !on_rim.contains(v) => {
                    torn.insert(*v);
                    continue;
                }
                Err(e) => return Err(e),
            },
        };
        moved.insert(*v, q);
        up.insert(*v, s.add_vertex(q));
    }
    // Too sharp a fold has no mitre (close to doubling back on itself):
    // checked for every crease up front, at the same points `smooth_join`
    // samples, so the reason is specific. The rest are classified inside
    // or outside (part 2b): an outside corner's mitre would reach past
    // one face's own edge, which a NURBS surface cannot extend to meet.
    let mut outside: BTreeMap<EdgeId, bool> = BTreeMap::new();
    for (e, fa, fb) in &creases {
        let edge_data = model.edge(*e).ok_or(NOTHING)?;
        let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
        let along = |t: f64| edge_data.curve.point(pa, pb, t);
        let (face_a, face_b) = (
            model.face(*fa).ok_or(NOTHING)?,
            model.face(*fb).ok_or(NOTHING)?,
        );
        let sharp = (0..=8).any(|i| {
            let p = along(i as f64 / 8.0);
            let ns = unique_normals(
                &[face_a.surface.normal_at(p), face_b.surface.normal_at(p)],
                &tol,
            );
            corner(&ns, d, &tol).is_none()
        });
        if sharp {
            return Err(SHARP);
        }
        let (off_a, off_b) = (&offsets[fa], &offsets[fb]);
        let raw_meet = |p: DVec3| {
            let ns = unique_normals(
                &[face_a.surface.normal_at(p), face_b.surface.normal_at(p)],
                &tol,
            );
            let guess = corner(&ns, d, &tol).map_or(p, |x| p + x);
            meet(off_a, off_b, guess, &tol)
        };
        let extends = (0..=8).any(|i| {
            let x = raw_meet(along(i as f64 / 8.0));
            !truly_on(off_a, x, &tol) || !truly_on(off_b, x, &tol)
        });
        outside.insert(*e, extends);
    }
    // At an outside corner's end, a face whose own offset the shared
    // corner misses splits away there, to the plain offset of that end
    // along its own normal (there is nothing else at a corner like
    // that to correct it towards); one whose own offset the shared
    // corner is truly still on keeps it (part 2c: a crease that samples
    // outside along its length can still land truly on both its own
    // faces right at this particular end, most often a crease fading
    // towards tangent there, as `unique_normals` already allows along
    // its length). The ordinary two-face corner splits both, both
    // always missing the one point there is only room to share between
    // them. More than one missing it among three or more faces is parted
    // into runs (below); on a rim that is still refused by the same name
    // as a true multi-face corner (the rim walls' risers there would have
    // to close the gap as well, which is not built yet).
    let mut split: BTreeMap<(VertexId, FaceId), VertexId> = BTreeMap::new();
    let mut split_pt: BTreeMap<(VertexId, FaceId), DVec3> = BTreeMap::new();
    let mut checked: BTreeSet<VertexId> = torn.clone();
    // Corners parted into runs, with the faces there that miss the shared
    // corner (at a torn one, every face: there is none).
    let mut parted: Vec<(VertexId, Vec<FaceId>)> =
        torn.iter().map(|v| (*v, at_vertex[v].clone())).collect();
    for (e, _fa, _fb) in &creases {
        if !outside[e] {
            continue;
        }
        let edge_data = model.edge(*e).ok_or(NOTHING)?;
        for v in [edge_data.a, edge_data.b] {
            if !checked.insert(v) {
                continue;
            }
            let p = model.vertex(v);
            let bad: Vec<FaceId> = at_vertex[&v]
                .iter()
                .copied()
                .filter(|f| !truly_on(&offsets[f], moved[&v], &tol))
                .collect();
            // None of them misses the shared corner: nothing to split,
            // even though this crease alone looked outside along the
            // way. One does: it splits on its own. Both of an ordinary
            // two-face corner do: the usual case, both split. More than
            // one among three or more faces: parted away from a rim, and
            // refused on one, as above.
            match bad.len() {
                0 => continue,
                1 => {}
                n if n == at_vertex[&v].len() && n == 2 => {}
                _ if !on_rim.contains(&v) => {
                    parted.push((v, bad));
                    continue;
                }
                _ => return Err(CORNER),
            }
            for f in bad {
                let q = p + model.face(f).ok_or(NOTHING)?.surface.normal_at(p) * d;
                split_pt.insert((v, f), q);
                split.insert((v, f), s.add_vertex(q));
            }
        }
    }
    // A corner away from any rim with more than one face missing the
    // shared corner, or with no shared corner at all (torn): the faces
    // round it between two outside creases (a run) share one point, the
    // corner of just those faces; a run with any face that misses the
    // shared corner moves to its own one, as one split. This is how the
    // 2D Offset tool lets its lines cross rather than refuse: the gap
    // left between the runs is closed by a flat face (below), however odd
    // its shape, for the user to tidy. Refused only where there is no
    // outside crease to part the faces at, or a run has no corner itself.
    for (v, bad) in &parted {
        let p = model.vertex(*v);
        let round = ring(model, *v, &at_vertex[v]).ok_or(CORNER)?;
        let cut = |i: usize| outside.get(&round[i].1) == Some(&true);
        let Some(first) = (0..round.len()).find(|i| cut(*i)) else {
            return Err(CORNER);
        };
        let mut runs: Vec<Vec<FaceId>> = vec![vec![]];
        for k in 1..=round.len() {
            let i = (first + k) % round.len();
            runs.last_mut().ok_or(CORNER)?.push(round[i].0);
            if cut(i) && k < round.len() {
                runs.push(vec![]);
            }
        }
        // The points made here so far (None: the shared corner): a run
        // landing on one of them (faces fading to tangent at the corner)
        // shares it rather than leave two points in the same place.
        let mut made: Vec<(DVec3, Option<VertexId>)> =
            moved.get(v).map(|q| (*q, None)).into_iter().collect();
        for run in runs.iter().filter(|r| r.iter().any(|f| bad.contains(f))) {
            let q = polish_corner(&offsets, run, move_point(model, run, p, d, &tol)?, &tol);
            let (q, id) = match made.iter().find(|(x, _)| tol.same_point(*x, q)) {
                Some((_, None)) => continue,
                Some((x, Some(id))) => (*x, *id),
                None => {
                    let id = s.add_vertex(q);
                    // A torn corner's first run stands for the shared corner.
                    if !moved.contains_key(v) {
                        moved.insert(*v, q);
                        up.insert(*v, id);
                        made.push((q, None));
                        continue;
                    }
                    made.push((q, Some(id)));
                    (q, id)
                }
            };
            for f in run {
                split_pt.insert((*v, *f), q);
                split.insert((*v, *f), id);
            }
        }
    }
    // Where some faces split away from a shared corner, it is polished
    // onto the true offsets of just the faces that keep it (it could not
    // be onto all of them, so it was left as the flat corner).
    for (v, q) in moved.iter_mut() {
        let keep: Vec<FaceId> = at_vertex[v]
            .iter()
            .copied()
            .filter(|f| !split.contains_key(&(*v, *f)))
            .collect();
        if keep.len() < at_vertex[v].len() && !keep.is_empty() && !flush_at.contains_key(v) {
            *q = polish_corner(&offsets, &keep, *q, &tol);
            s.set_vertex(up[v], *q);
        }
    }
    let vertex_for = |v: VertexId, f: FaceId| split.get(&(v, f)).copied().unwrap_or(up[&v]);
    let point_for = |v: VertexId, f: FaceId| split_pt.get(&(v, f)).copied().unwrap_or(moved[&v]);
    // Each original edge's offset is fitted once, shared by every face
    // that uses it: a rim or a smooth join by whichever kept face drives
    // it, a crease by solving where its two faces' offsets truly meet
    // (or, at an outside corner, by each face on its own: part 2b).
    let mut bottom_of: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut top_of: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut fitted: BTreeMap<EdgeId, (Nurbs, Nurbs)> = BTreeMap::new();
    // At an outside corner (part 2b), the two faces either side of a
    // crease keep their own top edge rather than share a mitre: looked up
    // by the edge and the face using it, falling back to `top_of` (the
    // usual, shared top edge) everywhere else.
    let mut crease_top: BTreeMap<(EdgeId, FaceId), EdgeId> = BTreeMap::new();
    // An outside corner's own bridge, finished once the rims below have
    // given it the risers it closes against.
    let mut bridges: Vec<Bridge> = Vec::new();
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        for side in f.sides() {
            if bottom_of.contains_key(&side.edge) || driver.get(&side.edge) != Some(id) {
                continue;
            }
            let e = model.edge(side.edge).ok_or(NOTHING)?;
            let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
            let along = |t: f64| e.curve.point(pa, pb, t);
            let naive = |p: DVec3| p + f.surface.normal_at(p) * d;
            // Corrected to meet the vertex's true corner at each end (it
            // may be mitred with a face this edge does not touch, or be
            // this face's own corner of an outside crease); blended
            // smoothly across the edge, zero where there is no mitre.
            let (corr_a, corr_b) = (
                point_for(e.a, *id) - naive(pa),
                point_for(e.b, *id) - naive(pb),
            );
            let span = (pb - pa).length_squared().max(1e-300);
            // A flush rim (part 3): where the face's own offset meets the
            // plane (it must meet it within its own edge: a surface
            // cannot be extended to reach it).
            let plane = flush.get(&side.edge).map(|p| Surface::Plane(*p));
            let missed = std::cell::Cell::new(false);
            let off = |p: DVec3| {
                let t = ((p - pa).dot(pb - pa) / span).clamp(0.0, 1.0);
                let blended = naive(p) + corr_a * (1.0 - t) + corr_b * t;
                if let Some(plane) = &plane {
                    let x = meet_to(&offsets[id], plane, blended, &tol, eps * 1e-3);
                    if !truly_on(&offsets[id], x, &tol) || !truly_on(plane, x, &tol) {
                        missed.set(true);
                    }
                    return x;
                }
                // The straight-line blend of a correction can stray off
                // a curved face's own offset even where each of its own
                // ends is truly on it; snapped back onto it (never past
                // its own edge, since a mitred end is already there).
                offsets[id].project(blended)
            };
            let breaks = match f.surface.patch() {
                Some(patch) => crate::thicken::kinks(&e.curve, &along, patch),
                None => vec![0.0, 1.0],
            };
            let fit = crate::thicken::edge_pair(&along, &off, &breaks, eps);
            if missed.get() {
                return Err(FLUSH_REACH);
            }
            let (low, high) = fit.ok_or(TOO_THICK)?;
            let b = edge(&mut s, base[&e.a], base[&e.b], e.curve.clone(), None)?;
            let t = edge(
                &mut s,
                vertex_for(e.a, *id),
                vertex_for(e.b, *id),
                Curve::Spline(Spline::whole(high.clone())),
                None,
            )?;
            bottom_of.insert(side.edge, b);
            top_of.insert(side.edge, t);
            fitted.insert(side.edge, (low, high));
        }
    }
    for (e, fa, fb) in &creases {
        let edge_data = model.edge(*e).ok_or(NOTHING)?;
        let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
        let along = |t: f64| edge_data.curve.point(pa, pb, t);
        let (face_a, face_b) = (
            model.face(*fa).ok_or(NOTHING)?,
            model.face(*fb).ok_or(NOTHING)?,
        );
        let (off_a, off_b) = (&offsets[fa], &offsets[fb]);
        let span = (pb - pa).length_squared().max(1e-300);
        let mut breaks = match face_a.surface.patch() {
            Some(patch) => crate::thicken::kinks(&edge_data.curve, &along, patch),
            None => vec![0.0, 1.0],
        };
        if let Some(patch) = face_b.surface.patch() {
            breaks.extend(crate::thicken::kinks(&edge_data.curve, &along, patch));
            breaks.sort_by(f64::total_cmp);
            breaks.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
        }
        if outside[e] {
            // An outside corner (part 2b): each face keeps its own
            // straight offset edge instead of a mitre that would reach
            // past one of them, bridged by a small face between the two
            // once the rims below give it the risers it closes against.
            let corrected = |surface: &Surface, offset: &Surface, face_id: FaceId, p: DVec3| {
                let naive = |q: DVec3| q + surface.normal_at(q) * d;
                let t = ((p - pa).dot(pb - pa) / span).clamp(0.0, 1.0);
                let (ea, eb) = (
                    point_for(edge_data.a, face_id),
                    point_for(edge_data.b, face_id),
                );
                let blended = naive(p) + (ea - naive(pa)) * (1.0 - t) + (eb - naive(pb)) * t;
                // The straight-line blend of a correction can stray off a
                // curved face's own offset even where each of its own
                // ends (checked by `truly_on`) does not; snapped back
                // onto it (never past its own edge, since a split end is
                // already there, not beyond it).
                offset.project(blended)
            };
            let fn_a = |t: f64| corrected(&face_a.surface, off_a, *fa, along(t));
            let fn_b = |t: f64| corrected(&face_b.surface, off_b, *fb, along(t));
            let (high_a, high_b) =
                crate::thicken::twin_fit(&fn_a, &fn_b, &breaks, eps).ok_or(TOO_THICK)?;
            let b = edge(
                &mut s,
                base[&edge_data.a],
                base[&edge_data.b],
                edge_data.curve.clone(),
                None,
            )?;
            let ta = edge(
                &mut s,
                vertex_for(edge_data.a, *fa),
                vertex_for(edge_data.b, *fa),
                Curve::Spline(Spline::whole(high_a.clone())),
                None,
            )?;
            let tb = edge(
                &mut s,
                vertex_for(edge_data.a, *fb),
                vertex_for(edge_data.b, *fb),
                Curve::Spline(Spline::whole(high_b.clone())),
                None,
            )?;
            bottom_of.insert(*e, b);
            crease_top.insert((*e, *fa), ta);
            crease_top.insert((*e, *fb), tb);
            bridges.push(Bridge {
                crease: *e,
                fa: *fa,
                fb: *fb,
                ea: edge_data.a,
                eb: edge_data.b,
                ta,
                tb,
                high_a,
                high_b,
            });
            continue;
        }
        // Corrected to meet the vertex's true corner at each end (it may
        // involve a third face this crease does not touch); blended
        // smoothly across the edge, zero where there is no such face.
        let raw_meet = |p: DVec3| {
            let ns = unique_normals(
                &[face_a.surface.normal_at(p), face_b.surface.normal_at(p)],
                &tol,
            );
            let guess = corner(&ns, d, &tol).map_or(p, |x| p + x);
            meet_to(off_a, off_b, guess, &tol, eps * 1e-3)
        };
        // The mitre ends where both its faces are: the shared corner, or
        // their run's own at a torn corner (both split there together).
        let end = |v: VertexId| {
            if split.contains_key(&(v, *fa)) && vertex_for(v, *fa) == vertex_for(v, *fb) {
                (vertex_for(v, *fa), point_for(v, *fa))
            } else {
                (up[&v], moved[&v])
            }
        };
        let (end_a, end_b) = (end(edge_data.a), end(edge_data.b));
        let (corr_a, corr_b) = (end_a.1 - raw_meet(pa), end_b.1 - raw_meet(pb));
        let meet_at = |t: f64| -> DVec3 {
            let p = along(t);
            let bt = ((p - pa).dot(pb - pa) / span).clamp(0.0, 1.0);
            let r = raw_meet(p);
            // The correction slides the point along the mitre; it is put
            // back on both offsets (a straight-line blend of it strays off
            // a curved mitre between its ends).
            meet_to(
                off_a,
                off_b,
                r + corr_a * (1.0 - bt) + corr_b * bt,
                &tol,
                eps * 1e-3,
            )
        };
        let curve = crease_curve(&meet_at, off_a, off_b, &breaks, eps).ok_or(TOO_THICK)?;
        let b = edge(
            &mut s,
            base[&edge_data.a],
            base[&edge_data.b],
            edge_data.curve.clone(),
            None,
        )?;
        let t = edge(
            &mut s,
            end_a.0,
            end_b.0,
            Curve::Spline(Spline::whole(curve)),
            None,
        )?;
        bottom_of.insert(*e, b);
        top_of.insert(*e, t);
    }
    let copy = |vertex_for: &dyn Fn(VertexId) -> VertexId,
                edge_for: &dyn Fn(EdgeId) -> Option<EdgeId>,
                f: &Face,
                surface: Surface|
     -> Result<Face, &'static str> {
        let mut vs: Vec<Vec<VertexId>> = f
            .loops()
            .map(|l| l.iter().map(|v| vertex_for(*v)).collect())
            .collect();
        let mut es: Vec<Vec<EdgeId>> = Vec::with_capacity(vs.len());
        for (k, vrow) in vs.iter().enumerate() {
            let mut row = Vec::with_capacity(vrow.len());
            for x in f.loop_edges(k) {
                row.push(edge_for(x.edge).ok_or(NOTHING)?);
            }
            es.push(row);
        }
        let outer = vs.remove(0);
        let mut c = Face::with_edges(outer, vs, surface, es);
        c.paint = f.paint;
        Ok(c)
    };
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let mut low = copy(
            &|v| base[&v],
            &|e| bottom_of.get(&e).copied(),
            f,
            f.surface.clone(),
        )?;
        let mut high = copy(
            &|v| vertex_for(v, *id),
            &|e| {
                crease_top
                    .get(&(e, *id))
                    .or_else(|| top_of.get(&e))
                    .copied()
            },
            f,
            offsets[id].clone(),
        )?;
        if d > 0.0 {
            low.reverse();
        } else {
            high.reverse();
        }
        s.add_face(low);
        s.add_face(high);
    }
    // A line up from each point of a rim, and its wall: a ruled surface
    // between the edge and its offset, cut square across. At a rim
    // whose end an outside corner split (part 2b), the line goes up to
    // that face's own corner instead of the shared one, so two rims that
    // share a split vertex but not a face get their own, separate lines.
    let mut rise: BTreeMap<(VertexId, Option<FaceId>), EdgeId> = BTreeMap::new();
    for (_, a, b, face) in &rims {
        for v in [a, b] {
            let key = (*v, split.contains_key(&(*v, *face)).then_some(*face));
            if let std::collections::btree_map::Entry::Vacant(e) = rise.entry(key) {
                e.insert(edge(
                    &mut s,
                    base[v],
                    vertex_for(*v, *face),
                    Curve::Line,
                    None,
                )?);
            }
        }
    }
    for (e_id, from, to, face) in &rims {
        let e = model.edge(*e_id).ok_or(NOTHING)?;
        let (low, high) = fitted.get(e_id).ok_or(TOO_THICK)?;
        let (b, t) = (bottom_of[e_id], top_of[e_id]);
        let flip = (*from == e.a) != (d > 0.0);
        let ruled = Patch {
            flipped: flip,
            ..Patch::whole(crate::thicken::ruled(low, high).ok_or(TOO_THICK)?)
        };
        let (va, vb) = (base[&e.a], base[&e.b]);
        let (ua, ub) = (vertex_for(e.a, *face), vertex_for(e.b, *face));
        let key = |v: VertexId| (v, split.contains_key(&(v, *face)).then_some(*face));
        let (ra, rb) = (rise[&key(e.a)], rise[&key(e.b)]);
        let (loop_v, loop_e) = if flip {
            (vec![va, ua, ub, vb], vec![ra, t, rb, b])
        } else {
            (vec![va, vb, ub, ua], vec![b, rb, t, ra])
        };
        let _ = to;
        s.add_face(Face::with_edges(
            loop_v,
            vec![],
            crate::thicken::flat(&ruled, eps).unwrap_or(Surface::Nurbs(ruled)),
            vec![loop_e],
        ));
    }
    // Each outside corner's own bridge (part 2b): a ruled face between
    // the two faces' top edges, closed at either end that splits by
    // `close_bridge_end` (a triangle where both of this bridge's own
    // faces split there, part 2c's plain shared edge where only one
    // does, nothing where neither does: a third face there kept the one
    // shared corner for both). Built before the surface's own `flipped`
    // is chosen, so that choice can match the loop's own winding
    // directly (part 2c: pinched to a triangle at an end nothing
    // splits, the loop is shorter, but still the one true winding to
    // match, found the same way regardless).
    let mut asym_cap: BTreeMap<(VertexId, VertexId), EdgeId> = BTreeMap::new();
    for br in bridges {
        // Each end's own cap faces past that end, along the crease: the
        // same direction its two rim walls' own end caps already face
        // (where it is a triangle against risers, which must match).
        let tangent = model.vertex(br.eb) - model.vertex(br.ea);
        let mut loop_v = vec![vertex_for(br.ea, br.fa), vertex_for(br.eb, br.fa)];
        let mut loop_e = vec![br.ta];
        if let Some((cap, v)) = close_bridge_end(
            &mut s,
            br.eb,
            br.fa,
            br.fb,
            tangent,
            &base,
            &up,
            &rise,
            &split,
            &mut asym_cap,
        )? {
            loop_e.push(cap);
            loop_v.push(v);
        }
        loop_e.push(br.tb);
        if let Some((cap, v)) = close_bridge_end(
            &mut s,
            br.ea,
            br.fa,
            br.fb,
            -tangent,
            &base,
            &up,
            &rise,
            &split,
            &mut asym_cap,
        )? {
            loop_e.push(cap);
            loop_v.push(v);
        }
        // Walked against the moved copy of `fa` beside it (which goes
        // along `ta` the way `fa` goes along the crease, turned round
        // with it where the copy is), so each edge the bridge shares is
        // walked once each way.
        let fa_ahead = model
            .face(br.fa)
            .ok_or(NOTHING)?
            .sides()
            .any(|x| x.edge == br.crease && x.from == br.ea);
        if fa_ahead == (d > 0.0) {
            loop_v[1..].reverse();
            loop_e.reverse();
        }
        let raw = Patch::whole(crate::thicken::ruled(&br.high_a, &br.high_b).ok_or(TOO_THICK)?);
        // Wound by the points along its edges, not just its corners: a
        // bridge pinched at both ends has only two.
        let drawn = Face::with_edges(
            loop_v.clone(),
            vec![],
            Surface::Nurbs(raw.clone()),
            vec![loop_e.clone()],
        );
        let flipped = raw.normal(0.5, 0.5).dot(newell(&s.loop_points(&drawn, 0))) < 0.0;
        let bridge = Patch { flipped, ..raw };
        s.add_face(Face::with_edges(
            loop_v,
            vec![],
            crate::thicken::flat(&bridge, eps).unwrap_or(Surface::Nurbs(bridge)),
            vec![loop_e],
        ));
    }
    // A split corner away from any rim whose faces' points, in order
    // round it, are three or more apart: the bridges' ends there leave a
    // gap, closed by a flat face facing the way the faces move (the high
    // side of the slab). Two points need nothing more: the one edge
    // between them is already both bridges' end.
    let cornered: BTreeSet<VertexId> = split.keys().map(|k| k.0).collect();
    for v in cornered.iter().filter(|v| !on_rim.contains(v)) {
        let round = ring(model, *v, &at_vertex[v]).ok_or(CORNER)?;
        let gap =
            gap_loop(round.iter().map(|(f, _)| vertex_for(*v, *f)).collect()).ok_or(CORNER)?;
        if gap.len() < 3 {
            continue;
        }
        let p = model.vertex(*v);
        let mut out = DVec3::ZERO;
        for f in &at_vertex[v] {
            out += model.face(*f).ok_or(NOTHING)?.surface.normal_at(p);
        }
        close_gap(&mut s, gap, out * d.signum(), &asym_cap, &tol)?;
    }
    if !crate::boolean::is_solid(&s) {
        return Err(TOO_THICK);
    }
    Ok((s, body.clone()))
}

/// An outside corner's own bridge (part 2b), kept until the rims below
/// have given it the risers its ends close against.
struct Bridge {
    crease: EdgeId,
    fa: FaceId,
    fb: FaceId,
    ea: VertexId,
    eb: VertexId,
    ta: EdgeId,
    tb: EdgeId,
    high_a: Nurbs,
    high_b: Nurbs,
}

/// The small flat face closing the gap, at one end of an outside
/// corner's bridge, between the base vertex and the two faces' own
/// corners there (`pa_v`, `pb_v`): against the risers `ra`, `rb` either
/// side of it (each a face's own where it split there, or the one
/// shared riser of whichever there did not, part 2c), facing `out_ref`
/// (past that end, along the crease). Returns the new edge between the
/// two corners, which the bridge itself also uses.
#[allow(clippy::too_many_arguments)]
fn corner_triangle(
    s: &mut Model,
    v: VertexId,
    ra: EdgeId,
    rb: EdgeId,
    pa_v: VertexId,
    pb_v: VertexId,
    out_ref: DVec3,
    base: &BTreeMap<VertexId, VertexId>,
) -> Result<EdgeId, &'static str> {
    let cap = edge(s, pa_v, pb_v, Curve::Line, None)?;
    let pts = [s.vertex(base[&v]), s.vertex(pa_v), s.vertex(pb_v)];
    let n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalize_or_zero();
    let (tri_v, tri_e, n) = if n.dot(out_ref) >= 0.0 {
        ([base[&v], pa_v, pb_v], [ra, cap, rb], n)
    } else {
        ([base[&v], pb_v, pa_v], [rb, cap, ra], -n)
    };
    s.add_face(Face::with_edges(
        tri_v.to_vec(),
        vec![],
        Surface::Plane(Plane::new(pts[0], n)),
        vec![tri_e.to_vec()],
    ));
    Ok(cap)
}

/// Closes one end of an outside corner's bridge, where `fa` and `fb`
/// (this bridge's two faces) may or may not themselves have split
/// there (part 2c: a third face can keep the shared corner for
/// whichever of `fa`, `fb` it is truly on). Their two points there
/// equal (neither split, or a third face's own split happened to land
/// on both anyway): nothing to close, the top edges already meet.
/// Otherwise, wherever each of the two is on the rim has its own riser
/// there already (its own where it split, the one shared riser of
/// whichever did not) to close `corner_triangle` against; with no rim
/// at all there (an interior corner, part 2c), a plain edge between
/// the two points does it instead, built once per pair of points and
/// reused by every bridge (and the flat face closing a gap of three or
/// more points) that meets it. Returns the new edge, and the vertex at
/// its far end, for the bridge's own loop.
#[allow(clippy::too_many_arguments)]
fn close_bridge_end(
    s: &mut Model,
    v: VertexId,
    fa: FaceId,
    fb: FaceId,
    out_ref: DVec3,
    base: &BTreeMap<VertexId, VertexId>,
    up: &BTreeMap<VertexId, VertexId>,
    rise: &BTreeMap<(VertexId, Option<FaceId>), EdgeId>,
    split: &BTreeMap<(VertexId, FaceId), VertexId>,
    asym_cap: &mut BTreeMap<(VertexId, VertexId), EdgeId>,
) -> Result<Option<(EdgeId, VertexId)>, &'static str> {
    let key = |f: FaceId| (v, split.contains_key(&(v, f)).then_some(f));
    let pa_v = split.get(&(v, fa)).copied().unwrap_or(up[&v]);
    let pb_v = split.get(&(v, fb)).copied().unwrap_or(up[&v]);
    if pa_v == pb_v {
        return Ok(None);
    }
    match (rise.get(&key(fa)), rise.get(&key(fb))) {
        (Some(&ra), Some(&rb)) => Ok(Some((
            corner_triangle(s, v, ra, rb, pa_v, pb_v, out_ref, base)?,
            pb_v,
        ))),
        (None, None) => {
            let key = (pa_v.min(pb_v), pa_v.max(pb_v));
            let cap = match asym_cap.get(&key) {
                Some(e) => *e,
                None => {
                    let e = edge(s, pa_v, pb_v, Curve::Line, None)?;
                    asym_cap.insert(key, e);
                    e
                }
            };
            Ok(Some((cap, pb_v)))
        }
        _ => Err(TOO_THICK),
    }
}

/// The gap a split corner leaves, from each face's own corner vertex in
/// order round it: faces next to each other that share a point leave no
/// gap between them, and a point a bridge goes out to and straight back
/// from (x, y, x) is one edge both bridges' ends share, so it leaves none
/// either. What is left is the loop the gap's own face closes (fewer than
/// three points: none needed). None where that loop passes a point twice
/// (two gaps touching at a point, which one flat face cannot close).
fn gap_loop(mut seq: Vec<VertexId>) -> Option<Vec<VertexId>> {
    loop {
        let n = seq.len();
        if let Some(i) = (0..n).find(|i| n > 1 && seq[*i] == seq[(i + 1) % n]) {
            seq.remove(i);
            continue;
        }
        if n < 3 {
            return Some(seq);
        }
        match (0..n).find(|i| seq[(i + n - 1) % n] == seq[(i + 1) % n]) {
            Some(i) => {
                seq.remove(i);
            }
            None => break,
        }
    }
    let each: BTreeSet<VertexId> = seq.iter().copied().collect();
    (each.len() == seq.len()).then_some(seq)
}

/// The flat face closing the gap `gap` (a loop of three or more points,
/// each next pair already joined by a bridge's end in `asym_cap`), facing
/// `out`: one face where the points lie on one plane, a fan of triangles
/// from the first point where they do not (the gap need not be tidy:
/// as with an over-long 2D offset, tidying it is the user's to do).
fn close_gap(
    s: &mut Model,
    mut gap: Vec<VertexId>,
    out: DVec3,
    asym_cap: &BTreeMap<(VertexId, VertexId), EdgeId>,
    tol: &Tol,
) -> Result<(), &'static str> {
    if loop_normal(s, &gap).dot(out) < 0.0 {
        gap.reverse();
    }
    let n = gap.len();
    let mut sides: Vec<EdgeId> = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b) = (gap[i], gap[(i + 1) % n]);
        sides.push(*asym_cap.get(&(a.min(b), a.max(b))).ok_or(CORNER)?);
    }
    let face = |s: &mut Model, l: Vec<VertexId>, es: Vec<EdgeId>| {
        let pts: Vec<DVec3> = l.iter().map(|x| s.vertex(*x)).collect();
        let normal = newell(&pts).normalize_or_zero();
        s.add_face(Face::with_edges(
            l,
            vec![],
            Surface::Plane(Plane::new(pts[0], normal)),
            vec![es],
        ));
    };
    if n == 3 || flat_plane(s, &gap, tol).is_some() {
        face(s, gap, sides);
        return Ok(());
    }
    // From the first point to each it does not already reach.
    let mut spoke = vec![sides[0]];
    for x in &gap[2..n - 1] {
        spoke.push(edge(s, gap[0], *x, Curve::Line, None)?);
    }
    spoke.push(sides[n - 1]);
    for i in 1..n - 1 {
        face(
            s,
            vec![gap[0], gap[i], gap[i + 1]],
            vec![spoke[i - 1], sides[i], spoke[i]],
        );
    }
    Ok(())
}

/// The point of `s` nearest `x`, and its normal there: for a freeform
/// face past its own edge (an outside corner's mitre can reach past
/// it), its surface does not extend, so the edge's own point and normal
/// stand for it there, as a straight continuation would (a flat or
/// other analytic surface already has no edge to reach past).
fn probe(s: &Surface, x: DVec3) -> (DVec3, DVec3) {
    match s {
        Surface::Nurbs(patch) => {
            let q = patch.project(x);
            (q, patch.normal_at(q))
        }
        _ => (s.project(x), s.normal_at(x)),
    }
}

/// Whether `x` is genuinely on `s` (within tolerance of its nearest
/// point): a flat or other analytic surface always is (unbounded); a
/// freeform one is not once `x` is past its own edge.
fn truly_on(s: &Surface, x: DVec3, tol: &Tol) -> bool {
    match s {
        Surface::Nurbs(patch) => patch.project(x).distance(x) <= tol.dist,
        _ => true,
    }
}

/// How far `x` is from `s`, signed by its normal (`probe`'s tangent
/// stands in for the true surface past a freeform face's own edge).
fn offset_distance(s: &Surface, x: DVec3) -> f64 {
    let (q, n) = probe(s, x);
    (x - q).dot(n)
}

/// The point on both `a` and `b` closest to `guess`: Newton, moving only
/// in the plane of their two normals (as `corner` does exactly for flat
/// surfaces; a curved one is only locally flat, so this is iterated).
/// Falls back to the last point tried if it does not converge; the
/// caller checks the result against the true surfaces itself.
fn meet(a: &Surface, b: &Surface, guess: DVec3, tol: &Tol) -> DVec3 {
    meet_to(a, b, guess, tol, tol.dist)
}

/// [`meet`], iterated until both distances are under `stop` (a fit that
/// is checked finer than the tolerance wants a point found that closely).
fn meet_to(a: &Surface, b: &Surface, guess: DVec3, tol: &Tol, stop: f64) -> DVec3 {
    let mut x = guess;
    for _ in 0..40 {
        let (qa, na) = probe(a, x);
        let (qb, nb) = probe(b, x);
        let (fa, fb) = ((x - qa).dot(na), (x - qb).dot(nb));
        if fa.abs() < stop && fb.abs() < stop {
            return x;
        }
        let Some(step) = solve2(na, nb, fa, fb, tol) else {
            return x;
        };
        x -= step;
    }
    x
}

/// The vector in the plane of unit `a` and `b` with `a·v = p` and
/// `b·v = q`; None where they are too nearly parallel to separate (as
/// `corner` finds a fold too sharp to move both ways at once).
fn solve2(a: DVec3, b: DVec3, p: f64, q: f64, tol: &Tol) -> Option<DVec3> {
    if tol.parallel(a, b) {
        return None;
    }
    let c = a.dot(b);
    let denom = 1.0 - c * c;
    let s = (p - q * c) / denom;
    let t = (q - p * c) / denom;
    Some(a * s + b * t)
}

/// The curve where two creased faces' offsets truly meet: `meet_at(t)`
/// for the nodes given, fitted to a NURBS curve of degree 5, split until
/// the fit itself lies on both true offset surfaces within `eps` (as
/// `thicken::edge_pair` checks a face's own offset).
fn crease_curve(
    meet_at: &dyn Fn(f64) -> DVec3,
    off_a: &Surface,
    off_b: &Surface,
    nodes: &[f64],
    eps: f64,
) -> Option<prism_geom::Nurbs> {
    let mut ts = nodes.to_vec();
    for _ in 0..crate::thicken::ROUNDS {
        let knots = prism_geom::knots_over(crate::thicken::DEGREE, &ts, nodes)?;
        let curve = prism_geom::Nurbs::fitted(meet_at, crate::thicken::DEGREE, knots)?;
        let error = |t: f64| {
            let p = curve.eval(t);
            offset_distance(off_a, p)
                .abs()
                .max(offset_distance(off_b, p).abs())
        };
        let split: Vec<bool> = ts
            .windows(2)
            .map(|w| crate::thicken::off_between(w, &error, eps))
            .collect();
        if !split.contains(&true) {
            return Some(curve);
        }
        ts = crate::thicken::halved(&ts, &split);
    }
    None
}

fn edge(
    s: &mut Model,
    a: VertexId,
    b: VertexId,
    curve: Curve,
    cid: Option<prism_topo::CurveId>,
) -> Result<EdgeId, &'static str> {
    match curve {
        Curve::Line => s.add_edge(a, b),
        c => s.add_curve_edge(a, b, c, cid),
    }
    .ok_or(TOO_THICK)
}

/// The surface `d` along a face's normal from `s`; None where it folds
/// through itself (or is freeform).
fn offset_surface(s: &Surface, d: f64, tol: &Tol) -> Option<Surface> {
    let sign = |outward: bool| if outward { d } else { -d };
    let ok = |r: f64| (r > tol.dist).then_some(r);
    Some(match s {
        Surface::Plane(p) => Surface::Plane(Plane::new(p.origin + p.normal * d, p.normal)),
        Surface::Cylinder(c) => Surface::Cylinder(prism_geom::Cylinder {
            radius: ok(c.radius + sign(c.outward))?,
            ..*c
        }),
        Surface::Sphere(x) => Surface::Sphere(prism_geom::Sphere {
            radius: ok(x.radius + sign(x.outward))?,
            ..*x
        }),
        Surface::Torus(t) => {
            let minor = ok(t.minor + sign(t.outward))?;
            if minor >= t.major - tol.dist {
                return None;
            }
            Surface::Torus(prism_geom::Torus { minor, ..*t })
        }
        // A cone's offset is the same cone with its tip moved along the
        // axis: out (away from the axis) moves it back.
        Surface::Cone(c) => Surface::Cone(Cone {
            apex: c.apex - c.axis * (sign(c.outward) / c.half_angle.sin()),
            ..*c
        }),
        Surface::Nurbs(_) => return None,
    })
}

/// The wall between an arc and its moved copy (same axis): a flat ring, a
/// cylinder or a cone, facing `out` at `at` on the arc.
#[allow(clippy::too_many_arguments)]
fn arc_wall(
    c1: DVec3,
    axis: DVec3,
    r1: f64,
    c2: DVec3,
    r2: f64,
    at: DVec3,
    out: DVec3,
    tol: &Tol,
) -> Result<Surface, &'static str> {
    let rise = (c2 - c1).dot(axis);
    if (c2 - c1 - axis * rise).length() > tol.dist {
        return Err(FREEFORM);
    }
    let wall = if tol.is_zero(rise) {
        Surface::Plane(Plane::new(c1, axis))
    } else if tol.is_zero(r1 - r2) {
        Surface::Cylinder(prism_geom::Cylinder {
            origin: c1,
            axis,
            radius: r1,
            outward: true,
        })
    } else {
        let apex = c1 + (c2 - c1) * (r1 / (r1 - r2));
        let into = c1 - apex;
        Surface::Cone(Cone {
            apex,
            axis: into.normalize(),
            half_angle: (r1 / into.length()).atan(),
            outward: true,
        })
    };
    Ok(if wall.normal_at(at).dot(out) < 0.0 {
        wall.flipped()
    } else {
        wall
    })
}

fn plan(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
) -> Result<Plan, &'static str> {
    let tol = model.tol();
    if !d.is_finite() || tol.is_zero(d) {
        return Err(ZERO);
    }
    let body = body(model, faces);
    if body.is_empty() {
        return Err(NOTHING);
    }
    let kept: Vec<FaceId> = body.iter().copied().filter(|f| !open.contains(f)).collect();
    if kept.is_empty() {
        return Err(ALL_OPEN);
    }
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let spline = f
            .edge_ids()
            .any(|e| model.edge(e).is_none_or(|x| x.curve.is_spline()));
        if f.surface.patch().is_some() || spline {
            return Err(FREEFORM);
        }
    }
    // How the body's faces use each edge.
    let mut uses: BTreeMap<EdgeId, Vec<(bool, VertexId, VertexId, FaceId)>> = BTreeMap::new();
    for id in &body {
        let f = model.face(*id).ok_or(NOTHING)?;
        let keep = !open.contains(id);
        for side in f.sides() {
            uses.entry(side.edge)
                .or_default()
                .push((keep, side.from, side.to, *id));
        }
    }
    let mut rims = Vec::new();
    let mut at_edge: BTreeMap<EdgeId, Vec<FaceId>> = BTreeMap::new();
    for (e, u) in &uses {
        if u.len() > 2 {
            return Err(BRANCHES);
        }
        let kept: Vec<_> = u.iter().filter(|x| x.0).collect();
        match kept.as_slice() {
            [one] => rims.push((*e, one.1, one.2, one.3)),
            [x, y] if x.1 == y.1 => return Err(MIXED),
            _ => {}
        }
        at_edge.insert(*e, kept.iter().map(|x| x.3).collect());
    }
    // Where each point goes: the nearest point on every moved face it is
    // on.
    let mut at_vertex: BTreeMap<VertexId, Vec<FaceId>> = BTreeMap::new();
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        for v in f.loops().flatten() {
            let fs = at_vertex.entry(*v).or_default();
            if !fs.contains(id) {
                fs.push(*id);
            }
        }
    }
    let on_rim: BTreeSet<VertexId> = rims.iter().flat_map(|r| [r.1, r.2]).collect();
    let mut moved = BTreeMap::new();
    let mut splits = BTreeMap::new();
    for (v, fs) in &at_vertex {
        match move_point(model, fs, model.vertex(*v), d, &tol) {
            Ok(p) => {
                moved.insert(*v, p);
            }
            Err(CORNER) if !on_rim.contains(v) => {
                splits.insert(*v, split_corner(model, *v, fs, d)?);
            }
            Err(e) => return Err(e),
        }
    }
    // Where an edge's moved copy ends at `v`.
    let end = |v: VertexId, e: EdgeId| -> DVec3 {
        moved.get(&v).copied().unwrap_or_else(|| {
            splits[&v]
                .iter()
                .find(|(x, _)| *x == e)
                .map_or(DVec3::ZERO, |(_, q)| *q)
        })
    };
    // Each edge moves to the same kind of curve.
    let mut curves = BTreeMap::new();
    for (e, fs) in &at_edge {
        if fs.is_empty() {
            continue;
        }
        let x = model.edge(*e).ok_or(NOTHING)?;
        let (a, b) = (end(x.a, *e), end(x.b, *e));
        let (pa, pb) = (model.vertex(x.a), model.vertex(x.b));
        let at = |t: f64| move_point(model, fs, x.curve.point(pa, pb, t), d, &tol);
        let c = match &x.curve {
            Curve::Line => {
                let m = at(0.5)?;
                if !tol.is_zero(distance_to_line(m, a, b)) || (b - a).dot(pb - pa) <= 0.0 {
                    return Err(TOO_THICK);
                }
                Curve::Line
            }
            Curve::Arc { axis, .. } => {
                let centre = circumcentre(a, at(0.5)?, b).ok_or(TOO_THICK)?;
                let radius = a.distance(centre);
                for t in [0.25, 0.75] {
                    let q = at(t)?;
                    if !tol.is_zero(q.distance(centre) - radius)
                        || !tol.is_zero((q - centre).dot(*axis))
                    {
                        return Err(CORNER);
                    }
                }
                if radius <= tol.dist {
                    return Err(TOO_THICK);
                }
                Curve::Arc {
                    centre,
                    axis: *axis,
                    radius,
                }
            }
            Curve::Spline(_) => return Err(FREEFORM),
        };
        curves.insert(*e, c);
    }
    // No flat face of straight sides may turn inside out.
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let Some(plane) = f.plane() else {
            continue;
        };
        if f.edge_ids()
            .any(|e| !matches!(curves.get(&e), Some(Curve::Line)))
        {
            continue;
        }
        let sides: Vec<_> = f.loop_edges(0).collect();
        let mut pts: Vec<DVec3> = Vec::new();
        for (i, side) in sides.iter().enumerate() {
            let prev = sides[(i + sides.len() - 1) % sides.len()].edge;
            pts.push(end(side.from, prev));
            pts.push(end(side.from, side.edge));
        }
        if newell(&pts).dot(plane.normal) <= 0.0 {
            return Err(TOO_THICK);
        }
    }
    Ok(Plan {
        body,
        kept,
        moved,
        splits,
        curves,
        rims,
    })
}

/// A corner of four or more flat faces whose moved copies miss one point:
/// the edges round it in order, each with where its moved copy ends. The
/// ends lie on one plane across the corner (square to the faces' mean
/// direction), so a small flat face fills the gap; each face still moves
/// by the thickness.
fn split_corner(
    model: &Model,
    v: VertexId,
    fs: &[FaceId],
    d: f64,
) -> Result<Vec<(EdgeId, DVec3)>, &'static str> {
    let p = model.vertex(v);
    // Each face's edge in to the corner and out of it.
    let mut out_of: BTreeMap<EdgeId, (EdgeId, DVec3)> = BTreeMap::new();
    let mut sum = DVec3::ZERO;
    for f in fs {
        let face = model.face(*f).ok_or(NOTHING)?;
        let n = face.plane().ok_or(CORNER)?.normal;
        sum += n;
        let sides: Vec<_> = face.sides().collect();
        let e_in = sides.iter().find(|s| s.to == v).ok_or(CORNER)?.edge;
        let e_out = sides.iter().find(|s| s.from == v).ok_or(CORNER)?.edge;
        out_of.insert(e_in, (e_out, n));
    }
    let across = sum.normalize_or_zero();
    let lean: Vec<f64> = fs
        .iter()
        .filter_map(|f| {
            model
                .face(*f)
                .and_then(|x| x.plane())
                .map(|pl| pl.normal.dot(across))
        })
        .collect();
    if lean.iter().any(|c| *c <= ANGLE.sqrt()) {
        return Err(CORNER);
    }
    // As far across as the faces move on average.
    let h = d * lean.len() as f64 / lean.iter().sum::<f64>();
    // Round the corner: from each edge, through the face it leads into.
    let start = *out_of.keys().next().ok_or(CORNER)?;
    let mut fan = Vec::new();
    let mut e = start;
    loop {
        let (next, n_before) = out_of[&e];
        let (_, n_after) = *out_of.get(&next).ok_or(CORNER)?;
        // The edge `next` lies between the face before it and the one after.
        let x = solve3([n_before, n_after, across], [d, d, h]).ok_or(CORNER)?;
        fan.push((next, p + x));
        e = next;
        if e == start {
            break;
        }
        if fan.len() > fs.len() {
            return Err(CORNER);
        }
    }
    if fan.len() != fs.len() {
        return Err(CORNER);
    }
    Ok(fan)
}

/// The point x with a·x, b·x and c·x the given values, if the three
/// directions are independent.
fn solve3([a, b, c]: [DVec3; 3], [p, q, r]: [f64; 3]) -> Option<DVec3> {
    let det = a.dot(b.cross(c));
    (det.abs() > ANGLE).then(|| (b.cross(c) * p + c.cross(a) * q + a.cross(b) * r) / det)
}

/// Where `p`, on the faces `fs`, moves to.
fn move_point(
    model: &Model,
    fs: &[FaceId],
    p: DVec3,
    d: f64,
    tol: &Tol,
) -> Result<DVec3, &'static str> {
    let mut ns: Vec<DVec3> = Vec::new();
    for f in fs {
        ns.push(model.face(*f).ok_or(NOTHING)?.surface.normal_at(p));
    }
    let ns = unique_normals(&ns, tol);
    corner(&ns, d, tol).map(|x| p + x).ok_or(CORNER)
}

/// [`move_point`] where the point is also on rims that end flush in
/// `planes` (each passes through the point and holds it): the point moves
/// `d` along every face's normal and not at all off each plane, nor off
/// the wall planes `walls` (a square rim's wall, through its edge).
fn move_point_flush(
    model: &Model,
    fs: &[FaceId],
    p: DVec3,
    d: f64,
    planes: &[Plane],
    walls: &[DVec3],
    tol: &Tol,
) -> Result<DVec3, &'static str> {
    let mut want: Vec<(DVec3, f64)> = Vec::new();
    for f in fs {
        want.push((model.face(*f).ok_or(NOTHING)?.surface.normal_at(p), d));
    }
    for plane in planes {
        want.push((plane.normal, 0.0));
    }
    // The walls only place the corner while the faces and planes leave it
    // free (fewer than three independent directions).
    let solid = corner_mixed(&want, tol);
    let with_walls = {
        let mut w = want.clone();
        w.extend(walls.iter().map(|m| (*m, 0.0)));
        corner_mixed(&w, tol)
    };
    with_walls.or(solid).map(|x| p + x).ok_or(CORNER)
}

/// Where the offset of a square rim `e` of face `f` meets the flush plane
/// at its end `v`: marched along the edge from `v` until the point of the
/// face's own offset above it is in the plane. A meeting before the edge
/// starts (past `v`) needs the edge and the offset carried on past their
/// ends (`reach`, in `freeform_joined`); where the offset was not grown
/// that far it is refused as `FLUSH_REACH`.
#[allow(clippy::too_many_arguments)]
fn flush_end(
    model: &Model,
    f: FaceId,
    e: EdgeId,
    v: VertexId,
    offset: &Surface,
    plane: &Plane,
    d: f64,
    tol: &Tol,
) -> Result<DVec3, &'static str> {
    let edge_data = model.edge(e).ok_or(NOTHING)?;
    let face = model.face(f).ok_or(NOTHING)?;
    let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
    // Distance from the plane of the offset edge's point at `t`, with the
    // point itself.
    let at = |t: f64| {
        // Past the edge's own end the edge is carried on (a flush rim
        // that leans away meets its plane there), and so is the offset.
        let p = edge_data.curve.point_past(pa, pb, t);
        let x = offset.project(p + face.surface.normal_at(p) * d);
        (plane.distance(x), x)
    };
    // The end `v`, as a parameter along the edge.
    let t0 = if edge_data.a == v { 0.0 } else { 1.0 };
    let mut t = t0;
    for _ in 0..60 {
        let (g, x) = at(t);
        if g.abs() < 1e-12 {
            // Along the edge, or carried on past `v` as far as the offset
            // surface was grown (it is `truly_on` only where it was).
            let (lo, hi) = (-1.0, 2.0);
            return if (lo..=hi).contains(&t) && truly_on(offset, x, tol) {
                // Polished to much finer than the tolerance, as the edges
                // that end here are fitted.
                Ok(meet_to(
                    offset,
                    &Surface::Plane(*plane),
                    x,
                    tol,
                    tol.dist * 1e-4,
                ))
            } else {
                Err(FLUSH_REACH)
            };
        }
        let h = 1e-6;
        let slope = (at(t + h).0 - at(t - h).0) / (2.0 * h);
        if slope.abs() < 1e-12 {
            break;
        }
        t -= g / slope;
        if !(-1.0..=2.0).contains(&t) {
            break;
        }
    }
    Err(FLUSH_REACH)
}

/// A corner found from flat approximations, moved onto the true offset
/// surfaces of the faces `fs` and the `planes` (Newton, as `meet` does for
/// two surfaces), so the edges that end there end exactly on it.
fn refine_flush_corner(
    offsets: &BTreeMap<FaceId, Surface>,
    fs: &[FaceId],
    planes: &[Plane],
    start: DVec3,
    tol: &Tol,
) -> DVec3 {
    let mut x = start;
    for _ in 0..20 {
        let mut want: Vec<(DVec3, f64)> = Vec::new();
        for f in fs {
            let (q, n) = probe(&offsets[f], x);
            want.push((n, (x - q).dot(n)));
        }
        for plane in planes {
            want.push((plane.normal, plane.distance(x)));
        }
        // To much finer than the tolerance: the edges that end here are
        // fitted to a hundredth of it and must meet this point.
        if want.iter().all(|(_, c)| c.abs() < tol.dist * 1e-4) {
            break;
        }
        let Some(step) = corner_mixed(&want, tol) else {
            break;
        };
        x -= step;
    }
    x
}

/// A corner `q` of the faces `fs` polished onto their true offsets where
/// it is already close (the flat corner is off a curved offset by a
/// hair), or wherever the polished point is truly on every one of them
/// (a cylinder's offset curves away from the flat corner by more than a
/// hair): the edges that end there are fitted to a hundredth of the
/// tolerance and must meet it. A corner a face's offset does not reach is
/// left as it is (it splits, in `freeform_slab`).
fn polish_corner(offsets: &BTreeMap<FaceId, Surface>, fs: &[FaceId], q: DVec3, tol: &Tol) -> DVec3 {
    let polished = refine_flush_corner(offsets, fs, &[], q, tol);
    let on_all = fs.iter().all(|f| {
        truly_on(&offsets[f], polished, tol)
            && offset_distance(&offsets[f], polished).abs() <= tol.dist * 1e-3
    });
    if polished.distance(q) < tol.dist * 10.0 || on_all {
        polished
    } else {
        q
    }
}

/// The faces round `v` in order, each with the edge it leaves `v` by
/// (which the next face comes in by): None unless every face in `fs`
/// passes `v` exactly once and they close one ring.
fn ring(model: &Model, v: VertexId, fs: &[FaceId]) -> Option<Vec<(FaceId, EdgeId)>> {
    let mut by_in: BTreeMap<EdgeId, (FaceId, EdgeId)> = BTreeMap::new();
    for f in fs {
        let sides: Vec<_> = model.face(*f)?.sides().collect();
        let ins: Vec<_> = sides.iter().filter(|s| s.to == v).collect();
        let outs: Vec<_> = sides.iter().filter(|s| s.from == v).collect();
        let ([e_in], [e_out]) = (ins.as_slice(), outs.as_slice()) else {
            return None;
        };
        by_in.insert(e_in.edge, (*f, e_out.edge));
    }
    let start = *by_in.values().next()?;
    let mut out = vec![start];
    while out.len() < fs.len() {
        out.push(*by_in.get(&out[out.len() - 1].1)?);
    }
    let each: BTreeSet<FaceId> = out.iter().map(|x| x.0).collect();
    (each.len() == fs.len() && by_in.get(&out[out.len() - 1].1) == Some(&start)).then_some(out)
}

/// The shortest move `x` with `n·x = c` for every `(n, c)`: exact where up
/// to three independent directions meet; None where they cannot all hold
/// (two that oppose or repeat with different distances, or more than
/// three independent ones).
fn corner_mixed(want: &[(DVec3, f64)], tol: &Tol) -> Option<DVec3> {
    // A direction given twice (or opposite) is one constraint if it agrees.
    let mut each: Vec<(DVec3, f64)> = Vec::new();
    for (n, c) in want {
        match each.iter().find(|(m, _)| tol.parallel(*m, *n)) {
            Some((m, k)) => {
                let agree = if m.dot(*n) > 0.0 { *c } else { -*c };
                if !tol.is_zero(agree - *k) {
                    return None;
                }
            }
            None => each.push((*n, *c)),
        }
    }
    let basis: Vec<(DVec3, f64)> = {
        let mut out: Vec<(DVec3, f64)> = Vec::new();
        for (n, c) in &each {
            let mut r = *n;
            for b in &orthonormal(&out.iter().map(|x| x.0).collect::<Vec<_>>()) {
                r -= *b * r.dot(*b);
            }
            if r.length_squared() > 2.0 * ANGLE && out.len() < 3 {
                out.push((*n, *c));
            }
        }
        out
    };
    let x = match basis.as_slice() {
        [(a, p)] => *a * *p,
        [(a, p), (b, q)] => solve2(*a, *b, *p, *q, tol)?,
        [(a, p), (b, q), (c, r)] => solve3([*a, *b, *c], [*p, *q, *r])?,
        _ => return None,
    };
    each.iter()
        .all(|(n, c)| tol.is_zero(n.dot(x) - c))
        .then_some(x)
}

fn distance_to_line(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let u = (b - a).normalize_or_zero();
    let v = p - a;
    (v - u * v.dot(u)).length()
}

/// The centre of the circle through three points.
fn circumcentre(a: DVec3, b: DVec3, c: DVec3) -> Option<DVec3> {
    let (ab, ac) = (b - a, c - a);
    let n = ab.cross(ac);
    let nn = n.length_squared();
    (nn > 0.0).then(|| {
        a + (ac.cross(n) * ab.length_squared() + n.cross(ab) * ac.length_squared()) / (2.0 * nn)
    })
}

/// `ns`, with any pointing the same way (within tolerance) as an
/// earlier one dropped: `corner` only wants to see each direction once,
/// and only refuses for directions that truly oppose (it reads a
/// surface merely tangent to another one, not creased, the same way).
fn unique_normals(ns: &[DVec3], tol: &Tol) -> Vec<DVec3> {
    let mut out: Vec<DVec3> = Vec::new();
    for n in ns {
        if !out
            .iter()
            .any(|m: &DVec3| tol.parallel(*m, *n) && m.dot(*n) > 0.0)
        {
            out.push(*n);
        }
    }
    out
}

/// The shortest move that puts a point `d` along each unit normal in
/// `ns`: exact where up to three independent planes meet; None where
/// more do not meet at one point, or the faces fold back on each other.
fn corner(ns: &[DVec3], d: f64, tol: &Tol) -> Option<DVec3> {
    // Faces folded flat back onto each other cannot both move.
    for (i, a) in ns.iter().enumerate() {
        if ns[i + 1..].iter().any(|b| tol.parallel(*a, *b)) {
            return None;
        }
    }
    let mut basis: Vec<DVec3> = Vec::new();
    for n in ns {
        let mut r = *n;
        for b in &orthonormal(&basis) {
            r -= *b * r.dot(*b);
        }
        // Not in the planes so far (as `Tol::parallel` judges an angle).
        if r.length_squared() > 2.0 * ANGLE && basis.len() < 3 {
            basis.push(*n);
        }
    }
    let x = match basis.as_slice() {
        [a] => *a * d,
        [a, b] => (*a + *b) * (d / (1.0 + a.dot(*b))),
        [a, b, c] => {
            let det = a.dot(b.cross(*c));
            (b.cross(*c) + c.cross(*a) + a.cross(*b)) * (d / det)
        }
        _ => return None,
    };
    ns.iter().all(|n| tol.is_zero(n.dot(x) - d)).then_some(x)
}

fn orthonormal(vs: &[DVec3]) -> Vec<DVec3> {
    let mut out: Vec<DVec3> = Vec::new();
    for v in vs {
        let mut r = *v;
        for b in &out {
            r -= *b * r.dot(*b);
        }
        out.push(r.normalize_or_zero());
    }
    out
}

fn loop_normal(s: &Model, l: &[VertexId]) -> DVec3 {
    let pts: Vec<DVec3> = l.iter().map(|v| s.vertex(*v)).collect();
    newell(&pts)
}

/// The plane of a loop of points, if they all lie on one.
fn flat_plane(s: &Model, l: &[VertexId], tol: &Tol) -> Option<Plane> {
    let n = loop_normal(s, l).normalize_or_zero();
    if n == DVec3::ZERO {
        return None;
    }
    let o = s.vertex(l[0]);
    l.iter()
        .all(|v| tol.is_zero((s.vertex(*v) - o).dot(n)))
        .then(|| Plane::new(o, n))
}
