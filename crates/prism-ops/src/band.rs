//! Curved sides that follow a moved end (owner, 2026-11-25: scaling,
//! sliding or turning the top of a cylinder is everyday SketchUp; "it's
//! just a loop on either end with a field between them").
//!
//! A curved face with no holes whose outline is two runs, one moving and
//! one staying, joined by two straight lines (the side of a cylinder or a
//! cone, or a band an earlier edit made), is a band. When a Move, Rotate
//! or Scale takes one run and not the other, the band does not bend: it
//! is rebuilt as the ruled surface between the run that stays and the run
//! where it lands. That is exact, not fitted: each run is an exact NURBS
//! curve (an arc is a rational one), the moved run is the same curve sent
//! through the same affine map, and a ruled surface between two such
//! curves holds every straight line between their matching points. Where
//! the result is still a cylinder or a cone (the top slid along the axis,
//! or scaled evenly about it), it stays one. Under a scale that is not
//! even, the moved run's arcs become ellipses, stored exactly as NURBS
//! (DECISIONS #38) rather than as segments.

use prism_geom::{Cone, Curve, Cylinder, Nurbs, Patch, Spline, Surface};
use prism_math::{DAffine3, DVec3};
use prism_topo::{EdgeId, FaceId, Model, VertexId};
use std::collections::BTreeSet;

/// A band to rebuild after the move.
#[derive(Clone, Debug)]
pub(crate) struct Band {
    face: FaceId,
    /// The run that stays, walked the face's way.
    fixed: Nurbs,
    /// The run that moves, walked so that its points match `fixed`'s.
    moving: Nurbs,
    /// A point of the fixed run and the face's normal there, before.
    side: (DVec3, DVec3),
    /// The fixed run's circle, when it is one arc (centre, axis, radius).
    fixed_arc: Option<(DVec3, DVec3, f64)>,
    /// The moving run, when it is one arc: its edge.
    moving_arc: Option<EdgeId>,
}

impl Band {
    pub(crate) fn face(&self) -> FaceId {
        self.face
    }
}

/// The part of `edge` as an exact NURBS curve, walked from `from`.
fn piece(model: &Model, edge: EdgeId, from: VertexId) -> Option<Nurbs> {
    let e = model.edge(edge)?;
    let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
    let n = match &e.curve {
        Curve::Line => Nurbs::line(pa, pb)?,
        Curve::Arc { centre, axis, .. } => Nurbs::arc(*centre, *axis, pa, e.curve.sweep(pa, pb))?,
        Curve::Spline(s) => {
            let (lo, hi) = (s.t0.min(s.t1), s.t0.max(s.t1));
            let part = s.curve.between(lo, hi)?;
            if s.t1 < s.t0 { part.reversed() } else { part }
        }
    };
    Some(if from == e.a { n } else { n.reversed() })
}

/// The run of sides as one curve.
fn run_curve(model: &Model, sides: &[(VertexId, EdgeId)]) -> Option<Nurbs> {
    let pieces: Option<Vec<Nurbs>> = sides.iter().map(|(v, e)| piece(model, *e, *v)).collect();
    Nurbs::joined(&pieces?)
}

