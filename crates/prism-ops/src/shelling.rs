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
//! the gap between the two. A fold right back on itself has no mitre at
//! all (the offsets are parallel there): thickened away from the fold it
//! is bridged the same way, out round the fold; thickened into it, it is
//! refused by name (`SHARP`).
//!
//! Faces bounded by a freeform curve are built as a freeform face's rim
//! is: the curve's offset fitted through points moved along the face's
//! normal, and a ruled wall between them.
//!
//! Build rather than refuse: faces that face different ways are turned to
//! agree first (as Solid > Heal would), a rim style that cannot be built
//! is cut square instead, and more than two faces at one edge are parted
//! into sheets, each shelled on its own, their shells joined by a union.
//! Each such fallback is reported with what it made ([`slab_flagged`],
//! [`shell_flagged`]), for the tool to mark and explain.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Cone, Curve, Nurbs, Surface};
use prism_math::tol::ANGLE;
use prism_math::{DVec2, DVec3, Plane, Tol, newell};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

pub const NOTHING: &str = "Refused: click a face to shell or thicken.";
pub const ZERO: &str = "Refused: a thickness of zero makes nothing.";
pub const ALL_OPEN: &str = "Refused: every face is left open, so there is nothing to thicken.";
pub const FREEFORM: &str = "Refused: an arc's moved copy here is not round the same axis, so Shell / Thicken cannot wall the gap between them yet.";
pub const MIXED: &str =
    "Refused: these faces face different ways. Solid > Heal turns them the same way.";
pub const BRANCHES: &str = "Refused: more than two faces meet at an edge here, and they cannot be parted into sheets that each thicken on their own (or their shells could not be joined).";
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

/// The old refusal for a wall under [`MIN_THICKNESS`]; such a wall is now
/// built with the [`THIN`] warning (owner, 2026-11-26).
pub const TOO_THIN: &str = THIN;

/// [`THIN`] for a thickness (signed) that is not zero and under the least;
/// zero is left to the kernel's own message (nothing sized yet).
pub fn thin_warning(thickness: f64) -> Option<&'static str> {
    // A size under a nanometre is nothing sized yet (the pointer just
    // pressed), not a thin wall.
    (thickness.abs() > 1e-9 && thickness.abs() < MIN_THICKNESS * (1.0 - 1e-9)).then_some(THIN)
}

pub const FLUSH_REACH: &str = "Refused: to end flush in that plane the surface would have to be carried on a long way past its own edge. Use a plane closer to square, or Square.";
pub const SHARP: &str = "Refused: these faces fold right back onto each other here, and thickening into the fold (or a fold that ends at a corner of other faces) is not built yet. Try the other way.";

// What Shell / Thicken built in place of refusing (see [`Flags`]): each
// names a fallback taken, for the tool to mark where it was taken and say
// why. None of them is a refusal: the shell was made.

/// More than two faces met at an edge: the body was parted there into
/// pieces of at most two faces at each edge, each shelled on its own and
/// the shells joined (overlaps merged).
pub const BRANCHED: &str = "More than two faces meet at an edge here: each sheet was thickened on its own and their shells were joined where they overlap.";
/// Two faces folded right back onto each other, thickened away from each
/// other, were bridged (out round the fold) where no mitre exists.
pub const BRIDGED: &str = "These faces fold right back onto each other here, so their offsets never meet: the gap between them is bridged round the fold instead of mitred.";
/// Faces that faced the other way from the faces round them were turned
/// to match (as Solid > Heal would) before shelling.
pub const TURNED: &str = "Some of these faces faced the other way from the faces round them, so they were turned to match (as Solid > Heal does) before shelling.";
/// A rim style the shape cannot take was cut square instead.
pub const SQUARED: &str =
    "That rim style cannot be built on this shape here, so this rim was cut square instead.";
/// A flush plane that does not hold its rim's edge was cut square instead.
pub const SQUARED_OFF: &str = "The flush plane chosen for this rim does not pass through its edge, so the rim was cut square instead.";
/// A flush plane along the face at its rim was cut square instead.
pub const SQUARED_ALONG: &str = "The flush plane chosen for this rim lies along the face there, so the rim was cut square instead.";

/// What a fallback changed, by reason (one of the constants above): the
/// new points, edges and faces it made or changed, in the ids of the model
/// they are in (the slab on its own from [`slab_flagged`], the model
/// itself after [`shell_flagged`]). Empty when nothing needed one.
pub type Flags = BTreeMap<&'static str, crate::health::Found>;

/// A slab with what was flagged while building it.
#[derive(Clone, Debug)]
pub struct Flagged {
    /// The new solid on its own.
    pub model: Model,
    /// The faces it replaces.
    pub replaced: BTreeSet<FaceId>,
    /// What fallbacks were taken, and where in `model`.
    pub flags: Flags,
}

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
    // Each style is tried as it is: one that would be cut square instead
    // is no pick at all.
    let builds = |styles: &RimStyles| {
        slab_marked(model, faces, open, d, styles, false, Fallback::Off).is_ok()
    };
    if !builds(&styles) {
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
        if builds(&trial) {
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
    /// Of `splits`, the points on a rim ([`rim_ends`]): a small triangle
    /// closes the gap between the two rims' walls there.
    rim_splits: BTreeSet<VertexId>,
    /// Each kept face's edges moved with it.
    curves: BTreeMap<EdgeId, Curve>,
    /// Edges with a kept face on one side only: the edge, the way that face
    /// walks it, and the face.
    rims: Vec<(EdgeId, VertexId, VertexId, FaceId)>,
}

/// Why the shell cannot be built as asked, if it cannot. [`slab`] and
/// [`shell`] build it anyway, face by face, and flag it (owner,
/// 2026-11-26: warn, never refuse); this says why they had to.
pub fn refusal(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    thickness: f64,
) -> Option<&'static str> {
    refusal_with(model, faces, open, thickness, &RimStyles::new())
}

/// [`refusal`] with rim styles.
pub fn refusal_with(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    thickness: f64,
    styles: &RimStyles,
) -> Option<&'static str> {
    slab_marked(model, faces, open, thickness, styles, true, Fallback::Off).err()
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
    shell_flagged(model, faces, open, thickness, styles).map(|_| ())
}

/// [`shell_with`], and what was flagged while building it, in the ids of
/// `model` once the shell is joined into it.
pub fn shell_flagged(
    model: &mut Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    thickness: f64,
    styles: &RimStyles,
) -> Result<Flags, &'static str> {
    let built = slab_flagged(model, faces, open, thickness, styles)?;
    let tol = model.tol();
    crate::thicken::remove_faces(model, &built.replaced);
    let (vmap, emap, fmap) = crate::boolean::join_mapped(model, &built.model, &tol);
    let mut flags = Flags::new();
    for (why, found) in built.flags {
        let at = flags.entry(why).or_default();
        // Faces left as they were are the model's own already.
        if why == SKIPPED {
            at.faces.extend(found.faces);
            continue;
        }
        at.vertices
            .extend(found.vertices.iter().filter_map(|v| vmap.get(v)));
        at.edges
            .extend(found.edges.iter().filter_map(|e| emap.get(e)));
        at.faces
            .extend(found.faces.iter().filter_map(|f| fmap.get(f)));
    }
    Ok(flags)
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
/// `Square` needs a joined body with a freeform face; where one cannot be
/// built, that rim is cut square instead (flagged: [`slab_flagged`]).
pub fn slab_with(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    styles: &RimStyles,
) -> Result<(Model, BTreeSet<FaceId>), &'static str> {
    slab_flagged(model, faces, open, d, styles).map(|b| (b.model, b.replaced))
}

/// [`slab_with`], and what was flagged while building it: where the
/// shell was built some other way than asked rather than refused (faces
/// turned round to match the rest, a rim style cut square instead), with
/// the reason for each, in the slab's own ids.
pub fn slab_flagged(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    styles: &RimStyles,
) -> Result<Flagged, &'static str> {
    slab_marked(model, faces, open, d, styles, true, Fallback::Full)
}

