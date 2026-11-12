//! Follow Me (Exact Curves 5c and 5d): a profile face swept along a path
//! of lines and arcs, as in SketchUp. The profile need not touch the path.
//! It is brought square to the path's start (projected along a first
//! line, or turned about a first arc's axis), carried along each line to
//! the corner's mitre plane (the plane halving the corner), turned about
//! each arc's axis, and ends square to the path's end. A closed path makes
//! a ring with no ends.
//!
//! Along lines, profile lines make flat walls and profile arcs cylinders
//! (only on a path with no corners: at a mitre an arc would be an ellipse,
//! DECISIONS #32). Along arcs, the profile lies in a plane through the
//! arc's axis: its lines make flat rings, cylinders or cones, and its arcs
//! make tori, or spheres when centred on the axis. Where the path meets an
//! arc the two must be tangent.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Cone, Curve, Cylinder, Sphere, Surface, Torus};
use prism_math::{DAffine3, DVec3, Plane, Tol};
use prism_topo::{CurveId, EdgeId, Face, FaceId, Model, VertexId};

/// Said with no path selected.
pub const NO_PATH: &str =
    "Select a path first (lines and arcs joined end to end), then click the profile face.";
/// Said when the selection is not one chain of lines and arcs.
pub const NOT_A_PATH: &str = "The path must be lines and arcs joined end to end in one chain.";
/// Said when the profile is not a flat face.
/// Said when the path has a freeform curve (until Freeform NURBS part 5).
pub const FREEFORM_PATH: &str = "Refused: Follow Me along a freeform curve is not built yet.";
/// Said when the profile has a freeform curve (until Freeform NURBS part 4).
pub const FREEFORM_PROFILE: &str =
    "Refused: Follow Me with a freeform curve in the profile is not built yet.";
pub const NOT_A_PROFILE: &str = "The profile must be a flat face.";
/// Said when the profile lies along the path's first line.
pub const ALONG_PATH: &str = "The profile lies along the path's first line: it must cross it.";
/// Said when a profile away from the path's start cannot be moved there.
pub const NOT_SQUARE: &str = "The profile is not square to the path where it starts, and it is joined to other faces (or has arcs), so it cannot be moved there.";
/// Said when a profile with arcs meets a corner or is tilted.
pub const ARCS: &str = "A profile with arcs cannot turn a sharp corner of the path or be tilted to it: its arcs would become ellipses.";
/// Said when a corner is too sharp for the profile's width.
pub const CROSSES_ITSELF: &str =
    "The profile is too wide for a corner of this path: the sweep would cross itself.";
/// Said when the sweep would run into itself further along the path.
pub const OVERLAPS: &str =
    "The sweep would run into itself further along the path: the profile is too wide for it.";
/// Said when a closed path does not bring the profile back to its start.
pub const RING_TWISTS: &str =
    "Round this closed path the profile would not come back to where it started.";
/// Said when the path turns a corner where it meets an arc.
pub const NOT_TANGENT: &str = "Where the path meets an arc it must turn smoothly (tangent): a corner there would need an ellipse.";
/// Said when a profile on an arc of the path is not in a plane through
/// the arc's axis.
pub const OFF_AXIS: &str =
    "On an arc of the path, the profile must lie in a plane through the arc's centre and axis.";
/// Said when a profile on an arc of the path reaches across its axis.
pub const CROSSES_AXIS: &str =
    "On an arc of the path, the profile must stay on one side of the arc's axis.";

/// The path's points in order, its pieces (piece `k` joins point `k` to
/// point `k + 1`, the last back to the first when it closes), and whether
/// it closes: `path` must be lines and arcs joined end to end in one chain.
pub fn chain(model: &Model, path: &BTreeSet<EdgeId>) -> Option<(Vec<VertexId>, Vec<EdgeId>, bool)> {
    let mut adj: BTreeMap<VertexId, Vec<(EdgeId, VertexId)>> = BTreeMap::new();
    for id in path {
        let e = model.edge(*id)?;
        if e.a == e.b {
            return None;
        }
        adj.entry(e.a).or_default().push((*id, e.b));
        adj.entry(e.b).or_default().push((*id, e.a));
    }
    if adj.is_empty() || adj.values().any(|n| n.len() > 2) {
        return None;
    }
    let ends: Vec<VertexId> = adj
        .iter()
        .filter(|(_, n)| n.len() == 1)
        .map(|(v, _)| *v)
        .collect();
    let closed = match ends.len() {
        0 => true,
        2 => false,
        _ => return None,
    };
    let start = if closed { *adj.keys().next()? } else { ends[0] };
    let mut vs = vec![start];
    let mut es: Vec<EdgeId> = Vec::new();
    let mut cur = start;
    loop {
        let step = adj[&cur].iter().copied().find(|(e, _)| !es.contains(e));
        let Some((e, next)) = step else { break };
        es.push(e);
        if next == start {
            break;
        }
        if vs.contains(&next) {
            return None;
        }
        vs.push(next);
        cur = next;
    }
    // One chain: every vertex and every piece reached.
    let pieces = if closed { vs.len() } else { vs.len() - 1 };
    (vs.len() == adj.len() && es.len() == path.len() && es.len() == pieces)
        .then_some((vs, es, closed))
}

/// One piece of the path.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// Along a unit direction.
    Line(DVec3),
    /// Turning by `angle` (positive) counter-clockwise about the unit
    /// `axis` through `centre`.
    Arc {
        centre: DVec3,
        axis: DVec3,
        angle: f64,
        curve: Option<CurveId>,
    },
}

impl Step {
    /// The direction a point at `p` moves in.
    fn tangent(&self, p: DVec3) -> DVec3 {
        match *self {
            Step::Line(d) => d,
            Step::Arc { centre, axis, .. } => axis.cross(p - centre).normalize_or_zero(),
        }
    }
}

/// Turning by `angle` about the unit `axis` through `centre`.
fn turn(centre: DVec3, axis: DVec3, angle: f64) -> DAffine3 {
    DAffine3::from_translation(centre)
        * DAffine3::from_axis_angle(axis, angle)
        * DAffine3::from_translation(-centre)
}

