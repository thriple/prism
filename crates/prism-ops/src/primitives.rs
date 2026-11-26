//! The primitives (Shapes & Placement part 4): Box, Cylinder, Cone, Sphere,
//! Torus and Capsule as exact closed solids, each standing on the ground
//! plane with its middle on the z axis, and the parameters that make them.
//! A group made from one remembers its [`Primitive`] so the Inspector can
//! change the size later (the group's geometry is made again).

use prism_geom::Surface;
use prism_math::{DVec3, Plane};
use prism_topo::{Face, Model, VertexId};

/// A cylinder of radius `r` and height `h` on the ground, built by hand
/// for tests (Push/Pull makes them from Exact Curves 4b): two discs, two
/// seams, and two cylinder faces facing out.
pub fn cylinder(r: f64, h: f64) -> Model {
    let mut m = Model::new();
    crate::add_circle(&mut m, DVec3::ZERO, DVec3::Z, r);
    crate::add_circle(&mut m, DVec3::Z * h, DVec3::Z, r);
    let tol = m.tol();
    let at = |m: &Model, z: f64| -> Vec<VertexId> {
        m.vertices()
            .filter(|(_, p)| tol.is_zero(p.z - z))
            .map(|(id, _)| id)
            .collect()
    };
    let (bottom, top) = (at(&m, 0.0), at(&m, h));
    let above = |m: &Model, b: VertexId| -> VertexId {
        let p = m.vertex(b);
        *top.iter()
            .find(|t| m.vertex(**t).truncate().distance(p.truncate()) < tol.dist)
            .expect("a seam")
    };
    let (b0, b1) = (bottom[0], bottom[1]);
    let (t0, t1) = (above(&m, b0), above(&m, b1));
    let seams = [(m.vertex(b0), m.vertex(t0)), (m.vertex(b1), m.vertex(t1))];
    for (p, q) in seams {
        crate::add_segment(&mut m, p, q);
    }
    // The arc between `a` and `b` that runs counter-clockwise about +Z
    // from `a`.
    let arc = |m: &Model, a: VertexId, b: VertexId| -> prism_topo::EdgeId {
        m.edges()
            .find(|(_, e)| {
                e.has(a) && e.has(b) && {
                    let prism_geom::Curve::Arc { axis, .. } = e.curve else {
                        return false;
                    };
                    (if e.a == a { axis } else { -axis }).z > 0.0
                }
            })
            .map(|(id, _)| id)
            .expect("an arc")
    };
    let line = |m: &Model, a: VertexId, b: VertexId| -> prism_topo::EdgeId {
        m.edges()
            .find(|(_, e)| e.has(a) && e.has(b) && !e.curve.is_arc())
            .map(|(id, _)| id)
            .expect("a seam")
    };
    let side = Surface::Cylinder(prism_geom::Cylinder {
        origin: DVec3::ZERO,
        axis: DVec3::Z,
        radius: r,
        outward: true,
    });
    for (a, b, ta, tb) in [(b0, b1, t0, t1), (b1, b0, t1, t0)] {
        // Round the bottom, up a seam, back round the top, down.
        let edges = vec![
            arc(&m, a, b),
            line(&m, b, tb),
            arc(&m, ta, tb),
            line(&m, ta, a),
        ];
        m.add_face(Face::with_edges(
            vec![a, b, tb, ta],
            vec![],
            side.clone(),
            vec![edges],
        ));
    }
    m
}

/// An arc from `a` to `b`, counter-clockwise about `axis`, as a new edge.
fn arc_edge(
    m: &mut Model,
    a: VertexId,
    b: VertexId,
    centre: DVec3,
    axis: DVec3,
    curve: prism_topo::CurveId,
) -> prism_topo::EdgeId {
    let radius = m.vertex(a).distance(centre);
    m.add_curve_edge(
        a,
        b,
        prism_geom::Curve::Arc {
            centre,
            axis,
            radius,
        },
        Some(curve),
    )
    .expect("an arc")
}

/// A sphere of radius `r` about `centre`, built by hand for tests (Follow
/// Me makes them from Exact Curves 5d): two half circles from pole to pole,
/// and the two faces between them, facing out.
pub fn sphere(centre: DVec3, r: f64) -> Model {
    let mut m = Model::new();
    let n = m.add_vertex(centre + DVec3::Z * r);
    let s = m.add_vertex(centre - DVec3::Z * r);
    let (c0, c1) = (m.new_curve_id(), m.new_curve_id());
    // Pole to pole through +x, and through -x.
    let m0 = arc_edge(&mut m, n, s, centre, DVec3::Y, c0);
    let m1 = arc_edge(&mut m, n, s, centre, -DVec3::Y, c1);
    let surface = Surface::Sphere(prism_geom::Sphere {
        centre,
        axis: DVec3::Z,
        radius: r,
        outward: true,
    });
    for edges in [vec![m0, m1], vec![m1, m0]] {
        m.add_face(Face::with_edges(
            vec![n, s],
            vec![],
            surface.clone(),
            vec![edges],
        ));
    }
    m
}

