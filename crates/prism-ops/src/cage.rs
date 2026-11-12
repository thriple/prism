//! Cage editing (Freeform NURBS part 3): moving the control points of a
//! NURBS curve or surface. Every edge on the curve, and every face on the
//! surface with its four sides and corners, follows. What is joined to
//! them must still fit: a flat face beside stays flat, or the edit is
//! refused with a reason.

use std::collections::BTreeSet;
use std::sync::Arc;

use prism_geom::{Curve, Nurbs, NurbsSurface, Patch, Spline, Surface};
use prism_math::{DAffine3, DVec3};
use prism_topo::{EdgeId, FaceId, Model, VertexId};
use serde::{Deserialize, Serialize};

/// Whose control points: the curve an edge is on, or the surface a face is
/// on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Cage {
    Curve(EdgeId),
    Surface(FaceId),
}

pub const NO_CAGE: &str =
    "Select a freeform curve or face first: lines, arcs and flat faces have no control points.";
pub const SIDE_OF_FACE: &str =
    "Refused: this curve is the side of a freeform face. Move the face's control points instead.";
pub const FLAT_BESIDE: &str = "Refused: a flat face beside it would bend. Move these points within its plane, or erase that face first.";
pub const NEIGHBOUR: &str =
    "Refused: another curved face or curve is joined here and would come apart.";
pub const CLOSED_ADD: &str =
    "Refused: control points cannot be added round a closed curve or surface yet.";
pub const COLLAPSED: &str = "Refused: the curve or face would shrink to nothing.";

fn curve_of(model: &Model, e: EdgeId) -> Option<Arc<Nurbs>> {
    Some(model.edge(e)?.curve.spline()?.curve.clone())
}

fn surface_of(model: &Model, f: FaceId) -> Option<Arc<NurbsSurface>> {
    Some(model.face(f)?.surface.patch()?.surface.clone())
}

fn same<T: PartialEq>(a: &Arc<T>, b: &Arc<T>) -> bool {
    Arc::ptr_eq(a, b) || a == b
}

/// The cage a selection of one kind points at, if it is one curve or one
/// surface.
pub fn cage_of(model: &Model, edges: &BTreeSet<EdgeId>, faces: &BTreeSet<FaceId>) -> Option<Cage> {
    let mut surfaces = faces
        .iter()
        .filter_map(|f| Some((*f, surface_of(model, *f)?)));
    if let Some((f, s)) = surfaces.next() {
        return surfaces
            .all(|(_, x)| same(&x, &s))
            .then_some(Cage::Surface(f));
    }
    let mut curves = edges
        .iter()
        .filter_map(|e| Some((*e, curve_of(model, *e)?)));
    let (e, c) = curves.next()?;
    curves.all(|(_, x)| same(&x, &c)).then_some(Cage::Curve(e))
}

/// The control points that can be moved, in order (a surface's row by row
/// along u).
pub fn points(model: &Model, cage: Cage) -> Vec<DVec3> {
    match cage {
        Cage::Curve(e) => curve_of(model, e).map_or(Vec::new(), |c| c.cage().to_vec()),
        Cage::Surface(f) => surface_of(model, f).map_or(Vec::new(), |s| {
            let [nu, nv] = s.cage();
            (0..nu)
                .flat_map(|i| (0..nv).map(move |j| (i, j)))
                .map(|(i, j)| s.control(i, j))
                .collect()
        }),
    }
}

/// The lines of the cage, as pairs of places in [`points`].
pub fn lines(model: &Model, cage: Cage) -> Vec<(usize, usize)> {
    match cage {
        Cage::Curve(e) => {
            let Some(c) = curve_of(model, e) else {
                return Vec::new();
            };
            let n = c.cage().len();
            let mut out: Vec<(usize, usize)> = (0..n - 1).map(|i| (i, i + 1)).collect();
            if c.is_periodic() {
                out.push((n - 1, 0));
            }
            out
        }
        Cage::Surface(f) => {
            let Some(s) = surface_of(model, f) else {
                return Vec::new();
            };
            let [nu, nv] = s.cage();
            let [pu, pv] = s.periodic();
            let mut out = Vec::new();
            for i in 0..nu {
                for j in 0..nv {
                    if i + 1 < nu || pu {
                        out.push((i * nv + j, ((i + 1) % nu) * nv + j));
                    }
                    if j + 1 < nv || pv {
                        out.push((i * nv + j, i * nv + (j + 1) % nv));
                    }
                }
            }
            out
        }
    }
}

/// The edges on the cage's curve, or the faces on its surface.
pub fn members(model: &Model, cage: Cage) -> (Vec<EdgeId>, Vec<FaceId>) {
    match cage {
        Cage::Curve(e) => {
            let Some(c) = curve_of(model, e) else {
                return Default::default();
            };
            let es = model
                .edges()
                .filter(|(_, x)| x.curve.spline().is_some_and(|s| same(&s.curve, &c)))
                .map(|(id, _)| id)
                .collect();
            (es, Vec::new())
        }
        Cage::Surface(f) => {
            let Some(s) = surface_of(model, f) else {
                return Default::default();
            };
            let fs = model
                .faces()
                .filter(|(_, x)| x.surface.patch().is_some_and(|p| same(&p.surface, &s)))
                .map(|(id, _)| id)
                .collect();
            (Vec::new(), fs)
        }
    }
}