/// `p` measured from the axis through `centre`: the part square to the
/// axis, and the height along it.
fn radial(centre: DVec3, axis: DVec3, p: DVec3) -> (DVec3, f64) {
    let q = p - centre;
    let h = q.dot(axis);
    (q - axis * h, h)
}

/// A profile loop: its vertices and edges.
struct ProfileLoop {
    vs: Vec<VertexId>,
    es: Vec<EdgeId>,
}

/// What a sweep makes: the profile's points at each section.
struct Plan {
    closed: bool,
    /// The path's pieces (lines in a row merged).
    steps: Vec<Step>,
    /// The profile's loops.
    loops: Vec<ProfileLoop>,
    /// Points of each loop at each section: `[section][loop][i]`. A closed
    /// path has one section per piece; an open one, one more.
    sections: Vec<Vec<Vec<DVec3>>>,
    /// The rigid motion taking the profile to each section, where there
    /// is one (none after a mitre or a projection).
    motions: Vec<Option<DAffine3>>,
    /// Whether the path turns smoothly at each section (its seam is soft).
    smooth: Vec<bool>,
    /// Whether the profile faces along the path.
    along: bool,
    /// The profile's vertices must move to the path's start.
    squares_up: bool,
    /// Pieces that are chords of a path arc followed as fine segments
    /// (DECISIONS #32): what they make is approximate.
    chords: Vec<bool>,
    /// Sections between two chords of one arc: their seams are soft,
    /// smooth and approximate.
    seams: Vec<bool>,
}

