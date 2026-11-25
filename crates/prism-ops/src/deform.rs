//! Bend, Twist and Taper (Shapes & Placement part 7, brief 51): a smooth
//! map about an axis, applied to a whole model, accurate to tolerance
//! (DECISIONS #40). Every vertex moves. An edge that is still a line or
//! still a circle stays one, a face that is still flat stays a plane;
//! anything else is refit, an edge as a NURBS curve and a face as a NURBS
//! patch trimmed by its own refit edges, both within a tenth of the
//! model's tolerance (the fitter Shell/Thicken uses), so they still meet.

use std::collections::BTreeMap;

use prism_geom::{Curve, Patch, Spline, Surface};
use prism_math::{DVec2, DVec3, Plane};
use prism_topo::{EdgeId, Face, FaceId, Model};

pub const AXIS: &str = "A deform needs an axis to work about.";
pub const SPAN: &str = "The two ends of a deform must be different places along its axis.";
pub const SCALE: &str = "A taper's far end must be a positive size, not zero or less.";
pub const DIR: &str = "A bend must bend towards a direction square to its axis.";
pub const FOLD: &str = "This bend is too tight: part of the shape would fold through itself.";
pub const CURVED: &str =
    "Cylinders, cones, spheres and tori cannot be bent, twisted or tapered yet.";
pub const FAILED: &str = "Prism could not fit the deformed shape closely enough.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DeformKind {
    /// Bent round a circular arc towards `dir`, by `amount` radians.
    Bend,
    /// Turned about the axis by `amount` radians at the top.
    Twist,
    /// Scaled about the axis by `amount` at the top.
    Taper,
}

impl DeformKind {
    pub fn name(self) -> &'static str {
        match self {
            DeformKind::Bend => "Bend",
            DeformKind::Twist => "Twist",
            DeformKind::Taper => "Taper",
        }
    }
}

/// One deform: an axis through `origin`, the span `low..high` along it,
/// and how much (see [`DeformKind`]). `dir` matters only to a bend.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Deform {
    pub kind: DeformKind,
    pub origin: DVec3,
    pub axis: DVec3,
    pub dir: DVec3,
    pub low: f64,
    pub high: f64,
    pub amount: f64,
}

/// The bend's own frame: the axis, the way it bends (square to the
/// axis, turned round for a negative amount), and the angle, positive.
struct BendFrame {
    a: DVec3,
    d: DVec3,
    e: DVec3,
    phi: f64,
    radius: f64,
}

impl Deform {
    /// No change at all: a bend or twist of nothing, a taper of 1.
    pub fn is_identity(&self) -> bool {
        match self.kind {
            DeformKind::Bend | DeformKind::Twist => self.amount.abs() < 1e-12,
            DeformKind::Taper => (self.amount - 1.0).abs() < 1e-12,
        }
    }

