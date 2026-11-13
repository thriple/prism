//! Loft (Freeform NURBS part 4): a surface through two or more curves, and
//! the laying of any NURBS surface into the model as freeform faces.
//!
//! The curves are made alike exactly (`Nurbs::alike`), the kernel skins a
//! surface through them, and the surface is laid as a grid of faces: cut
//! along its length wherever a section has a vertex, and across at every
//! section, so each section's own edges become the faces' sides.

use std::collections::BTreeSet;
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
}

fn same_curve(a: &Arc<Nurbs>, b: &Arc<Nurbs>) -> bool {
    Arc::ptr_eq(a, b) || a == b
}

/// The selected edges as sections: a spline with every edge on it, the
/// arcs of one circle together, each line alone.
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
                out.push(Section {
                    curve: (*s.curve).clone(),
                    edges: on,
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
                        || n.cross(*axis).length() > 1e-9
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
                if let Some(c) = start.and_then(|s| Nurbs::arc(*centre, *axis, s, total)) {
                    out.push(Section {
                        curve: c,
                        edges: parts.iter().map(|p| p.0).collect(),
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
        let c = &mut out[j].curve;
        let ends = |x: &Nurbs| {
            let (a, b) = x.domain();
            (x.eval(a), x.eval(b))
        };
        if c.is_closed() && prev.is_closed() {
            if newell(c.cage()).dot(newell(prev.cage())) < 0.0 {
                *c = c.reversed();
            }
            if c.is_periodic() {
                let start = ends(&prev).0;
                let n = c.cage().len();
                let best = (0..n)
                    .min_by(|a, b| {
                        ends(&c.started_at(*a))
                            .0
                            .distance(start)
                            .total_cmp(&ends(&c.started_at(*b)).0.distance(start))
                    })
                    .unwrap_or(0);
                *c = c.started_at(best);
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
    let (nu, nv) = (us.len(), rows.len());
    // The grid's vertices.
    let mut grid: Vec<Vec<VertexId>> = Vec::with_capacity(nu);
    for (i, u) in us.iter().enumerate() {
        if closed && i == nu - 1 {
            let first = grid[0].clone();
            grid.push(first);
            continue;
        }
        let col = rows
            .iter()
            .map(|(v, _)| insert_point(model, s.eval(*u, *v), &tol))
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
                t0: rows[j].0,
                t1: rows[j + 1].0,
            };
            col.push(edge(model, grid[i][j], grid[i][j + 1], side, inside)?);
        }
        across.push(col);
    }
    let mut faces = Vec::new();
    for i in 0..nu - 1 {
        for j in 0..nv - 1 {
            let v = (rows[j].0, rows[j + 1].0);
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

pub const NO_AXIS: &str = "Select the curves to turn and one straight line to turn them about (a line that touches none of the others).";
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
        // Facing away from the axis.
        let [(u0, _), _] = surface.domain();
        let v = (vs[0] + vs[1]) / 2.0;
        let p = surface.eval(u0, v);
        let d = p - origin;
        let out = d - dir * d.dot(dir);
        let flipped = surface.normal(u0, v).dot(out) < 0.0;
        faces.extend(lay(&mut w, surface, &rows, flipped)?);
    }
    Ok((w, faces))
}

/// Turn curves about an axis line ([`revolved`]). The model is left alone
/// when refused.
pub fn revolve(model: &mut Model, edges: &BTreeSet<EdgeId>) -> Result<Vec<FaceId>, &'static str> {
    let (w, faces) = revolved(model, edges)?;
    *model = w;
    Ok(faces)
}
