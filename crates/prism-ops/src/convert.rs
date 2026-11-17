//! Solid > Convert > Curve Chain ↔ NURBS Curve (brief 43, part 4b).
//!
//! A chain of lines and arcs joined end to end becomes one NURBS curve,
//! exactly: each line and arc is a rational piece of it, with a knot (a
//! corner allowed) where they met. The points in between go, and the faces
//! along it keep it as their side. One NURBS curve becomes a chain again:
//! split at its knots, each piece that is a line or an arc comes back as
//! one (arcs on one circle merged), and the rest become fine straight
//! segments by the segment fallback's rule (DECISIONS #32), marked
//! approximate. The new chain is one curve (welded). Faces on curved
//! surfaces along it are refused for now: their outlines are tied to
//! their edges.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;
use std::sync::Arc;

use prism_geom::{Curve, Nurbs, Spline};
use prism_math::{DVec3, Tol};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

pub const NOTHING: &str = "Select a chain of lines and arcs, or one NURBS curve, then Convert.";
pub const NOT_A_CHAIN: &str =
    "Refused: Convert takes edges joined end to end in one chain, without branches.";
pub const MIXED: &str =
    "Refused: Convert takes a chain of lines and arcs, or one NURBS curve, not both.";
pub const MEETS: &str =
    "Refused: other edges meet this chain partway; its points between would have to stay.";
pub const CURVED: &str =
    "Refused: a curved face runs along this chain; Convert takes chains with flat faces or none.";
pub const FAILED: &str = "Refused: Prism could not make this curve.";

/// One edge of a chain, walked from `from` to `to`.
#[derive(Clone, Copy, Debug)]
struct Step {
    edge: EdgeId,
    from: VertexId,
    to: VertexId,
}

/// The edges in order along the chain, and whether it closes.
fn walk(model: &Model, edges: &BTreeSet<EdgeId>) -> Result<(Vec<Step>, bool), &'static str> {
    let es: Vec<EdgeId> = edges
        .iter()
        .copied()
        .filter(|e| model.edge(*e).is_some())
        .collect();
    if es.is_empty() {
        return Err(NOTHING);
    }
    let mut at: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for e in &es {
        let x = model.edge(*e).ok_or(NOTHING)?;
        for v in [x.a, x.b] {
            at.entry(v).or_default().push(*e);
        }
    }
    if at.values().any(|v| v.len() > 2) {
        return Err(NOT_A_CHAIN);
    }
    let closed = at.values().all(|v| v.len() == 2);
    let start = at
        .iter()
        .find(|(_, v)| closed || v.len() == 1)
        .map(|(k, _)| *k)
        .ok_or(NOT_A_CHAIN)?;
    let mut steps = Vec::new();
    let mut used = BTreeSet::new();
    let mut v = start;
    while let Some(e) = at[&v].iter().copied().find(|e| !used.contains(e)) {
        used.insert(e);
        let x = model.edge(e).ok_or(NOTHING)?;
        let to = if x.a == v { x.b } else { x.a };
        steps.push(Step {
            edge: e,
            from: v,
            to,
        });
        v = to;
    }
    if steps.len() != es.len() {
        return Err(NOT_A_CHAIN);
    }
    Ok((steps, closed))
}

/// The faces along `edges`, refused if any is curved.
fn flat_faces(model: &Model, edges: &BTreeSet<EdgeId>) -> Result<BTreeSet<FaceId>, &'static str> {
    let ef = model.edge_faces();
    let faces: BTreeSet<FaceId> = edges
        .iter()
        .filter_map(|e| ef.get(e))
        .flatten()
        .copied()
        .collect();
    if faces
        .iter()
        .any(|f| model.face(*f).is_none_or(|x| x.surface.is_curved()))
    {
        return Err(CURVED);
    }
    Ok(faces)
}

/// Whether `edges` are spline pieces of one NURBS curve.
fn one_spline(model: &Model, edges: &BTreeSet<EdgeId>) -> Option<Arc<Nurbs>> {
    let mut curve: Option<Arc<Nurbs>> = None;
    for e in edges {
        let Curve::Spline(s) = &model.edge(*e)?.curve else {
            return None;
        };
        match &curve {
            None => curve = Some(s.curve.clone()),
            Some(c) if Arc::ptr_eq(c, &s.curve) || **c == *s.curve => {}
            Some(_) => return None,
        }
    }
    curve
}