/// `face` as a band for moving `vs`, if it is one.
pub(crate) fn of_face(model: &Model, face: FaceId, vs: &BTreeSet<VertexId>) -> Option<Band> {
    let f = model.face(face)?;
    if !f.is_curved() || !f.holes.is_empty() {
        return None;
    }
    let sides: Vec<_> = f.sides().collect();
    let moved = |v: VertexId| vs.contains(&v);
    // The two lines from one run to the other.
    let crossing: Vec<usize> = (0..sides.len())
        .filter(|i| moved(sides[*i].from) != moved(sides[*i].to))
        .collect();
    let [x, y] = crossing.as_slice() else {
        return None;
    };
    for i in [x, y] {
        if !model
            .edge(sides[*i].edge)
            .is_some_and(|e| e.curve == Curve::Line)
        {
            return None;
        }
    }
    // Start after the line from the moving run to the fixed one.
    let into_fixed = if moved(sides[*x].from) { *x } else { *y };
    let n = sides.len();
    let mut fixed = Vec::new();
    let mut moving = Vec::new();
    let mut in_fixed = true;
    for k in 1..n {
        let s = &sides[(into_fixed + k) % n];
        if moved(s.from) != moved(s.to) {
            in_fixed = false;
            continue;
        }
        if in_fixed {
            fixed.push((s.from, s.edge));
        } else {
            moving.push((s.from, s.edge));
        }
    }
    if fixed.is_empty() || moving.is_empty() {
        return None;
    }
    let fixed_curve = run_curve(model, &fixed)?;
    // Walked back, so that its start meets the fixed run's start.
    let moving_curve = run_curve(model, &moving)?.reversed();
    let moving_curve = matched(&fixed_curve, &moving_curve)?;
    let (t0, t1) = fixed_curve.domain();
    let p = fixed_curve.eval((t0 + t1) / 2.0);
    let arc_of = |run: &[(VertexId, EdgeId)]| match run {
        [(_, e)] => model.edge(*e).and_then(|x| match x.curve {
            Curve::Arc {
                centre,
                axis,
                radius,
            } => Some((*e, (centre, axis, radius))),
            _ => None,
        }),
        _ => None,
    };
    Some(Band {
        face,
        fixed: fixed_curve,
        moving: moving_curve,
        side: (p, f.surface.normal_at(p)),
        fixed_arc: arc_of(&fixed).map(|x| x.1),
        moving_arc: arc_of(&moving).map(|x| x.0),
    })
}

/// Knots this close are the same: a curve parameter, not a distance.
const SAME_KNOT: f64 = 1e-9;

/// `b` with `a`'s knots, if the two are built alike (the same degree,
/// count and, to rounding, knots): a ruled surface joins them point for
/// point.
fn matched(a: &Nurbs, b: &Nurbs) -> Option<Nurbs> {
    let same = a.degree() == b.degree()
        && a.points().len() == b.points().len()
        && a.knots().len() == b.knots().len()
        && a.knots()
            .iter()
            .zip(b.knots())
            .all(|(x, y)| (x - y).abs() <= SAME_KNOT);
    if !same {
        return None;
    }
    Nurbs::new(
        a.degree(),
        a.knots().to_vec(),
        b.points().to_vec(),
        b.weights().to_vec(),
        false,
    )
}

/// The curved faces that `vs` moves partly, as bands; None if any of them
/// is not one (then the move is refused as bending a curved face).
pub(crate) fn plan(model: &Model, vs: &BTreeSet<VertexId>) -> Option<Vec<Band>> {
    let mut out = Vec::new();
    for (id, f) in model.faces() {
        if !f.is_curved() {
            continue;
        }
        let n = f.loops().flatten().filter(|v| vs.contains(v)).count();
        if n == 0 || n == f.loops().flatten().count() {
            continue;
        }
        out.push(of_face(model, id, vs)?);
    }
    Some(out)
}

/// Whether `face` is a band `vs` can move one end of.
pub(crate) fn is_band(model: &Model, face: FaceId, vs: &BTreeSet<VertexId>) -> bool {
    of_face(model, face, vs).is_some()
}

/// Before a move that would not keep circles round (an uneven scale),
/// each band's moving arcs become the same curves as exact NURBS, which
/// the move then carries exactly (ellipses, DECISIONS #38), rather than
/// segments.
pub(crate) fn exact_ends(model: &mut Model, vs: &BTreeSet<VertexId>, m: &DAffine3) {
    let Some(bands) = plan(model, vs) else {
        return;
    };
    let mut arcs: BTreeSet<EdgeId> = BTreeSet::new();
    for b in &bands {
        let Some(f) = model.face(b.face) else {
            continue;
        };
        for s in f.sides() {
            // An arc the move keeps round (even in its own plane) stays
            // an arc.
            if vs.contains(&s.from)
                && vs.contains(&s.to)
                && model
                    .edge(s.edge)
                    .is_some_and(|e| e.curve.is_arc() && e.curve.transformed(m).is_none())
            {
                arcs.insert(s.edge);
            }
        }
    }
    for e in arcs {
        let Some(edge) = model.edge(e) else {
            continue;
        };
        let (pa, pb) = (model.vertex(edge.a), model.vertex(edge.b));
        let Curve::Arc { centre, axis, .. } = edge.curve else {
            continue;
        };
        if let Some(n) = Nurbs::arc(centre, axis, pa, edge.curve.sweep(pa, pb)) {
            model.set_curve(e, Curve::Spline(Spline::whole(n)));
        }
    }
}