/// [`slab_flagged`] for a preview: where it is built face by face, the
/// pieces are left unjoined (joining them is the slow part), which is
/// enough to show it and what to check. Make it with [`shell_flagged`].
pub fn slab_preview(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    styles: &RimStyles,
) -> Result<Flagged, &'static str> {
    slab_marked(model, faces, open, d, styles, true, Fallback::Preview)
}

/// Where a slab is built some other way than asked, rather than
/// refused: the rims and faces to mark in it (in the model's ids) and
/// why, and what has been marked so far (in the slab's ids).
#[derive(Clone, Debug, Default)]
struct Marks {
    rims: BTreeMap<EdgeId, &'static str>,
    faces: BTreeMap<FaceId, &'static str>,
    flags: Flags,
}

impl Marks {
    /// The copy `copy` in the slab of the face `f`: marked if `f` is.
    fn copied(&mut self, f: FaceId, copy: FaceId) {
        if let Some(why) = self.faces.get(&f) {
            self.flags.entry(why).or_default().faces.insert(copy);
        }
    }

    /// The wall `wall` in the slab (of `s`) at the rim `e`: marked, with
    /// its edges, if `e` is.
    fn walled(&mut self, s: &Model, e: EdgeId, wall: FaceId) {
        if let Some(why) = self.rims.get(&e) {
            let at = self.flags.entry(why).or_default();
            at.faces.insert(wall);
            at.edges
                .extend(s.face(wall).into_iter().flat_map(|f| f.edge_ids()));
        }
    }
}

/// Whether some edge of the body of `faces` has two kept faces walking it
/// the same way (they face different ways: Solid > Heal turns them).
fn mixed(model: &Model, faces: &BTreeSet<FaceId>, open: &BTreeSet<FaceId>) -> bool {
    let mut from: BTreeMap<EdgeId, Vec<VertexId>> = BTreeMap::new();
    let mut uses: BTreeMap<EdgeId, usize> = BTreeMap::new();
    for id in body(model, faces) {
        let Some(f) = model.face(id) else { continue };
        for side in f.sides() {
            *uses.entry(side.edge).or_default() += 1;
            if !open.contains(&id) {
                from.entry(side.edge).or_default().push(side.from);
            }
        }
    }
    from.iter()
        .any(|(e, v)| uses[e] == 2 && matches!(v[..], [a, b] if a == b))
}

/// The model to shell: `model` itself, or, where kept faces face different
/// ways (`mixed`), a copy with the faces Solid > Heal would turn round in
/// the body turned (marked `TURNED`), so the shell is built as if Heal had
/// been run first.
fn turned<'a>(
    model: &'a Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    marks: &mut Marks,
) -> std::borrow::Cow<'a, Model> {
    if !mixed(model, faces, open) {
        return std::borrow::Cow::Borrowed(model);
    }
    let body = body(model, faces);
    let wrong: BTreeSet<FaceId> = crate::health::reversed(model)
        .intersection(&body)
        .copied()
        .collect();
    let mut m = model.clone();
    crate::health::reverse(&mut m, &wrong);
    for f in wrong {
        marks.faces.insert(f, TURNED);
    }
    std::borrow::Cow::Owned(m)
}

/// The flush styles in `styles` that cannot work at all (the plane does
/// not hold the rim's edge, or lies along the face there), each taken out
/// (so cut square) and marked: every other style as it is.
fn sound_styles(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    styles: &RimStyles,
    marks: &mut Marks,
) -> RimStyles {
    let tol = model.tol();
    // The kept face at each rim (an edge with one kept face).
    let mut kept_at: BTreeMap<EdgeId, Vec<FaceId>> = BTreeMap::new();
    for id in body(model, faces).iter().filter(|f| !open.contains(*f)) {
        let Some(f) = model.face(*id) else { continue };
        for side in f.sides() {
            kept_at.entry(side.edge).or_default().push(*id);
        }
    }
    let mut out = styles.clone();
    for (e, style) in styles {
        let (RimStyle::Flush(plane), Some([face])) = (style, kept_at.get(e).map(|v| v.as_slice()))
        else {
            continue;
        };
        let (Some(edge_data), Some(f)) = (model.edge(*e), model.face(*face)) else {
            continue;
        };
        if let Err(why) = flush_fits(model, edge_data, f, plane, &tol) {
            out.remove(e);
            marks.rims.insert(
                *e,
                if why == FLUSH_OFF {
                    SQUARED_OFF
                } else {
                    SQUARED_ALONG
                },
            );
        }
    }
    out
}

/// Whether a flush plane can end the rim `edge` of face `f`: it must hold
/// the edge, and the face must not lie along it there.
fn flush_fits(
    model: &Model,
    edge: &prism_topo::Edge,
    f: &Face,
    plane: &Plane,
    tol: &Tol,
) -> Result<(), &'static str> {
    let (pa, pb) = (model.vertex(edge.a), model.vertex(edge.b));
    for i in 0..=8 {
        let p = edge.curve.point(pa, pb, f64::from(i) / 8.0);
        if !tol.is_zero(plane.distance(p)) {
            return Err(FLUSH_OFF);
        }
        if tol.parallel(f.surface.normal_at(p), plane.normal) {
            return Err(FLUSH_PARALLEL);
        }
    }
    Ok(())
}

/// The slab, built from the model with its faces turned to agree first.
/// With `squaring`, a rim style that cannot be built is cut square instead
/// (marked); without it (for trying the program's own picks), a style is
/// built as it is or refused. A plane that leans too far to reach
/// (`FLUSH_REACH`) is still refused: the tool drags the plane, and says so
/// while it is dragged.
/// Whether [`slab_marked`] builds what it cannot build as asked anyway.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fallback {
    /// No: refused, with why.
    Off,
    /// Yes, its pieces only (not joined): enough to see it, quickly (the
    /// tool's ghost, redrawn as the pointer moves).
    Preview,
    /// Yes, the pieces joined into one solid.
    Full,
}

/// With `fallback`, what cannot be built as asked is built anyway (owner,
/// 2026-11-26: "just give everything a warning"): a rim style that cannot
/// be reached is cut square, and a shape that still cannot be built as one
/// piece is built face by face ([`apart`]), each flagged with why. Only
/// nothing to build (no face, no thickness, every face open) is refused.
fn slab_marked(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    styles: &RimStyles,
    squaring: bool,
    fallback: Fallback,
) -> Result<Flagged, &'static str> {
    let anyway = fallback != Fallback::Off;
    match as_asked(model, faces, open, d, styles, squaring, anyway) {
        Err(why) if anyway && ![NOTHING, ZERO, ALL_OPEN].contains(&why) => {
            let work = turned(model, faces, open, &mut Marks::default());
            let join = fallback == Fallback::Full;
            apart(work.as_ref(), faces, open, d, why, join).ok_or(why)
        }
        r => r,
    }
}

/// [`slab_marked`] short of building face by face.
fn as_asked(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    styles: &RimStyles,
    squaring: bool,
    fallback: bool,
) -> Result<Flagged, &'static str> {
    let mut marks = Marks::default();
    let work = turned(model, faces, open, &mut marks);
    let m = work.as_ref();
    let styles = if squaring {
        sound_styles(m, faces, open, styles, &mut marks)
    } else {
        styles.clone()
    };
    let attempt = |styles: &RimStyles, marks: &Marks| {
        let mut marks = marks.clone();
        built(m, faces, open, d, styles, &mut marks).map(|(s, body)| Flagged {
            model: s,
            replaced: body,
            flags: marks.flags,
        })
    };
    let first = attempt(&styles, &marks);
    let styled = |s: &RimStyles| s.values().any(|x| *x != RimStyle::Square);
    match first {
        // A flush plane out of reach is refused as such for the tool's
        // rim handle to say so; built anyway, that rim is cut square.
        Err(why) if squaring && styled(&styles) && (fallback || why != FLUSH_REACH) => {}
        r => return r,
    }
    // Some style cannot be built here: every rim square first (if even
    // that cannot be built, that is the reason to give), then each
    // boundary's own style put back where it still builds.
    attempt(&RimStyles::new(), &marks)?;
    let mut groups: Vec<(RimStyle, Vec<EdgeId>)> = Vec::new();
    for run in boundaries(m, faces, open) {
        let mut each: Vec<(RimStyle, Vec<EdgeId>)> = Vec::new();
        for e in run {
            let Some(style) = styles.get(&e).copied().filter(|s| *s != RimStyle::Square) else {
                continue;
            };
            match each.iter_mut().find(|(s, _)| *s == style) {
                Some((_, g)) => g.push(e),
                None => each.push((style, vec![e])),
            }
        }
        groups.extend(each);
    }
    let mut keep = RimStyles::new();
    for (style, group) in groups {
        let mut trial = keep.clone();
        trial.extend(group.iter().map(|e| (*e, style)));
        if attempt(&trial, &Marks::default()).is_ok() {
            keep = trial;
        } else {
            for e in group {
                marks.rims.insert(e, SQUARED);
            }
        }
    }
    attempt(&keep, &marks)
}

