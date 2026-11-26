//! Loft (Freeform NURBS part 4): a surface through two or more curves, and
//! the laying of any NURBS surface into the model as freeform faces.
//!
//! The curves are made alike exactly (`Nurbs::alike`), the kernel skins a
//! surface through them, and the surface is laid as a grid of faces: cut
//! along its length wherever a section has a vertex, and across at every
//! section, so each section's own edges become the faces' sides.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use prism_geom::{Curve, Nurbs, NurbsSurface, Patch, Spline, Surface};
use prism_math::{DVec3, Tol, newell};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

use crate::cage::side_curve;
use crate::sticky::{insert_point, tol_with};

pub const TWO_CURVES: &str = "Select two or more curves (or lines or arcs) to loft through.";
pub const MIXED: &str =
    "Refused: open and closed curves cannot be lofted together. Select all open or all closed.";
pub const NOT_ALIKE: &str = "Refused: closed curves need the same number of control points to be lofted (or use circles for all of them).";
pub const RING: &str = "These curves join end to end in a ring: that is a boundary to fill, not sections to loft through. Use Draw > Surface > Patch (four sides for now).";
pub const DEGENERATE: &str =
    "Refused: the curves touch or cross, so the surface would pinch to a point.";

/// One curve to loft through: its shape, and the edges on it.
pub(crate) struct Section {
    pub curve: Nurbs,
    pub edges: Vec<EdgeId>,
    /// A whole circle's centre, axis and radius: it can start anywhere.
    pub circle: Option<(DVec3, DVec3, f64)>,
}

fn same_curve(a: &Arc<Nurbs>, b: &Arc<Nurbs>) -> bool {
    Arc::ptr_eq(a, b) || a == b
}

