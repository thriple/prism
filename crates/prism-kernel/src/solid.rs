//! The bridge between Prism's solids and the kernel's (Fillet Joins step
//! A, brief 32). Prism's topology is its own and non-manifold; the kernel
//! wants a closed solid with a curve in each face's coordinates for every
//! edge. A [`Region`] is the plain description both sides understand: the
//! faces of one closed solid, their loops, edges and corners, each carrying
//! Prism's id as a key. [`Solid::build`] makes the kernel's solid from it
//! (and checks it); [`Solid::region`] reads a solid back, every entity the
//! kernel kept still carrying its key, and new ones with none.
//!
//! Planes, cylinders, cones, spheres, tori and freeform (NURBS) faces
//! cross, bounded by lines, arcs and splines. Each side's curve in the
//! face's (u, v) is moved by whole periods to lie by the face's middle,
//! and where a loop passes through a pole or a cone's tip an edge of no
//! length is added across it (Solids & CAD part 3c). Anything else is
//! refused in plain words.

use std::collections::BTreeMap;

use arris_geom::{Curve as ACurve, Curve2, Surface as ASurface, pcurve_on};
use arris_math::{Control, Frame, Interval, Meter, Precision, UnitVec3, Vec3};
use arris_ops::{BuildKeys, build};
use arris_topo::builder::{
    Assembly, Builder, EdgeKey, EdgeSpec, FaceSpec, UseSpec, VertexKey, VertexSpec, effective_uses,
};
use arris_topo::entity::EdgeGeometry;
use arris_topo::provenance::ConsumerKey;
use arris_topo::{Body, Model, Orientation, Origin, Role, Shape};
use prism_geom::{Cone, Curve, Cylinder, Nurbs, Sphere, Spline, Surface, Torus};
use prism_math::{DAffine3, DVec3, Plane};

use crate::{back, point};

/// Said when the faces given do not close a solid.
pub const NOT_CLOSED: &str = "Refused: these faces do not close a solid.";
/// Said when the kernel's checker finds a fault in the solid.
pub const FAULTY: &str =
    "Refused: the kernel found a fault in this solid (faces crossing, or pointing inward).";
/// Said for a face the bridge cannot send yet.
pub const SURFACE_NOT_YET: &str = "Refused: a face's surface cannot go to the kernel.";
/// Said for an edge the bridge cannot send.
pub const CURVE_NOT_YET: &str = "Refused: an edge of this solid cannot go to the kernel.";
/// Said for an edge that does not lie on a face it bounds.
pub const OFF_FACE: &str = "Refused: an edge does not lie on the face it bounds.";
/// Said when the kernel gives back something Prism cannot store yet.
pub const CANNOT_STORE: &str = "Refused: the kernel made a shape Prism cannot store yet.";

/// Said when a round is too large for a face beside an edge or for a
/// neighbouring round.
pub const TOO_LARGE: &str =
    "Refused: the size is too large for a face beside an edge, or for a round next to it.";
/// Said when the kernel has no rounding for the faces at an edge.
pub const NO_BLEND: &str =
    "Refused: edges can be rounded only between flat, cylindrical and conical faces so far.";
/// Said when the kernel cannot round the edges for another reason.
pub const NOT_ROUNDED: &str = "Refused: the kernel could not round these edges.";
/// Said when an edge to round is not an edge of the solid.
pub const NOT_IN_SOLID: &str = "Refused: an edge to round is not an edge of this solid.";

/// A corner of a region, with Prism's id when it has one.
#[derive(Clone, Debug, PartialEq)]
pub struct Vertex {
    pub key: Option<u64>,
    pub at: DVec3,
}

/// An edge of a region from corner `a` to corner `b` (indices into
/// `Region::vertices`), along `curve` (an arc runs counter-clockwise about
/// its axis from `a` to `b`, as in Prism).
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub key: Option<u64>,
    pub a: usize,
    pub b: usize,
    pub curve: Curve,
}

/// A face of a region: its surface, facing out of the solid, and its
/// loops, the outer one first. Each loop is its edges (indices into
/// `Region::edges`) in order, each walked from `a` to `b` (true) or back;
/// seen from outside, the outer loop runs counter-clockwise and the holes
/// clockwise.
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    pub key: Option<u64>,
    pub surface: Surface,
    pub loops: Vec<Vec<(usize, bool)>>,
    /// A point inside it, when known: a curved surface's seam is put
    /// opposite it (the middle of its edges can miss it: a half sphere's
    /// lie round its centre).
    pub inside: Option<DVec3>,
}

/// The faces of one closed solid, and the edges and corners they use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Region {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub faces: Vec<Face>,
}

