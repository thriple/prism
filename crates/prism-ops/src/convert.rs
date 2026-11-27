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
    "Convert takes edges joined end to end in one chain, without branches.";
pub const MIXED: &str = "Convert takes a chain of lines and arcs, or one NURBS curve, not both.";
pub const MEETS: &str =
    "Other edges meet this chain partway; its points between would have to stay.";
pub const CURVED: &str =
    "A curved face runs along this chain; Convert takes chains with flat faces or none.";
pub const FAILED: &str = "Prism could not make this curve.";

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
            && x2.dot(*axis) > 1.0 - prism_math::tol::ANGLE
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

// ---------- Faces ↔ Smooth Patch (part 4c) ----------

pub const NO_FACES: &str = "Select a grid of four-sided faces, or one freeform face, then Convert.";
pub const NOT_A_GRID: &str = "Convert takes four-sided flat faces joined edge to edge in rows and columns, or one freeform face.";
pub const BORDERS: &str =
    "Other faces or edges meet this grid's sides; Convert takes a grid standing on its own.";
pub const TRIMMED: &str = "This freeform face is cut from its patch; Convert takes a whole one.";

/// Why `convert_faces` would refuse, if it would.
pub fn faces_refusal(model: &Model, faces: &BTreeSet<FaceId>) -> Option<&'static str> {
    let mut m = model.clone();
    convert_faces(&mut m, faces).err()
}

/// A grid of four-sided faces becomes one freeform face through their
/// points, or one freeform face a grid of flat faces.
pub fn convert_faces(model: &mut Model, faces: &BTreeSet<FaceId>) -> Result<(), &'static str> {
    let fs: Vec<FaceId> = faces
        .iter()
        .copied()
        .filter(|f| model.face(*f).is_some())
        .collect();
    match fs.as_slice() {
        [] => Err(NO_FACES),
        [f] if model.face(*f).is_some_and(|x| x.surface.patch().is_some()) => {
            patch_to_faces(model, *f)
        }
        _ => grid_to_patch(model, faces),
    }
}