/// Why `convert` would refuse, if it would.
pub fn refusal(model: &Model, edges: &BTreeSet<EdgeId>) -> Option<&'static str> {
    let mut m = model.clone();
    convert(&mut m, edges).err()
}

/// Convert the selected chain to one NURBS curve, or one NURBS curve to a
/// chain, whichever it is.
pub fn convert(model: &mut Model, edges: &BTreeSet<EdgeId>) -> Result<(), &'static str> {
    let (steps, closed) = walk(model, edges)?;
    let faces = flat_faces(model, edges)?;
    if one_spline(model, edges).is_some() {
        return to_chain(model, &steps, &faces);
    }
    if edges
        .iter()
        .any(|e| model.edge(*e).is_some_and(|x| x.curve.is_spline()))
    {
        return Err(MIXED);
    }
    to_nurbs(model, &steps, closed, &faces)
}

fn to_nurbs(
    model: &mut Model,
    steps: &[Step],
    closed: bool,
    faces: &BTreeSet<FaceId>,
) -> Result<(), &'static str> {
    let chain: BTreeSet<EdgeId> = steps.iter().map(|s| s.edge).collect();
    // Its points between: nothing else may meet them.
    let inner: Vec<VertexId> = steps[1..].iter().map(|s| s.from).collect();
    for v in &inner {
        if model
            .edges()
            .any(|(id, e)| e.has(*v) && !chain.contains(&id))
        {
            return Err(MEETS);
        }
    }
    // Each piece walked the chain's way.
    let mut pieces = Vec::new();
    for s in steps {
        let e = model.edge(s.edge).ok_or(NOTHING)?;
        let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
        let piece = match &e.curve {
            Curve::Line => Nurbs::line(pa, pb),
            Curve::Arc { centre, axis, .. } => {
                let sweep = sweep_of(*centre, *axis, pa, pb);
                Nurbs::arc(*centre, *axis, pa, sweep)
            }
            Curve::Spline(_) => None,
        }
        .ok_or(FAILED)?;
        pieces.push(if s.from == e.a {
            piece
        } else {
            piece.reversed()
        });
    }
    let curve = Arc::new(Nurbs::joined(&pieces).ok_or(FAILED)?);
    let (d0, d1) = curve.domain();
    let start = steps[0].from;
    // An open chain is one edge; a closed one two, meeting at its start
    // and halfway, as a circle's halves do.
    let mut made: Vec<(VertexId, VertexId, EdgeId)> = Vec::new();
    let mut keep = vec![start];
    if closed {
        let mid = steps[steps.len() / 2].from;
        if mid == start {
            return Err(FAILED);
        }
        let tm = curve.closest(model.vertex(mid), d0, d1);
        let id = model.new_curve_id();
        let a = spline_edge(model, start, mid, &curve, d0, tm, Some(id))?;
        let b = spline_edge(model, mid, start, &curve, tm, d1, Some(id))?;
        made.push((start, mid, a));
        made.push((mid, start, b));
        keep.push(mid);
    } else {
        let end = steps[steps.len() - 1].to;
        let e = spline_edge(model, start, end, &curve, d0, d1, None)?;
        made.push((start, end, e));
        keep.push(end);
    }
    // Which new edge each old one now belongs to, and where that starts.
    let mut part_of: BTreeMap<EdgeId, usize> = BTreeMap::new();
    let mut k = 0;
    for s in steps {
        if k + 1 < made.len() && s.from == made[k + 1].0 {
            k += 1;
        }
        part_of.insert(s.edge, k);
    }
    for f in faces {
        relink(model, *f, |side_from, side_to, edge| {
            let k = *part_of.get(&edge)?;
            let (a, b, new) = made[k];
            // Emit the new edge once, where the face's walk enters it.
            if side_from == a || side_from == b {
                let _ = side_to;
                Some(vec![(side_from, new)])
            } else {
                Some(Vec::new())
            }
        })?;
    }
    for e in &chain {
        model.remove_edge(*e);
    }
    for v in inner {
        if !keep.contains(&v) {
            model.remove_vertex(v);
        }
    }
    Ok(())
}

fn spline_edge(
    model: &mut Model,
    a: VertexId,
    b: VertexId,
    curve: &Arc<Nurbs>,
    t0: f64,
    t1: f64,
    cid: Option<prism_topo::CurveId>,
) -> Result<EdgeId, &'static str> {
    let s = Spline {
        curve: curve.clone(),
        t0,
        t1,
    };
    model
        .add_curve_edge(a, b, Curve::Spline(s), cid)
        .ok_or(FAILED)
}