impl Region {
    /// The region moved by `m`: its corners, curves and surfaces. A mirror
    /// turns each loop round, so it still runs counter-clockwise seen from
    /// outside. None if a curve or surface cannot take the move (an arc
    /// scaled unevenly).
    pub fn transformed(&self, m: &DAffine3) -> Option<Region> {
        let mirror = m.matrix3.determinant() < 0.0;
        let turn = |l: &Vec<(usize, bool)>| -> Vec<(usize, bool)> {
            if mirror {
                l.iter().rev().map(|(e, f)| (*e, !f)).collect()
            } else {
                l.clone()
            }
        };
        Some(Region {
            vertices: self
                .vertices
                .iter()
                .map(|v| Vertex {
                    key: v.key,
                    at: m.transform_point3(v.at),
                })
                .collect(),
            edges: self
                .edges
                .iter()
                .map(|e| {
                    Some(Edge {
                        curve: e.curve.transformed(m)?,
                        ..e.clone()
                    })
                })
                .collect::<Option<_>>()?,
            faces: self
                .faces
                .iter()
                .map(|f| {
                    Some(Face {
                        surface: f.surface.transformed(m)?,
                        inside: f.inside.map(|p| m.transform_point3(p)),
                        loops: f.loops.iter().map(turn).collect(),
                        key: f.key,
                    })
                })
                .collect::<Option<_>>()?,
        })
    }
}

/// A solid in the kernel.
pub struct Solid {
    model: Model,
    body: Body,
    provenance: arris_topo::Provenance,
}

/// The keys of one build are in this namespace.
const NAMESPACE: u32 = 1;

/// A key carries its kind above its id, so a new face made from an edge
/// (a round) is never read back as the face with that edge's number.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Vertex = 1,
    Edge = 2,
    Face = 3,
    /// An edge of no length the bridge adds at a pole or a tip.
    Pole = 4,
}

const TAG_SHIFT: u32 = 40;

fn tagged(kind: Kind, key: Option<u64>) -> u64 {
    match key {
        Some(k) if k < (1 << TAG_SHIFT) => ((kind as u64) << TAG_SHIFT) | k,
        // No Prism id: never read back as one.
        _ => u64::MAX,
    }
}

/// The kernel's precision for a model whose tolerance is `tol` (Prism's
/// relative tolerance, set at the boundary in this one place).
pub(crate) fn precision(tol: f64) -> Precision {
    let tol = if tol.is_finite() && tol > 0.0 {
        tol
    } else {
        1e-9
    };
    Precision {
        default_tolerance: tol,
        min_tolerance: Precision::DEFAULT.min_tolerance.min(tol * 1e-3),
        max_tolerance: Precision::DEFAULT.max_tolerance.max(tol * 1e3),
        angular_tolerance: 1e-9,
        parametric_tolerance: tol,
        ..Precision::DEFAULT
    }
}

fn vec(v: DVec3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

fn dvec(v: &Vec3) -> DVec3 {
    DVec3::new(v.x, v.y, v.z)
}

fn orientation(forward: bool) -> Orientation {
    if forward {
        Orientation::Forward
    } else {
        Orientation::Reversed
    }
}

/// An edge's curve and range in the kernel, running from `a` to `b`.
fn edge_curve(curve: &Curve, a: DVec3, b: DVec3) -> Result<(ACurve, Interval), &'static str> {
    match curve {
        Curve::Line => {
            let d = b - a;
            let len = d.length();
            let range = Interval::new(0.0, len).map_err(|_| NOT_CLOSED)?;
            Ok((
                ACurve::Line {
                    origin: point(a),
                    direction: UnitVec3::new_normalize(vec(d)),
                },
                range,
            ))
        }
        Curve::Arc {
            centre,
            axis,
            radius,
        } => {
            let n = axis.normalize();
            let frame =
                Frame::new(point(*centre), vec(n), vec(a - *centre)).map_err(|_| CURVE_NOT_YET)?;
            let (xa, xb) = (a - *centre, b - *centre);
            let mut sweep = n.dot(xa.cross(xb)).atan2(xa.dot(xb));
            if sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
            let range = Interval::new(0.0, sweep).map_err(|_| CURVE_NOT_YET)?;
            Ok((
                ACurve::Circle {
                    frame,
                    radius: *radius,
                },
                range,
            ))
        }
        Curve::Spline(sp) => {
            // The kernel's edge runs along its curve's parameter: one that
            // runs the other way from `a` to `b` is turned round.
            let (c, t0, t1) = if sp.t0 <= sp.t1 {
                ((*sp.curve).clone(), sp.t0, sp.t1)
            } else {
                let (lo, hi) = sp.curve.domain();
                (sp.curve.reversed(), lo + hi - sp.t0, lo + hi - sp.t1)
            };
            let n = crate::curve(&c).ok_or(CURVE_NOT_YET)?;
            let range = Interval::new(t0, t1).map_err(|_| CURVE_NOT_YET)?;
            Ok((ACurve::Nurbs(n), range))
        }
    }
}