/// A cone of base radius `r` and height `h` standing on the ground, built
/// by hand for tests: the base disc, two seams up to the apex, and two
/// cone faces facing out.
pub fn cone(r: f64, h: f64) -> Model {
    let mut m = Model::new();
    let b0 = m.add_vertex(DVec3::X * r);
    let b1 = m.add_vertex(-DVec3::X * r);
    let apex = m.add_vertex(DVec3::Z * h);
    let base = m.new_curve_id();
    // Round the base, counter-clockwise about +z: through +y, then -y.
    let a01 = arc_edge(&mut m, b0, b1, DVec3::ZERO, DVec3::Z, base);
    let a10 = arc_edge(&mut m, b1, b0, DVec3::ZERO, DVec3::Z, base);
    let e0 = m.add_new_edge(apex, b0);
    let e1 = m.add_new_edge(apex, b1);
    m.add_face(Face::with_edges(
        vec![b0, b1],
        vec![],
        Surface::Plane(Plane::new(DVec3::ZERO, -DVec3::Z)),
        vec![vec![a10, a01]],
    ));
    let side = Surface::Cone(prism_geom::Cone {
        apex: DVec3::Z * h,
        axis: -DVec3::Z,
        half_angle: (r / h).atan(),
        outward: true,
    });
    m.add_face(Face::with_edges(
        vec![b0, b1, apex],
        vec![],
        side.clone(),
        vec![vec![a01, e1, e0]],
    ));
    m.add_face(Face::with_edges(
        vec![b1, b0, apex],
        vec![],
        side,
        vec![vec![a10, e0, e1]],
    ));
    m
}

/// A torus about the z axis through the origin: its tube of radius
/// `minor` runs round a circle of radius `major`. Built by hand for tests:
/// two tube circles, the outer and inner circles round the axis, and four
/// faces between them, facing out.
pub fn torus(major: f64, minor: f64) -> Model {
    let mut m = Model::new();
    let (o0, i0) = (
        m.add_vertex(DVec3::X * (major + minor)),
        m.add_vertex(DVec3::X * (major - minor)),
    );
    let (o1, i1) = (
        m.add_vertex(-DVec3::X * (major + minor)),
        m.add_vertex(-DVec3::X * (major - minor)),
    );
    let ids: Vec<_> = (0..4).map(|_| m.new_curve_id()).collect();
    let (k0, k1) = (DVec3::X * major, -DVec3::X * major);
    // The tube's circles at +x and -x: over the top, then under.
    let top0 = arc_edge(&mut m, o0, i0, k0, -DVec3::Y, ids[0]);
    let bottom0 = arc_edge(&mut m, i0, o0, k0, -DVec3::Y, ids[0]);
    let top1 = arc_edge(&mut m, o1, i1, k1, DVec3::Y, ids[1]);
    let bottom1 = arc_edge(&mut m, i1, o1, k1, DVec3::Y, ids[1]);
    // Round the axis, counter-clockwise about +z: through +y, then -y.
    let out_a = arc_edge(&mut m, o0, o1, DVec3::ZERO, DVec3::Z, ids[2]);
    let out_b = arc_edge(&mut m, o1, o0, DVec3::ZERO, DVec3::Z, ids[2]);
    let in_a = arc_edge(&mut m, i0, i1, DVec3::ZERO, DVec3::Z, ids[3]);
    let in_b = arc_edge(&mut m, i1, i0, DVec3::ZERO, DVec3::Z, ids[3]);
    let surface = Surface::Torus(prism_geom::Torus {
        centre: DVec3::ZERO,
        axis: DVec3::Z,
        major,
        minor,
        outward: true,
    });
    let faces = [
        (vec![o0, o1, i1, i0], vec![out_a, top1, in_a, top0]),
        (vec![o0, i0, i1, o1], vec![bottom0, in_a, bottom1, out_a]),
        (vec![o1, o0, i0, i1], vec![out_b, top0, in_b, top1]),
        (vec![o1, i1, i0, o0], vec![bottom1, in_b, bottom0, out_b]),
    ];
    for (l, e) in faces {
        m.add_face(Face::with_edges(l, vec![], surface.clone(), vec![e]));
    }
    m
}