/// The selected edges as sections: a spline with every edge on it (the
/// stretch of it they cover), the arcs of one circle together, each line
/// alone.
pub(crate) fn sections(model: &Model, edges: &BTreeSet<EdgeId>) -> Vec<Section> {
    let tol = model.tol();
    let mut out: Vec<Section> = Vec::new();
    let mut done: BTreeSet<EdgeId> = BTreeSet::new();
    for id in edges {
        if done.contains(id) {
            continue;
        }
        let Some(e) = model.edge(*id) else { continue };
        let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
        match &e.curve {
            Curve::Line => {
                done.insert(*id);
                if let Some(c) = Nurbs::line(pa, pb) {
                    out.push(Section {
                        curve: c,
                        edges: vec![*id],
                        circle: None,
                    });
                }
            }
            Curve::Spline(s) => {
                let on: Vec<EdgeId> = model
                    .edges()
                    .filter(|(_, x)| {
                        x.curve
                            .spline()
                            .is_some_and(|y| same_curve(&y.curve, &s.curve))
                    })
                    .map(|(k, _)| k)
                    .collect();
                done.extend(on.iter().copied());
                // Only the stretch its edges still cover: a spline cut
                // short (a piece erased) is not the whole curve.
                let (lo, hi) = on
                    .iter()
                    .filter_map(|k| model.edge(*k)?.curve.spline().map(|x| (x.t0, x.t1)))
                    .fold((f64::MAX, f64::MIN), |(lo, hi), (a, b)| {
                        (lo.min(a.min(b)), hi.max(a.max(b)))
                    });
                let (d0, d1) = s.curve.domain();
                let eps = (d1 - d0) * 1e-12;
                let curve = if lo > d0 + eps || hi < d1 - eps {
                    s.curve.between(lo.max(d0), hi.min(d1))
                } else {
                    None
                };
                out.push(Section {
                    curve: curve.unwrap_or_else(|| (*s.curve).clone()),
                    edges: on,
                    circle: None,
                });
            }
            Curve::Arc {
                centre,
                axis,
                radius,
            } => {
                // The selected arcs of this circle, as (start, sweep) about
                // this edge's axis.
                let mut parts: Vec<(EdgeId, DVec3, DVec3, f64)> = Vec::new();
                for k in edges {
                    let Some(x) = model.edge(*k) else { continue };
                    let Curve::Arc {
                        centre: c,
                        axis: n,
                        radius: r,
                    } = &x.curve
                    else {
                        continue;
                    };
                    if !tol.same_point(*c, *centre)
                        || (r - radius).abs() > tol.dist
                        || !tol.parallel_over(*n, *axis, *radius)
                    {
                        continue;
                    }
                    let (xa, xb) = (model.vertex(x.a), model.vertex(x.b));
                    let sweep = x.curve.sweep(xa, xb);
                    if n.dot(*axis) > 0.0 {
                        parts.push((*k, xa, xb, sweep));
                    } else {
                        parts.push((*k, xb, xa, sweep));
                    }
                }
                done.extend(parts.iter().map(|p| p.0));
                let total: f64 = parts.iter().map(|p| p.3).sum();
                // Start where no other part ends (any part of a full circle).
                let start = parts
                    .iter()
                    .find(|p| !parts.iter().any(|q| tol.same_point(q.2, p.1)))
                    .or(parts.first())
                    .map(|p| p.1);
                let whole = (total - std::f64::consts::TAU).abs() < 1e-9;
                if let Some(c) = start.and_then(|s| Nurbs::arc(*centre, *axis, s, total)) {
                    out.push(Section {
                        curve: c,
                        edges: parts.iter().map(|p| p.0).collect(),
                        circle: whole.then_some((*centre, *axis, *radius)),
                    });
                }
            }
        }
    }
    // Lines end to end in one straight run are one section.
    let is_line = |s: &Section| s.curve.degree() == 1 && s.curve.points().len() == 2;
    loop {
        let mut pair = None;
        'find: for i in 0..out.len() {
            for j in i + 1..out.len() {
                if !is_line(&out[i]) || !is_line(&out[j]) {
                    continue;
                }
                let (a, b) = (out[i].curve.points(), out[j].curve.points());
                for (x, y) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                    let (far_a, far_b) = (a[1 - x], b[1 - y]);
                    let straight = (a[x] - far_a)
                        .normalize_or_zero()
                        .dot((far_b - b[y]).normalize_or_zero())
                        > 1.0 - 1e-12;
                    if tol.same_point(a[x], b[y]) && straight {
                        pair = Some((i, j, far_a, far_b));
                        break 'find;
                    }
                }
            }
        }
        let Some((i, j, a, b)) = pair else { break };
        let other = out.remove(j);
        if let Some(c) = Nurbs::line(a, b) {
            out[i].curve = c;
        }
        out[i].edges.extend(other.edges);
    }
    out
}

pub(crate) fn middle(c: &Nurbs) -> DVec3 {
    let pts = c.cage();
    pts.iter().copied().sum::<DVec3>() / pts.len() as f64
}

/// The sections in order (nearest next, from an end), turned the same way
/// and, where they are closed, started beside each other.
fn arranged(mut list: Vec<Section>) -> Vec<Section> {
    let centre = list.iter().map(|s| middle(&s.curve)).sum::<DVec3>() / list.len() as f64;
    let far = (0..list.len())
        .max_by(|a, b| {
            middle(&list[*a].curve)
                .distance(centre)
                .total_cmp(&middle(&list[*b].curve).distance(centre))
                .then(b.cmp(a))
        })
        .unwrap_or(0);
    let mut out = vec![list.swap_remove(far)];
    while !list.is_empty() {
        let last = middle(&out[out.len() - 1].curve);
        let next = (0..list.len())
            .min_by(|a, b| {
                middle(&list[*a].curve)
                    .distance(last)
                    .total_cmp(&middle(&list[*b].curve).distance(last))
                    .then(a.cmp(b))
            })
            .unwrap_or(0);
        out.push(list.swap_remove(next));
    }
    for j in 1..out.len() {
        let prev = out[j - 1].curve.clone();
        let ends = |x: &Nurbs| {
            let (a, b) = x.domain();
            (x.eval(a), x.eval(b))
        };
        let circle = out[j].circle;
        let c = &mut out[j].curve;
        if c.is_closed() && prev.is_closed() {
            if newell(c.cage()).dot(newell(prev.cage())) < 0.0 {
                *c = c.reversed();
            }
            if let Some((centre, axis, radius)) = circle {
                // A whole circle starts beside the last section's start,
                // turning the same way, so the loft does not twist.
                let to = ends(&prev).0 - centre;
                let flat = to - axis * axis.dot(to);
                let axis = if newell(prev.cage()).dot(axis) < 0.0 {
                    -axis
                } else {
                    axis
                };
                if flat.length() > radius * 1e-9
                    && let Some(turned) = Nurbs::arc(
                        centre,
                        axis,
                        centre + flat.normalize() * radius,
                        std::f64::consts::TAU,
                    )
                {
                    *c = turned;
                }
            } else {
                *c = beside(&prev, c);
            }
        } else {
            let ((a0, a1), (b0, b1)) = (ends(&prev), ends(c));
            if a0.distance(b0) + a1.distance(b1) > a0.distance(b1) + a1.distance(b0) {
                *c = c.reversed();
            }
        }
    }
    out
}