/// A side of a patch as an edge's curve: a plain line when it is straight.
pub(crate) fn side_curve(s: Spline, tol: f64) -> Curve {
    let (a, b) = (s.point(0.0), s.point(1.0));
    let n = s.curve.steps(s.t0, s.t1).max(4);
    let straight = (1..n)
        .map(|i| s.point(i as f64 / n as f64))
        .all(|p| Curve::Line.closest_point(a, b, p).distance(p) <= tol);
    if straight {
        Curve::Line
    } else {
        Curve::Spline(s)
    }
}

/// The vertices that moved, the edges that changed, and the cage's own
/// faces.
type Changed = (BTreeSet<VertexId>, BTreeSet<EdgeId>, Vec<FaceId>);

/// Put the new curve or surface in place of the old on everything that is
/// on it, moving ends, corners and sides to match. Returns the vertices and
/// edges that changed and the faces that are the cage's own.
fn replace(
    w: &mut Model,
    cage: Cage,
    curve: Option<Arc<Nurbs>>,
    surface: Option<Arc<NurbsSurface>>,
) -> Result<Changed, &'static str> {
    let tol = w.tol().dist;
    let (edges, faces) = members(w, cage);
    let mut moved: BTreeSet<VertexId> = BTreeSet::new();
    let mut touched: BTreeSet<EdgeId> = BTreeSet::new();
    let mut put = |w: &mut Model, v: VertexId, p: DVec3| {
        if w.vertex(v).distance(p) > tol * 1e-6 {
            moved.insert(v);
        }
        w.set_vertex(v, p);
    };
    if let Some(c) = curve {
        for e in edges {
            let Some(x) = w.edge(e).cloned() else {
                continue;
            };
            let Some(old) = x.curve.spline() else {
                continue;
            };
            let s = Spline {
                curve: c.clone(),
                t0: old.t0,
                t1: old.t1,
            };
            put(w, x.a, s.point(0.0));
            put(w, x.b, s.point(1.0));
            w.set_curve(e, Curve::Spline(s));
            touched.insert(e);
        }
    }
    if let Some(s) = surface {
        for f in &faces {
            let Some(g) = w.face(*f).cloned() else {
                continue;
            };
            let Some(old) = g.surface.patch() else {
                continue;
            };
            if g.outer.len() != 4 || !g.holes.is_empty() {
                return Err(NEIGHBOUR);
            }
            let patch = Patch {
                surface: s.clone(),
                ..old.clone()
            };
            for (v, p) in g.outer.iter().zip(patch.corners()) {
                put(w, *v, p);
            }
            let ids = g.loop_edge_ids(0).to_vec();
            for (k, e) in ids.iter().enumerate() {
                let (Some(side), Some(x)) = (patch.side(k), w.edge(*e).cloned()) else {
                    return Err(COLLAPSED);
                };
                let c = side_curve(side, tol);
                w.set_curve(*e, if g.outer[k] == x.a { c } else { c.reversed() });
                touched.insert(*e);
            }
            w.edit_face(*f, |g| g.surface = Surface::Nurbs(patch));
        }
    }
    Ok((moved, touched, faces))
}

/// Whether everything joined to what changed still fits.
fn still_fits(
    w: &mut Model,
    moved: &BTreeSet<VertexId>,
    touched: &BTreeSet<EdgeId>,
    own: &[FaceId],
) -> Result<(), &'static str> {
    let tol = w.tol();
    for e in touched {
        let Some(x) = w.edge(*e) else { continue };
        if w.vertex(x.a).distance(w.vertex(x.b)) <= tol.dist {
            return Err(COLLAPSED);
        }
    }
    let mut replane = Vec::new();
    for (id, f) in w.faces() {
        if own.contains(&id) {
            continue;
        }
        let near = f.loops().flatten().any(|v| moved.contains(v))
            || f.edge_ids().any(|e| touched.contains(&e));
        if !near {
            continue;
        }
        let Some(old) = f.plane().copied() else {
            return Err(NEIGHBOUR);
        };
        // Still flat: in its plane, or in a new one it moved to whole.
        let fits = |pl: &prism_math::Plane| {
            f.loops().flatten().all(|v| pl.contains(w.vertex(*v), &tol))
                && f.edge_ids().all(|e| {
                    w.edge(e).is_some_and(|x| {
                        Surface::Plane(*pl).holds(&x.curve, w.vertex(x.a), w.vertex(x.b), tol.dist)
                    })
                })
        };
        if fits(&old) {
            continue;
        }
        let pts = w.loop_points(f, 0);
        let new = prism_math::Plane::fit(&pts, &tol)
            .map(|pl| {
                if pl.normal.dot(old.normal) < 0.0 {
                    pl.flipped()
                } else {
                    pl
                }
            })
            .filter(|pl| fits(pl));
        match new {
            Some(pl) => replane.push((id, pl)),
            None => return Err(FLAT_BESIDE),
        }
    }
    for (id, pl) in replane {
        w.edit_face(id, |g| g.surface = Surface::Plane(pl));
    }
    // A curve that only ends here would come off its end.
    for (id, x) in w.edges() {
        if !touched.contains(&id)
            && x.curve.is_curved()
            && (moved.contains(&x.a) || moved.contains(&x.b))
        {
            return Err(NEIGHBOUR);
        }
    }
    Ok(())
}