    /// Why this deform is refused on its own, before any shape.
    pub fn check(&self) -> Option<&'static str> {
        let a = self.axis.normalize_or_zero();
        if a == DVec3::ZERO || !self.amount.is_finite() {
            return Some(AXIS);
        }
        if (self.high - self.low).is_nan() || self.high - self.low <= 1e-9 {
            return Some(SPAN);
        }
        match self.kind {
            DeformKind::Taper if self.amount <= 0.0 => Some(SCALE),
            DeformKind::Bend if (self.dir - a * self.dir.dot(a)).length() < 1e-9 => Some(DIR),
            _ => None,
        }
    }

    fn axis(&self) -> DVec3 {
        self.axis.normalize_or_zero()
    }

    fn bend(&self) -> Option<BendFrame> {
        let a = self.axis();
        let d = (self.dir - a * self.dir.dot(a)).normalize_or_zero();
        if self.amount.abs() < 1e-12 || d == DVec3::ZERO {
            return None;
        }
        let (phi, d) = if self.amount < 0.0 {
            (-self.amount, -d)
        } else {
            (self.amount, d)
        };
        Some(BendFrame {
            a,
            d,
            e: a.cross(d),
            phi,
            radius: (self.high - self.low) / phi,
        })
    }

    /// Where the deform takes `p`.
    pub fn map(&self, p: DVec3) -> DVec3 {
        let a = self.axis();
        let rel = p - self.origin;
        let h = rel.dot(a);
        let radial = rel - a * h;
        let frac = (h - self.low) / (self.high - self.low);
        match self.kind {
            DeformKind::Taper => self.origin + a * h + radial * (1.0 + frac * (self.amount - 1.0)),
            DeformKind::Twist => {
                let t = self.amount * frac;
                self.origin + a * h + radial * t.cos() + a.cross(radial) * t.sin()
            }
            DeformKind::Bend => {
                let Some(b) = self.bend() else { return p };
                let along = h - self.low;
                if along <= 0.0 {
                    return p;
                }
                let x = rel.dot(b.d);
                let y = rel.dot(b.e);
                let centre = self.origin + b.a * self.low + b.d * b.radius;
                let span = self.high - self.low;
                let t = b.phi * along.min(span) / span;
                let mut q = centre + (-b.d * t.cos() + b.a * t.sin()) * (b.radius - x) + b.e * y;
                if along > span {
                    q += (b.d * b.phi.sin() + b.a * b.phi.cos()) * (along - span);
                }
                q
            }
        }
    }

    /// Whether `p` is at or past a bend's own centre, where the shape
    /// would fold through itself.
    fn folds(&self, p: DVec3, margin: f64) -> bool {
        let Some(b) = self.bend().filter(|_| self.kind == DeformKind::Bend) else {
            return false;
        };
        let rel = p - self.origin;
        rel.dot(b.a) - self.low > 0.0 && rel.dot(b.d) >= b.radius - margin
    }
}

/// Why `deform` would refuse `d` on `model`, if it would.
pub fn refusal(model: &Model, d: &Deform) -> Option<&'static str> {
    let mut m = model.clone();
    deform(&mut m, d).err()
}

/// Bend, twist or taper every vertex, edge and face of `model` by `d`.
/// Nothing changes when refused.
pub fn deform(model: &mut Model, d: &Deform) -> Result<(), &'static str> {
    if let Some(why) = d.check() {
        return Err(why);
    }
    if d.is_identity() {
        return Ok(());
    }
    let tol = model.tol();
    let eps = tol.dist * 0.1;
    if model.faces().any(|(_, f)| match &f.surface {
        Surface::Plane(_) => false,
        Surface::Nurbs(p) => p.surface.periodic().contains(&true),
        _ => true,
    }) {
        return Err(CURVED);
    }

    // Everything is worked out from the shape as it was, then set at once.
    let curves: BTreeMap<EdgeId, Curve> = model
        .edges()
        .map(|(id, e)| {
            let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
            if (0..=8).any(|i| d.folds(e.curve.point(pa, pb, i as f64 / 8.0), tol.dist)) {
                return Err(FOLD);
            }
            refit_edge(&e.curve, pa, pb, d, eps).map(|c| (id, c))
        })
        .collect::<Result<_, _>>()?;
    let surfaces: BTreeMap<FaceId, Surface> = model
        .faces()
        .map(|(id, f)| refit_face(model, f, d, eps).map(|s| (id, s)))
        .collect::<Result<_, _>>()?;

    let before = model.clone();
    let moved: Vec<_> = model.vertices().map(|(v, p)| (v, d.map(p))).collect();
    for (v, p) in moved {
        model.set_vertex(v, p);
    }
    for (e, c) in curves {
        model.set_curve(e, c);
    }
    for (f, s) in surfaces {
        model.edit_face(f, |g| g.surface = s);
    }
    if !model.bookkeeping_problems().is_empty() {
        *model = before;
        return Err(FAILED);
    }
    Ok(())
}