/// A side of a face's loop in its surface's (u, v): its edge, whether it
/// is walked forward, its curve there, and where it starts and ends.
type Side = (usize, bool, Curve2, [f64; 2], [f64; 2]);

/// A face's surface in the kernel, and whether the surface's own normal
/// points out of the solid. A cylinder's seam is put opposite `middle`
/// (a point inside the face), so no face of Prism's crosses it.
fn face_surface(s: &Surface, middle: DVec3) -> Result<(ASurface, bool), &'static str> {
    match s {
        Surface::Plane(p) => Ok((
            ASurface::Plane {
                frame: Frame::from_z(point(p.origin), vec(p.normal))
                    .map_err(|_| SURFACE_NOT_YET)?,
            },
            true,
        )),
        Surface::Cylinder(c) => {
            let away = c.axis_point(middle) - middle;
            let frame = Frame::new(point(c.origin), vec(c.axis), vec(away))
                .or_else(|_| Frame::from_z(point(c.origin), vec(c.axis)))
                .map_err(|_| SURFACE_NOT_YET)?;
            Ok((
                ASurface::Cylinder {
                    frame,
                    radius: c.radius,
                },
                c.outward,
            ))
        }
        Surface::Cone(c) => {
            // Placed level with the face's middle, the seam opposite it.
            let h = (middle - c.apex).dot(c.axis).max(1e-9);
            let o = c.apex + c.axis * h;
            let away = o - middle;
            let frame = Frame::new(point(o), vec(c.axis), vec(away))
                .or_else(|_| Frame::from_z(point(o), vec(c.axis)))
                .map_err(|_| SURFACE_NOT_YET)?;
            Ok((
                ASurface::Cone {
                    frame,
                    radius: h * c.half_angle.tan(),
                    half_angle: c.half_angle,
                },
                c.outward,
            ))
        }
        Surface::Sphere(sp) => {
            // The poles on Prism's axis (where its faces have their
            // corners), the seam opposite the face's middle.
            let flat = middle - sp.centre;
            let away = -(flat - sp.axis * flat.dot(sp.axis));
            let frame = Frame::new(point(sp.centre), vec(sp.axis), vec(away))
                .or_else(|_| Frame::from_z(point(sp.centre), vec(sp.axis)))
                .map_err(|_| SURFACE_NOT_YET)?;
            Ok((
                ASurface::Sphere {
                    frame,
                    radius: sp.radius,
                },
                sp.outward,
            ))
        }
        Surface::Torus(t) => {
            let flat = middle - t.centre;
            let away = -(flat - t.axis * flat.dot(t.axis));
            let frame = Frame::new(point(t.centre), vec(t.axis), vec(away))
                .or_else(|_| Frame::from_z(point(t.centre), vec(t.axis)))
                .map_err(|_| SURFACE_NOT_YET)?;
            Ok((
                ASurface::Torus {
                    frame,
                    major_radius: t.major,
                    minor_radius: t.minor,
                },
                t.outward,
            ))
        }
        // As it is: its own normal is along u crossed with along v.
        Surface::Nurbs(p) => {
            let s = &p.surface;
            let [ku, kv] = s.knots();
            let n = arris_geom::NurbsSurface::new(
                s.degree(),
                [ku.to_vec(), kv.to_vec()],
                s.points().iter().map(|x| point(*x)).collect(),
                s.weights().to_vec(),
            )
            .map_err(|_| SURFACE_NOT_YET)?;
            Ok((ASurface::Nurbs(n), !p.flipped))
        }
    }
}