/// The grid's points, [i][j] with i along u and j along v, and its cells.
fn grid_of(model: &Model, faces: &BTreeSet<FaceId>) -> Result<Vec<Vec<VertexId>>, &'static str> {
    for f in faces {
        let x = model.face(*f).ok_or(NOT_A_GRID)?;
        let straight = x
            .edge_ids()
            .all(|e| model.edge(e).is_some_and(|y| !y.curve.is_curved()));
        if x.plane().is_none() || !x.holes.is_empty() || x.outer.len() != 4 || !straight {
            return Err(NOT_A_GRID);
        }
    }
    let ef = model.edge_faces();
    let inside = |e: EdgeId| -> Vec<FaceId> {
        ef.get(&e)
            .map(|v| v.iter().copied().filter(|f| faces.contains(f)).collect())
            .unwrap_or_default()
    };
    // The face across the side a-b of `from`, and the points it carries on
    // from a and from b.
    let across = |from: FaceId, a: VertexId, b: VertexId| -> Option<(FaceId, VertexId, VertexId)> {
        let e = model.line_between(a, b)?;
        let other = inside(e).into_iter().find(|f| *f != from)?;
        let l = &model.face(other)?.outer;
        let n = l.len();
        let ia = l.iter().position(|v| *v == a)?;
        let ib = l.iter().position(|v| *v == b)?;
        let next_of = |i: usize, not: usize| {
            let (p, q) = ((i + 1) % n, (i + n - 1) % n);
            if p == not { l[q] } else { l[p] }
        };
        Some((other, next_of(ia, ib), next_of(ib, ia)))
    };
    // A corner face: its sides shared within the grid meet at one point
    // at most, and the corner opposite is on none of them.
    let corner = faces
        .iter()
        .copied()
        .find_map(|f| {
            let l = model.face(f)?.outer.clone();
            let shared: Vec<usize> = (0..4)
                .filter(|i| {
                    model
                        .line_between(l[*i], l[(i + 1) % 4])
                        .is_some_and(|e| inside(e).len() == 2)
                })
                .collect();
            let ok = match shared.as_slice() {
                [] => Some(0),
                // One shared side (a strip's end): a corner across from it.
                [s] => Some((s + 2) % 4),
                // Two shared sides meeting at a point: the corner opposite.
                [s, t] if (t + 4 - s) % 4 == 1 => Some((t + 2) % 4),
                [s, t] if (s + 4 - t) % 4 == 1 => Some((s + 2) % 4),
                _ => None,
            }?;
            Some((f, l, ok))
        })
        .ok_or(NOT_A_GRID)?;
    let (c, l, k) = corner;
    // l[k] is the outer corner: u runs to l[k+1], v to l[k+3].
    let v0 = l[k];
    let v1 = l[(k + 1) % 4];
    let v2 = l[(k + 2) % 4];
    let v3 = l[(k + 3) % 4];
    // Along u when the side v1-v2 is shared, else swap u and v.
    let shared = |a: VertexId, b: VertexId| {
        model
            .line_between(a, b)
            .is_some_and(|e| inside(e).len() == 2)
    };
    let (u1, v_1, u1v1) = if shared(v1, v2) || !shared(v3, v2) {
        (v1, v3, v2)
    } else {
        (v3, v1, v2)
    };
    // The first row of cells, along u.
    let mut cols: Vec<[VertexId; 2]> = vec![[v0, v_1], [u1, u1v1]];
    let mut cells: Vec<Vec<FaceId>> = vec![vec![c]];
    while let Some((f, a, b)) = {
        let n = cols.len();
        across(cells[0][n - 2], cols[n - 1][0], cols[n - 1][1])
    } {
        if cells[0].contains(&f) {
            return Err(NOT_A_GRID);
        }
        cells[0].push(f);
        cols.push([a, b]);
    }
    let nu = cells[0].len();
    let mut g: Vec<Vec<VertexId>> = cols.iter().map(|c| c.to_vec()).collect();
    // Each next row, cell by cell, across the row's top side.
    loop {
        let j = g[0].len() - 1;
        let mut row = Vec::new();
        let mut tops: Vec<VertexId> = Vec::new();
        for i in 0..nu {
            match across(cells[j - 1][i], g[i][j], g[i + 1][j]) {
                Some((f, a, b)) => {
                    if i == 0 {
                        tops.push(a);
                    } else if tops[i] != a {
                        return Err(NOT_A_GRID);
                    }
                    tops.push(b);
                    row.push(f);
                }
                None if i == 0 => break,
                None => return Err(NOT_A_GRID),
            }
        }
        if row.is_empty() {
            break;
        }
        if row.len() != nu {
            return Err(NOT_A_GRID);
        }
        for (i, t) in tops.into_iter().enumerate() {
            g[i].push(t);
        }
        cells.push(row);
    }
    let all: BTreeSet<FaceId> = cells.iter().flatten().copied().collect();
    if all.len() != faces.len() || all.len() != cells.iter().map(|r| r.len()).sum::<usize>() {
        return Err(NOT_A_GRID);
    }
    // Standing on its own: its sides border no other face, and nothing
    // else meets its points.
    let used: BTreeSet<EdgeId> = faces
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.edge_ids().collect::<Vec<_>>())
        .collect();
    if used.iter().any(|e| {
        ef.get(e)
            .is_some_and(|v| v.iter().any(|f| !faces.contains(f)))
    }) {
        return Err(BORDERS);
    }
    let points: BTreeSet<VertexId> = g.iter().flatten().copied().collect();
    if model
        .edges()
        .any(|(id, e)| !used.contains(&id) && (points.contains(&e.a) || points.contains(&e.b)))
    {
        return Err(BORDERS);
    }
    Ok(g)
}

/// Chord-length parameters along each line of points, averaged.
fn params(lines: &[Vec<DVec3>]) -> Option<Vec<f64>> {
    let n = lines.first()?.len();
    let mut ts = vec![0.0; n];
    for l in lines {
        let total: f64 = l.windows(2).map(|w| w[0].distance(w[1])).sum();
        if total <= 0.0 {
            return None;
        }
        let mut acc = 0.0;
        for i in 1..n {
            acc += l[i - 1].distance(l[i]);
            ts[i] += acc / total / lines.len() as f64;
        }
    }
    ts[n - 1] = 1.0;
    Some(ts)
}