/// Built face by face ([`apart`]) because the shape could not be built
/// as one piece, by the reason it could not.
pub const APART_CORNER: &str = "Built face by face: more than three faces meet at a corner here and their moved copies do not meet at one point. Each face was thickened on its own and the pieces joined: check the corners for gaps or overlaps.";
pub const APART_SHARP: &str = "Built face by face: these faces fold right back onto each other, and thickening into the fold does not make one piece. Each face was thickened on its own and the pieces joined: check the fold.";
pub const APART_THICK: &str = "Built face by face: at this thickness a face turns inside out or the walls cannot close. Each face was thickened on its own and the pieces joined: check for overlaps, or try a smaller thickness.";
pub const APART_FREEFORM: &str = "Built face by face: an arc's moved copy is not round the same axis, so the wall between them could not be made as one piece. Each face was thickened on its own and the pieces joined: check the walls.";
pub const APART_BRANCHES: &str = "Built face by face: more than two faces meet at an edge and the sheets could not be parted and joined. Each face was thickened on its own and the pieces joined: check the edges where they meet.";
pub const APART: &str = "Built face by face: this shape could not be shelled as one piece. Each face was thickened on its own and the pieces joined: check the corners and edges.";
/// Faces the face-by-face build could not thicken (or join) at all: left
/// as they were.
pub const SKIPPED: &str = "Some faces could not be thickened even on their own at this thickness, so they were left as they were: the shell has gaps there.";
/// Pieces of a face-by-face build that would not join into one solid,
/// kept side by side.
pub const UNJOINED: &str = "Some pieces of this face-by-face shell would not join into one solid, so they were kept side by side: they may overlap (Model Health shows where).";
/// A wall thinner than the least the tool makes without a word.
pub const THIN: &str =
    "Very thin: under 0.1 cm (1 mm). Built anyway: check it is the size you meant.";

/// Face-by-face builds of more kept faces than this are made a piece at a
/// time ([`ShellJob`]), shown as they go, and can be stopped (owner,
/// 2026-11-26: no limit on the pieces joined; a long job ripples).
pub const JOB_FACES: usize = 12;

/// The warning for building face by face because of `why`.
fn apart_note(why: &str) -> &'static str {
    match why {
        x if x == CORNER => APART_CORNER,
        x if x == SHARP => APART_SHARP,
        x if x == TOO_THICK => APART_THICK,
        x if x == FREEFORM => APART_FREEFORM,
        x if x == BRANCHES => APART_BRANCHES,
        _ => APART,
    }
}

/// What a face-by-face build made, kept so the same build is not made
/// twice (a job made a piece at a time, then the Shell command that makes
/// it part of the model; a live shell run again with the same inputs).
/// The result is exactly what building it again would give.
type Built = (Model, BTreeSet<FaceId>, bool);

/// The inputs of a face-by-face build: the model (faces turned to match),
/// the faces, those left open, the thickness and whether it joins.
type BuildKey = (Model, BTreeSet<FaceId>, BTreeSet<FaceId>, u64, bool);

/// How many face-by-face builds are kept.
const KEPT_BUILDS: usize = 4;

thread_local! {
    static BUILDS: std::cell::RefCell<Vec<(BuildKey, Built)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn kept_build(key: &BuildKey) -> Option<Built> {
    BUILDS.with(|b| {
        b.borrow()
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    })
}

fn keep_build(key: BuildKey, built: Built) {
    BUILDS.with(|b| {
        let mut b = b.borrow_mut();
        b.retain(|(k, _)| *k != key);
        b.push((key, built));
        if b.len() > KEPT_BUILDS {
            b.remove(0);
        }
    });
}

/// A face-by-face shell built a piece at a time: each kept face thickened
/// on its own (rims square), then joined by a union into the pieces made
/// so far. Overlaps merge; gaps (an outside corner, where the faces' slabs
/// do not reach each other) stay. A face that cannot be thickened even on
/// its own is left as it was; a piece that will not join any other is
/// kept beside them (flagged).
pub struct ShellJob {
    model: Model,
    faces: BTreeSet<FaceId>,
    open: BTreeSet<FaceId>,
    d: f64,
    why: &'static str,
    join: bool,
    body: BTreeSet<FaceId>,
    todo: Vec<FaceId>,
    next: usize,
    solids: Vec<Model>,
    skipped: BTreeSet<FaceId>,
}

impl ShellJob {
    fn new(
        model: &Model,
        faces: &BTreeSet<FaceId>,
        open: &BTreeSet<FaceId>,
        d: f64,
        why: &'static str,
        join: bool,
    ) -> ShellJob {
        let body = body(model, faces);
        let todo = body.iter().filter(|f| !open.contains(f)).copied().collect();
        ShellJob {
            model: model.clone(),
            faces: faces.clone(),
            open: open.clone(),
            d,
            why,
            join,
            body,
            todo,
            next: 0,
            solids: Vec::new(),
            skipped: BTreeSet::new(),
        }
    }

    fn key(&self) -> BuildKey {
        (
            self.model.clone(),
            self.faces.clone(),
            self.open.clone(),
            self.d.to_bits(),
            self.join,
        )
    }

    /// Pieces made so far, and how many there are in all.
    pub fn progress(&self) -> (usize, usize) {
        (self.next, self.todo.len())
    }

    /// Why it is built face by face.
    pub fn why(&self) -> &'static str {
        self.why
    }

    /// Make the next piece and join it in. False when there are no more.
    pub fn step(&mut self) -> bool {
        let Some(f) = self.todo.get(self.next).copied() else {
            return false;
        };
        self.next += 1;
        // The face on its own, so nothing else is near.
        let mut m = self.model.clone();
        let others: Vec<FaceId> = m.faces().map(|(id, _)| id).filter(|id| *id != f).collect();
        for id in others {
            m.remove_face(id);
        }
        let one: BTreeSet<FaceId> = [f].into();
        let piece = built(
            &m,
            &one,
            &BTreeSet::new(),
            self.d,
            &RimStyles::new(),
            &mut Marks::default(),
        );
        let Ok((slab, _)) = piece else {
            self.skipped.insert(f);
            return self.next < self.todo.len();
        };
        // Unjoined, for a preview: the pieces side by side.
        let joined = self.join
            && self.solids.iter_mut().any(|w| {
                match crate::boolean::boolean(w, &slab, crate::boolean::Op::Union) {
                    Ok(j) if crate::boolean::is_solid(&j) => {
                        *w = j;
                        true
                    }
                    _ => false,
                }
            });
        if !joined {
            self.solids.push(slab);
        }
        self.next < self.todo.len()
    }

    /// What it looks like so far: the model with the faces done so far
    /// replaced by the pieces made from them.
    pub fn shown(&self) -> Model {
        let tol = self.model.tol();
        let mut m = self.model.clone();
        for f in &self.todo[..self.next] {
            if !self.skipped.contains(f) {
                m.remove_face(*f);
            }
        }
        for w in &self.solids {
            crate::boolean::join_mapped(&mut m, w, &tol);
        }
        m
    }

    /// Every piece made and joined (what is left is made now), kept so the
    /// same build is not made again. None if no face could be thickened.
    pub fn finish(mut self) -> Option<Flagged> {
        while self.step() {}
        let tol = self.model.tol();
        let unjoined = self.solids.len() > 1 && self.join;
        let mut solids = std::mem::take(&mut self.solids).into_iter();
        let mut whole = solids.next()?;
        for w in solids {
            crate::boolean::join_mapped(&mut whole, &w, &tol);
        }
        let built: Built = (whole, self.skipped.clone(), unjoined);
        keep_build(self.key(), built.clone());
        Some(self.flagged(built))
    }

    fn flagged(&self, (whole, skipped, unjoined): Built) -> Flagged {
        let mut flags = Flags::new();
        if unjoined {
            flags.entry(UNJOINED).or_default();
        }
        let at = flags.entry(apart_note(self.why)).or_default();
        at.faces.extend(whole.faces().map(|(id, _)| id));
        at.edges.extend(whole.edges().map(|(id, _)| id));
        if !skipped.is_empty() {
            // In the model's own ids: these faces stay in it.
            flags.entry(SKIPPED).or_default().faces.extend(&skipped);
        }
        Flagged {
            model: whole,
            replaced: self.body.difference(&skipped).copied().collect(),
            flags,
        }
    }
}