/// The kernel's specs for `region`: its geometry added to `model`, its
/// corners, edges and faces as one shell, in the region's order (the
/// edges of no length at poles and tips after the region's own).
pub(crate) fn specs(
    model: &mut Model,
    region: &Region,
    prec: Precision,
) -> Result<Assembly, &'static str> {
    let t = prec.default_tolerance;
    let mut meter = Meter::default();
    let vertices = region
        .vertices
        .iter()
        .map(|v| VertexSpec::New {
            point: point(v.at),
            tolerance: t,
        })
        .collect();
    let mut curves = Vec::with_capacity(region.edges.len());
    let mut edges = Vec::with_capacity(region.edges.len());
    for e in &region.edges {
        let (pa, pb) = (
            region.vertices.get(e.a).ok_or(NOT_CLOSED)?.at,
            region.vertices.get(e.b).ok_or(NOT_CLOSED)?.at,
        );
        let (c, range) = edge_curve(&e.curve, pa, pb)?;
        edges.push(EdgeSpec::New {
            geometry: EdgeGeometry::Curve {
                curve: model.add_curve(c.clone()),
                range,
            },
            start: VertexKey::New(e.a),
            end: VertexKey::New(e.b),
            tolerance: t,
        });
        curves.push((c, range));
    }
    let mut faces = Vec::with_capacity(region.faces.len());
    for f in &region.faces {
        // The middles of the outer loop's edges (not its corners: a
        // half cylinder's corners average onto the axis).
        let mids: Vec<DVec3> = f
            .loops
            .first()
            .ok_or(NOT_CLOSED)?
            .iter()
            .filter_map(|(e, _)| curves.get(*e))
            .map(|(c, r)| back(&c.point(0.5 * (r.lo() + r.hi()))))
            .collect();
        let middle = f
            .inside
            .unwrap_or(mids.iter().copied().sum::<DVec3>() / mids.len().max(1) as f64);
        let (surface, out) = face_surface(&f.surface, middle)?;
        let period = surface.period();
        let singular = !surface.singularities().is_empty();
        // The face's middle in (u, v): each side is moved by whole
        // periods to lie within half a period of it.
        let anchor = surface
            .project(point(middle))
            .ok()
            .map(|x| [x.uv.x, x.uv.y]);
        let mut loops = Vec::with_capacity(f.loops.len());
        for l in &f.loops {
            // Each side's curve in (u, v), moved by whole periods to
            // start where the one before ended (a face across a seam).
            let mut sides: Vec<Side> = Vec::new();
            for &(e, forward) in l {
                let (c, range) = curves.get(e).ok_or(NOT_CLOSED)?;
                let mut pc = pcurve_on(c, *range, &surface, prec.tolerance(), &mut meter)
                    .map_err(|_| OFF_FACE)?;
                let at = |pc: &Curve2, t: f64| {
                    let q = pc.point(t);
                    [q.x, q.y]
                };
                let (t0, t1) = if forward {
                    (range.lo(), range.hi())
                } else {
                    (range.hi(), range.lo())
                };
                // Judged at the side's middle, near the face's middle;
                // else its start, at the last side's end.
                let target = match (anchor, sides.last()) {
                    (Some(a), _) => Some((at(&pc, 0.5 * (t0 + t1)), a)),
                    (None, Some(last)) => Some((at(&pc, t0), last.4)),
                    (None, None) => None,
                };
                if let Some((from, to)) = target {
                    let mut by = [0.0; 2];
                    for d in 0..2 {
                        if let Some(p) = period[d] {
                            by[d] = ((to[d] - from[d]) / p).round() * p;
                        }
                    }
                    if by != [0.0; 2] {
                        pc = pc.translated(arris_math::Vec2::new(by[0], by[1]));
                    }
                }
                let (a, b) = (at(&pc, t0), at(&pc, t1));
                sides.push((e, forward, pc, a, b));
            }
            let mut uses = Vec::with_capacity(sides.len() + 2);
            let n = sides.len();
            for i in 0..n {
                let (e, forward, pc, _, end) = sides[i].clone();
                uses.push(UseSpec {
                    edge: EdgeKey::New(e),
                    orientation: orientation(forward),
                    pcurve: model.add_curve2(pc),
                });
                // Where the next side starts elsewhere in (u, v) at one
                // point of the surface (a sphere's pole, a cone's tip):
                // an edge of no length across it.
                let next = sides[(i + 1) % n].3;
                let gap = arris_math::Vec2::new(next[0] - end[0], next[1] - end[1]);
                let len = gap.norm();
                if singular && len > 1e-6 {
                    let re = &region.edges[e];
                    let corner = if forward { re.b } else { re.a };
                    let k = edges.len();
                    edges.push(EdgeSpec::New {
                        geometry: EdgeGeometry::Degenerate {
                            range: Interval::new(0.0, len).map_err(|_| NOT_CLOSED)?,
                        },
                        start: VertexKey::New(corner),
                        end: VertexKey::New(corner),
                        tolerance: t,
                    });
                    let line = Curve2::Line {
                        origin: arris_math::Point2::new(end[0], end[1]),
                        direction: arris_math::UnitVec2::new_normalize(gap),
                    };
                    uses.push(UseSpec {
                        edge: EdgeKey::New(k),
                        orientation: Orientation::Forward,
                        pcurve: model.add_curve2(line),
                    });
                }
            }
            loops.push(uses);
        }
        faces.push(FaceSpec::New {
            surface: model.add_surface(surface),
            orientation: orientation(out),
            loops,
            tolerance: t,
        });
    }
    Ok(Assembly {
        vertices,
        edges,
        shells: vec![faces],
    })
}