fn grid_to_patch(model: &mut Model, faces: &BTreeSet<FaceId>) -> Result<(), &'static str> {
    let g = grid_of(model, faces)?;
    let (nu, nv) = (g.len(), g[0].len());
    let p: Vec<Vec<DVec3>> = g
        .iter()
        .map(|col| col.iter().map(|v| model.vertex(*v)).collect())
        .collect();
    // Along u for each j, along v for each i.
    let rows_u: Vec<Vec<DVec3>> = (0..nv)
        .map(|j| (0..nu).map(|i| p[i][j]).collect())
        .collect();
    let tu = params(&rows_u).ok_or(FAILED)?;
    let tv = params(&p).ok_or(FAILED)?;
    // Fit each row along u, then each column of their control points
    // along v: the surface passes through every point of the grid.
    let fits_u: Vec<Nurbs> = rows_u
        .iter()
        .map(|r| Nurbs::through_at(r, &tu))
        .collect::<Option<_>>()
        .ok_or(FAILED)?;
    let ku = fits_u[0].knots().to_vec();
    let cu = fits_u[0].points().len();
    let mut ctrl = vec![DVec3::ZERO; cu * nv];
    let mut kv = Vec::new();
    for i in 0..cu {
        let col: Vec<DVec3> = fits_u.iter().map(|c| c.points()[i]).collect();
        let fit = Nurbs::through_at(&col, &tv).ok_or(FAILED)?;
        kv = fit.knots().to_vec();
        for (j, q) in fit.points().iter().enumerate() {
            ctrl[i * nv + j] = *q;
        }
    }
    let du = fits_u[0].degree();
    let dv = Nurbs::through_at(&p[0], &tv).ok_or(FAILED)?.degree();
    let n = ctrl.len();
    let surface = prism_geom::NurbsSurface::new(
        [du, dv],
        [ku, kv],
        [cu, nv],
        ctrl,
        vec![1.0; n],
        [false, false],
    )
    .ok_or(FAILED)?;
    let iso = |d: usize, t: f64| surface.iso(d, t).map(Arc::new);
    let (c00, c10, c11, c01) = (g[0][0], g[nu - 1][0], g[nu - 1][nv - 1], g[0][nv - 1]);
    let (b0, b1) = (iso(0, 0.0).ok_or(FAILED)?, iso(0, 1.0).ok_or(FAILED)?);
    let (l0, l1) = (iso(1, 0.0).ok_or(FAILED)?, iso(1, 1.0).ok_or(FAILED)?);
    // Which way the old faces looked, at the first cell.
    let look = faces
        .iter()
        .filter_map(|f| model.face(*f))
        .find(|f| f.outer.contains(&c00))
        .and_then(|f| f.plane())
        .map(|pl| pl.normal)
        .ok_or(FAILED)?;
    // Out with the grid, in with the patch.
    let inner: BTreeSet<VertexId> = g
        .iter()
        .flatten()
        .copied()
        .filter(|v| ![c00, c10, c11, c01].contains(v))
        .collect();
    let old: BTreeSet<EdgeId> = faces
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.edge_ids().collect::<Vec<_>>())
        .collect();
    for f in faces {
        model.remove_face(*f);
    }
    for e in old {
        model.remove_edge(e);
    }
    for v in inner {
        model.remove_vertex(v);
    }
    let e0 = spline_edge(model, c00, c10, &b0, 0.0, 1.0, None)?;
    let e1 = spline_edge(model, c10, c11, &l1, 0.0, 1.0, None)?;
    let e2 = spline_edge(model, c11, c01, &b1, 1.0, 0.0, None)?;
    let e3 = spline_edge(model, c01, c00, &l0, 1.0, 0.0, None)?;
    let mut face = Face::with_edges(
        vec![c00, c10, c11, c01],
        vec![],
        prism_geom::Surface::Nurbs(prism_geom::Patch::whole(surface)),
        vec![vec![e0, e1, e2, e3]],
    );
    let at = model.vertex(c00);
    if face.surface.normal_at(at).dot(look) < 0.0 {
        face.reverse();
    }
    model.add_face(face);
    Ok(())
}