/// A closed curve started beside where `prev` starts (its closest point
/// to that start), so a loft between them does not twist. The same shape:
/// a smooth closed curve moves its seam to the nearest control point, any
/// other closed curve exactly to that point.
pub(crate) fn beside(prev: &Nurbs, c: &Nurbs) -> Nurbs {
    let start = prev.eval(prev.domain().0);
    if c.is_periodic() {
        let n = c.cage().len();
        let first = |x: &Nurbs| x.eval(x.domain().0);
        let best = (0..n)
            .min_by(|a, b| {
                first(&c.started_at(*a))
                    .distance(start)
                    .total_cmp(&first(&c.started_at(*b)).distance(start))
            })
            .unwrap_or(0);
        return c.started_at(best);
    }
    let (lo, hi) = c.domain();
    c.restarted(c.closest(start, lo, hi))
        .unwrap_or_else(|| c.clone())
}

/// The edge joining `a` and `b` that passes through `mid`.
fn edge_through(model: &Model, a: VertexId, b: VertexId, mid: DVec3, tol: &Tol) -> Option<EdgeId> {
    model
        .edges()
        .filter(|(_, e)| e.has(a) && e.has(b))
        .find(|(_, e)| {
            e.curve
                .closest_point(model.vertex(e.a), model.vertex(e.b), mid)
                .distance(mid)
                <= tol.dist * 10.0
        })
        .map(|(id, _)| id)
}

/// Lay `surface` into the model as freeform faces: a grid cut across at
/// each of `rows` (values of v, rising, the first and last its ends) and
/// along wherever an edge already on a row has a vertex. Edges already
/// there are used; the rest are made. `flipped` faces them against the
/// surface's own normal.
pub fn lay(
    model: &mut Model,
    surface: NurbsSurface,
    rows: &[(f64, Vec<EdgeId>)],
    flipped: bool,
) -> Result<Vec<FaceId>, &'static str> {
    let s = Arc::new(surface);
    let [(u0, u1), _] = s.domain();
    let mut corners: Vec<DVec3> = s.points().to_vec();
    corners.extend(model.vertices().map(|(_, p)| p));
    let tol = tol_with(model, &corners);
    let eps = (u1 - u0) * 1e-9;
    let closed = rows
        .iter()
        .all(|(v, _)| s.eval(u0, *v).distance(s.eval(u1, *v)) <= tol.dist);
    // Where to cut along u: at the vertices of the edges on the rows.
    let row_curves: Vec<Arc<Nurbs>> = rows
        .iter()
        .map(|(v, _)| s.iso(0, *v).map(Arc::new))
        .collect::<Option<_>>()
        .ok_or(DEGENERATE)?;
    let mut us = vec![u0, u1];
    for ((_, edges), c) in rows.iter().zip(&row_curves) {
        for e in edges {
            let Some(x) = model.edge(*e) else { continue };
            for p in [model.vertex(x.a), model.vertex(x.b)] {
                let u = c.closest(p, u0, u1);
                if c.eval(u).distance(p) <= tol.dist * 10.0
                    && !us.iter().any(|k| (k - u).abs() <= eps)
                {
                    us.push(u);
                }
            }
        }
    }
    us.sort_by(f64::total_cmp);
    if closed && us.len() == 2 {
        us.insert(1, (u0 + u1) / 2.0);
    }
    let vs: Vec<f64> = rows.iter().map(|r| r.0).collect();
    grid_faces(model, s, row_curves, &us, &vs, closed, flipped, &tol)
}

