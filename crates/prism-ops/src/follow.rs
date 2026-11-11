//! Follow Me along lines (Exact Curves 5c): a profile face swept along a
//! path of straight lines, as in SketchUp. The profile need not touch the
//! path. It is projected square to the path's first line, carried along
//! each line to the corner's mitre plane (the plane halving the corner),
//! and ends square to the last line. A closed path makes a ring with no
//! ends.
//!
//! Profile lines make flat walls. A profile arc makes a cylinder wall, so
//! only along a straight path square to it (at a corner, or tilted, it
//! would be an ellipse: DECISIONS #32).

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Curve, Cylinder, Surface};
use prism_math::{DVec3, Plane, newell};
use prism_topo::{CurveId, EdgeId, Face, FaceId, Model, VertexId};

/// Said with no path selected.
pub const NO_PATH: &str =
    "Select a path first (lines joined end to end), then click the profile face.";
/// Said when the selection is not one chain of lines.
pub const NOT_A_PATH: &str =
    "The path must be straight lines joined end to end in one chain (arcs in a path come later).";
/// Said when the profile is not a flat face.
pub const NOT_A_PROFILE: &str = "The profile must be a flat face.";
/// Said when the profile lies along the path's first line.
pub const ALONG_PATH: &str = "The profile lies along the path's first line: it must cross it.";
/// Said when a tilted profile cannot be squared up.
pub const NOT_SQUARE: &str = "The profile is not square to the path's first line, and it is joined to other faces, so it cannot be squared up.";
/// Said when a profile with arcs meets a corner or is tilted.
pub const ARCS: &str = "A profile with arcs can follow only a straight path square to it: otherwise its arcs would become ellipses.";
/// Said when a corner is too sharp for the profile's width.
pub const CROSSES_ITSELF: &str =
    "The profile is too wide for a corner of this path: the sweep would cross itself.";
/// Said when a closed path does not bring the profile back to its start.
pub const RING_TWISTS: &str =
    "Round this closed path the profile would not come back to where it started.";