impl Solid {
    /// The kernel's solid for `region`, checked by the kernel's checker.
    /// `tol` is Prism's tolerance for the model.
    pub fn build(region: &Region, tol: f64) -> Result<Solid, &'static str> {
        Self::build_checked(region, tol).map_err(|(why, _)| why)
    }

    /// What the kernel's checker finds wrong with `region`, in plain words
    /// with counts ("faces cross (2)"); empty when it is a valid solid.
    /// Errs when the region cannot reach the checker at all (a freeform
    /// face, faces that do not close).
    pub fn faults(region: &Region, tol: f64) -> Result<Vec<String>, &'static str> {
        match Self::build_checked(region, tol) {
            Ok(_) => Ok(Vec::new()),
            Err((_, Some(words))) => Ok(words),
            Err((why, None)) => Err(why),
        }
    }

    /// As `build`, with the checker's findings when it rejects the solid.
    fn build_checked(
        region: &Region,
        tol: f64,
    ) -> Result<Solid, (&'static str, Option<Vec<String>>)> {
        let no = |why: &'static str| (why, None);
        let prec = precision(tol);
        let mut model = Model::new(prec).map_err(|_| no(NOT_CLOSED))?;
        let t = prec.default_tolerance;
        let assembly = specs(&mut model, region, prec).map_err(no)?;
        let (builder, slots) =
            Builder::assemble(&model, t, assembly).map_err(|_| no(NOT_CLOSED))?;
        let mut keys = BuildKeys {
            namespace: NAMESPACE,
            shells: vec![0],
            ..BuildKeys::default()
        };
        for (i, v) in region.vertices.iter().enumerate() {
            keys.vertices
                .insert(slots.vertices[i], tagged(Kind::Vertex, v.key));
        }
        for (i, e) in region.edges.iter().enumerate() {
            keys.edges.insert(slots.edges[i], tagged(Kind::Edge, e.key));
        }
        for (i, slot) in slots.edges.iter().enumerate().skip(region.edges.len()) {
            keys.edges.insert(*slot, tagged(Kind::Pole, Some(i as u64)));
        }
        for (i, f) in region.faces.iter().enumerate() {
            keys.faces
                .insert(slots.faces[0][i], tagged(Kind::Face, f.key));
        }
        let (body, provenance) =
            build(&mut model, builder, &keys, &Control::NONE).map_err(|e| match e {
                arris_ops::OpError::Rejected(arris_ops::Rejection::Checker(r)) => {
                    (FAULTY, Some(plain_faults(r.violations())))
                }
                _ => no(NOT_CLOSED),
            })?;
        Ok(Solid {
            model,
            body,
            provenance,
        })
    }

    /// The volume it encloses.
    pub fn volume(&self) -> Result<f64, &'static str> {
        arris_ops::measure::mass_properties(&self.model, self.body, &Control::NONE)
            .map(|m| m.volume)
            .map_err(|_| FAULTY)
    }

    /// Prism's id for a kernel entity of `kind`, if it is one of Prism's
    /// (kept, or a trimmed piece of one).
    fn key(&self, kind: Kind, id: impl Into<arris_topo::EntityId>) -> Option<u64> {
        self.provenance
            .origins(Shape::new(id.into(), Orientation::Forward))
            .into_iter()
            .find_map(|(_, o)| match o {
                Origin::Role(Role::Consumer(ConsumerKey { namespace, key }))
                    if namespace == NAMESPACE && key >> TAG_SHIFT == kind as u64 =>
                {
                    Some(key & ((1 << TAG_SHIFT) - 1))
                }
                _ => None,
            })
    }

    /// Round the edges whose Prism ids are `edges`: a fillet of radius
    /// `size`, or a chamfer `size` back along each face.
    pub fn round(&mut self, edges: &[u64], size: f64, fillet: bool) -> Result<(), &'static str> {
        let m = &self.model;
        let mut picked = Vec::with_capacity(edges.len());
        for &k in edges {
            let e = m
                .edges(self.body)
                .map_err(|_| NOT_IN_SOLID)?
                .into_iter()
                .find(|e| self.key(Kind::Edge, e.id) == Some(k))
                .ok_or(NOT_IN_SOLID)?;
            picked.push(e);
        }
        let done = if fillet {
            arris_ops::fillet(&mut self.model, self.body, &picked, size, &Control::NONE)
        } else {
            arris_ops::chamfer(&mut self.model, self.body, &picked, size, &Control::NONE)
        };
        let (body, p) = done.map_err(|e| match e {
            arris_ops::OpError::Unsupported { .. } => NO_BLEND,
            arris_ops::OpError::Degenerate {
                reason: arris_ops::Reason::Blend(arris_ops::BlendReason::TooLarge),
                ..
            } => TOO_LARGE,
            _ => NOT_ROUNDED,
        })?;
        self.body = body;
        self.provenance = self.provenance.then(&p);
        Ok(())
    }

    /// The solid read back as a region.
    pub fn region(&self) -> Result<Region, &'static str> {
        read_region(&self.model, self.body, &|kind, id| self.key(kind, id))
    }
}