/// Plan a sweep. With `chords`, the path's arcs are followed as fine
/// segments (the fallback, when they cannot be turned about exactly).
fn plan(
    model: &Model,
    path: &BTreeSet<EdgeId>,
    profile: FaceId,
    chords: bool,
) -> Result<Plan, &'static str> {
    if path.is_empty() {
        return Err(NO_PATH);
    }
    let tol = model.tol();
    let (mut vs, mut es, closed) = chain(model, path).ok_or(NOT_A_PATH)?;
    let face = model.face(profile).ok_or(NOT_A_PROFILE)?;
    let pl = *face.plane().ok_or(NOT_A_PROFILE)?;
    // The profile's own edges are not a path.
    if face.edge_ids().any(|x| path.contains(&x)) {
        return Err(NOT_A_PATH);
    }
    let loops: Vec<ProfileLoop> = face
        .loops()
        .enumerate()
        .map(|(k, l)| ProfileLoop {
            vs: l.clone(),
            es: face.loop_edge_ids(k).to_vec(),
        })
        .collect();
    let orig: Vec<Vec<DVec3>> = loops
        .iter()
        .map(|l| l.vs.iter().map(|v| model.vertex(*v)).collect())
        .collect();
    let centre = orig[0].iter().copied().sum::<DVec3>() / orig[0].len() as f64;
    let arcs = loops
        .iter()
        .flat_map(|l| &l.es)
        .any(|e| model.edge(*e).is_some_and(|x| x.curve.is_curved()));

    // The path, starting at the end (or point) nearest the profile.
    let near = |v: &VertexId| model.vertex(*v).distance(centre);
    if closed {
        let k = (0..vs.len())
            .min_by(|a, b| near(&vs[*a]).total_cmp(&near(&vs[*b])))
            .unwrap_or(0);
        vs.rotate_left(k);
        es.rotate_left(k);
    } else if near(&vs[vs.len() - 1]) < near(&vs[0]) {
        vs.reverse();
        es.reverse();
    }
    let corners: Vec<DVec3> = vs.iter().map(|v| model.vertex(*v)).collect();
    let nc = corners.len();
    let mut pts: Vec<DVec3> = Vec::with_capacity(nc);
    let mut steps: Vec<Step> = Vec::with_capacity(es.len());
    // Which path piece each step comes from, when it is a chord of an arc.
    let mut from_arc: Vec<Option<usize>> = Vec::with_capacity(es.len());
    for (k, id) in es.iter().enumerate() {
        let e = model.edge(*id).ok_or(NOT_A_PATH)?;
        let (p, q) = (corners[k], corners[(k + 1) % nc]);
        match e.curve {
            Curve::Spline(_) => return Err(FREEFORM_PATH),
            Curve::Line => {
                pts.push(p);
                steps.push(Step::Line((q - p).normalize_or_zero()));
                from_arc.push(None);
            }
            Curve::Arc { centre, axis, .. } => {
                let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
                let angle = e.curve.sweep(pa, pb);
                if chords {
                    let n = crate::approx::fine_count(angle);
                    let fwd = e.a == vs[k];
                    let at = |i: usize| {
                        let t = i as f64 / n as f64;
                        e.curve.point(pa, pb, if fwd { t } else { 1.0 - t })
                    };
                    for i in 0..n {
                        let (x, y) = (
                            if i == 0 { p } else { at(i) },
                            if i + 1 == n { q } else { at(i + 1) },
                        );
                        pts.push(x);
                        steps.push(Step::Line((y - x).normalize_or_zero()));
                        from_arc.push(Some(k));
                    }
                } else {
                    pts.push(p);
                    steps.push(Step::Arc {
                        centre,
                        axis: if e.a == vs[k] { axis } else { -axis },
                        angle,
                        curve: e.curve_id,
                    });
                    from_arc.push(None);
                }
            }
        }
    }
    if !closed {
        pts.push(corners[nc - 1]);
    }
    // Lines in a row make one line.
    loop {
        let (n, lines) = (pts.len(), steps.len());
        let drop = (0..n).find(|&i| {
            if !closed && (i == 0 || i == n - 1) || lines <= if closed { 3 } else { 1 } {
                return false;
            }
            let (a, b) = ((i + lines - 1) % lines, i % lines);
            match (steps[a], steps[b]) {
                (Step::Line(x), Step::Line(y)) => {
                    from_arc[a].is_none()
                        && from_arc[b].is_none()
                        && tol.parallel(x, y)
                        && x.dot(y) > 0.0
                }
                _ => false,
            }
        });
        match drop {
            Some(i) => {
                pts.remove(i);
                steps.remove(i % lines);
                from_arc.remove(i % lines);
            }
            None => break,
        }
    }
    let (n, lines) = (pts.len(), steps.len());
    let start_t = |k: usize| steps[k].tangent(pts[k]);
    let end_t = |k: usize| steps[k].tangent(pts[(k + 1) % n]);
    // The section plane at path point `j` (a mitre at a corner of lines,
    // square to the path where it turns smoothly), and whether it is
    // smooth there.
    let join = |j: usize| -> Result<((DVec3, DVec3), bool), &'static str> {
        let (pk, k) = ((j + lines - 1) % lines, j % lines);
        let (a, b) = (end_t(pk), start_t(k));
        let p = pts[j % n];
        if tol.parallel(a, b) && a.dot(b) > 0.0 {
            return Ok(((p, b), true));
        }
        if !matches!(steps[pk], Step::Line(_)) || !matches!(steps[k], Step::Line(_)) {
            return Err(NOT_TANGENT);
        }
        (a + b)
            .try_normalize()
            .map(|m| ((p, m), false))
            .ok_or(CROSSES_ITSELF)
    };
    let mut smooth = vec![false; if closed { lines } else { lines + 1 }];
    for j in if closed { 0..lines } else { 1..lines } {
        smooth[j] = join(j)?.1;
    }
    // Carry a point along `d` to the plane through `p` with normal `m`.
    let carry = |x: DVec3, d: DVec3, (p, m): (DVec3, DVec3)| -> Option<(DVec3, f64)> {
        let den = d.dot(m);
        if den.abs() < 1e-12 {
            return None;
        }
        let t = (p - x).dot(m) / den;
        Some((x + d * t, t))
    };

    // The first section.
    let (first, motion0, normal0) = match steps[0] {
        Step::Line(d0) => {
            if tol.perpendicular(pl.normal, d0) {
                return Err(ALONG_PATH);
            }
            // On the first corner's plane (closed), or square to the first
            // line through the profile (open).
            // A profile beyond the first piece (a short chord of an arc)
            // starts at the path's start instead.
            let first_len = (pts[1 % n] - pts[0]).dot(d0);
            let plane = if closed {
                join(0)?.0
            } else if (centre - pts[0]).dot(d0) >= first_len - tol.dist && lines > 1 {
                (pts[0], d0)
            } else {
                (centre, d0)
            };
            let mut first = Vec::new();
            for l in &orig {
                let sec: Option<Vec<DVec3>> = l
                    .iter()
                    .map(|x| carry(*x, d0, plane).map(|c| c.0))
                    .collect();
                first.push(sec.ok_or(ALONG_PATH)?);
            }
            // A plain move when the profile is square to the line and
            // lands square to it.
            let m = (tol.parallel(pl.normal, d0) && tol.parallel(plane.1, d0))
                .then(|| DAffine3::from_translation(first[0][0] - orig[0][0]));
            (first, m, pl.normal)
        }
        Step::Arc {
            centre: c, axis, ..
        } => {
            // Turned about the axis to the path's start: the profile's
            // plane must hold the axis.
            if !tol.perpendicular(pl.normal, axis) || pl.distance(c).abs() > tol.dist {
                return Err(OFF_AXIS);
            }
            // Its side of the axis, from its points and arc middles (a
            // half disc has both points on the axis).
            let mids = loops.iter().flat_map(|l| &l.es).filter_map(|e| {
                let e = model.edge(*e)?;
                Some(e.curve.point(model.vertex(e.a), model.vertex(e.b), 0.5))
            });
            let far = orig
                .iter()
                .flatten()
                .copied()
                .chain(mids)
                .map(|p| radial(c, axis, p).0)
                .max_by(|a, b| a.length().total_cmp(&b.length()))
                .unwrap_or(DVec3::ZERO);
            let (from, to) = (
                far.try_normalize().ok_or(CROSSES_AXIS)?,
                radial(c, axis, pts[0]).0.normalize_or_zero(),
            );
            let angle = from.cross(to).dot(axis).atan2(from.dot(to));
            let m = turn(c, axis, angle);
            let first: Vec<Vec<DVec3>> = orig
                .iter()
                .map(|l| l.iter().map(|p| m.transform_point3(*p)).collect())
                .collect();
            (first, Some(m), m.transform_vector3(pl.normal))
        }
    };
    let t0 = start_t(0);
    if tol.perpendicular(normal0, t0) {
        return Err(ALONG_PATH);
    }
    let squares_up = !closed
        && orig
            .iter()
            .flatten()
            .zip(first.iter().flatten())
            .any(|(a, b)| !tol.same_point(*a, *b));
    if arcs && motion0.is_none() {
        return Err(ARCS);
    }
    if squares_up {
        let own: BTreeSet<VertexId> = loops.iter().flat_map(|l| l.vs.iter().copied()).collect();
        let shared = model
            .faces()
            .any(|(id, f)| id != profile && f.loops().flatten().any(|v| own.contains(v)));
        if shared || arcs {
            return Err(NOT_SQUARE);
        }
    }
    let mut sections = vec![first];
    let mut motions = vec![motion0];
    for (k, step) in steps.iter().enumerate() {
        let cur = &sections[k];
        let (next, motion) = match *step {
            Step::Line(d) => {
                let target = if !closed && k == lines - 1 {
                    (pts[n - 1], d)
                } else {
                    join(k + 1)?.0
                };
                let mut next = Vec::new();
                for l in cur {
                    let mut sec = Vec::new();
                    for x in l {
                        let (y, t) = carry(*x, d, target).ok_or(CROSSES_ITSELF)?;
                        if t <= tol.dist {
                            return Err(CROSSES_ITSELF);
                        }
                        sec.push(y);
                    }
                    next.push(sec);
                }
                let m = motions[k]
                    .filter(|x| {
                        tol.parallel(x.transform_vector3(pl.normal), d) && tol.parallel(target.1, d)
                    })
                    .map(|x| DAffine3::from_translation(next[0][0] - cur[0][0]) * x);
                (next, m)
            }
            Step::Arc {
                centre: c,
                axis,
                angle,
                ..
            } => {
                // Every point on the path's side of the axis (or on it).
                let side = radial(c, axis, pts[k]).0.normalize_or_zero();
                if cur
                    .iter()
                    .flatten()
                    .any(|p| radial(c, axis, *p).0.dot(side) < -tol.dist)
                {
                    return Err(CROSSES_AXIS);
                }
                // A profile arc can reach across between its ends (a
                // sphere's arc of more than half a circle, past the poles).
                let across = |li: usize, i: usize| -> bool {
                    let l = &loops[li];
                    let (Some(e), Some(mo)) = (model.edge(l.es[i]), motions[k]) else {
                        return false;
                    };
                    let Some(curve) = e.curve.transformed(&mo).filter(|x| x.is_arc()) else {
                        return false;
                    };
                    let curve = if e.a == l.vs[i] {
                        curve
                    } else {
                        curve.reversed()
                    };
                    let (p, q) = (cur[li][i], cur[li][(i + 1) % l.vs.len()]);
                    (1..64).any(|j| {
                        let x = curve.point(p, q, j as f64 / 64.0);
                        radial(c, axis, x).0.dot(side) < -tol.dist
                    })
                };
                if (0..loops.len()).any(|li| (0..loops[li].vs.len()).any(|i| across(li, i))) {
                    return Err(CROSSES_AXIS);
                }
                let m = turn(c, axis, angle);
                let next: Vec<Vec<DVec3>> = cur
                    .iter()
                    .map(|l| l.iter().map(|p| m.transform_point3(*p)).collect())
                    .collect();
                (next, motions[k].map(|x| m * x))
            }
        };
        sections.push(next);
        motions.push(motion);
    }
    if arcs && motions.iter().any(|m| m.is_none()) {
        return Err(ARCS);
    }
    if closed {
        let back = sections.pop().unwrap_or_default();
        motions.pop();
        let twist = back
            .iter()
            .flatten()
            .zip(sections[0].iter().flatten())
            .any(|(a, b)| !tol.same_point(*a, *b));
        if twist {
            return Err(RING_TWISTS);
        }
    }
    let nsec = sections.len();
    let seams: Vec<bool> = (0..nsec)
        .map(|j| {
            (closed || (j > 0 && j < lines))
                && from_arc[j % lines].is_some()
                && from_arc[j % lines] == from_arc[(j + lines - 1) % lines]
        })
        .collect();
    let plan = Plan {
        chords: from_arc.iter().map(Option::is_some).collect(),
        seams,
        closed,
        steps,
        loops,
        sections,
        motions,
        smooth,
        along: normal0.dot(t0) > 0.0,
        squares_up,
    };
    // Every side makes a surface Prism has.
    for k in 0..plan.steps.len() {
        for li in 0..plan.loops.len() {
            for i in 0..plan.loops[li].vs.len() {
                let (p, q, c) = side(model, &plan, k, li, i).ok_or(NOT_A_PROFILE)?;
                swept(&plan.steps[k], p, q, c, &tol)?;
            }
        }
    }
    Ok(plan)
}