/// The path's points in order, and whether it closes: `path` must be lines
/// joined end to end in one chain.
pub fn chain(model: &Model, path: &BTreeSet<EdgeId>) -> Option<(Vec<VertexId>, bool)> {
    let mut adj: BTreeMap<VertexId, Vec<VertexId>> = BTreeMap::new();
    for id in path {
        let e = model.edge(*id)?;
        if e.curve.is_arc() {
            return None;
        }
        adj.entry(e.a).or_default().push(e.b);
        adj.entry(e.b).or_default().push(e.a);
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
    let mut out = vec![start];
    let mut prev: Option<VertexId> = None;
    let mut cur = start;
    loop {
        let next = adj[&cur].iter().copied().find(|n| Some(*n) != prev);
        let Some(next) = next else { break };
        if next == start {
            break;
        }
        if out.contains(&next) {
            return None;
        }
        out.push(next);
        prev = Some(cur);
        cur = next;
    }
    // One chain: every vertex reached, and the right number of lines.
    let lines = if closed { out.len() } else { out.len() - 1 };
    (out.len() == adj.len() && lines == path.len()).then_some((out, closed))
}

/// A profile loop: its vertices, edges and points.
struct ProfileLoop {
    vs: Vec<VertexId>,
    es: Vec<EdgeId>,
}

/// What a sweep makes: the profile's points at each section.
struct Plan {
    closed: bool,
    /// Path directions, one per line (after merging lines in a row).
    dirs: Vec<DVec3>,
    /// The profile's loops.
    loops: Vec<ProfileLoop>,
    /// Points of each loop at each section: `[section][loop][i]`. A closed
    /// path has one section per corner; an open one, one more than lines.
    sections: Vec<Vec<Vec<DVec3>>>,
    /// The profile's normal, and whether it points along the first line.
    normal: DVec3,
    along: bool,
    /// The profile's vertices must move to be square to the first line.
    squares_up: bool,
}

fn plan(model: &Model, path: &BTreeSet<EdgeId>, profile: FaceId) -> Result<Plan, &'static str> {
    if path.is_empty() {
        return Err(NO_PATH);
    }
    let tol = model.tol();
    let (vs, closed) = chain(model, path).ok_or(NOT_A_PATH)?;
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
    let outer: Vec<DVec3> = loops[0].vs.iter().map(|v| model.vertex(*v)).collect();
    let centre = outer.iter().copied().sum::<DVec3>() / outer.len() as f64;

    // The path's points, starting at the end (or corner) nearest the
    // profile, with lines in a row merged.
    let mut pts: Vec<DVec3> = vs.iter().map(|v| model.vertex(*v)).collect();
    if closed {
        let near = (0..pts.len())
            .min_by(|a, b| {
                pts[*a]
                    .distance(centre)
                    .total_cmp(&pts[*b].distance(centre))
            })
            .unwrap_or(0);
        pts.rotate_left(near);
    } else if pts[pts.len() - 1].distance(centre) < pts[0].distance(centre) {
        pts.reverse();
    }
    let straight = |a: DVec3, b: DVec3, c: DVec3| {
        let (u, w) = ((b - a).normalize_or_zero(), (c - b).normalize_or_zero());
        tol.parallel(u, w) && u.dot(w) > 0.0
    };
    loop {
        let n = pts.len();
        let drop = (0..n).find(|&i| {
            if !closed && (i == 0 || i == n - 1) {
                return false;
            }
            n > if closed { 3 } else { 2 }
                && straight(pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n])
        });
        match drop {
            Some(i) => {
                pts.remove(i);
            }
            None => break,
        }
    }
    let n = pts.len();
    let lines = if closed { n } else { n - 1 };
    let dirs: Vec<DVec3> = (0..lines)
        .map(|k| (pts[(k + 1) % n] - pts[k]).normalize_or_zero())
        .collect();
    let d0 = dirs[0];
    if tol.perpendicular(pl.normal, d0) {
        return Err(ALONG_PATH);
    }
    let arcs = loops
        .iter()
        .flat_map(|l| &l.es)
        .any(|e| model.edge(*e).is_some_and(|x| x.curve.is_arc()));
    if arcs && (lines > 1 || !tol.parallel(pl.normal, d0)) {
        return Err(ARCS);
    }
    // The mitre plane at corner `j` (through it, halving the angle).
    let mitre = |j: usize| -> Option<(DVec3, DVec3)> {
        let (a, b) = (dirs[(j + lines - 1) % lines], dirs[j % lines]);
        (a + b).try_normalize().map(|m| (pts[j % n], m))
    };
    // Carry a point along `d` to the plane through `p` with normal `m`.
    let carry = |x: DVec3, d: DVec3, (p, m): (DVec3, DVec3)| -> Option<(DVec3, f64)> {
        let den = d.dot(m);
        if den.abs() < 1e-12 {
            return None;
        }
        let t = (p - x).dot(m) / den;
        Some((x + d * t, t))
    };
    // The first section: square to the first line, through the profile
    // (open), or on the first corner's mitre plane (closed).
    let first_plane = if closed {
        mitre(0).ok_or(CROSSES_ITSELF)?
    } else {
        (centre, d0)
    };
    let mut first: Vec<Vec<DVec3>> = Vec::new();
    let mut squares_up = false;
    for l in &loops {
        let mut sec = Vec::new();
        for v in &l.vs {
            let x = model.vertex(*v);
            let (y, _) = carry(x, d0, first_plane).ok_or(ALONG_PATH)?;
            squares_up |= !closed && !tol.same_point(x, y);
            sec.push(y);
        }
        first.push(sec);
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
    for (k, d) in dirs.iter().enumerate() {
        let target = if !closed && k == lines - 1 {
            (pts[n - 1], *d)
        } else {
            mitre(k + 1).ok_or(CROSSES_ITSELF)?
        };
        let mut next = Vec::new();
        for l in &sections[k] {
            let mut sec = Vec::new();
            for x in l {
                let (y, t) = carry(*x, *d, target).ok_or(CROSSES_ITSELF)?;
                if t <= tol.dist {
                    return Err(CROSSES_ITSELF);
                }
                sec.push(y);
            }
            next.push(sec);
        }
        sections.push(next);
    }
    if closed {
        let back = sections.pop().unwrap_or_default();
        let twist = back
            .iter()
            .flatten()
            .zip(sections[0].iter().flatten())
            .any(|(a, b)| !tol.same_point(*a, *b));
        if twist {
            return Err(RING_TWISTS);
        }
    }
    Ok(Plan {
        closed,
        dirs,
        loops,
        sections,
        normal: pl.normal,
        along: pl.normal.dot(d0) > 0.0,
        squares_up,
    })
}