/// The shell of `faces` as a job made a piece at a time, if it is built
/// face by face from more than [`JOB_FACES`] kept faces and has not been
/// built already; None if it is built at once (as one piece, or face by
/// face from few faces, or made before).
pub fn shell_job(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    styles: &RimStyles,
) -> Option<ShellJob> {
    let why = match as_asked(model, faces, open, d, styles, true, true) {
        Err(why) if ![NOTHING, ZERO, ALL_OPEN].contains(&why) => why,
        _ => return None,
    };
    let tol = model.tol();
    if !d.is_finite() || tol.is_zero(d) {
        return None;
    }
    let work = turned(model, faces, open, &mut Marks::default());
    let job = ShellJob::new(work.as_ref(), faces, open, d, why, true);
    (job.todo.len() > JOB_FACES && kept_build(&job.key()).is_none()).then_some(job)
}

/// The body of `faces` shelled face by face, where it could not be shelled
/// as one piece (`why`): [`ShellJob`] made all at once (or as made
/// before). None if no face could be thickened. Without `join` (a
/// preview) the pieces are put side by side instead of joined.
fn apart(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    why: &'static str,
    join: bool,
) -> Option<Flagged> {
    let tol = model.tol();
    if !d.is_finite() || tol.is_zero(d) {
        return None;
    }
    let job = ShellJob::new(model, faces, open, d, why, join);
    match kept_build(&job.key()) {
        Some(built) => Some(job.flagged(built)),
        None => job.finish(),
    }
}

/// More than two kept faces at one edge (a T, a fin, a box with a wall
/// inside it): every wall built elsewhere is between two faces, so the
/// body is parted there into pieces that each have at most two faces at
/// every edge. At each such edge, faces next to each other round it that
/// walk it opposite ways (they could be one sheet folded there) are
/// joined, as if they were the only two faces there: their crease is
/// mitred or bridged as any is. A face left over has the edge as a rim of
/// its own. Faces of one piece joined elsewhere must be joined here too.
/// Each piece is shelled on its own, and the pieces' shells are joined by
/// a boolean union: where they overlap they merge, and what one piece
/// would leave open past the edge another fills (the excess trimmed, as
/// the 2D Offset tool lets offsets cross and tidies nothing it need not).
/// None when no edge has more than two kept faces.
fn branched(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    styles: &RimStyles,
    marks: &mut Marks,
) -> Option<Result<(Model, BTreeSet<FaceId>), &'static str>> {
    let body = body(model, faces);
    let kept: Vec<FaceId> = body.iter().copied().filter(|f| !open.contains(f)).collect();
    // How the kept faces walk each edge: each face, and whether it walks
    // it from its `a` end.
    let mut at: BTreeMap<EdgeId, Vec<(FaceId, bool)>> = BTreeMap::new();
    for id in &kept {
        let f = model.face(*id)?;
        for side in f.sides() {
            let e = model.edge(side.edge)?;
            at.entry(side.edge)
                .or_default()
                .push((*id, side.from == e.a));
        }
    }
    let branches: Vec<EdgeId> = at
        .iter()
        .filter(|(_, u)| u.len() > 2)
        .map(|(e, _)| *e)
        .collect();
    if branches.is_empty() {
        return None;
    }
    Some(branched_slab(
        model, body, &kept, &at, &branches, d, styles, marks,
    ))
}

/// [`branched`], once the edges with more than two kept faces are known.
#[allow(clippy::too_many_arguments)]
fn branched_slab(
    model: &Model,
    body: BTreeSet<FaceId>,
    kept: &[FaceId],
    at: &BTreeMap<EdgeId, Vec<(FaceId, bool)>>,
    branches: &[EdgeId],
    d: f64,
    styles: &RimStyles,
    marks: &mut Marks,
) -> Result<(Model, BTreeSet<FaceId>), &'static str> {
    let tol = model.tol();
    if !d.is_finite() || tol.is_zero(d) {
        return Err(ZERO);
    }
    // The pieces, joined by every edge of two kept faces, then by the
    // pairs chosen at each branched edge.
    let index: BTreeMap<FaceId, usize> = kept.iter().enumerate().map(|(i, f)| (*f, i)).collect();
    let mut parent: Vec<usize> = (0..kept.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let join = |parent: &mut Vec<usize>, a: FaceId, b: FaceId| {
        let (ra, rb) = (find(parent, index[&a]), find(parent, index[&b]));
        parent[ra.max(rb)] = ra.min(rb);
    };
    for u in at.values() {
        if let [(a, _), (b, _)] = u[..] {
            join(&mut parent, a, b);
        }
    }
    let mut pairs: BTreeMap<EdgeId, Vec<(FaceId, FaceId)>> = BTreeMap::new();
    for e in branches {
        let round = around(model, *e, &at[e]).ok_or(BRANCHES)?;
        let n = round.len();
        let next = |i: usize, j: usize| (i + 1) % n == j || (j + 1) % n == i;
        let fits = |i: usize, j: usize| round[i].1 != round[j].1 && next(i, j);
        let mut taken = vec![false; n];
        let mut here: Vec<(FaceId, FaceId)> = Vec::new();
        // Two faces of one piece already: they must be the pair here.
        for i in 0..n {
            for j in i + 1..n {
                let (ri, rj) = (
                    find(&mut parent, index[&round[i].0]),
                    find(&mut parent, index[&round[j].0]),
                );
                if ri != rj {
                    continue;
                }
                if taken[i] || taken[j] || !fits(i, j) {
                    return Err(BRANCHES);
                }
                taken[i] = true;
                taken[j] = true;
                here.push((round[i].0, round[j].0));
            }
        }
        // Then neighbours round it that can be one sheet.
        for i in 0..n {
            let j = (i + 1) % n;
            if !taken[i] && !taken[j] && fits(i, j) {
                taken[i] = true;
                taken[j] = true;
                join(&mut parent, round[i].0, round[j].0);
                here.push((round[i].0, round[j].0));
            }
        }
        pairs.insert(*e, here);
    }
    // A piece joined to itself at a branched edge through another one,
    // but left unpaired here, cannot be parted.
    for e in branches {
        let fs: Vec<FaceId> = at[e].iter().map(|x| x.0).collect();
        for (i, a) in fs.iter().enumerate() {
            for b in &fs[i + 1..] {
                let same = find(&mut parent, index[a]) == find(&mut parent, index[b]);
                let paired = pairs[e]
                    .iter()
                    .any(|(x, y)| (x, y) == (a, b) || (x, y) == (b, a));
                if same && !paired {
                    return Err(BRANCHES);
                }
            }
        }
    }
    let mut pieces: BTreeMap<usize, BTreeSet<FaceId>> = BTreeMap::new();
    for f in kept {
        let r = find(&mut parent, index[f]);
        pieces.entry(r).or_default().insert(*f);
    }
    // Each piece shelled on its own (the model with only its faces, so
    // nothing of another piece is near), then all joined.
    let mut whole: Option<Model> = None;
    let mut flags = Flags::new();
    for piece in pieces.values() {
        let mut m = model.clone();
        let others: Vec<FaceId> = m
            .faces()
            .map(|(id, _)| id)
            .filter(|id| !piece.contains(id))
            .collect();
        for id in others {
            m.remove_face(id);
        }
        let mut own = Marks {
            flags: Flags::new(),
            ..marks.clone()
        };
        let (slab, _) = built(&m, piece, &BTreeSet::new(), d, styles, &mut own)?;
        whole = Some(match whole {
            None => {
                flags = own.flags;
                slab
            }
            Some(w) => {
                let joined = crate::boolean::boolean(&w, &slab, crate::boolean::Op::Union)
                    .map_err(|_| BRANCHES)?;
                let mut carried = carry(&flags, &w, &joined, &tol);
                for (why, found) in carry(&own.flags, &slab, &joined, &tol) {
                    let at = carried.entry(why).or_default();
                    at.vertices.extend(found.vertices);
                    at.edges.extend(found.edges);
                    at.faces.extend(found.faces);
                }
                flags = carried;
                joined
            }
        });
    }
    let whole = whole.ok_or(ALL_OPEN)?;
    if !crate::boolean::is_solid(&whole) {
        return Err(BRANCHES);
    }
    // Where the pieces were parted: what of the shell lies within reach of
    // a branched edge (twice the thickness, where their shells meet).
    let reach = 2.0 * d.abs() + tol.dist;
    let near = |p: DVec3| {
        branches.iter().any(|e| {
            model.edge(*e).is_some_and(|x| {
                let (pa, pb) = (model.vertex(x.a), model.vertex(x.b));
                x.curve.closest_point(pa, pb, p).distance(p) <= reach
            })
        })
    };
    let at_branch = flags.entry(BRANCHED).or_default();
    for (id, x) in whole.edges() {
        let (pa, pb) = (whole.vertex(x.a), whole.vertex(x.b));
        if near(pa) && near(pb) && near(x.curve.point(pa, pb, 0.5)) {
            at_branch.edges.insert(id);
        }
    }
    for (id, f) in whole.faces() {
        if f.edge_ids().any(|e| at_branch.edges.contains(&e)) {
            at_branch.faces.insert(id);
        }
    }
    marks.flags = flags;
    Ok((whole, body))
}