/// Side `i` of loop `li` at section `k`: its ends, walked round the loop,
/// and its curve walked so.
fn side(
    model: &Model,
    plan: &Plan,
    k: usize,
    li: usize,
    i: usize,
) -> Option<(DVec3, DVec3, Curve)> {
    let l = &plan.loops[li];
    let m = l.vs.len();
    let (p, q) = (plan.sections[k][li][i], plan.sections[k][li][(i + 1) % m]);
    let e = model.edge(l.es[i])?;
    let curve = match e.curve.clone() {
        Curve::Line => Curve::Line,
        c => {
            let c = c.transformed(plan.motions[k].as_ref()?)?;
            if e.a == l.vs[i] { c } else { c.reversed() }
        }
    };
    Some((p, q, curve))
}

/// The surface a profile side from `p` to `q` (along `curve`) sweeps on
/// `step`, facing either way. None for a side on an arc's axis, which
/// sweeps nothing.
fn swept(
    step: &Step,
    p: DVec3,
    q: DVec3,
    curve: Curve,
    tol: &Tol,
) -> Result<Option<Surface>, &'static str> {
    let surface = match (*step, curve) {
        (_, Curve::Spline(_)) => return Err(FREEFORM_PROFILE),
        (Step::Line(d), Curve::Line) => Surface::Plane(Plane::new(
            p,
            (q - p).cross(d).try_normalize().ok_or(ALONG_PATH)?,
        )),
        (
            Step::Line(_),
            Curve::Arc {
                centre,
                axis,
                radius,
            },
        ) => Surface::Cylinder(Cylinder {
            origin: centre,
            axis,
            radius,
            outward: true,
        }),
        (Step::Arc { centre, axis, .. }, Curve::Line) => {
            let ((rp, hp), (rq, hq)) = (radial(centre, axis, p), radial(centre, axis, q));
            let (rp, rq) = (rp.length(), rq.length());
            if rp <= tol.dist && rq <= tol.dist {
                return Ok(None);
            }
            if (hp - hq).abs() <= tol.dist {
                Surface::Plane(Plane::new(p, axis))
            } else if (rp - rq).abs() <= tol.dist {
                Surface::Cylinder(Cylinder {
                    origin: centre,
                    axis,
                    radius: (rp + rq) / 2.0,
                    outward: true,
                })
            } else {
                // A cone, its apex where the line meets the axis.
                let h0 = hp + (hq - hp) * rp / (rp - rq);
                let (rm, hm) = ((rp + rq) / 2.0, (hp + hq) / 2.0 - h0);
                Surface::Cone(Cone {
                    apex: centre + axis * h0,
                    axis: axis * hm.signum(),
                    half_angle: rm.atan2(hm.abs()),
                    outward: true,
                })
            }
        }
        (
            Step::Arc { centre, axis, .. },
            Curve::Arc {
                centre: c, radius, ..
            },
        ) => {
            let (r, h) = radial(centre, axis, c);
            let foot = centre + axis * h;
            let major = r.length();
            if major <= tol.dist {
                Surface::Sphere(Sphere {
                    centre: foot,
                    axis,
                    radius,
                    outward: true,
                })
            } else if major <= radius + tol.dist {
                return Err(CROSSES_AXIS);
            } else {
                Surface::Torus(Torus {
                    centre: foot,
                    axis,
                    major,
                    minor: radius,
                    outward: true,
                })
            }
        }
    };
    if surface.is_valid() {
        Ok(Some(surface))
    } else {
        Err(CROSSES_AXIS)
    }
}