/// The turn from `a` to `b` counter-clockwise about `axis` round
/// `centre`: more than nothing, a full turn at most.
fn sweep_of(centre: DVec3, axis: DVec3, a: DVec3, b: DVec3) -> f64 {
    let n = axis.normalize();
    let (u, v) = (a - centre, b - centre);
    let mut t = n.dot(u.cross(v)).atan2(u.dot(v));
    if t <= 1e-12 {
        t += TAU;
    }
    t
}

/// One piece of a chain made from a NURBS curve.
#[derive(Clone, Debug)]
enum Piece {
    /// From this point straight to the next piece's start.
    Line(DVec3),
    Arc {
        a: DVec3,
        b: DVec3,
        centre: DVec3,
        axis: DVec3,
        radius: f64,
    },
    /// Fine straight segments through these points (approximate).
    Steps(Vec<DVec3>),
}

impl Piece {
    fn start(&self) -> DVec3 {
        match self {
            Piece::Line(a) | Piece::Arc { a, .. } => *a,
            Piece::Steps(p) => p[0],
        }
    }
}

fn to_chain(
    model: &mut Model,
    steps: &[Step],
    faces: &BTreeSet<FaceId>,
) -> Result<(), &'static str> {
    let tol = model.tol();
    let id = model.new_curve_id();
    // Each old edge, walked the chain's way, becomes these new edges in
    // order (from its start).
    let mut runs: BTreeMap<EdgeId, Vec<(VertexId, EdgeId)>> = BTreeMap::new();
    for s in steps {
        let e = model.edge(s.edge).ok_or(NOTHING)?.clone();
        let Curve::Spline(sp) = &e.curve else {
            return Err(MIXED);
        };
        // The parameter the walk starts and ends at.
        let (p, q) = if s.from == e.a {
            (sp.t0, sp.t1)
        } else {
            (sp.t1, sp.t0)
        };
        let pieces = pieces_of(&sp.curve, p, q, &tol);
        let mut run = Vec::new();
        let mut at = s.from;
        for (i, piece) in pieces.iter().enumerate() {
            let last = i + 1 == pieces.len();
            let next = if last {
                s.to
            } else {
                model.add_vertex(pieces[i + 1].start())
            };
            match piece {
                Piece::Line(..) => {
                    let x = model.add_new_edge(at, next);
                    model.set_curve_id(x, Some(id));
                    run.push((at, x));
                }
                Piece::Arc {
                    centre,
                    axis,
                    radius,
                    ..
                } => {
                    let curve = Curve::Arc {
                        centre: *centre,
                        axis: *axis,
                        radius: *radius,
                    };
                    let x = model
                        .add_curve_edge(at, next, curve, Some(id))
                        .ok_or(FAILED)?;
                    run.push((at, x));
                }
                Piece::Steps(pts) => {
                    let mut from = at;
                    for (j, pt) in pts.iter().enumerate().skip(1) {
                        let to = if j + 1 == pts.len() {
                            next
                        } else {
                            model.add_vertex(*pt)
                        };
                        let x = model.add_new_edge(from, to);
                        model.set_curve_id(x, Some(id));
                        model.set_approx(x, true);
                        run.push((from, x));
                        from = to;
                    }
                }
            }
            at = next;
        }
        runs.insert(s.edge, run);
    }
    for f in faces {
        relink(model, *f, |side_from, _, edge| {
            let run = runs.get(&edge)?;
            // Walked the chain's way, the run as it is; else backwards,
            // each edge starting at its other end.
            if run.first().is_some_and(|(v, _)| *v == side_from) {
                Some(run.clone())
            } else {
                let mut back = Vec::new();
                for (k, (_, x)) in run.iter().enumerate().rev() {
                    let to = run.get(k + 1).map(|(v, _)| *v);
                    let start = to.unwrap_or(side_from);
                    back.push((start, *x));
                }
                Some(back)
            }
        })?;
    }
    for s in steps {
        model.remove_edge(s.edge);
    }
    Ok(())
}