/// The edge from `pa` to `pb` along `curve`, deformed: still a line or an
/// arc where it is one within `eps`, else a NURBS curve within `eps`.
fn refit_edge(
    curve: &Curve,
    pa: DVec3,
    pb: DVec3,
    d: &Deform,
    eps: f64,
) -> Result<Curve, &'static str> {
    let along = |t: f64| d.map(curve.point(pa, pb, t));
    let (a, b) = (along(0.0), along(1.0));
    let samples = || (1..16).map(|i| i as f64 / 16.0);
    match curve {
        Curve::Line if samples().all(|t| along(t).distance(a.lerp(b, t)) <= eps) => {
            return Ok(Curve::Line);
        }
        Curve::Arc { centre, .. } => {
            let c = d.map(*centre);
            let r = a.distance(c);
            let n = (a - c).cross(along(0.25) - c).normalize_or_zero();
            let on = |p: DVec3| (p.distance(c) - r).abs() <= eps && (p - c).dot(n).abs() <= eps;
            if n != DVec3::ZERO && on(b) && samples().all(|t| on(along(t))) {
                return Ok(Curve::Arc {
                    centre: c,
                    axis: n,
                    radius: r,
                });
            }
        }
        _ => {}
    }
    // Knots allowed to crease where the curve itself does (a spline's own
    // knots) and where it crosses either end of a bend's span.
    let mut nodes = vec![0.0, 1.0];
    // Where it crosses a span's end its speed jumps, so it is only joined
    // there, not smooth.
    let mut corners: Vec<f64> = Vec::new();
    if let Curve::Spline(sp) = curve {
        let (t0, t1) = (sp.t0, sp.t1);
        nodes.extend(
            sp.curve
                .knots()
                .iter()
                .map(|k| (k - t0) / (t1 - t0))
                .filter(|x| *x > 1e-9 && *x < 1.0 - 1e-9),
        );
    }
    if d.kind == DeformKind::Bend {
        let ax = d.axis();
        let height = |t: f64| (curve.point(pa, pb, t) - d.origin).dot(ax);
        const N: usize = 64;
        for i in 0..N {
            let (t0, t1) = (i as f64 / N as f64, (i + 1) as f64 / N as f64);
            let (h0, h1) = (height(t0), height(t1));
            for end in [d.low, d.high] {
                // On or across it (a sample can land exactly on an end).
                if (h0 - end) * (h1 - end) <= 0.0 && h0 != h1 {
                    let t = t0 + (t1 - t0) * (end - h0) / (h1 - h0);
                    if t > 1e-9 && t < 1.0 - 1e-9 {
                        nodes.push(t);
                        corners.push(t);
                    }
                }
            }
        }
    }
    nodes.sort_by(f64::total_cmp);
    nodes.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    corners.retain(|c| nodes.contains(c));
    let (mut fit, _) =
        crate::thicken::twin_fit_corners(&along, &along, &nodes, &corners, eps).ok_or(FAILED)?;
    let last = fit.points().len() - 1;
    fit.set_control(0, a);
    fit.set_control(last, b);
    Ok(Curve::Spline(Spline::whole(fit)))
}