/// A closed box straight into the model: `lo` to `hi`, every face
/// counter-clockwise seen from outside.
fn box_solid(lo: DVec3, hi: DVec3) -> Model {
    let mut m = Model::new();
    let c: Vec<VertexId> = (0..8)
        .map(|i| {
            m.add_vertex(DVec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            ))
        })
        .collect();
    for l in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        let outer: Vec<VertexId> = l.iter().map(|i| c[*i]).collect();
        for k in 0..4 {
            m.add_edge(outer[k], outer[(k + 1) % 4]);
        }
        let pts = m.points(&outer);
        let centre = pts.iter().copied().sum::<DVec3>() / 4.0;
        m.add_face(Face::new(
            outer,
            vec![],
            Surface::Plane(Plane::new(centre, prism_math::newell(&pts))),
        ));
    }
    m
}

/// A capsule: a cylinder of radius `r` and straight length `h` with a
/// half sphere on each end, standing on the ground (`2r + h` tall). Each
/// half sphere is two faces with a pole, as the sphere is.
pub fn capsule(r: f64, h: f64) -> Model {
    let mut m = Model::new();
    let (zb, zt) = (r, r + h);
    let (c_lo, c_hi) = (DVec3::Z * zb, DVec3::Z * zt);
    let b0 = m.add_vertex(DVec3::new(r, 0.0, zb));
    let b1 = m.add_vertex(DVec3::new(-r, 0.0, zb));
    let t0 = m.add_vertex(DVec3::new(r, 0.0, zt));
    let t1 = m.add_vertex(DVec3::new(-r, 0.0, zt));
    let ps = m.add_vertex(DVec3::ZERO);
    let pn = m.add_vertex(DVec3::Z * (zt + r));
    let ids: Vec<_> = (0..6).map(|_| m.new_curve_id()).collect();
    // The rims, each in two halves: through +y, then through -y.
    let lo_a = arc_edge(&mut m, b0, b1, c_lo, DVec3::Z, ids[0]);
    let lo_b = arc_edge(&mut m, b1, b0, c_lo, DVec3::Z, ids[0]);
    let hi_a = arc_edge(&mut m, t0, t1, c_hi, DVec3::Z, ids[1]);
    let hi_b = arc_edge(&mut m, t1, t0, c_hi, DVec3::Z, ids[1]);
    // The seams of the straight part.
    let v0 = m.add_new_edge(b0, t0);
    let v1 = m.add_new_edge(b1, t1);
    // Quarter circles from the rims to the poles.
    let s0 = arc_edge(&mut m, b0, ps, c_lo, DVec3::Y, ids[2]);
    let s1 = arc_edge(&mut m, b1, ps, c_lo, -DVec3::Y, ids[3]);
    let n0 = arc_edge(&mut m, t0, pn, c_hi, -DVec3::Y, ids[4]);
    let n1 = arc_edge(&mut m, t1, pn, c_hi, DVec3::Y, ids[5]);
    let side = Surface::Cylinder(prism_geom::Cylinder {
        origin: DVec3::ZERO,
        axis: DVec3::Z,
        radius: r,
        outward: true,
    });
    let globe = |centre| {
        Surface::Sphere(prism_geom::Sphere {
            centre,
            axis: DVec3::Z,
            radius: r,
            outward: true,
        })
    };
    // The straight part: round the bottom, up a seam, back round the top,
    // down (as the cylinder's).
    m.add_face(Face::with_edges(
        vec![b0, b1, t1, t0],
        vec![],
        side.clone(),
        vec![vec![lo_a, v1, hi_a, v0]],
    ));
    m.add_face(Face::with_edges(
        vec![b1, b0, t0, t1],
        vec![],
        side,
        vec![vec![lo_b, v0, hi_b, v1]],
    ));
    // The end caps, like the cone's two faces (the lower one's loops run
    // the other way, as it faces down and out).
    for (l, e) in [
        (vec![b0, ps, b1], vec![s0, s1, lo_a]),
        (vec![b1, ps, b0], vec![s1, s0, lo_b]),
    ] {
        m.add_face(Face::with_edges(l, vec![], globe(c_lo), vec![e]));
    }
    for (l, e) in [
        (vec![t0, t1, pn], vec![hi_a, n1, n0]),
        (vec![t1, t0, pn], vec![hi_b, n0, n1]),
    ] {
        m.add_face(Face::with_edges(l, vec![], globe(c_hi), vec![e]));
    }
    crate::health::orient(&mut m, None);
    m
}

/// What a primitive is made from: its kind and sizes (metres).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Primitive {
    /// Width (red), depth (green) and height (blue).
    Box {
        w: f64,
        d: f64,
        h: f64,
    },
    Cylinder {
        r: f64,
        h: f64,
    },
    Cone {
        r: f64,
        h: f64,
    },
    Sphere {
        r: f64,
    },
    /// The ring's radius to the tube's middle, and the tube's.
    Torus {
        major: f64,
        minor: f64,
    },
    /// The radius and the straight part's length (the whole is longer by
    /// two radii).
    Capsule {
        r: f64,
        h: f64,
    },
}