/// Lay `surface` (not closed) into the model as freeform faces cut along
/// at each of `us` and across at each of `vs` (both rising, ends
/// included), as [`lay`] does.
pub(crate) fn lay_at(
    model: &mut Model,
    surface: NurbsSurface,
    us: Vec<f64>,
    vs: &[f64],
    flipped: bool,
) -> Result<Vec<FaceId>, &'static str> {
    let s = Arc::new(surface);
    let mut corners: Vec<DVec3> = s.points().to_vec();
    corners.extend(model.vertices().map(|(_, p)| p));
    let tol = tol_with(model, &corners);
    let row_curves: Vec<Arc<Nurbs>> = vs
        .iter()
        .map(|v| s.iso(0, *v).map(Arc::new))
        .collect::<Option<_>>()
        .ok_or(DEGENERATE)?;
    grid_faces(model, s, row_curves, &us, vs, false, flipped, &tol)
}

#[allow(clippy::too_many_arguments)]
fn grid_faces(
    model: &mut Model,
    s: Arc<NurbsSurface>,
    row_curves: Vec<Arc<Nurbs>>,
    us: &[f64],
    vs: &[f64],
    closed: bool,
    flipped: bool,
    tol: &Tol,
) -> Result<Vec<FaceId>, &'static str> {
    if us.len() < 2 || vs.len() < 2 {
        return Err(DEGENERATE);
    }
    let tol = *tol;
    let (nu, nv) = (us.len(), vs.len());
    // The grid's vertices.
    let mut grid: Vec<Vec<VertexId>> = Vec::with_capacity(nu);
    for (i, u) in us.iter().enumerate() {
        if closed && i == nu - 1 {
            let first = grid[0].clone();
            grid.push(first);
            continue;
        }
        let col = vs
            .iter()
            .map(|v| insert_point(model, s.eval(*u, *v), &tol))
            .collect();
        grid.push(col);
    }
    // An edge of the grid: the one already there, with the surface's own
    // curve put on it, or a new one.
    let edge = |model: &mut Model, a: VertexId, b: VertexId, side: Spline, soft: bool| {
        if a == b {
            return Err(DEGENERATE);
        }
        let mid = side.point(0.5);
        let curve = side_curve(side, tol.dist);
        if let Some(e) = edge_through(model, a, b, mid, &tol) {
            let x = model.edge(e).cloned().ok_or(DEGENERATE)?;
            model.set_curve(e, if x.a == a { curve } else { curve.reversed() });
            return Ok(e);
        }
        let made = match curve {
            Curve::Line => model.add_edge(a, b),
            c => model.add_curve_edge(a, b, c, None),
        }
        .ok_or(DEGENERATE)?;
        if soft {
            model.set_soft(made, true, true);
        }
        Ok(made)
    };
    let mut along: Vec<Vec<EdgeId>> = Vec::new();
    for i in 0..nu - 1 {
        let mut col = Vec::new();
        for (j, c) in row_curves.iter().enumerate() {
            let side = Spline {
                curve: c.clone(),
                t0: us[i],
                t1: us[i + 1],
            };
            col.push(edge(model, grid[i][j], grid[i + 1][j], side, false)?);
        }
        along.push(col);
    }
    let mut across: Vec<Vec<EdgeId>> = Vec::new();
    for (i, u) in us.iter().enumerate() {
        if closed && i == nu - 1 {
            let first = across[0].clone();
            across.push(first);
            continue;
        }
        let c = Arc::new(s.iso(1, *u).ok_or(DEGENERATE)?);
        let inside = closed || (i > 0 && i < nu - 1);
        let mut col = Vec::new();
        for j in 0..nv - 1 {
            let side = Spline {
                curve: c.clone(),
                t0: vs[j],
                t1: vs[j + 1],
            };
            col.push(edge(model, grid[i][j], grid[i][j + 1], side, inside)?);
        }
        across.push(col);
    }
    let mut faces = Vec::new();
    for i in 0..nu - 1 {
        for j in 0..nv - 1 {
            let v = (vs[j], vs[j + 1]);
            let (patch, outer, edges) = if flipped {
                (
                    Patch {
                        surface: s.clone(),
                        u: (us[i + 1], us[i]),
                        v,
                        flipped: true,
                    },
                    vec![
                        grid[i + 1][j],
                        grid[i][j],
                        grid[i][j + 1],
                        grid[i + 1][j + 1],
                    ],
                    vec![along[i][j], across[i][j], along[i][j + 1], across[i + 1][j]],
                )
            } else {
                (
                    Patch {
                        surface: s.clone(),
                        u: (us[i], us[i + 1]),
                        v,
                        flipped: false,
                    },
                    vec![
                        grid[i][j],
                        grid[i + 1][j],
                        grid[i + 1][j + 1],
                        grid[i][j + 1],
                    ],
                    vec![along[i][j], across[i + 1][j], along[i][j + 1], across[i][j]],
                )
            };
            let distinct: BTreeSet<VertexId> = outer.iter().copied().collect();
            if distinct.len() != 4 {
                return Err(DEGENERATE);
            }
            faces.push(model.add_face(Face::with_edges(
                outer,
                vec![],
                Surface::Nurbs(patch),
                vec![edges],
            )));
        }
    }
    Ok(faces)
}