fn edited(
    model: &Model,
    cage: Cage,
    curve: impl FnOnce(&Nurbs) -> Result<Nurbs, &'static str>,
    surface: impl FnOnce(&NurbsSurface) -> Result<NurbsSurface, &'static str>,
) -> Result<Model, &'static str> {
    let mut w = model.clone();
    let (c, s) = match cage {
        Cage::Curve(e) => {
            let old = curve_of(model, e).ok_or(NO_CAGE)?;
            let (es, _) = members(model, cage);
            let on_face = model
                .faces()
                .any(|(_, f)| f.surface.patch().is_some() && f.edge_ids().any(|x| es.contains(&x)));
            if on_face {
                return Err(SIDE_OF_FACE);
            }
            (Some(Arc::new(curve(&old)?)), None)
        }
        Cage::Surface(f) => {
            let old = surface_of(model, f).ok_or(NO_CAGE)?;
            (None, Some(Arc::new(surface(&old)?)))
        }
    };
    let (moved, touched, own) = replace(&mut w, cage, c, s)?;
    still_fits(&mut w, &moved, &touched, &own)?;
    Ok(w)
}

/// The model with the control points `picked` (places in [`points`]) sent
/// through `m`, or why not.
pub fn moved(
    model: &Model,
    cage: Cage,
    picked: &BTreeSet<usize>,
    m: &DAffine3,
) -> Result<Model, &'static str> {
    edited(
        model,
        cage,
        |old| {
            let mut c = old.clone();
            for i in picked {
                if let Some(p) = old.cage().get(*i) {
                    c.set_control(*i, m.transform_point3(*p));
                }
            }
            Ok(c)
        },
        |old| {
            let mut s = old.clone();
            let [nu, nv] = old.cage();
            // A direction that closes on itself with a seam (a turned
            // surface): its first and last rows are one, and move as one.
            let seam_u = nu > 1 && (0..nv).all(|j| old.control(0, j) == old.control(nu - 1, j));
            let seam_v = nv > 1 && (0..nu).all(|i| old.control(i, 0) == old.control(i, nv - 1));
            for k in picked {
                let (i, j) = (k / nv, k % nv);
                if i >= nu {
                    continue;
                }
                let p = m.transform_point3(old.control(i, j));
                let twin = |a: usize, n: usize, seam: bool| match a {
                    0 if seam => vec![0, n - 1],
                    a if seam && a == n - 1 => vec![0, n - 1],
                    a => vec![a],
                };
                for a in twin(i, nu, seam_u) {
                    for b in twin(j, nv, seam_v) {
                        s.set_control(a, b, p);
                    }
                }
            }
            Ok(s)
        },
    )
}

/// Move control points ([`moved`]). The model is left alone when refused.
pub fn move_points(
    model: &mut Model,
    cage: Cage,
    picked: &BTreeSet<usize>,
    m: &DAffine3,
) -> Result<(), &'static str> {
    *model = moved(model, cage, picked, m)?;
    Ok(())
}

/// The middle of the widest knot span.
fn widest(knots: &[f64], degree: usize, count: usize) -> f64 {
    let mut best = (0.0, 0.0);
    for k in degree..count {
        let w = knots[k + 1] - knots[k];
        if w > best.0 {
            best = (w, (knots[k] + knots[k + 1]) / 2.0);
        }
    }
    best.1
}

/// Add control points without changing the shape: one more along a curve,
/// one more row each way across a surface, in the widest gap.
pub fn add_points(model: &mut Model, cage: Cage) -> Result<(), &'static str> {
    let w = edited(
        model,
        cage,
        |old| {
            old.insert_knot(widest(old.knots(), old.degree(), old.points().len()))
                .ok_or(CLOSED_ADD)
        },
        |old| {
            let mut s = old.clone();
            let mut any = false;
            for d in 0..2 {
                let t = widest(s.knots()[d], s.degree()[d], s.count()[d]);
                if let Some(x) = s.insert_knot(d, t) {
                    s = x;
                    any = true;
                }
            }
            any.then_some(s).ok_or(CLOSED_ADD)
        },
    )?;
    *model = w;
    Ok(())
}