/// Refusals the fallback (DECISIONS #32) answers: arcs that cannot be
/// turned about or carried exactly.
const FALLBACK: [&str; 4] = [NOT_TANGENT, OFF_AXIS, ARCS, NOT_SQUARE];

/// Replace the profile's arcs by fine approximate segments.
fn segment_profile(model: &mut Model, profile: FaceId) {
    let arcs: Vec<EdgeId> = model
        .face(profile)
        .map(|f| {
            f.edge_ids()
                .filter(|e| model.edge(*e).is_some_and(|x| x.curve.is_curved()))
                .collect()
        })
        .unwrap_or_default();
    crate::approx::segment_arcs(model, &arcs);
}

/// The first way the sweep can be made: exactly; with the path's arcs as
/// fine segments; with the profile's arcs as fine segments; with both.
/// Returns (path arcs as segments, profile arcs as segments).
fn best(
    model: &Model,
    path: &BTreeSet<EdgeId>,
    profile: FaceId,
) -> Result<(bool, bool), &'static str> {
    let mut err = match plan(model, path, profile, false) {
        Ok(_) => return Ok((false, false)),
        Err(e) => e,
    };
    if !FALLBACK.contains(&err) {
        return Err(err);
    }
    match plan(model, path, profile, true) {
        Ok(_) => return Ok((true, false)),
        Err(e) => err = e,
    }
    let arcs = model.face(profile).is_some_and(|f| {
        f.edge_ids()
            .any(|e| model.edge(e).is_some_and(|x| x.curve.is_curved()))
    });
    if arcs {
        let mut m = model.clone();
        segment_profile(&mut m, profile);
        for chords in [false, true] {
            match plan(&m, path, profile, chords) {
                Ok(_) => return Ok((chords, true)),
                Err(e) => err = e,
            }
        }
    }
    Err(err)
}

/// The last sweep checked: the model before, the path and profile, and
/// the model after (None when refused). The tools check a sweep and then
/// run it, so `follow` reuses what `refusal` built rather than building
/// it twice. Same inputs, same result: it only saves the work.
type Swept = (
    Model,
    BTreeSet<EdgeId>,
    FaceId,
    Result<Model, Option<&'static str>>,
);

thread_local! {
    static LAST: std::cell::RefCell<Option<Swept>> = const { std::cell::RefCell::new(None) };
}

/// Build the sweep in a copy of the model: the result, or why not (None:
/// it could not be built, with no reason to give, as before).
fn attempt(
    model: &Model,
    path: &BTreeSet<EdgeId>,
    profile: FaceId,
) -> Result<Model, Option<&'static str>> {
    let cached = LAST.with(|c| {
        c.borrow().as_ref().and_then(|(m, p, f, out)| {
            (*f == profile && p == path && m == model).then(|| out.clone())
        })
    });
    if let Some(out) = cached {
        return out;
    }
    let out = (|| {
        best(model, path, profile).map_err(Some)?;
        let mut m = model.clone();
        let before = build(&mut m, path, profile).ok_or(None)?;
        if crosses_itself(&m, model, profile) {
            return Err(Some(OVERLAPS));
        }
        let tol = m.tol();
        stick(&mut m, &before, &tol);
        Ok(m)
    })();
    // A plan refused for its own reason is not kept (it is quick to find).
    if !matches!(out, Err(Some(e)) if e != OVERLAPS) {
        LAST.with(|c| {
            *c.borrow_mut() = Some((model.clone(), path.clone(), profile, out.clone()));
        });
    }
    out
}

/// Cut the path at `end` (Follow Me stopped partway along an edge, owner
/// 2026-11-12): the path edge that `end` lies inside is split there, and
/// the piece on the profile's side is kept in the path. The far piece
/// stays in the model as an edge. Returns the path to follow; unchanged
/// if `end` is at a path point or on no path edge.
pub fn cut_path(
    model: &mut Model,
    path: &BTreeSet<EdgeId>,
    profile: FaceId,
    end: DVec3,
) -> BTreeSet<EdgeId> {
    let tol = model.tol();
    let Some(e) = path.iter().copied().find(|e| {
        model
            .edge(*e)
            .is_some_and(|x| crate::sticky::on_edge_inside(model, x, end, &tol))
    }) else {
        return path.clone();
    };
    let Some(x) = model.edge(e).cloned() else {
        return path.clone();
    };
    if model.vertex(x.a).distance(end) < tol.dist || model.vertex(x.b).distance(end) < tol.dist {
        return path.clone();
    }
    // Which end of the cut edge leads back to the profile: the one other
    // path edges or the profile's own corners touch.
    let corners: BTreeSet<VertexId> = model
        .face(profile)
        .map(|f| f.loops().flatten().copied().collect())
        .unwrap_or_default();
    let joined = |v: VertexId| {
        corners.contains(&v)
            || path
                .iter()
                .any(|o| *o != e && model.edge(*o).is_some_and(|y| y.has(v)))
    };
    let keep_a = if joined(x.a) != joined(x.b) {
        joined(x.a)
    } else {
        // A lone edge not touching the profile: the end nearer it.
        let c = model
            .face(profile)
            .map(|f| model.points(&f.outer))
            .and_then(|p| {
                (!p.is_empty()).then(|| p.iter().copied().sum::<DVec3>() / p.len() as f64)
            })
            .unwrap_or(end);
        model.vertex(x.a).distance(c) <= model.vertex(x.b).distance(c)
    };
    let v = model.add_vertex(end);
    let Some((first, second)) = model.split_edge(e, v) else {
        return path.clone();
    };
    let mut out = path.clone();
    out.remove(&e);
    out.insert(if keep_a { first } else { second });
    out
}