/// Whether three or more open curves join end to end in one ring (every
/// end meets the end of exactly one other curve): a boundary, not sections.
fn is_ring(list: &[Section], tol: f64) -> bool {
    if list.len() < 3 || list.iter().any(|s| s.curve.is_closed()) {
        return false;
    }
    let ends: Vec<DVec3> = list
        .iter()
        .flat_map(|s| {
            let (a, b) = s.curve.domain();
            [s.curve.eval(a), s.curve.eval(b)]
        })
        .collect();
    ends.iter().enumerate().all(|(i, p)| {
        ends.iter()
            .enumerate()
            .filter(|(j, q)| j / 2 != i / 2 && p.distance(**q) <= tol)
            .count()
            == 1
    })
}

/// The model with a surface lofted through the curves the edges are on,
/// and its faces; or why not.
pub fn lofted(
    model: &Model,
    edges: &BTreeSet<EdgeId>,
) -> Result<(Model, Vec<FaceId>), &'static str> {
    let list = sections(model, edges);
    if list.len() < 2 {
        return Err(TWO_CURVES);
    }
    if is_ring(&list, model.tol().dist * 10.0) {
        return Err(RING);
    }
    let list = arranged(list);
    let closed = list[0].curve.is_closed();
    if list.iter().any(|s| s.curve.is_closed() != closed) {
        return Err(MIXED);
    }
    let curves: Vec<Nurbs> = list.iter().map(|s| s.curve.clone()).collect();
    let alike = Nurbs::alike(&curves).ok_or(NOT_ALIKE)?;
    // Across: by how far apart the sections' middles are.
    let mut params = vec![0.0];
    for w in alike.windows(2) {
        let d = middle(&w[0]).distance(middle(&w[1]));
        params.push(params[params.len() - 1] + d);
    }
    let total = params[params.len() - 1];
    if total <= 0.0 || params.windows(2).any(|w| w[1] <= w[0]) {
        return Err(DEGENERATE);
    }
    for p in &mut params {
        *p /= total;
    }
    let surface = prism_kernel::kernel().skin(&alike, &params)?;
    // A tube faces outward.
    let flipped = closed && {
        let [(a, _), (b, _)] = surface.domain();
        let p = surface.eval(a, b);
        surface.normal(a, b).dot(p - middle(&alike[0])) < 0.0
    };
    let rows: Vec<(f64, Vec<EdgeId>)> = params
        .iter()
        .zip(&list)
        .map(|(v, s)| (*v, s.edges.clone()))
        .collect();
    let mut w = model.clone();
    let faces = lay(&mut w, surface, &rows, flipped)?;
    Ok((w, faces))
}