/// The smallest size a primitive is made at (a hundredth of a millimetre).
pub const SMALLEST: f64 = 1e-5;

/// Why a size is refused.
pub const TOO_SMALL: &str = "That size is too small (at least 0.01 mm)";
pub const TORUS_TUBE: &str = "A torus's tube must be thinner than its ring is wide";

impl Primitive {
    pub fn name(&self) -> &'static str {
        match self {
            Primitive::Box { .. } => "Box",
            Primitive::Cylinder { .. } => "Cylinder",
            Primitive::Cone { .. } => "Cone",
            Primitive::Sphere { .. } => "Sphere",
            Primitive::Torus { .. } => "Torus",
            Primitive::Capsule { .. } => "Capsule",
        }
    }

    /// The kinds, with default sizes (a metre or so).
    pub fn kinds() -> [Primitive; 6] {
        [
            Primitive::Box {
                w: 1.0,
                d: 1.0,
                h: 1.0,
            },
            Primitive::Cylinder { r: 0.5, h: 1.0 },
            Primitive::Cone { r: 0.5, h: 1.0 },
            Primitive::Sphere { r: 0.5 },
            Primitive::Torus {
                major: 0.5,
                minor: 0.15,
            },
            Primitive::Capsule { r: 0.25, h: 0.5 },
        ]
    }

    /// The sizes, each with a key and a label for the Inspector.
    pub fn fields(&self) -> Vec<(&'static str, &'static str, f64)> {
        match *self {
            Primitive::Box { w, d, h } => vec![
                ("w", "Width X", w),
                ("d", "Depth Y", d),
                ("h", "Height Z", h),
            ],
            Primitive::Cylinder { r, h } | Primitive::Cone { r, h } => {
                vec![("r", "Radius", r), ("h", "Height", h)]
            }
            Primitive::Sphere { r } => vec![("r", "Radius", r)],
            Primitive::Torus { major, minor } => {
                vec![
                    ("major", "Ring radius", major),
                    ("minor", "Tube radius", minor),
                ]
            }
            Primitive::Capsule { r, h } => {
                vec![("r", "Radius", r), ("h", "Straight length", h)]
            }
        }
    }

    /// The same primitive with one size changed, or why not.
    pub fn with(&self, key: &str, v: f64) -> Result<Primitive, &'static str> {
        if v.is_nan() || v < SMALLEST {
            return Err(TOO_SMALL);
        }
        let mut p = *self;
        match (&mut p, key) {
            (Primitive::Box { w, .. }, "w") => *w = v,
            (Primitive::Box { d, .. }, "d") => *d = v,
            (Primitive::Box { h, .. }, "h") => *h = v,
            (Primitive::Cylinder { r, .. } | Primitive::Cone { r, .. }, "r") => *r = v,
            (Primitive::Cylinder { h, .. } | Primitive::Cone { h, .. }, "h") => *h = v,
            (Primitive::Sphere { r }, "r") => *r = v,
            (Primitive::Capsule { r, .. }, "r") => *r = v,
            (Primitive::Capsule { h, .. }, "h") => *h = v,
            (Primitive::Torus { major, .. }, "major") => *major = v,
            (Primitive::Torus { minor, .. }, "minor") => *minor = v,
            _ => return Err("No such size"),
        }
        if let Primitive::Torus { major, minor } = p
            && minor >= major
        {
            return Err(TORUS_TUBE);
        }
        Ok(p)
    }

    /// How tall it stands.
    pub fn height(&self) -> f64 {
        match *self {
            Primitive::Box { h, .. }
            | Primitive::Cylinder { h, .. }
            | Primitive::Cone { h, .. } => h,
            Primitive::Sphere { r } => 2.0 * r,
            Primitive::Torus { minor, .. } => 2.0 * minor,
            Primitive::Capsule { r, h } => h + 2.0 * r,
        }
    }

    /// The solid: standing on the ground with its middle on the z axis,
    /// every face turned to look out.
    pub fn build(&self) -> Model {
        let mut m = self.raw();
        crate::health::orient(&mut m, None);
        m
    }

    fn raw(&self) -> Model {
        match *self {
            Primitive::Box { w, d, h } => box_solid(
                DVec3::new(-w / 2.0, -d / 2.0, 0.0),
                DVec3::new(w / 2.0, d / 2.0, h),
            ),
            Primitive::Cylinder { r, h } => cylinder(r, h),
            Primitive::Cone { r, h } => cone(r, h),
            Primitive::Sphere { r } => sphere(DVec3::Z * r, r),
            Primitive::Torus { major, minor } => {
                let mut m = torus(major, minor);
                crate::transform::affine(
                    &mut m,
                    &prism_math::DAffine3::from_translation(DVec3::Z * minor),
                );
                m
            }
            Primitive::Capsule { r, h } => capsule(r, h),
        }
    }
}