/// A cylinder or cone through the fixed circle and the moved one, when
/// they share an axis (the top slid along it, or scaled evenly about it).
///
/// Same place, same size and same axis are judged by the document's
/// tolerance (the axes over the larger of the two radii and the height).
fn turned(
    fixed: (DVec3, DVec3, f64),
    moved: (DVec3, DVec3, f64),
    tol: &prism_math::Tol,
) -> Option<Surface> {
    let ((c1, a1, r1), (c2, a2, r2)) = (fixed, moved);
    let d = c2 - c1;
    let h = d.length();
    let scale = r1.max(r2).max(h);
    if h <= tol.dist || !tol.parallel_over(a1, a2, scale) || d.cross(a1).length() > tol.dist {
        return None;
    }
    let eps = tol.dist;
    let u = d / h;
    if (r1 - r2).abs() <= eps {
        return Some(Surface::Cylinder(Cylinder {
            origin: c1,
            axis: u,
            radius: r1,
            outward: true,
        }));
    }
    let slope = (r2 - r1) / h;
    let half_angle = slope.abs().atan();
    if half_angle <= 1e-9 || half_angle >= std::f64::consts::FRAC_PI_2 - 1e-9 {
        return None;
    }
    Some(Surface::Cone(Cone {
        apex: c1 - u * (r1 / slope),
        axis: if slope > 0.0 { u } else { -u },
        half_angle,
        outward: true,
    }))
}

/// Turn a turned surface's side to face along `n` at `p`.
fn facing(s: Surface, p: DVec3, n: DVec3) -> Surface {
    if s.normal_at(p).dot(n) < 0.0 {
        s.flipped()
    } else {
        s
    }
}

/// After the move: each band's face takes the surface between its run
/// that stayed and the one that moved by `m`. False if one cannot be built
/// (nothing is changed for that face then; the model check will say).
pub(crate) fn rebuild(model: &mut Model, bands: &[Band], m: &DAffine3) -> bool {
    let tol = model.tol();
    let mut ok = true;
    for b in bands {
        let Some(f) = model.face(b.face) else {
            continue;
        };
        // Points along every side, not just the corners: a top turned
        // about the axis keeps its corners on the cylinder, but its lines
        // up now lean across it.
        let samples: Vec<DVec3> = f
            .edge_ids()
            .filter_map(|e| model.edge(e))
            .flat_map(|e| {
                let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
                [0.0, 0.25, 0.5, 0.75, 1.0].map(|t| e.curve.point(pa, pb, t))
            })
            .collect();
        let (p, n) = b.side;
        let on = |s: &Surface| samples.iter().all(|q| s.distance(*q).abs() <= tol.dist);
        // Still a cylinder or cone: the moved run is still one arc, on the
        // same axis.
        let moved_arc = b.moving_arc.and_then(|e| match model.edge(e)?.curve {
            Curve::Arc {
                centre,
                axis,
                radius,
            } => Some((centre, axis, radius)),
            _ => None,
        });
        let turned = match (b.fixed_arc, moved_arc) {
            (Some(x), Some(y)) => turned(x, y, &tol)
                .map(|s| facing(s, p, n))
                .filter(|s| on(s)),
            _ => None,
        };
        let surface = turned.or_else(|| {
            let moving = b.moving.transformed(m);
            let ruled = crate::thicken::ruled(&b.fixed, &moving)?;
            let patch = Patch::whole(ruled);
            let (t0, t1) = b.fixed.domain();
            let along = patch.normal((t0 + t1) / 2.0, 0.0);
            let patch = if along.dot(n) < 0.0 {
                patch.flipped()
            } else {
                patch
            };
            let s = Surface::Nurbs(patch);
            on(&s).then_some(s)
        });
        match surface {
            Some(s) => {
                model.edit_face(b.face, |f| f.surface = s);
            }
            None => ok = false,
        }
    }
    ok
}
