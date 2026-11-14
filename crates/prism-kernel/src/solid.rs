//! The bridge between Prism's solids and the kernel's (Fillet Joins step
//! A, brief 32). Prism's topology is its own and non-manifold; the kernel
//! wants a closed solid with a curve in each face's coordinates for every
//! edge. A [`Region`] is the plain description both sides understand: the
//! faces of one closed solid, their loops, edges and corners, each carrying
//! Prism's id as a key. [`Solid::build`] makes the kernel's solid from it
//! (and checks it); [`Solid::region`] reads a solid back, every entity the
//! kernel kept still carrying its key, and new ones with none.
//!
//! Only planes and cylinders bounded by lines and arcs cross so far: what
//! Fillet Joins needs. Everything else is refused in plain words.

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
use prism_geom::{Curve, Cylinder, Surface};
use prism_math::{DVec3, Plane};

use crate::{back, point};

/// Said when the faces given do not close a solid.
pub const NOT_CLOSED: &str = "Refused: these faces do not close a solid.";
/// Said when the kernel's checker finds a fault in the solid.
pub const FAULTY: &str =
    "Refused: the kernel found a fault in this solid (faces crossing, or pointing inward).";
/// Said for a face the bridge cannot send yet.
pub const SURFACE_NOT_YET: &str =
    "Refused: only flat and cylindrical faces can go to the kernel so far.";
/// Said for an edge the bridge cannot send.
pub const CURVE_NOT_YET: &str =
    "Refused: only straight lines and arcs can go to the kernel so far.";
/// Said for an edge that does not lie on a face it bounds.
pub const OFF_FACE: &str = "Refused: an edge does not lie on the face it bounds.";
/// Said when the kernel gives back something Prism cannot store yet.
pub const CANNOT_STORE: &str = "Refused: the kernel made a shape Prism cannot store yet.";

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
}

/// The faces of one closed solid, and the edges and corners they use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Region {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub faces: Vec<Face>,
}

/// A solid in the kernel.
pub struct Solid {
    model: Model,
    body: Body,
    provenance: arris_topo::Provenance,
}

/// The keys of one build are in this namespace.
const NAMESPACE: u32 = 1;

/// The kernel's precision for a model whose tolerance is `tol` (Prism's
/// relative tolerance, set at the boundary in this one place).
fn precision(tol: f64) -> Precision {
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
        Curve::Spline(_) => Err(CURVE_NOT_YET),
    }
}

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
        _ => Err(SURFACE_NOT_YET),
    }
}

impl Solid {
    /// The kernel's solid for `region`, checked by the kernel's checker.
    /// `tol` is Prism's tolerance for the model.
    pub fn build(region: &Region, tol: f64) -> Result<Solid, &'static str> {
        let prec = precision(tol);
        let mut model = Model::new(prec).map_err(|_| NOT_CLOSED)?;
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
            let middle = mids.iter().copied().sum::<DVec3>() / mids.len().max(1) as f64;
            let (surface, out) = face_surface(&f.surface, middle)?;
            let mut loops = Vec::with_capacity(f.loops.len());
            for l in &f.loops {
                let mut uses = Vec::with_capacity(l.len());
                for &(e, forward) in l {
                    let (c, range) = curves.get(e).ok_or(NOT_CLOSED)?;
                    let pc = pcurve_on(c, *range, &surface, prec.tolerance(), &mut meter)
                        .map_err(|_| OFF_FACE)?;
                    uses.push(UseSpec {
                        edge: EdgeKey::New(e),
                        orientation: orientation(forward),
                        pcurve: model.add_curve2(pc),
                    });
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
        let assembly = Assembly {
            vertices,
            edges,
            shells: vec![faces],
        };
        let (builder, slots) = Builder::assemble(&model, t, assembly).map_err(|_| NOT_CLOSED)?;
        let mut keys = BuildKeys {
            namespace: NAMESPACE,
            shells: vec![0],
            ..BuildKeys::default()
        };
        // Entities with no Prism id are keyed past every id (never read
        // back as one: `region` gives them no key).
        let key = |k: Option<u64>| k.unwrap_or(u64::MAX);
        for (i, v) in region.vertices.iter().enumerate() {
            keys.vertices.insert(slots.vertices[i], key(v.key));
        }
        for (i, e) in region.edges.iter().enumerate() {
            keys.edges.insert(slots.edges[i], key(e.key));
        }
        for (i, f) in region.faces.iter().enumerate() {
            keys.faces.insert(slots.faces[0][i], key(f.key));
        }
        let (body, provenance) =
            build(&mut model, builder, &keys, &Control::NONE).map_err(|e| match e {
                arris_ops::OpError::Rejected(arris_ops::Rejection::Checker(_)) => FAULTY,
                _ => NOT_CLOSED,
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

    /// Prism's id for a kernel entity, if it came from one.
    fn key(&self, id: impl Into<arris_topo::EntityId>) -> Option<u64> {
        self.provenance
            .origins(Shape::new(id.into(), Orientation::Forward))
            .into_iter()
            .find_map(|(_, o)| match o {
                Origin::Role(Role::Consumer(ConsumerKey { namespace, key }))
                    if namespace == NAMESPACE && key != u64::MAX =>
                {
                    Some(key)
                }
                _ => None,
            })
    }

    /// The solid read back as a region.
    pub fn region(&self) -> Result<Region, &'static str> {
        let m = &self.model;
        let mut out = Region::default();
        let mut vmap: BTreeMap<arris_topo::VertexId, usize> = BTreeMap::new();
        let mut emap: BTreeMap<arris_topo::EdgeId, usize> = BTreeMap::new();
        let mut vertex =
            |out: &mut Region, id: arris_topo::VertexId| -> Result<usize, &'static str> {
                if let Some(i) = vmap.get(&id) {
                    return Ok(*i);
                }
                let at = back(&m.vertex(id).map_err(|_| CANNOT_STORE)?.point());
                out.vertices.push(Vertex {
                    key: self.key(id),
                    at,
                });
                vmap.insert(id, out.vertices.len() - 1);
                Ok(out.vertices.len() - 1)
            };
        for fu in m.faces(self.body).map_err(|_| CANNOT_STORE)? {
            let face = m.face(fu.id).map_err(|_| CANNOT_STORE)?;
            let forward = !fu.orientation.is_reversed();
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
                _ => return Err(CANNOT_STORE),
            };
            let mut loops: Vec<(f64, Vec<(usize, bool)>)> = Vec::new();
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
                                _ => return Err(CANNOT_STORE),
                            };
                            let a = vertex(&mut out, e.start())?;
                            let b = vertex(&mut out, e.end())?;
                            out.edges.push(Edge {
                                key: self.key(eid),
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
            }
            loops.sort_by(|a, b| b.0.total_cmp(&a.0));
            out.faces.push(Face {
                key: self.key(fu.id),
                surface,
                loops: loops.into_iter().map(|(_, l)| l).collect(),
            });
        }
        Ok(out)
    }
}

fn curve2_point(c: &Curve2, t: f64) -> [f64; 2] {
    let p = c.point(t);
    [p.x, p.y]
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