/// The faces `fs` at edge `e` (each with whether it walks the edge from
/// its `a` end) in order round it, by the way each goes into its own
/// inside from the edge's middle. None where a face's way in cannot be
/// told (it runs along the edge).
fn around(model: &Model, e: EdgeId, fs: &[(FaceId, bool)]) -> Option<Vec<(FaceId, bool)>> {
    let x = model.edge(e)?;
    let (pa, pb) = (model.vertex(x.a), model.vertex(x.b));
    let p = x.curve.point(pa, pb, 0.5);
    let along =
        (x.curve.point(pa, pb, 0.5 + 1e-4) - x.curve.point(pa, pb, 0.5 - 1e-4)).normalize_or_zero();
    let u = along.any_orthonormal_vector();
    let v = along.cross(u);
    let mut out: Vec<(f64, FaceId, bool)> = Vec::new();
    for (f, forward) in fs {
        let n = model.face(*f)?.surface.normal_at(p);
        // A face's inside is to the left of the way it walks its edge,
        // about its normal.
        let walk = if *forward { along } else { -along };
        let into = n.cross(walk);
        let flat = into - along * into.dot(along);
        if flat.length_squared() < 1e-18 {
            return None;
        }
        out.push((flat.dot(v).atan2(flat.dot(u)), *f, *forward));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    Some(out.into_iter().map(|(_, f, w)| (f, w)).collect())
}

/// Flags of the solid `from` carried onto `to`, a boolean of it with
/// another: a face of `to` lying on a flagged face's surface, within its
/// box, is flagged for the same reason, and so are its edges and points
/// along a flagged edge or at a flagged point.
fn carry(flags: &Flags, from: &Model, to: &Model, tol: &Tol) -> Flags {
    let mut out = Flags::new();
    for (why, found) in flags {
        let at = out.entry(why).or_default();
        for f in &found.faces {
            let Some(face) = from.face(*f) else { continue };
            let pts: Vec<DVec3> = face.loops().flatten().map(|v| from.vertex(*v)).collect();
            let (lo, hi) = pts.iter().fold(
                (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
                |(a, b), p| (a.min(*p), b.max(*p)),
            );
            let (lo, hi) = (lo - DVec3::splat(tol.dist), hi + DVec3::splat(tol.dist));
            for (id, g) in to.faces() {
                let on = g.loops().flatten().all(|v| {
                    let q = to.vertex(*v);
                    q.cmpge(lo).all()
                        && q.cmple(hi).all()
                        && face.surface.project(q).distance(q) <= tol.dist
                });
                if on {
                    at.faces.insert(id);
                }
            }
        }
        for e in &found.edges {
            let Some(x) = from.edge(*e) else { continue };
            let (pa, pb) = (from.vertex(x.a), from.vertex(x.b));
            let on = |q: DVec3| x.curve.closest_point(pa, pb, q).distance(q) <= tol.dist;
            for (id, y) in to.edges() {
                let (qa, qb) = (to.vertex(y.a), to.vertex(y.b));
                if on(qa) && on(qb) && on(y.curve.point(qa, qb, 0.5)) {
                    at.edges.insert(id);
                }
            }
        }
        for v in &found.vertices {
            let p = from.vertex(*v);
            at.vertices.extend(
                to.vertices()
                    .filter(|(_, q)| tol.same_point(p, *q))
                    .map(|(id, _)| id),
            );
        }
    }
    out
}

/// The slab of the (agreeing) faces, with each rim style as given.
fn built(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
    styles: &RimStyles,
    marks: &mut Marks,
) -> Result<(Model, BTreeSet<FaceId>), &'static str> {
    if let Some(r) = branched(model, faces, open, d, styles, marks) {
        return r;
    }
    let styled = styles.values().any(|s| *s != RimStyle::Square);
    if styled {
        // A lone freeform face takes a chosen style too, by the same route
        // (the program does not pick one for it: `default_rims_from`).
        return freeform_joined(model, faces, open, d, styles, marks).unwrap_or(Err(RIM_STYLE));
    }
    if let Some(r) = freeform_alone(model, faces, open, d) {
        return r;
    }
    if let Some(r) = freeform_joined(model, faces, open, d, styles, marks) {
        return r;
    }
    let plan = plan(model, faces, open, d)?;
    let tol = model.tol();
    let mut s = Model::with_tol(tol);
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
        let low = s.add_face(low);
        marks.copied(*id, low);
        let high = s.add_face(high);
        marks.copied(*id, high);
    }
    // The small face at each split corner, walked against the faces round
    // it (as a moved copy is, so it turns round with them).
    for (v, fan) in plan
        .splits
        .iter()
        .filter(|(v, _)| !plan.rim_splits.contains(v))
    {
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
    // A line up from each point of a rim (to where that rim ends there:
    // at a rim's split point each rim has its own).
    let mut rise: BTreeMap<(VertexId, VertexId), EdgeId> = BTreeMap::new();
    for (id, a, b, _) in &plan.rims {
        for v in [a, b] {
            let top = end(*v, *id);
            if let std::collections::btree_map::Entry::Vacant(at) = rise.entry((*v, top)) {
                at.insert(edge(&mut s, base[v], top, Curve::Line, None)?);
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
        let (up_p, up_q) = (end(p, *id), end(q, *id));
        let (rise_p, rise_q) = (rise[&(p, up_p)], rise[&(q, up_q)]);
        match &e.curve {
            Curve::Line => {
                let pts = [base[&p], base[&q], up_q, up_p];
                if flat_plane(&s, &pts, &tol).is_some() {
                    let plane = Plane::new(s.vertex(pts[0]), loop_normal(&s, &pts).normalize());
                    let wall = s.add_face(Face::with_edges(
                        pts.to_vec(),
                        vec![],
                        Surface::Plane(plane),
                        vec![vec![lo, rise_q, hi, rise_p]],
                    ));
                    marks.walled(&s, *id, wall);
                } else {
                    let diag = edge(&mut s, pts[0], pts[2], Curve::Line, None)?;
                    for (l, es) in [
                        ([pts[0], pts[1], pts[2]], [lo, rise_q, diag]),
                        ([pts[0], pts[2], pts[3]], [diag, hi, rise_p]),
                    ] {
                        let plane = Plane::new(s.vertex(l[0]), loop_normal(&s, &l).normalize());
                        let wall = s.add_face(Face::with_edges(
                            l.to_vec(),
                            vec![],
                            Surface::Plane(plane),
                            vec![es.to_vec()],
                        ));
                        marks.walled(&s, *id, wall);
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
                let wall = s.add_face(Face::with_edges(
                    vec![base[&p], base[&q], up_q, up_p],
                    vec![],
                    wall,
                    vec![vec![lo, rise_q, hi, rise_p]],
                ));
                marks.walled(&s, *id, wall);
            }
            Curve::Spline(_) => return Err(FREEFORM),
        }
    }
    // At a rim's split point, a small triangle between the two rims' walls
    // and the short edge the faces' copies made there, walked against the
    // wall beside it.
    for v in &plan.rim_splits {
        let shorts_here: Vec<EdgeId> = shorts
            .iter()
            .filter(|((u, _), x)| u == v && x.is_some())
            .filter_map(|(_, x)| *x)
            .collect();
        let tops: Vec<(VertexId, EdgeId)> = rise
            .iter()
            .filter(|((u, _), _)| u == v)
            .map(|((_, t), e)| (*t, *e))
            .collect();
        let ([short], [(ta, ra), (tb, rb)]) = (shorts_here.as_slice(), tops.as_slice()) else {
            return Err(CORNER);
        };
        let pts = [s.vertex(base[v]), s.vertex(*ta), s.vertex(*tb)];
        let n = newell(&pts).normalize_or_zero();
        let mut cap = Face::with_edges(
            vec![base[v], *ta, *tb],
            vec![],
            Surface::Plane(Plane::new(pts[0], n)),
            vec![vec![*ra, *short, *rb]],
        );
        // The wall beside it walks its rise from the base up: the cap goes
        // the other way.
        let wall_up = s
            .faces()
            .any(|(_, f)| f.sides().any(|x| x.edge == *ra && x.from == base[v]));
        if wall_up {
            cap.reverse();
        }
        s.add_face(cap);
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
    marks: &mut Marks,
) -> Option<Result<(Model, BTreeSet<FaceId>), &'static str>> {
    let body = body(model, faces);
    let styled = styles.values().any(|s| *s != RimStyle::Square);
    if body.iter().any(|f| model.face(*f).is_none()) {
        return None;
    }
    // A face bounded by a freeform curve (flat or curved, alone or not)
    // is built this way too: its rim's offset is fitted through points
    // moved along the face's own normal, as a freeform face's is, and
    // walled the same way.
    let spline = body.iter().any(|f| {
        model.face(*f).is_some_and(|x| {
            x.edge_ids()
                .any(|e| model.edge(e).is_some_and(|c| c.curve.is_spline()))
        })
    });
    if body.len() < 2 && !(styled && body.len() == 1) && !spline {
        return None;
    }
    if !spline
        && !body
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
    let mut kept = marks.clone();
    for reach in REACHES {
        kept = marks.clone();
        result = freeform_slab(model, &body, open, d, styles, reach, &mut kept);
        wanted |= matches!(result, Err(FLUSH_REACH));
        if !styled || !matches!(result, Err(FLUSH_REACH)) {
            break;
        }
    }
    *marks = kept;
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
    marks: &mut Marks,
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
        let kept_uses: Vec<_> = u.iter().filter(|x| x.0).collect();
        if kept_uses.len() > 2 {
            return Err(BRANCHES);
        }
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
        let _ = (from, to);
        let f = model.face(*face).ok_or(NOTHING)?;
        flush_fits(model, edge_data, f, &plane, &tol)?;
        flush.insert(*e, plane);
        for v in [edge_data.a, edge_data.b] {
            let planes = flush_at.entry(v).or_default();
            if !planes.contains(&plane) {
                planes.push(plane);
            }
        }
    }
    let mut s = Model::with_tol(tol);
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
    // Rim corners of two faces folded right back onto each other: no
    // shared corner at all (their offsets are parallel there). Each face
    // keeps its own, split below where the fold is bridged.
    let mut folds: BTreeSet<VertexId> = BTreeSet::new();
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
                Err(CORNER)
                    if fs.len() == 2 && on_rim.contains(v) && opposed(model, fs, p, &tol) =>
                {
                    folds.insert(*v);
                    let n = model.face(fs[0]).ok_or(NOTHING)?.surface.normal_at(p);
                    polish_corner(&offsets, &fs[..1], p + n * d, &tol)
                }
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
    // Creases are classified inside or outside (part 2b): an outside
    // corner's mitre would reach past one face's own edge, which a NURBS
    // surface cannot extend to meet. A fold right back on itself (the two
    // faces' normals opposite somewhere along it) has no mitre at all:
    // the offsets are parallel there and never meet. Thickened away from
    // each other, that is a gap like any outside corner's, and is
    // bridged the same way (`folded`: through a middle line, so the
    // bridge keeps a thickness at the fold rather than pass through it).
    // Thickened into each other, the offsets cross well away from the
    // fold, not along it: refused by name.
    let mut outside: BTreeMap<EdgeId, bool> = BTreeMap::new();
    let mut folded: BTreeSet<EdgeId> = BTreeSet::new();
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
            if into_fold(model, *e, *fa, *fb, d, &tol)? {
                return Err(SHARP);
            }
            outside.insert(*e, true);
            folded.insert(*e);
            continue;
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
            // At a fold right back, the corner is the first face's own
            // point: the other face parts from it.
            let bad: Vec<FaceId> = if folds.contains(&v) {
                at_vertex[&v][1..].to_vec()
            } else {
                at_vertex[&v]
                    .iter()
                    .copied()
                    .filter(|f| !truly_on(&offsets[f], moved[&v], &tol))
                    .collect()
            };
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
            let span = (pb - pa).length_squared().max(prism_math::tol::NONZERO);
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
        let span = (pb - pa).length_squared().max(prism_math::tol::NONZERO);
        let mut breaks = match face_a.surface.patch() {
            Some(patch) => crate::thicken::kinks(&edge_data.curve, &along, patch),
            None => vec![0.0, 1.0],
        };
        if let Some(patch) = face_b.surface.patch() {
            breaks.extend(crate::thicken::kinks(&edge_data.curve, &along, patch));
            breaks.sort_by(f64::total_cmp);
            breaks.dedup_by(|x, y| (*x - *y).abs() < prism_math::tol::PARAM);
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
            // A fold right back (`folded`): the bridge goes out through a
            // middle line, the thickness out from the fold half way round
            // between the two faces' offsets, so it does not pass through
            // the fold itself. At an end where both faces still share one
            // point, the middle line ends there too.
            let mid = if folded.contains(e) {
                let fa_ahead = face_a
                    .sides()
                    .any(|x| x.edge == *e && x.from == edge_data.a);
                let out = |t: f64| {
                    let (t0, t1) = ((t - 1e-4).max(0.0), (t + 1e-4).min(1.0));
                    let ahead = (along(t1) - along(t0)).normalize_or_zero()
                        * if fa_ahead { 1.0 } else { -1.0 };
                    let p = along(t);
                    let (na, nb) = (face_a.surface.normal_at(p), face_b.surface.normal_at(p));
                    // Half way round from one offset to the other: away
                    // from both faces (past the fold) and, where they are
                    // not quite folded flat, between their normals.
                    ((na + nb) * d.signum() - na.cross(ahead) + nb.cross(ahead)).normalize_or_zero()
                };
                let mut ends: Vec<(VertexId, DVec3)> = Vec::new();
                for (v, t) in [(edge_data.a, 0.0), (edge_data.b, 1.0)] {
                    let w = out(t);
                    if w == DVec3::ZERO {
                        return Err(TOO_THICK);
                    }
                    ends.push(if vertex_for(v, *fa) == vertex_for(v, *fb) {
                        (vertex_for(v, *fa), point_for(v, *fa))
                    } else {
                        let q = model.vertex(v) + w * d.abs();
                        (s.add_vertex(q), q)
                    });
                }
                let (corr_a, corr_b) = (
                    ends[0].1 - (pa + out(0.0) * d.abs()),
                    ends[1].1 - (pb + out(1.0) * d.abs()),
                );
                let line = |t: f64| {
                    let p = along(t);
                    let bt = ((p - pa).dot(pb - pa) / span).clamp(0.0, 1.0);
                    p + out(t) * d.abs() + corr_a * (1.0 - bt) + corr_b * bt
                };
                let (high, _) =
                    crate::thicken::twin_fit(&line, &line, &breaks, eps).ok_or(TOO_THICK)?;
                let top = edge(
                    &mut s,
                    ends[0].0,
                    ends[1].0,
                    Curve::Spline(Spline::whole(high.clone())),
                    None,
                )?;
                Some(Mid {
                    top,
                    high,
                    at: [ends[0].0, ends[1].0],
                })
            } else {
                None
            };
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
                mid,
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
        let low = s.add_face(low);
        marks.copied(*id, low);
        let high = s.add_face(high);
        marks.copied(*id, high);
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
        let wall = s.add_face(Face::with_edges(
            loop_v,
            vec![],
            crate::thicken::flat(&ruled, eps).unwrap_or(Surface::Nurbs(ruled)),
            vec![loop_e],
        ));
        marks.walled(&s, *e_id, wall);
    }
    // Each outside corner's own bridge (part 2b): a ruled face between
    // the two faces' top edges, closed at either end that splits by
    // `end_cap` (a triangle where both of this bridge's own
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
        // Walked against the moved copy of `fa` beside it (which goes
        // along `ta` the way `fa` goes along the crease, turned round
        // with it where the copy is), so each edge the bridge shares is
        // walked once each way.
        let fa_ahead = model
            .face(br.fa)
            .ok_or(NOTHING)?
            .sides()
            .any(|x| x.edge == br.crease && x.from == br.ea);
        let turn = fa_ahead == (d > 0.0);
        let rise_of = |v: VertexId, f: FaceId| {
            rise.get(&(v, split.contains_key(&(v, f)).then_some(f)))
                .copied()
        };
        let side = |f: FaceId, top: EdgeId, curve: &Nurbs| Side {
            top,
            curve: curve.clone(),
            ends: [
                (vertex_for(br.ea, f), rise_of(br.ea, f)),
                (vertex_for(br.eb, f), rise_of(br.eb, f)),
            ],
        };
        let (a, b) = (
            side(br.fa, br.ta, &br.high_a),
            side(br.fb, br.tb, &br.high_b),
        );
        let Some(mid) = br.mid else {
            strip(
                &mut s,
                &a,
                &b,
                [br.ea, br.eb],
                tangent,
                turn,
                &base,
                &mut asym_cap,
                eps,
            )?;
            continue;
        };
        // A fold right back: two strips, out to the middle line and back.
        // Where the line ends at a point of its own on a rim, a riser
        // goes up to it, as to each face's own corner; off a rim there is
        // no corner face to close it against yet.
        let mut ends = [(mid.at[0], None), (mid.at[1], None)];
        for (k, v) in [br.ea, br.eb].into_iter().enumerate() {
            if mid.at[k] == a.ends[k].0 {
                continue;
            }
            if a.ends[k].1.is_none() || b.ends[k].1.is_none() {
                return Err(SHARP);
            }
            ends[k].1 = Some(edge(&mut s, base[&v], mid.at[k], Curve::Line, None)?);
        }
        let m = Side {
            top: mid.top,
            curve: mid.high.clone(),
            ends,
        };
        let alike =
            Nurbs::alike(&[a.curve.clone(), m.curve.clone(), b.curve.clone()]).ok_or(TOO_THICK)?;
        let [ca, cm, cb] = &alike[..] else {
            return Err(TOO_THICK);
        };
        let (a, m, b) = (
            Side {
                curve: ca.clone(),
                ..a
            },
            Side {
                curve: cm.clone(),
                ..m
            },
            Side {
                curve: cb.clone(),
                ..b
            },
        );
        let before = s.faces().map(|(id, _)| id).collect::<BTreeSet<_>>();
        strip(
            &mut s,
            &a,
            &m,
            [br.ea, br.eb],
            tangent,
            turn,
            &base,
            &mut asym_cap,
            eps,
        )?;
        strip(
            &mut s,
            &m,
            &b,
            [br.ea, br.eb],
            tangent,
            turn,
            &base,
            &mut asym_cap,
            eps,
        )?;
        let at = marks.flags.entry(BRIDGED).or_default();
        for id in s
            .faces()
            .map(|(id, _)| id)
            .filter(|id| !before.contains(id))
        {
            at.faces.insert(id);
            at.edges
                .extend(s.face(id).into_iter().flat_map(|f| f.edge_ids()));
        }
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
    /// A fold right back's middle line, which it is bridged through.
    mid: Option<Mid>,
}

/// The middle line a fold right back is bridged through: its edge, its
/// curve, and the points it ends at (at the crease's `a` end, then `b`).
struct Mid {
    top: EdgeId,
    high: Nurbs,
    at: [VertexId; 2],
}

/// One side of a bridge: its top edge and curve, and at each end of the
/// crease (`a`, then `b`) the point it ends at and the riser up to it
/// there (None off a rim).
#[derive(Clone)]
struct Side {
    top: EdgeId,
    curve: Nurbs,
    ends: [(VertexId, Option<EdgeId>); 2],
}

/// One bridge face between the sides `a` and `b` of the crease from
/// `ends[0]` to `ends[1]` (`tangent` along it), each end closed by
/// `end_cap`; turned round where `turn`, then given the winding its own
/// loop has.
#[allow(clippy::too_many_arguments)]
fn strip(
    s: &mut Model,
    a: &Side,
    b: &Side,
    ends: [VertexId; 2],
    tangent: DVec3,
    turn: bool,
    base: &BTreeMap<VertexId, VertexId>,
    asym_cap: &mut BTreeMap<(VertexId, VertexId), EdgeId>,
    eps: f64,
) -> Result<FaceId, &'static str> {
    use prism_geom::Patch;
    let mut loop_v = vec![a.ends[0].0, a.ends[1].0];
    let mut loop_e = vec![a.top];
    if let Some(cap) = end_cap(s, ends[1], a.ends[1], b.ends[1], tangent, base, asym_cap)? {
        loop_e.push(cap);
        loop_v.push(b.ends[1].0);
    }
    loop_e.push(b.top);
    if let Some(cap) = end_cap(s, ends[0], a.ends[0], b.ends[0], -tangent, base, asym_cap)? {
        loop_e.push(cap);
        loop_v.push(b.ends[0].0);
    }
    if turn {
        loop_v[1..].reverse();
        loop_e.reverse();
    }
    let raw = Patch::whole(crate::thicken::ruled(&a.curve, &b.curve).ok_or(TOO_THICK)?);
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
    Ok(s.add_face(Face::with_edges(
        loop_v,
        vec![],
        crate::thicken::flat(&bridge, eps).unwrap_or(Surface::Nurbs(bridge)),
        vec![loop_e],
    )))
}

/// Closes one end `v` of a bridge between the points `a` and `b` there
/// (each with its riser up from `v`, None off a rim): nothing where they
/// are one point; against both risers by `corner_triangle` on a rim; by a
/// plain edge off one (built once per pair of points, and shared by every
/// bridge, and the face closing a gap of three or more points, that meets
/// it). Returns the edge from `a` to `b`, if one is needed.
fn end_cap(
    s: &mut Model,
    v: VertexId,
    (pa_v, ra): (VertexId, Option<EdgeId>),
    (pb_v, rb): (VertexId, Option<EdgeId>),
    out_ref: DVec3,
    base: &BTreeMap<VertexId, VertexId>,
    asym_cap: &mut BTreeMap<(VertexId, VertexId), EdgeId>,
) -> Result<Option<EdgeId>, &'static str> {
    if pa_v == pb_v {
        return Ok(None);
    }
    match (ra, rb) {
        (Some(ra), Some(rb)) => Ok(Some(corner_triangle(
            s, v, ra, rb, pa_v, pb_v, out_ref, base,
        )?)),
        (None, None) => {
            let key = (pa_v.min(pb_v), pa_v.max(pb_v));
            Ok(Some(match asym_cap.get(&key) {
                Some(e) => *e,
                None => {
                    let e = edge(s, pa_v, pb_v, Curve::Line, None)?;
                    asym_cap.insert(key, e);
                    e
                }
            }))
        }
        _ => Err(TOO_THICK),
    }
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

/// Whether two of the faces `fs` face exactly opposite ways at `p` (a
/// fold right back on itself).
fn opposed(model: &Model, fs: &[FaceId], p: DVec3, tol: &Tol) -> bool {
    let ns: Vec<DVec3> = fs
        .iter()
        .filter_map(|f| model.face(*f).map(|x| x.surface.normal_at(p)))
        .collect();
    ns.iter().enumerate().any(|(i, a)| {
        ns[i + 1..]
            .iter()
            .any(|b| tol.parallel(*a, *b) && a.dot(*b) < 0.0)
    })
}

/// Whether, where the crease `e` between `fa` and `fb` folds right back,
/// thickening by `d` moves either face towards the other (so their
/// offsets cross somewhere past the fold, not along it). Each face is
/// looked at a thickness in from the fold, across from the other's
/// normal: where they lie on each other exactly, neither moves towards
/// the other (one moves each way), and the gap between them is bridged.
fn into_fold(
    model: &Model,
    e: EdgeId,
    fa: FaceId,
    fb: FaceId,
    d: f64,
    tol: &Tol,
) -> Result<bool, &'static str> {
    let edge_data = model.edge(e).ok_or(NOTHING)?;
    let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
    let (face_a, face_b) = (
        model.face(fa).ok_or(NOTHING)?,
        model.face(fb).ok_or(NOTHING)?,
    );
    // The way `fa` walks the edge (`fb` walks it the other way).
    let a_ahead = face_a.sides().any(|x| x.edge == e && x.from == edge_data.a);
    for i in 0..=8 {
        let t = f64::from(i) / 8.0;
        let p = edge_data.curve.point(pa, pb, t);
        let (na, nb) = (face_a.surface.normal_at(p), face_b.surface.normal_at(p));
        if !(tol.parallel(na, nb) && na.dot(nb) < 0.0) {
            continue;
        }
        let (t0, t1) = ((t - 1e-4).max(0.0), (t + 1e-4).min(1.0));
        let ahead = (edge_data.curve.point(pa, pb, t1) - edge_data.curve.point(pa, pb, t0))
            .normalize_or_zero()
            * if a_ahead { 1.0 } else { -1.0 };
        // A face's inside is to the left of the way it walks its edge,
        // about its normal.
        let into_a = na.cross(ahead);
        let into_b = nb.cross(-ahead);
        let qa = face_a.surface.project(p + into_a * d.abs());
        let qb = face_b.surface.project(p + into_b * d.abs());
        // How far each rises towards the way the other thickens: a
        // distance, judged by the tolerance (it was multiplied by `d`, an
        // area against a distance, so a fold seen at 0.001 mm was missed
        // at 0.01 mm: tolerance stage 2).
        let s = d.signum();
        if (qb - p).dot(na) * s > tol.dist || (qa - p).dot(nb) * s > tol.dist {
            return Ok(true);
        }
    }
    Ok(false)
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
        let kept: Vec<_> = u.iter().filter(|x| x.0).collect();
        if kept.len() > 2 {
            return Err(BRANCHES);
        }
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
    // Each kept face's moved surface: a corner where a curved face meets
    // others is polished onto them (the flat corner from the normals alone
    // is off a cylinder's offset where its floor and side cut it, as at
    // the foot of a half-round wheel well).
    let mut offsets: BTreeMap<FaceId, Surface> = BTreeMap::new();
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        offsets.insert(*id, offset_surface(&f.surface, d, &tol).ok_or(TOO_THICK)?);
    }
    let mut moved = BTreeMap::new();
    let mut splits = BTreeMap::new();
    let mut rim_splits = BTreeSet::new();
    for (v, fs) in &at_vertex {
        match move_point(model, fs, model.vertex(*v), d, &tol) {
            Ok(p) => {
                let curved = fs
                    .iter()
                    .any(|f| model.face(*f).is_some_and(|x| x.is_curved()));
                if !curved
                    && on_rim.contains(v)
                    && let Some(fan) = rim_ends(model, &rims, *v, fs, d, &tol)
                {
                    splits.insert(*v, fan);
                    rim_splits.insert(*v);
                    continue;
                }
                let p = if curved {
                    polish_corner(&offsets, fs, p, &tol)
                } else {
                    p
                };
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
        let curved = fs
            .iter()
            .any(|f| model.face(*f).is_some_and(|x| x.is_curved()));
        let at = |t: f64| {
            move_point(model, fs, x.curve.point(pa, pb, t), d, &tol).map(|q| {
                if curved {
                    polish_corner(&offsets, fs, q, &tol)
                } else {
                    q
                }
            })
        };
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
        rim_splits,
        curves,
        rims,
    })
}

/// Where each edge at a point `v` of a rim ends, when the rims there end
/// apart. Two faces meet at `v` along a crease (their offsets meet on a
/// line, so the point is free along it), each with its own rim: each rim's
/// moved copy ends where its square wall (through the rim and its face's
/// normal) crosses that line. Where the rims' walls agree (a box's open
/// top: both upright) that is one point, the one [`move_point`] gives, and
/// this is None. Where they do not (owner's truck cab, 2026-11-26: the top
/// of a leaning windscreen beside an upright side, its wall leaning with
/// it), each rim ends at its own point, the crease at the first rim's, and
/// a small triangle closes the gap (`built`). None too where the point is
/// not free along a line, or a rim there is not straight.
fn rim_ends(
    model: &Model,
    rims: &[(EdgeId, VertexId, VertexId, FaceId)],
    v: VertexId,
    fs: &[FaceId],
    d: f64,
    tol: &Tol,
) -> Option<Vec<(EdgeId, DVec3)>> {
    let p = model.vertex(v);
    let ns: Vec<DVec3> = fs
        .iter()
        .filter_map(|f| model.face(*f).map(|x| x.surface.normal_at(p)))
        .collect();
    let ns = unique_normals(&ns, tol);
    if ns.len() != 2 {
        return None;
    }
    let at_v: Vec<_> = rims.iter().filter(|r| r.1 == v || r.2 == v).collect();
    if at_v.len() != 2 {
        return None;
    }
    let mut out: Vec<(EdgeId, DVec3)> = Vec::new();
    for (e, _, _, f) in at_v {
        let x = model.edge(*e)?;
        if x.curve != Curve::Line {
            return None;
        }
        let other = if x.a == v { x.b } else { x.a };
        let along = (model.vertex(other) - p).normalize_or_zero();
        let n = model.face(*f)?.surface.normal_at(p);
        let wall = along.cross(n).normalize_or_zero();
        if wall == DVec3::ZERO {
            return None;
        }
        let mut want: Vec<(DVec3, f64)> = ns.iter().map(|n| (*n, d)).collect();
        want.push((wall, 0.0));
        out.push((*e, p + corner_mixed(&want, tol)?));
    }
    if tol.same_point(out[0].1, out[1].1) {
        return None;
    }
    // The crease (and any other edge of these faces at `v`) ends at the
    // first rim's point: every rim end is on the line both offsets share.
    let first = out[0].1;
    for f in fs {
        for side in model.face(*f)?.sides() {
            if (side.from == v || side.to == v) && !out.iter().any(|(e, _)| *e == side.edge) {
                out.push((side.edge, first));
            }
        }
    }
    Some(out)
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
        if g.abs() < prism_math::tol::GUARD {
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