/// The face's surface, deformed: still a plane where it is flat within
/// `eps`, else a NURBS patch within `eps` that its deformed edges lie on.
fn refit_face(model: &Model, f: &Face, d: &Deform, eps: f64) -> Result<Surface, &'static str> {
    let tol = model.tol();
    match &f.surface {
        Surface::Plane(pl) => {
            // Laid out with one way along the axis where the face slopes
            // along it at all, so the ends of a bend's span (where it goes
            // from straight to curved) are lines straight across, where
            // the fit can crease.
            let canonical = pl.frame();
            let along = d.axis() - canonical.normal * d.axis().dot(canonical.normal);
            let frame = if along.length() > 1e-9 {
                let v = along.normalize();
                prism_math::plane::Frame {
                    u: v.cross(canonical.normal),
                    v,
                    ..canonical
                }
            } else {
                canonical
            };
            let loops = model.face_loop_points(f);
            let flat: Vec<DVec2> = loops.iter().flatten().map(|p| frame.to_2d(*p)).collect();
            let (lo, hi) = flat
                .iter()
                .fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(l, h), p| {
                    (l.min(*p), h.max(*p))
                });
            // A little past the face, so its own edges are inside the patch.
            let pad = (hi - lo) * 0.01 + DVec2::splat(tol.dist);
            let (lo, hi) = (lo - pad, hi + pad);
            let at = |s: f64, t: f64| frame.to_3d(lo + (hi - lo) * DVec2::new(s, t));
            let grid: Vec<DVec3> = (0..=6)
                .flat_map(|i| (0..=6).map(move |j| (i as f64 / 6.0, j as f64 / 6.0)))
                .map(|(s, t)| at(s, t))
                .collect();
            if grid.iter().any(|p| d.folds(*p, tol.dist)) {
                return Err(FOLD);
            }
            // Which way the face looks once deformed: its own normal,
            // carried along.
            let middle = at(0.5, 0.5);
            let step = tol.dist * 100.0;
            let looks = d.map(middle + pl.normal * step) - d.map(middle);

            let outer: Vec<DVec3> = loops
                .first()
                .map_or_else(Vec::new, |l| l.iter().map(|p| d.map(*p)).collect());
            let everything = loops
                .iter()
                .flatten()
                .map(|p| d.map(*p))
                .chain(grid.iter().map(|p| d.map(*p)));
            if let Some(q) = Plane::fit(&outer, &tol) {
                let mut all = everything;
                if all.all(|p| (p - q.origin).dot(q.normal).abs() <= eps) {
                    let n = if q.normal.dot(looks) < 0.0 {
                        -q.normal
                    } else {
                        q.normal
                    };
                    return Ok(Surface::Plane(Plane::new(q.origin, n)));
                }
            }
            let g = |s: f64, t: f64| d.map(at(s, t));
            // Height along the axis is a function of t alone in this frame.
            let mut across = vec![0.0, 1.0];
            let mut corner = Vec::new();
            if d.kind == DeformKind::Bend {
                let height = |t: f64| (at(0.0, t) - d.origin).dot(d.axis());
                let (h0, h1) = (height(0.0), height(1.0));
                if (h1 - h0).abs() > 1e-12 {
                    corner = [d.low, d.high]
                        .iter()
                        .map(|end| (end - h0) / (h1 - h0))
                        .filter(|x| *x > 1e-9 && *x < 1.0 - 1e-9)
                        .collect();
                    across.extend(corner.iter().copied());
                    across.sort_by(f64::total_cmp);
                }
            }
            let creases = [vec![0.0, 1.0], across];
            let corners = [vec![], corner];
            let surface =
                crate::thicken::fit_unit_corners(&g, &creases, &corners, eps).ok_or(FAILED)?;
            let flipped = surface.normal(0.5, 0.5).dot(looks) < 0.0;
            Ok(Surface::Nurbs(Patch {
                flipped,
                ..Patch::whole(surface)
            }))
        }
        Surface::Nurbs(p) => {
            let s = &p.surface;
            let ((u0, u1), (v0, v1)) = (p.u, p.v);
            let at = |a: f64, b: f64| s.eval(u0 + (u1 - u0) * a, v0 + (v1 - v0) * b);
            if (0..=6)
                .flat_map(|i| (0..=6).map(move |j| (i as f64 / 6.0, j as f64 / 6.0)))
                .any(|(a, b)| d.folds(at(a, b), tol.dist))
            {
                return Err(FOLD);
            }
            let crease = |k: usize, lo: f64, hi: f64| -> Vec<f64> {
                let mut out = vec![0.0, 1.0];
                out.extend(
                    s.knots()[k]
                        .iter()
                        .map(|x| (x - lo) / (hi - lo))
                        .filter(|x| *x > 1e-9 && *x < 1.0 - 1e-9),
                );
                out.sort_by(f64::total_cmp);
                out.dedup_by(|x, y| (*x - *y).abs() < 1e-12);
                out
            };
            let creases = [crease(0, u0, u1), crease(1, v0, v1)];
            let g = |a: f64, b: f64| d.map(at(a, b));
            let surface = crate::thicken::fit_unit(&g, &creases, eps).ok_or(FAILED)?;
            Ok(Surface::Nurbs(Patch {
                flipped: p.flipped,
                ..Patch::whole(surface)
            }))
        }
        _ => Err(CURVED),
    }
}