/// Why following `path` with `profile` is refused, if it is.
pub fn refusal(model: &Model, path: &BTreeSet<EdgeId>, profile: FaceId) -> Option<&'static str> {
    if let Err(e) = best(model, path, profile) {
        return Some(e);
    }
    attempt(model, path, profile).err().flatten()
}

/// Sweep `profile` along `path`. Returns whether anything was made:
/// nothing when it is refused, or when the sweep would cross itself.
pub fn follow(model: &mut Model, path: &BTreeSet<EdgeId>, profile: FaceId) -> bool {
    match attempt(model, path, profile) {
        Ok(m) => {
            *model = m;
            true
        }
        Err(_) => false,
    }
}

/// Whether the sweep just built in `after` (from `before`) crosses itself:
/// two of its faces that share no point cross each other (a profile wider
/// than the gap to a later part of the path, or a path that crosses
/// itself).
fn crosses_itself(after: &Model, before: &Model, profile: FaceId) -> bool {
    let tol = after.tol();
    let new: Vec<FaceId> = after
        .faces()
        .map(|(id, _)| id)
        .filter(|id| *id == profile || before.face(*id).is_none())
        .collect();
    let points = |id: FaceId| -> BTreeSet<VertexId> {
        after
            .face(id)
            .map(|f| f.loops().flatten().copied().collect())
            .unwrap_or_default()
    };
    let boxes: Vec<Option<(DVec3, DVec3)>> = new
        .iter()
        .map(|id| crate::meet::face_bounds(after, *id))
        .collect();
    let vs: Vec<BTreeSet<VertexId>> = new.iter().map(|id| points(*id)).collect();
    for i in 0..new.len() {
        for j in i + 1..new.len() {
            if !vs[i].is_disjoint(&vs[j]) {
                continue;
            }
            let (Some(a), Some(b)) = (boxes[i], boxes[j]) else {
                continue;
            };
            let pad = DVec3::splat(tol.dist);
            if (a.0 - pad).cmpgt(b.1).any() || (b.0 - pad).cmpgt(a.1).any() {
                continue;
            }
            let (fa, fb) = (new[i], new[j]);
            let curved = |id: FaceId| after.face(id).is_some_and(|f| f.is_curved());
            let met = if curved(fa) {
                !crate::crossing::crossings_with(after, fa, fb).is_empty()
            } else if curved(fb) {
                !crate::crossing::crossings_with(after, fb, fa).is_empty()
            } else {
                match (
                    crate::intersect::Flat::drawn(after, fa),
                    crate::intersect::Flat::drawn(after, fb),
                ) {
                    (Some(x), Some(y)) => !crate::intersect::crossings(&x, &y, &tol).is_empty(),
                    _ => false,
                }
            };
            if met {
                return true;
            }
        }
    }
    false
}