/// Why following `path` with `profile` is refused, if it is.
pub fn refusal(model: &Model, path: &BTreeSet<EdgeId>, profile: FaceId) -> Option<&'static str> {
    plan(model, path, profile).err()
}

/// Sweep `profile` along `path`. Returns whether anything was made.
// Sections, loops and points are parallel tables, walked by index.
#[allow(clippy::needless_range_loop)]
pub fn follow(model: &mut Model, path: &BTreeSet<EdgeId>, profile: FaceId) -> bool {
    let Ok(plan) = plan(model, path, profile) else {
        return false;
    };
    let secs = plan.sections.len();
    // Vertices: an open path keeps the profile's own as its start.
    let mut vid: Vec<Vec<Vec<VertexId>>> = Vec::with_capacity(secs);
    for (k, sec) in plan.sections.iter().enumerate() {
        let mut lv = Vec::new();
        for (li, l) in sec.iter().enumerate() {
            let mut ids = Vec::new();
            for (i, p) in l.iter().enumerate() {
                if k == 0 && !plan.closed {
                    let v = plan.loops[li].vs[i];
                    if plan.squares_up {
                        model.set_vertex(v, *p);
                    }
                    ids.push(v);
                } else {
                    ids.push(model.add_vertex(*p));
                }
            }
            lv.push(ids);
        }
        vid.push(lv);
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
            let shift = plan.sections[k][li][0] - plan.sections[0][li][0];
            let m = l.vs.len();
            let mut ids = Vec::new();
            for i in 0..m {
                let Some(orig) = model.edge(l.es[i]).cloned() else {
                    return false;
                };
                let (a, b) = (vid[k][li][i], vid[k][li][(i + 1) % m]);
                // Keep the original edge's direction.
                let (a, b) = if orig.a == l.vs[i] { (a, b) } else { (b, a) };
                let id = match orig.curve {
                    Curve::Line => model.add_edge(a, b),
                    Curve::Arc {
                        centre,
                        axis,
                        radius,
                    } => {
                        let cid = orig.curve_id.map(|c| {
                            *new_curve
                                .entry((k, c))
                                .or_insert_with(|| model.new_curve_id())
                        });
                        model.add_curve_edge(
                            a,
                            b,
                            Curve::Arc {
                                centre: centre + shift,
                                axis,
                                radius,
                            },
                            cid,
                        )
                    }
                };
                let Some(id) = id else { return false };
                ids.push(id);
            }
            le.push(ids);
        }
        sec_edges.push(le);
    }
    let lines = plan.dirs.len();
    let next = |k: usize| (k + 1) % secs;
    // Edges along the path, from each profile point.
    let mut along: Vec<Vec<Vec<EdgeId>>> = Vec::with_capacity(lines);
    for k in 0..lines {
        let mut le = Vec::new();
        for li in 0..plan.loops.len() {
            let mut ids = Vec::new();
            for i in 0..plan.loops[li].vs.len() {
                let Some(id) = model.add_edge(vid[k][li][i], vid[next(k)][li][i]) else {
                    return false;
                };
                ids.push(id);
            }
            le.push(ids);
        }
        along.push(le);
    }
    // Walls: one per profile side and line. The loop walks counter-
    // clockwise about the profile's normal with the profile on its left,
    // so a wall's outside is (b - a) × normal; with the profile facing
    // along the path that is the order a, b, b', a'.
    for k in 0..lines {
        let k1 = next(k);
        for (li, l) in plan.loops.iter().enumerate() {
            let m = l.vs.len();
            for i in 0..m {
                let j = (i + 1) % m;
                let outer = vec![vid[k][li][i], vid[k][li][j], vid[k1][li][j], vid[k1][li][i]];
                let edges = vec![
                    sec_edges[k][li][i],
                    along[k][li][j],
                    sec_edges[k1][li][i],
                    along[k][li][i],
                ];
                let pts: Vec<DVec3> = outer.iter().map(|v| model.vertex(*v)).collect();
                let Some(e) = model.edge(sec_edges[k][li][i]).cloned() else {
                    return false;
                };
                let surface = match e.curve {
                    Curve::Line => {
                        let mut n = newell(&pts);
                        if !plan.along {
                            n = -n;
                        }
                        let c = pts.iter().copied().sum::<DVec3>() / 4.0;
                        Surface::Plane(Plane::new(c, n.normalize_or_zero()))
                    }
                    Curve::Arc {
                        centre,
                        axis,
                        radius,
                    } => {
                        // The profile is on the walk's left: the wall faces
                        // away from the axis when the left points at it.
                        let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
                        let mid = e.curve.point(pa, pb, 0.5);
                        let fwd = e.a == vid[k][li][i];
                        let t = axis.cross(mid - centre) * if fwd { 1.0 } else { -1.0 };
                        let left = plan.normal.cross(t);
                        Surface::Cylinder(Cylinder {
                            origin: centre,
                            axis,
                            radius,
                            outward: left.dot(centre - mid) > 0.0,
                        })
                    }
                };
                let mut f = Face::with_edges(outer, vec![], surface, vec![edges]);
                if !plan.along {
                    // Walk it the other way, keeping the surface's side.
                    f.reverse();
                    f.surface = surface;
                }
                model.add_face(f);
            }
        }
    }
    // Smooth where the profile turns smoothly (a line into an arc
    // tangentially, or two arcs of a circle).
    for li in 0..plan.loops.len() {
        let m = plan.loops[li].vs.len();
        for i in 0..m {
            let prev = (i + m - 1) % m;
            if smooth_at(model, &sec_edges[0][li], prev, i, &vid[0][li]) {
                for row in &along {
                    model.set_soft(row[li][i], true, true);
                }
            }
        }
    }
    // Ends.
    if plan.closed {
        remove_profile(model, profile);
    } else {
        let d0 = plan.dirs[0];
        let start = plan.sections[0][0][0];
        model.edit_face(profile, |f| {
            if plan.along {
                f.reverse();
            }
            f.surface = Surface::Plane(Plane::new(start, -d0));
        });
        let last = secs - 1;
        let d = plan.dirs[lines - 1];
        let edges: Vec<Vec<EdgeId>> = sec_edges[last].clone();
        let outer = vid[last][0].clone();
        let holes = vid[last][1..].to_vec();
        let p = plan.sections[last][0][0];
        let mut f = Face::with_edges(
            outer,
            holes,
            Surface::Plane(Plane::new(p, if plan.along { d } else { -d })),
            edges,
        );
        if !plan.along {
            f.reverse();
        }
        model.add_face(f);
    }
    true
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
        .any(|s| model.edge(es[*s]).is_some_and(|e| e.curve.is_arc()));
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