/// Loft through the curves the edges are on ([`lofted`]). The model is
/// left alone when refused.
pub fn loft(model: &mut Model, edges: &BTreeSet<EdgeId>) -> Result<Vec<FaceId>, &'static str> {
    let (w, faces) = lofted(model, edges)?;
    *model = w;
    Ok(faces)
}

pub const NO_AXIS: &str = "Select the outline or curves to turn (any shape, open or closed) and one straight line to turn them about (a line that touches none of the others).";
pub const TOUCHES_AXIS: &str = "Refused: a curve touches or crosses the axis. For shapes that close on the axis, use Follow Me with lines and arcs.";

/// The model with the selected curves turned a full turn about the
/// selected axis line, and the faces made; or why not. The axis is the one
/// selected straight line that touches no other selected edge.
pub fn revolved(
    model: &Model,
    edges: &BTreeSet<EdgeId>,
) -> Result<(Model, Vec<FaceId>), &'static str> {
    let lone: Vec<EdgeId> = edges
        .iter()
        .copied()
        .filter(|id| {
            model.edge(*id).is_some_and(|e| {
                !e.curve.is_curved()
                    && !edges.iter().any(|k| {
                        k != id && model.edge(*k).is_some_and(|x| x.has(e.a) || x.has(e.b))
                    })
            })
        })
        .collect();
    let [axis] = lone.as_slice() else {
        return Err(NO_AXIS);
    };
    let x = model.edge(*axis).ok_or(NO_AXIS)?;
    let origin = model.vertex(x.a);
    let dir = (model.vertex(x.b) - origin)
        .try_normalize()
        .ok_or(NO_AXIS)?;
    let rest: BTreeSet<EdgeId> = edges.iter().copied().filter(|e| e != axis).collect();
    let list = sections(model, &rest);
    if list.is_empty() {
        return Err(NO_AXIS);
    }
    let tol = model.tol();
    let off_axis = |p: DVec3| {
        let d = p - origin;
        (d - dir * d.dot(dir)).length()
    };
    // The profile as seen in a plane through the axis (distance from the
    // axis, height along it), when its curves close up.
    let frame = (origin, dir);
    let image = closed_image(model, &rest, &list, &frame);
    let mut w = model.clone();
    let mut faces = Vec::new();
    for s in list {
        let (lo, hi) = s.curve.domain();
        let touches = (0..=64)
            .map(|i| s.curve.eval(lo + (hi - lo) * i as f64 / 64.0))
            .any(|p| off_axis(p) <= tol.dist * 10.0);
        if touches {
            return Err(TOUCHES_AXIS);
        }
        let surface =
            prism_kernel::kernel().revolve(&s.curve, origin, dir, std::f64::consts::TAU)?;
        // Rows: a circle at each vertex of the curve's edges.
        let mut vs = vec![lo, hi];
        for e in &s.edges {
            let Some(x) = model.edge(*e) else { continue };
            for p in [model.vertex(x.a), model.vertex(x.b)] {
                let v = s.curve.closest(p, lo, hi);
                if !vs.iter().any(|k| (k - v).abs() <= (hi - lo) * 1e-9) {
                    vs.push(v);
                }
            }
        }
        vs.sort_by(f64::total_cmp);
        if s.curve.is_closed() && vs.len() == 2 {
            vs.insert(1, (lo + hi) / 2.0);
        }
        let rows: Vec<(f64, Vec<EdgeId>)> = vs.iter().map(|v| (*v, Vec::new())).collect();
        let [(u0, _), _] = surface.domain();
        // A closed profile makes a solid: every face looks out of it (so
        // the inner wall looks toward the axis). Anything else faces away
        // from the axis.
        let flipped = match &image {
            Some(image) => {
                out_of_profile(&surface, u0, (vs[0], *vs.last().unwrap()), image, &frame)
            }
            None => None,
        }
        .unwrap_or_else(|| {
            let v = (vs[0] + vs[1]) / 2.0;
            let p = surface.eval(u0, v);
            let d = p - origin;
            let out = d - dir * d.dot(dir);
            surface.normal(u0, v).dot(out) < 0.0
        });
        faces.extend(lay(&mut w, surface, &rows, flipped)?);
    }
    Ok((w, faces))
}