/// Build the sweep (not yet stuck to what it meets). Returns the points
/// there were before it, or None when it is refused.
// Sections, loops and points are parallel tables, walked by index.
#[allow(clippy::needless_range_loop)]
fn build(
    model: &mut Model,
    path: &BTreeSet<EdgeId>,
    profile: FaceId,
) -> Option<BTreeSet<VertexId>> {
    let (chords, segmented) = best(model, path, profile).ok()?;
    if segmented {
        segment_profile(model, profile);
    }
    let plan = plan(model, path, profile, chords).ok()?;
    let tol = model.tol();
    let before: BTreeSet<VertexId> = model.vertices().map(|(v, _)| v).collect();
    // A face of a body (every edge of it borders another face): swept out
    // of its front, the body grows and the face goes, as with Push/Pull
    // and in SketchUp (owner, 2026-11-12).
    let in_body = model.face(profile).is_some_and(|f| {
        f.edge_ids()
            .all(|e| model.faces().any(|(id, g)| id != profile && g.uses_edge(e)))
    });
    // The sweep takes the profile's paint, outside (DECISIONS #36).
    let paint = model.face(profile).map(|f| f.paint).unwrap_or_default();
    let secs = plan.sections.len();
    let lines = plan.steps.len();
    let next = |k: usize| (k + 1) % secs;
    // Whether point `i` of loop `li` is on the axis of step `k` (an arc):
    // it stays put there, one vertex.
    let pole = |k: usize, li: usize, i: usize| match plan.steps[k] {
        Step::Arc { centre, axis, .. } => {
            radial(centre, axis, plan.sections[k][li][i]).0.length() <= tol.dist
        }
        Step::Line(_) => false,
    };
    // Vertices: an open path keeps the profile's own as its start.
    let mut vid: Vec<Vec<Vec<VertexId>>> = Vec::with_capacity(secs);
    for k in 0..secs {
        let mut lv = Vec::new();
        for (li, l) in plan.sections[k].iter().enumerate() {
            let mut ids = Vec::new();
            for (i, p) in l.iter().enumerate() {
                if k == 0 && !plan.closed {
                    let v = plan.loops[li].vs[i];
                    if plan.squares_up {
                        model.set_vertex(v, *p);
                    }
                    ids.push(v);
                } else if k > 0 && pole(k - 1, li, i) {
                    ids.push(vid[k - 1][li][i]);
                } else {
                    ids.push(model.add_vertex(*p));
                }
            }
            lv.push(ids);
        }
        vid.push(lv);
    }
    // Round a closed path, a point on the last arc's axis is the first
    // section's vertex.
    if plan.closed {
        for li in 0..plan.loops.len() {
            for i in 0..plan.loops[li].vs.len() {
                let mut k = lines - 1;
                while pole(k, li, i) && vid[k][li][i] != vid[next(k)][li][i] {
                    let old = vid[k][li][i];
                    vid[k][li][i] = vid[next(k)][li][i];
                    model.remove_vertex(old);
                    if k == 0 {
                        break;
                    }
                    k -= 1;
                }
            }
        }
    }
    // Section edges: copies of the profile's edges (arcs moved with it).
    let mut new_curve: BTreeMap<(usize, CurveId), CurveId> = BTreeMap::new();
    let mut sec_edges: Vec<Vec<Vec<EdgeId>>> = Vec::with_capacity(secs);
    for k in 0..secs {
        let mut le = Vec::new();
        for (li, l) in plan.loops.iter().enumerate() {
            if k == 0 && !plan.closed {
                le.push(l.es.clone());
                continue;
            }
            let m = l.vs.len();
            let mut ids = Vec::new();
            for i in 0..m {
                let orig = model.edge(l.es[i]).cloned()?;
                let (a, b) = (vid[k][li][i], vid[k][li][(i + 1) % m]);
                // Keep the original edge's direction.
                let (a, b) = if orig.a == l.vs[i] { (a, b) } else { (b, a) };
                let id = match orig.curve {
                    // An approximate profile's segments stay approximate,
                    // one curve per section.
                    Curve::Line if orig.approx => {
                        let id = model.add_new_edge(a, b);
                        model.set_approx(id, true);
                        let cid = orig.curve_id.map(|x| {
                            *new_curve
                                .entry((k, x))
                                .or_insert_with(|| model.new_curve_id())
                        });
                        model.set_curve_id(id, cid);
                        Some(id)
                    }
                    Curve::Line => model.add_edge(a, b),
                    c => {
                        let c = plan.motions[k].and_then(|x| c.transformed(&x))?;
                        let cid = orig.curve_id.map(|x| {
                            *new_curve
                                .entry((k, x))
                                .or_insert_with(|| model.new_curve_id())
                        });
                        model.add_curve_edge(a, b, c, cid)
                    }
                };
                let id = id?;
                ids.push(id);
            }
            le.push(ids);
        }
        sec_edges.push(le);
    }
    // Edges along the path from each profile point: lines, or arcs about
    // an arc's axis (one curve per point and path curve). None for a point
    // on the axis.
    let mut circles: BTreeMap<(CurveId, usize, usize), CurveId> = BTreeMap::new();
    let mut along: Vec<Vec<Vec<Option<EdgeId>>>> = Vec::with_capacity(lines);
    for k in 0..lines {
        let mut le = Vec::new();
        for li in 0..plan.loops.len() {
            let mut ids = Vec::new();
            for i in 0..plan.loops[li].vs.len() {
                let (a, b) = (vid[k][li][i], vid[next(k)][li][i]);
                let id = match plan.steps[k] {
                    _ if pole(k, li, i) => None,
                    Step::Line(_) => {
                        let id = model.add_edge(a, b)?;
                        Some(id)
                    }
                    Step::Arc {
                        centre,
                        axis,
                        curve,
                        ..
                    } => {
                        let (r, h) = radial(centre, axis, plan.sections[k][li][i]);
                        let cid = match curve {
                            Some(c) => *circles
                                .entry((c, li, i))
                                .or_insert_with(|| model.new_curve_id()),
                            None => model.new_curve_id(),
                        };
                        let arc = Curve::Arc {
                            centre: centre + axis * h,
                            axis,
                            radius: r.length(),
                        };
                        let id = model.add_curve_edge(a, b, arc, Some(cid))?;
                        Some(id)
                    }
                };
                ids.push(id);
            }
            le.push(ids);
        }
        along.push(le);
    }
    // Walls: one per profile side and path piece. The loop walks counter-
    // clockwise about the profile's normal with the profile on its left,
    // so a wall's outside is t × n (t along the side, n the normal); with
    // the profile facing along the path (f) that is t × f, which the
    // order a, b, b', a' walks counter-clockwise about.
    let s = if plan.along { 1.0 } else { -1.0 };
    for k in 0..lines {
        let k1 = next(k);
        for (li, l) in plan.loops.iter().enumerate() {
            let m = l.vs.len();
            for i in 0..m {
                let j = (i + 1) % m;
                let (p, q, curve) = side(model, &plan, k, li, i)?;
                let surface = match swept(&plan.steps[k], p, q, curve.clone(), &tol) {
                    Ok(Some(x)) => x,
                    Ok(None) => continue,
                    Err(_) => return None,
                };
                // Face it outward.
                let (mid, t) = match curve {
                    Curve::Spline(_) => {
                        let mid = curve.point(p, q, 0.5);
                        (mid, curve.tangent_at(p, q, mid))
                    }
                    Curve::Line => ((p + q) / 2.0, q - p),
                    Curve::Arc { centre, axis, .. } => {
                        let mid = curve.point(p, q, 0.5);
                        (mid, axis.cross(mid - centre))
                    }
                };
                let out = t.cross(plan.steps[k].tangent(mid)) * s;
                let surface = if surface.normal_at(mid).dot(out) < 0.0 {
                    surface.flipped()
                } else {
                    surface
                };
                let (a, b, a1, b1) = (vid[k][li][i], vid[k][li][j], vid[k1][li][i], vid[k1][li][j]);
                let (sk, sk1) = (sec_edges[k][li][i], sec_edges[k1][li][i]);
                let (outer, edges) = match (along[k][li][i], along[k][li][j]) {
                    (Some(ei), Some(ej)) => (vec![a, b, b1, a1], vec![sk, ej, sk1, ei]),
                    (None, Some(ej)) => (vec![a, b, b1], vec![sk, ej, sk1]),
                    (Some(ei), None) => (vec![a, b, a1], vec![sk, sk1, ei]),
                    (None, None) => (vec![a, b], vec![sk, sk1]),
                };
                let mut f = Face::with_edges(outer, vec![], surface.clone(), vec![edges]);
                if !plan.along {
                    // Walk it the other way, keeping the surface's side.
                    f.reverse();
                    f.surface = surface;
                }
                f.paint = paint;
                model.add_face(f);
            }
        }
    }
    // Smooth where the profile turns smoothly (a line into an arc
    // tangentially, or two arcs of a circle) ...
    for li in 0..plan.loops.len() {
        let m = plan.loops[li].vs.len();
        for i in 0..m {
            let prev = (i + m - 1) % m;
            // Between two segments of one approximate curve, the edge
            // along the path is soft, smooth and approximate too.
            let fine = {
                let es = &sec_edges[0][li];
                let (x, y) = (model.edge(es[prev]), model.edge(es[i]));
                x.zip(y)
                    .is_some_and(|(x, y)| x.approx && y.approx && x.curve_id == y.curve_id)
            };
            if fine || smooth_at(model, &sec_edges[0][li], prev, i, &vid[0][li]) {
                for e in along.iter().filter_map(|row| row[li][i]) {
                    model.set_soft(e, true, true);
                    if fine {
                        model.set_approx(e, true);
                    }
                }
            }
        }
    }
    // ... and where the path does.
    for k in 0..secs {
        if plan.smooth[k] || plan.seams[k] {
            for e in sec_edges[k].iter().flatten() {
                model.set_soft(*e, true, true);
                if plan.seams[k] {
                    model.set_approx(*e, true);
                }
            }
        }
    }
    // Along a path arc followed as segments, the edges are approximate.
    for (k, row) in along.iter().enumerate() {
        if plan.chords[k] {
            for e in row.iter().flatten().flatten() {
                model.set_approx(*e, true);
            }
        }
    }
    // Ends.
    if plan.closed {
        remove_profile(model, profile);
    } else {
        if in_body && plan.along {
            model.remove_face(profile);
        }
        let t0 = plan.steps[0].tangent(plan.sections[0][0][0]);
        let start = plan.sections[0][0][0];
        model.edit_face(profile, |f| {
            if plan.along {
                f.reverse();
            }
            f.surface = Surface::Plane(Plane::new(start, -t0));
            // Its paint stays on the outside.
            f.paint = paint;
        });
        let last = secs - 1;
        let p = plan.sections[last][0][0];
        let d = plan.steps[lines - 1].tangent(p);
        let edges: Vec<Vec<EdgeId>> = sec_edges[last].clone();
        let outer = vid[last][0].clone();
        let holes = vid[last][1..].to_vec();
        let mut f = Face::with_edges(
            outer,
            holes,
            Surface::Plane(Plane::new(p, if plan.along { d } else { -d })),
            edges,
        );
        if !plan.along {
            f.reverse();
        }
        f.paint = paint;
        model.add_face(f);
    }
    // Take away what no face uses (a profile side on an arc's axis).
    let used: BTreeSet<EdgeId> = model
        .faces()
        .flat_map(|(_, x)| x.edge_ids().collect::<Vec<_>>())
        .collect();
    let made: BTreeSet<EdgeId> = sec_edges
        .iter()
        .skip(usize::from(!plan.closed))
        .flatten()
        .flatten()
        .copied()
        .chain(along.iter().flatten().flatten().flatten().copied())
        .collect();
    for e in made {
        if !used.contains(&e) {
            model.remove_edge(e);
        }
    }
    let live: BTreeSet<VertexId> = model.edges().flat_map(|(_, e)| [e.a, e.b]).collect();
    for v in vid
        .iter()
        .skip(usize::from(!plan.closed))
        .flatten()
        .flatten()
    {
        if !live.contains(v) {
            model.remove_vertex(*v);
        }
    }
    Some(before)
}