/// The curve from parameter `p` to `q` as lines, arcs and fine steps.
fn pieces_of(curve: &Nurbs, p: f64, q: f64, tol: &Tol) -> Vec<Piece> {
    let (lo, hi) = (p.min(q), p.max(q));
    let mut cuts: Vec<f64> = curve
        .knots()
        .iter()
        .copied()
        .filter(|k| *k > lo + 1e-12 && *k < hi - 1e-12)
        .collect();
    cuts.dedup_by(|a, b| (*a - *b).abs() <= 1e-12);
    let mut ts = vec![lo];
    ts.extend(cuts);
    ts.push(hi);
    if p > q {
        ts.reverse();
    }
    let mut out: Vec<Piece> = Vec::new();
    for w in ts.windows(2) {
        let piece = span(curve, w[0], w[1], tol);
        // Arcs on one circle, one after another, merge.
        if let (
            Some(Piece::Arc {
                a,
                centre,
                axis,
                radius,
                ..
            }),
            Piece::Arc {
                b: b2,
                centre: c2,
                axis: x2,
                radius: r2,
                ..
            },
        ) = (out.last(), &piece)
            && tol.same_point(*centre, *c2)
            && x2.dot(*axis) > 1.0 - 1e-9
            && tol.is_zero(radius - r2)
            && sweep_of(*centre, *axis, *a, *b2) + 1e-9 < TAU
            && !tol.same_point(*a, *b2)
        {
            let merged = Piece::Arc {
                a: *a,
                b: *b2,
                centre: *centre,
                axis: *axis,
                radius: *radius,
            };
            *out.last_mut().expect("an arc") = merged;
            continue;
        }
        out.push(piece);
    }
    out
}

/// One knot span from `t0` to `t1`: a line, an arc, or fine steps.
fn span(curve: &Nurbs, t0: f64, t1: f64, tol: &Tol) -> Piece {
    let at = |s: f64| curve.eval(t0 + (t1 - t0) * s);
    let (a, m, b) = (at(0.0), at(0.5), at(1.0));
    let checks = [0.125, 0.25, 0.375, 0.625, 0.75, 0.875];
    let chord = (b - a).normalize_or_zero();
    let off_line = |p: DVec3| {
        let v = p - a;
        (v - chord * v.dot(chord)).length()
    };
    if checks
        .iter()
        .chain(&[0.5])
        .all(|s| tol.is_zero(off_line(at(*s))))
    {
        return Piece::Line(a);
    }
    let (ab, am) = (b - a, m - a);
    let n = ab.cross(am);
    if n.length_squared() > 0.0 {
        let centre = a
            + (am.cross(n) * ab.length_squared() + n.cross(ab) * am.length_squared())
                / (2.0 * n.length_squared());
        let axis = (a - centre).cross(m - centre).normalize_or_zero();
        let radius = a.distance(centre);
        let on_arc = checks.iter().all(|s| {
            let p = at(*s);
            tol.is_zero(p.distance(centre) - radius) && tol.is_zero((p - centre).dot(axis))
        });
        if on_arc && axis != DVec3::ZERO {
            return Piece::Arc {
                a,
                b,
                centre,
                axis,
                radius,
            };
        }
    }
    let n = crate::approx::fine_count(curve.turning(t0.min(t1), t0.max(t1))).max(2);
    Piece::Steps((0..=n).map(|i| at(i as f64 / n as f64)).collect())
}

/// Rebuild face `f`'s loops: each side along a changed edge is replaced by
/// what `swap(from, to, edge)` gives (pairs of a start point and an edge,
/// in the face's walk); other sides stay.
fn relink(
    model: &mut Model,
    f: FaceId,
    swap: impl Fn(VertexId, VertexId, EdgeId) -> Option<Vec<(VertexId, EdgeId)>>,
) -> Result<(), &'static str> {
    let face = model.face(f).ok_or(NOTHING)?.clone();
    let mut loops: Vec<Vec<VertexId>> = Vec::new();
    let mut edges: Vec<Vec<EdgeId>> = Vec::new();
    for k in 0..=face.holes.len() {
        let (mut l, mut es) = (Vec::new(), Vec::new());
        for side in face.loop_edges(k) {
            match swap(side.from, side.to, side.edge) {
                Some(run) => {
                    for (v, e) in run {
                        l.push(v);
                        es.push(e);
                    }
                }
                None => {
                    l.push(side.from);
                    es.push(side.edge);
                }
            }
        }
        if l.len() < 2 {
            return Err(FAILED);
        }
        loops.push(l);
        edges.push(es);
    }
    let outer = loops.remove(0);
    let mut new = Face::with_edges(outer, loops, face.surface.clone(), edges);
    new.paint = face.paint;
    model.edit_face(f, |x| *x = new);
    Ok(())
}