/// A point as (distance from the axis, height along it).
fn rz(p: DVec3, (origin, dir): &(DVec3, DVec3)) -> (f64, f64) {
    let d = p - *origin;
    let h = d.dot(*dir);
    ((d - *dir * h).length(), h)
}

/// The turned curves drawn in a plane through the axis, as short
/// straight pieces, if every point where edges meet has an even number of
/// them (the curves close up: a profile with an inside). Turning a curve
/// about an axis gives the same surface as turning that drawing.
fn closed_image(
    model: &Model,
    edges: &BTreeSet<EdgeId>,
    list: &[Section],
    frame: &(DVec3, DVec3),
) -> Option<Vec<[(f64, f64); 2]>> {
    let mut count: BTreeMap<VertexId, u32> = BTreeMap::new();
    for e in edges.iter().filter_map(|e| model.edge(*e)) {
        *count.entry(e.a).or_default() += 1;
        *count.entry(e.b).or_default() += 1;
    }
    if count.is_empty() || count.values().any(|n| n % 2 != 0) {
        return None;
    }
    let mut out = Vec::new();
    for s in list {
        let (lo, hi) = s.curve.domain();
        let pts: Vec<(f64, f64)> = (0..=96)
            .map(|i| rz(s.curve.eval(lo + (hi - lo) * f64::from(i) / 96.0), frame))
            .collect();
        out.extend(pts.windows(2).map(|w| [w[0], w[1]]));
    }
    Some(out)
}

/// Whether the turned surface (as it comes from the kernel) looks into the
/// solid its profile makes, so must be flipped. Tried at several heights
/// along the curve, and the majority decides. None if it can not be told.
fn out_of_profile(
    surface: &NurbsSurface,
    u0: f64,
    (v0, v1): (f64, f64),
    image: &[[(f64, f64); 2]],
    frame: &(DVec3, DVec3),
) -> Option<bool> {
    let (origin, dir) = *frame;
    let size = image
        .iter()
        .flatten()
        .fold(0.0_f64, |m, (r, z)| m.max(r.abs()).max(z.abs()));
    let (mut inward, mut outward) = (0, 0);
    for k in 1..=7 {
        let v = v0 + (v1 - v0) * f64::from(k) / 8.0;
        let p = surface.eval(u0, v);
        let n = surface.normal(u0, v);
        let d = p - origin;
        let radial = (d - dir * d.dot(dir)).try_normalize()?;
        let (nr, nz) = (n.dot(radial), n.dot(dir));
        let len = nr.hypot(nz);
        if len < 1e-9 {
            continue;
        }
        let (r, z) = rz(p, frame);
        // A step off the curve along the normal: inside the profile or not.
        let step = size * 1e-4;
        let probe = (r + nr / len * step, z + nz / len * step);
        if inside(image, probe) {
            inward += 1;
        } else {
            outward += 1;
        }
    }
    (inward != outward).then_some(inward > outward)
}

/// Whether a point is inside the drawing (an odd number of pieces cross a
/// ray from it).
fn inside(image: &[[(f64, f64); 2]], (x, y): (f64, f64)) -> bool {
    // An odd direction, so it meets no corner of a tidy drawing.
    let (dx, dy) = (0.8137, 0.5812);
    let mut hits = 0;
    for [a, b] in image {
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let den = dx * ey - dy * ex;
        if den.abs() < 1e-14 {
            continue;
        }
        let (ax, ay) = (a.0 - x, a.1 - y);
        let t = (ax * ey - ay * ex) / den;
        let s = (ax * dy - ay * dx) / den;
        if t > 0.0 && (0.0..1.0).contains(&s) {
            hits += 1;
        }
    }
    hits % 2 == 1
}

/// Turn curves about an axis line ([`revolved`]). The model is left alone
/// when refused.
pub fn revolve(model: &mut Model, edges: &BTreeSet<EdgeId>) -> Result<Vec<FaceId>, &'static str> {
    let (w, faces) = revolved(model, edges)?;
    *model = w;
    Ok(faces)
}