/// The sweep stuck to what it meets (Exact Curves 6b), as Push/Pull's
/// result is: its new points weld onto old ones there, the planes it
/// touches are rebuilt (an end flush with a wall splits the wall), and
/// curved faces its edges lie on are split.
fn stick(model: &mut Model, before: &BTreeSet<VertexId>, tol: &prism_math::Tol) {
    let new: BTreeSet<VertexId> = model
        .vertices()
        .map(|(v, _)| v)
        .filter(|v| !before.contains(v))
        .collect();
    crate::transform::weld_new(model, &new, tol);
    let touched: BTreeSet<VertexId> = model
        .vertices()
        .map(|(v, _)| v)
        .filter(|v| !before.contains(v))
        .collect();
    crate::settle::settle(model, &touched);
    crate::settle::dedupe_faces(model);

    let edges: Vec<EdgeId> = model
        .edges()
        .filter(|(_, e)| touched.contains(&e.a) || touched.contains(&e.b))
        .map(|(id, _)| id)
        .collect();
    crate::cut::split_along(model, &edges);
}

/// Whether the profile's walk turns smoothly at point `i` (between sides
/// `prev` and `i`).
fn smooth_at(model: &Model, es: &[EdgeId], prev: usize, i: usize, vs: &[VertexId]) -> bool {
    let tangent = |side: usize, at_start: bool| -> Option<DVec3> {
        let e = model.edge(es[side])?;
        let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
        let fwd = e.a == vs[side];
        // The walk's direction at the side's start or end.
        let t = match (at_start, fwd) {
            (true, true) => e.curve.start_tangent(pa, pb),
            (true, false) => -e.curve.end_tangent(pa, pb),
            (false, true) => e.curve.end_tangent(pa, pb),
            (false, false) => -e.curve.start_tangent(pa, pb),
        };
        t.try_normalize()
    };
    let (Some(a), Some(b)) = (tangent(prev, false), tangent(i, true)) else {
        return false;
    };
    let arcs = [prev, i]
        .iter()
        .any(|s| model.edge(es[*s]).is_some_and(|e| e.curve.is_curved()));
    arcs && a.dot(b) > 1.0 - 1e-9
}

/// Take away a closed path's profile face, and its edges and points that
/// nothing else uses.
fn remove_profile(model: &mut Model, profile: FaceId) {
    let Some(f) = model.remove_face(profile) else {
        return;
    };
    let used: BTreeSet<EdgeId> = model
        .faces()
        .flat_map(|(_, x)| x.edge_ids().collect::<Vec<_>>())
        .collect();
    for e in f.edge_ids() {
        if !used.contains(&e) {
            model.remove_edge(e);
        }
    }
    let live: BTreeSet<VertexId> = model.edges().flat_map(|(_, e)| [e.a, e.b]).collect();
    for v in f.loops().flatten() {
        if !live.contains(v) {
            model.remove_vertex(*v);
        }
    }
}