fn patch_to_faces(model: &mut Model, f: FaceId) -> Result<(), &'static str> {
    let face = model.face(f).ok_or(NO_FACES)?.clone();
    if model.is_trimmed(&face) {
        return Err(TRIMMED);
    }
    let patch = face.surface.patch().ok_or(NO_FACES)?.clone();
    let rim: BTreeSet<EdgeId> = face.edge_ids().collect();
    let ef = model.edge_faces();
    if rim.iter().any(|e| ef.get(e).is_some_and(|v| v.len() > 1))
        || model
            .edges()
            .any(|(id, e)| !rim.contains(&id) && face.outer.iter().any(|v| e.has(*v)))
    {
        return Err(BORDERS);
    }
    let [u, v] = patch.range();
    let us = patch.surface.grid(0, u.0, u.1, v);
    let vs = patch.surface.grid(1, v.0, v.1, u);
    if us.len() < 2 || vs.len() < 2 {
        return Err(FAILED);
    }
    let tol = model.tol();
    for e in &rim {
        model.remove_edge(*e);
    }
    model.remove_face(f);
    // The points: the old corners where they fall, new ones elsewhere.
    let corners: Vec<(VertexId, DVec3)> =
        face.outer.iter().map(|v| (*v, model.vertex(*v))).collect();
    let mut g: Vec<Vec<VertexId>> = Vec::new();
    for su in &us {
        let mut col = Vec::new();
        for sv in &vs {
            let q = patch.surface.eval(*su, *sv);
            let id = corners
                .iter()
                .find(|(_, c)| tol.same_point(*c, q))
                .map(|(id, _)| *id)
                .unwrap_or_else(|| model.add_vertex(q));
            col.push(id);
        }
        g.push(col);
    }
    let (nu, nv) = (us.len(), vs.len());
    let id = model.new_curve_id();
    let line = |model: &mut Model, a: VertexId, b: VertexId, rim: bool| {
        let e = model.add_edge(a, b);
        if let Some(e) = e {
            if rim {
                model.set_approx(e, true);
                model.set_curve_id(e, Some(id));
            } else {
                model.set_soft(e, true, true);
            }
        }
    };
    for i in 0..nu {
        for j in 0..nv {
            if i + 1 < nu {
                line(model, g[i][j], g[i + 1][j], j == 0 || j + 1 == nv);
            }
            if j + 1 < nv {
                line(model, g[i][j], g[i][j + 1], i == 0 || i + 1 == nu);
            }
        }
    }
    for i in 0..nu - 1 {
        for j in 0..nv - 1 {
            let (a, b, c, d) = (g[i][j], g[i + 1][j], g[i + 1][j + 1], g[i][j + 1]);
            let mid = patch
                .surface
                .eval((us[i] + us[i + 1]) / 2.0, (vs[j] + vs[j + 1]) / 2.0);
            let look = patch.normal_at(mid);
            let pts = [a, b, c, d].map(|x| model.vertex(x));
            let n = prism_math::newell(&pts).normalize_or_zero();
            let flat = pts.iter().all(|p| tol.is_zero((*p - pts[0]).dot(n)));
            let loops: Vec<Vec<VertexId>> = if flat {
                vec![vec![a, b, c, d]]
            } else {
                line(model, a, c, false);
                vec![vec![a, b, c], vec![a, c, d]]
            };
            for mut l in loops {
                let q: Vec<DVec3> = l.iter().map(|x| model.vertex(*x)).collect();
                let mut n = prism_math::newell(&q).normalize_or_zero();
                if n == DVec3::ZERO {
                    continue;
                }
                if n.dot(look) < 0.0 {
                    l.reverse();
                    n = -n;
                }
                let mut nf = Face::new(
                    l,
                    vec![],
                    prism_geom::Surface::Plane(prism_math::Plane::new(q[0], n)),
                );
                nf.paint = face.paint;
                model.add_face(nf);
            }
        }
    }
    Ok(())
}