/// Body `body` of `m` read back as a region, each entity keyed by `key`.
pub(crate) fn read_region(
    m: &Model,
    body: Body,
    key: &dyn Fn(Kind, arris_topo::EntityId) -> Option<u64>,
) -> Result<Region, &'static str> {
    let mut out = Region::default();
    let mut vmap: BTreeMap<arris_topo::VertexId, usize> = BTreeMap::new();
    let mut emap: BTreeMap<arris_topo::EdgeId, usize> = BTreeMap::new();
    let mut vertex = |out: &mut Region, id: arris_topo::VertexId| -> Result<usize, &'static str> {
        if let Some(i) = vmap.get(&id) {
            return Ok(*i);
        }
        let at = back(&m.vertex(id).map_err(|_| CANNOT_STORE)?.point());
        out.vertices.push(Vertex {
            key: key(Kind::Vertex, id.into()),
            at,
        });
        vmap.insert(id, out.vertices.len() - 1);
        Ok(out.vertices.len() - 1)
    };
    for fu in m.faces(body).map_err(|_| CANNOT_STORE)? {
        let face = m.face(fu.id).map_err(|_| CANNOT_STORE)?;
        let forward = !fu.orientation.is_reversed();
        let mut loops: Vec<(f64, Vec<(usize, bool)>)> = Vec::new();
        let (mut face_lo, mut face_hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for l in face.loops() {
            let uses = effective_uses(
                fu.orientation,
                l.coedges()
                    .iter()
                    .map(|c| (c.edge(), c.orientation(), c.pcurve())),
            );
            let mut lp = Vec::with_capacity(uses.len());
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for (eid, o, pc) in uses {
                // A degenerate edge (a sphere's pole, where a ball
                // corner meets) has no curve: Prism's loops just pass
                // through the point.
                if m.edge(eid).is_ok_and(|e| e.is_degenerate()) {
                    continue;
                }
                let i = match emap.get(&eid) {
                    Some(i) => *i,
                    None => {
                        let e = m.edge(eid).map_err(|_| CANNOT_STORE)?;
                        let (cid, _) = e.curve().ok_or(CANNOT_STORE)?;
                        let curve = match m.curve(cid).map_err(|_| CANNOT_STORE)? {
                            ACurve::Line { .. } => Curve::Line,
                            ACurve::Circle { frame, radius } => Curve::Arc {
                                centre: back(&frame.origin()),
                                axis: dvec(&frame.z()),
                                radius: *radius,
                            },
                            // An ellipse (a mitre between two rounds)
                            // is stored exactly as a NURBS curve
                            // (DECISIONS #38).
                            ACurve::Ellipse {
                                frame,
                                major_radius,
                                minor_radius,
                            } => {
                                let n = arris_geom::NurbsCurve::ellipse(
                                    frame,
                                    *major_radius,
                                    *minor_radius,
                                    e.range(),
                                )
                                .map_err(|_| CANNOT_STORE)?;
                                spline(&n, None)?
                            }
                            ACurve::Nurbs(n) => {
                                let r = e.range();
                                spline(n, Some((r.lo(), r.hi())))?
                            }
                        };
                        let a = vertex(&mut out, e.start())?;
                        let b = vertex(&mut out, e.end())?;
                        out.edges.push(Edge {
                            key: key(Kind::Edge, eid.into()),
                            a,
                            b,
                            curve,
                        });
                        emap.insert(eid, out.edges.len() - 1);
                        out.edges.len() - 1
                    }
                };
                // The loop's extent in the face's coordinates: the
                // outer loop is the widest.
                if let (Ok(p), Ok(e)) = (m.curve2(pc), m.edge(eid)) {
                    let r = e.range();
                    for k in 0..=4 {
                        let q = curve2_point(p, r.lo() + (r.hi() - r.lo()) * k as f64 / 4.0);
                        lo = [lo[0].min(q[0]), lo[1].min(q[1])];
                        hi = [hi[0].max(q[0]), hi[1].max(q[1])];
                    }
                }
                lp.push((i, !o.is_reversed()));
            }
            loops.push(((hi[0] - lo[0]) * (hi[1] - lo[1]), lp));
            face_lo = [face_lo[0].min(lo[0]), face_lo[1].min(lo[1])];
            face_hi = [face_hi[0].max(hi[0]), face_hi[1].max(hi[1])];
        }
        let (lo, hi) = (face_lo, face_hi);
        let surface = match m.surface(face.surface()).map_err(|_| CANNOT_STORE)? {
            ASurface::Plane { frame } => {
                let n = dvec(&frame.z());
                Surface::Plane(Plane::new(
                    back(&frame.origin()),
                    if forward { n } else { -n },
                ))
            }
            ASurface::Cylinder { frame, radius } => Surface::Cylinder(Cylinder {
                origin: back(&frame.origin()),
                axis: dvec(&frame.z()),
                radius: *radius,
                outward: forward,
            }),
            ASurface::Cone {
                frame,
                radius,
                half_angle,
            } => {
                // P = O + (R + v sin a)(radial) + v cos a Z: the apex
                // is R / tan a below O along Z.
                let z = dvec(&frame.z());
                let (z, a) = if *half_angle < 0.0 {
                    (-z, -half_angle)
                } else {
                    (z, *half_angle)
                };
                if a <= 0.0 || a >= std::f64::consts::FRAC_PI_2 {
                    return Err(CANNOT_STORE);
                }
                let o = back(&frame.origin());
                Surface::Cone(Cone {
                    apex: o - z * (radius / a.tan()),
                    axis: z,
                    half_angle: a,
                    outward: forward,
                })
            }
            ASurface::Sphere { frame, radius } => Surface::Sphere(Sphere {
                centre: back(&frame.origin()),
                axis: dvec(&frame.z()),
                radius: *radius,
                outward: forward,
            }),
            ASurface::Torus {
                frame,
                major_radius,
                minor_radius,
            } => Surface::Torus(Torus {
                centre: back(&frame.origin()),
                axis: dvec(&frame.z()),
                major: *major_radius,
                minor: *minor_radius,
                outward: forward,
            }),
            ASurface::Nurbs(n) => Surface::Nurbs(patch(n, forward)?),
            // An elliptic cylinder: exactly NURBS over the face.
            other => {
                let span = |d: usize| Interval::new(lo[d], hi[d]).map_err(|_| CANNOT_STORE);
                let n = other
                    .to_nurbs([span(0)?, span(1)?])
                    .map_err(|_| CANNOT_STORE)?;
                Surface::Nurbs(patch(&n, forward)?)
            }
        };
        loops.sort_by(|a, b| b.0.total_cmp(&a.0));
        out.faces.push(Face {
            key: key(Kind::Face, fu.id.into()),
            surface,
            loops: loops.into_iter().map(|(_, l)| l).collect(),
            inside: None,
        });
    }
    Ok(out)
}

/// A kernel NURBS surface as one of Prism's patches, all of it, facing
/// along the surface's own normal when `forward`.
fn patch(n: &arris_geom::NurbsSurface, forward: bool) -> Result<prism_geom::Patch, &'static str> {
    let [ku, kv] = n.knots();
    let s = prism_geom::NurbsSurface::new(
        n.degree(),
        [ku.to_vec(), kv.to_vec()],
        n.counts(),
        n.control_points().iter().map(back).collect(),
        n.weights().to_vec(),
        [false; 2],
    )
    .ok_or(CANNOT_STORE)?;
    let mut p = prism_geom::Patch::whole(s);
    p.flipped = !forward;
    Ok(p)
}

/// A kernel NURBS curve as one of Prism's splines, over `range` of it (all
/// of it when None).
fn spline(n: &arris_geom::NurbsCurve, range: Option<(f64, f64)>) -> Result<Curve, &'static str> {
    let k = n.knots();
    let (t0, t1) = range.unwrap_or((k[0], k[k.len() - 1]));
    let c = Nurbs::new(
        n.degree(),
        k.to_vec(),
        n.control_points().iter().map(back).collect(),
        n.weights().to_vec(),
        false,
    )
    .ok_or(CANNOT_STORE)?;
    Ok(Curve::Spline(Spline {
        curve: std::sync::Arc::new(c),
        t0,
        t1,
    }))
}

fn curve2_point(c: &Curve2, t: f64) -> [f64; 2] {
    let p = c.point(t);
    [p.x, p.y]
}

/// The checker's findings in plain words, each kind once with its count.
fn plain_faults<V: std::fmt::Debug>(found: &[V]) -> Vec<String> {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for v in found {
        let name = format!("{v:?}");
        let kind: String = name.chars().take_while(|c| c.is_alphanumeric()).collect();
        *counts.entry(fault_words(&kind)).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(w, n)| format!("{w} ({n})"))
        .collect()
}

/// One kind of checker finding, in words.
fn fault_words(kind: &str) -> &'static str {
    match kind {
        "ShellOpen" | "LoopOpen" | "PcurveGap" => "a gap in the surface",
        "LoopsIntersect" | "EdgeSelfIntersects" | "Overlap" | "FaceMalformed" => "faces cross",
        "InsideOut" | "Orientation" | "SameOrientation" => "faces point inward",
        "VertexOffEdge" | "VertexOffFace" | "PcurveOffCurve" | "EdgeEnds" => {
            "an edge or point off the face it bounds"
        }
        "VertexTolerance" | "EdgeTolerance" | "FaceTolerance" | "ToleranceBound" => {
            "a gap wider than the tolerance"
        }
        "DegenerateEdge" | "ZeroArea" | "Empty" => "an edge or face of no size",
        "EdgeUses" | "FaceUsedTwice" | "EdgeReusedInFace" | "VertexOverused" => {
            "an edge used by too many faces"
        }
        _ => "a fault in how the solid is joined",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box from `lo` to `hi`, every entity keyed: corners 0..8, edges
    /// 100.., faces 200...
    fn cube(lo: DVec3, hi: DVec3) -> Region {
        let c = |i: usize| {
            DVec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            )
        };
        let vertices = (0..8)
            .map(|i| Vertex {
                key: Some(i as u64),
                at: c(i),
            })
            .collect();
        let mut edges: Vec<Edge> = Vec::new();
        // Each face, counter-clockwise seen from outside.
        let quads: [([usize; 4], DVec3); 6] = [
            ([0, 2, 3, 1], -DVec3::Z),
            ([4, 5, 7, 6], DVec3::Z),
            ([0, 1, 5, 4], -DVec3::Y),
            ([2, 6, 7, 3], DVec3::Y),
            ([0, 4, 6, 2], -DVec3::X),
            ([1, 3, 7, 5], DVec3::X),
        ];
        let mut faces = Vec::new();
        for (k, (q, n)) in quads.iter().enumerate() {
            let mut lp = Vec::new();
            for i in 0..4 {
                let (a, b) = (q[i], q[(i + 1) % 4]);
                let found = edges
                    .iter()
                    .position(|e| (e.a, e.b) == (a, b) || (e.a, e.b) == (b, a));
                let i = found.unwrap_or_else(|| {
                    edges.push(Edge {
                        key: Some(100 + edges.len() as u64),
                        a,
                        b,
                        curve: Curve::Line,
                    });
                    edges.len() - 1
                });
                lp.push((i, edges[i].a == a));
            }
            faces.push(Face {
                key: Some(200 + k as u64),
                surface: Surface::Plane(Plane::new(c(q[0]), *n)),
                loops: vec![lp],
                inside: None,
            });
        }
        Region {
            vertices,
            edges,
            faces,
        }
    }

    #[test]
    fn a_box_goes_to_the_kernel_and_comes_back_with_its_keys() {
        let r = cube(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0));
        let s = Solid::build(&r, 1e-9).unwrap();
        assert!((s.volume().unwrap() - 24.0).abs() < 1e-9);
        let back = s.region().unwrap();
        assert_eq!(back.vertices.len(), 8);
        assert_eq!(back.edges.len(), 12);
        assert_eq!(back.faces.len(), 6);
        let mut keys: Vec<u64> = back.faces.iter().filter_map(|f| f.key).collect();
        keys.sort();
        assert_eq!(keys, (200..206).collect::<Vec<_>>());
        for v in &back.vertices {
            let k = v.key.unwrap() as usize;
            assert!(v.at.distance(r.vertices[k].at) < 1e-12);
        }
        for f in &back.faces {
            let orig = &r.faces[(f.key.unwrap() - 200) as usize];
            let (Surface::Plane(p), Surface::Plane(q)) = (&f.surface, &orig.surface) else {
                panic!()
            };
            assert!(p.normal.distance(q.normal) < 1e-12, "faces still face out");
        }
    }

    #[test]
    fn an_open_box_or_an_inside_out_one_is_refused() {
        let mut r = cube(DVec3::ZERO, DVec3::ONE);
        r.faces.pop();
        assert_eq!(Solid::build(&r, 1e-9).err(), Some(NOT_CLOSED));
        let mut r = cube(DVec3::ZERO, DVec3::ONE);
        for f in &mut r.faces {
            let Surface::Plane(p) = &mut f.surface else {
                panic!()
            };
            p.normal = -p.normal;
            for l in &mut f.loops {
                l.reverse();
                for u in l.iter_mut() {
                    u.1 = !u.1;
                }
            }
        }
        assert!(Solid::build(&r, 1e-9).is_err());
    }
}
