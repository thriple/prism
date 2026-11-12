//! The model's binary entry: its own versioned layout, in postcard.

use prism_geom::{Curve, Surface};
use prism_math::{DVec3, Plane};
use prism_topo::{
    Counters, CurveId, Edge, EdgeId, Face, FaceId, Guide, GuideId, MaterialId, Model, ModelParts,
    VertexId,
};
use serde::{Deserialize, Serialize};

use crate::FileError;

/// The geometry layout this version writes. Layout 2 (Exact Curves part 1)
/// adds each loop's edges; a layout 1 file finds them when read. Layout 3
/// (part 2) adds arcs and curve ids. Layout 4 (part 4) adds soft and
/// smooth edges. Layout 5 (part 5b) adds cone, sphere and torus faces; it
/// is layout 4 with more kinds of surface, so a layout 4 entry reads as it
/// is. Layout 6 (part 6a) adds approximate edges (DECISIONS #32). Layout 7
/// (Basic Materials) adds each face's paint. Layout 8 (Basic Materials part
/// 2) adds the texture placement of painted flat faces that have one.
/// Layout 9 (Freeform NURBS) adds NURBS curves and patches, each stored
/// once and named by the edges and faces on it.
pub const VERSION: u32 = 9;

type P = [f64; 3];

#[derive(Serialize, Deserialize)]
enum CurveV1 {
    Line,
}

#[derive(Serialize, Deserialize)]
enum SurfaceV1 {
    Plane {
        origin: P,
        normal: P,
    },
    /// Exact Curves 4a. Old files have none, so the layout stays.
    Cylinder {
        origin: P,
        axis: P,
        radius: f64,
        outward: bool,
    },
    /// Exact Curves 5b (layout 5): added at the end, so older entries
    /// read unchanged.
    Cone {
        apex: P,
        axis: P,
        half_angle: f64,
        outward: bool,
    },
    Sphere {
        centre: P,
        axis: P,
        radius: f64,
        outward: bool,
    },
    Torus {
        centre: P,
        axis: P,
        major: f64,
        minor: f64,
        outward: bool,
    },
}

#[derive(Serialize, Deserialize)]
enum GuideV1 {
    Line { origin: P, dir: P },
    Point { at: P },
}

#[derive(Serialize, Deserialize)]
struct EdgeV1 {
    id: u32,
    a: u32,
    b: u32,
    curve: CurveV1,
}

#[derive(Serialize, Deserialize)]
struct FaceV1 {
    id: u32,
    outer: Vec<u32>,
    holes: Vec<Vec<u32>>,
    surface: SurfaceV1,
}

#[derive(Serialize, Deserialize)]
struct GeometryV1 {
    vertices: Vec<(u32, P)>,
    edges: Vec<EdgeV1>,
    faces: Vec<FaceV1>,
    guides: Vec<(u32, GuideV1)>,
    /// Vertex, edge, face and guide counters.
    counters: [u32; 4],
}

#[derive(Serialize, Deserialize)]
struct FaceV2 {
    id: u32,
    outer: Vec<u32>,
    holes: Vec<Vec<u32>>,
    /// Each loop's edges, the outer loop first: edge `i` joins vertex `i`
    /// to vertex `i + 1`.
    edges: Vec<Vec<u32>>,
    surface: SurfaceV1,
}

/// Layout 2: as layout 1, and faces name their loops' edges.
#[derive(Serialize, Deserialize)]
struct GeometryV2 {
    vertices: Vec<(u32, P)>,
    edges: Vec<EdgeV1>,
    faces: Vec<FaceV2>,
    guides: Vec<(u32, GuideV1)>,
    counters: [u32; 4],
}

#[derive(Serialize, Deserialize)]
enum CurveV3 {
    Line,
    Arc {
        centre: P,
        axis: P,
        radius: f64,
    },
    /// Freeform NURBS (layout 9): added at the end, so older entries read
    /// as they are. `curve` is a place in the layout's `splines`.
    Spline {
        curve: u32,
        t0: f64,
        t1: f64,
    },
}

/// A NURBS curve (layout 9).
#[derive(Serialize, Deserialize)]
struct SplineV9 {
    degree: u32,
    knots: Vec<f64>,
    points: Vec<P>,
    weights: Vec<f64>,
    periodic: bool,
}

/// A NURBS patch (layout 9): the control net row by row along u.
#[derive(Serialize, Deserialize)]
struct PatchV9 {
    degree: [u32; 2],
    knots: [Vec<f64>; 2],
    count: [u32; 2],
    points: Vec<P>,
    weights: Vec<f64>,
    periodic: [bool; 2],
}

/// Layout 9: as layout 8, and the NURBS curves and patches that edges and
/// faces name.
#[derive(Serialize, Deserialize)]
struct GeometryV9 {
    vertices: Vec<(u32, P)>,
    edges: Vec<EdgeV6>,
    faces: Vec<FaceV2>,
    guides: Vec<(u32, GuideV1)>,
    /// Vertex, edge, face, guide and curve counters.
    counters: [u32; 5],
    paint: Vec<(u32, Option<u32>, Option<u32>)>,
    uv: Vec<(u32, P, P, P)>,
    splines: Vec<SplineV9>,
    patches: Vec<PatchV9>,
}

impl From<GeometryV8> for GeometryV9 {
    fn from(g: GeometryV8) -> GeometryV9 {
        GeometryV9 {
            vertices: g.vertices,
            edges: g.edges,
            faces: g.faces,
            guides: g.guides,
            counters: g.counters,
            paint: g.paint,
            uv: g.uv,
            splines: Vec::new(),
            patches: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct EdgeV3 {
    id: u32,
    a: u32,
    b: u32,
    curve: CurveV3,
    curve_id: Option<u32>,
}

#[derive(Serialize, Deserialize)]
struct EdgeV4 {
    id: u32,
    a: u32,
    b: u32,
    curve: CurveV3,
    curve_id: Option<u32>,
    soft: bool,
    smooth: bool,
}

#[derive(Serialize, Deserialize)]
struct EdgeV6 {
    id: u32,
    a: u32,
    b: u32,
    curve: CurveV3,
    curve_id: Option<u32>,
    soft: bool,
    smooth: bool,
    approx: bool,
}

/// Layout 6: as layout 5, and edges may be approximate (Exact Curves 6a).
#[derive(Serialize, Deserialize)]
struct GeometryV6 {
    vertices: Vec<(u32, P)>,
    edges: Vec<EdgeV6>,
    faces: Vec<FaceV2>,
    guides: Vec<(u32, GuideV1)>,
    /// Vertex, edge, face, guide and curve counters.
    counters: [u32; 5],
}

impl From<GeometryV4> for GeometryV6 {
    fn from(g: GeometryV4) -> GeometryV6 {
        GeometryV6 {
            vertices: g.vertices,
            edges: g
                .edges
                .into_iter()
                .map(|e| EdgeV6 {
                    id: e.id,
                    a: e.a,
                    b: e.b,
                    curve: e.curve,
                    curve_id: e.curve_id,
                    soft: e.soft,
                    smooth: e.smooth,
                    approx: false,
                })
                .collect(),
            faces: g.faces,
            guides: g.guides,
            counters: g.counters,
        }
    }
}

/// Layout 7: as layout 6, and faces may be painted: (face, front, back)
/// material ids, for painted faces only (Basic Materials).
#[derive(Serialize, Deserialize)]
struct GeometryV7 {
    vertices: Vec<(u32, P)>,
    edges: Vec<EdgeV6>,
    faces: Vec<FaceV2>,
    guides: Vec<(u32, GuideV1)>,
    /// Vertex, edge, face, guide and curve counters.
    counters: [u32; 5],
    paint: Vec<(u32, Option<u32>, Option<u32>)>,
}

impl From<GeometryV6> for GeometryV7 {
    fn from(g: GeometryV6) -> GeometryV7 {
        GeometryV7 {
            vertices: g.vertices,
            edges: g.edges,
            faces: g.faces,
            guides: g.guides,
            counters: g.counters,
            paint: Vec::new(),
        }
    }
}

/// Layout 8: as layout 7, and painted flat faces may have a texture
/// placement: (face, origin, u, v).
#[derive(Serialize, Deserialize)]
struct GeometryV8 {
    vertices: Vec<(u32, P)>,
    edges: Vec<EdgeV6>,
    faces: Vec<FaceV2>,
    guides: Vec<(u32, GuideV1)>,
    /// Vertex, edge, face, guide and curve counters.
    counters: [u32; 5],
    paint: Vec<(u32, Option<u32>, Option<u32>)>,
    uv: Vec<(u32, P, P, P)>,
}

impl From<GeometryV7> for GeometryV8 {
    fn from(g: GeometryV7) -> GeometryV8 {
        GeometryV8 {
            vertices: g.vertices,
            edges: g.edges,
            faces: g.faces,
            guides: g.guides,
            counters: g.counters,
            paint: g.paint,
            uv: Vec::new(),
        }
    }
}

/// Layout 3: as layout 2, and edges may be arcs and share curve ids.
#[derive(Serialize, Deserialize)]
struct GeometryV3 {
    vertices: Vec<(u32, P)>,
    edges: Vec<EdgeV3>,
    faces: Vec<FaceV2>,
    guides: Vec<(u32, GuideV1)>,
    /// Vertex, edge, face, guide and curve counters.
    counters: [u32; 5],
}

/// Layout 4: as layout 3, and edges may be soft and smooth (Exact Curves
/// part 4). Cylinder faces came with layout 3 (old files have none).
#[derive(Serialize, Deserialize)]
struct GeometryV4 {
    vertices: Vec<(u32, P)>,
    edges: Vec<EdgeV4>,
    faces: Vec<FaceV2>,
    guides: Vec<(u32, GuideV1)>,
    /// Vertex, edge, face, guide and curve counters.
    counters: [u32; 5],
}

impl From<GeometryV3> for GeometryV4 {
    fn from(g: GeometryV3) -> GeometryV4 {
        GeometryV4 {
            vertices: g.vertices,
            edges: g
                .edges
                .into_iter()
                .map(|e| EdgeV4 {
                    id: e.id,
                    a: e.a,
                    b: e.b,
                    curve: e.curve,
                    curve_id: e.curve_id,
                    soft: false,
                    smooth: false,
                })
                .collect(),
            faces: g.faces,
            guides: g.guides,
            counters: g.counters,
        }
    }
}

impl From<GeometryV2> for GeometryV3 {
    fn from(g: GeometryV2) -> GeometryV3 {
        GeometryV3 {
            vertices: g.vertices,
            edges: g
                .edges
                .into_iter()
                .map(|e| EdgeV3 {
                    id: e.id,
                    a: e.a,
                    b: e.b,
                    curve: match e.curve {
                        CurveV1::Line => CurveV3::Line,
                    },
                    curve_id: None,
                })
                .collect(),
            faces: g.faces,
            guides: g.guides,
            counters: [
                g.counters[0],
                g.counters[1],
                g.counters[2],
                g.counters[3],
                0,
            ],
        }
    }
}

impl From<GeometryV1> for GeometryV2 {
    /// No loop edges: they are found when the model is put together.
    fn from(g: GeometryV1) -> GeometryV2 {
        GeometryV2 {
            vertices: g.vertices,
            edges: g.edges,
            faces: g
                .faces
                .into_iter()
                .map(|f| FaceV2 {
                    id: f.id,
                    outer: f.outer,
                    holes: f.holes,
                    edges: Vec::new(),
                    surface: f.surface,
                })
                .collect(),
            guides: g.guides,
            counters: g.counters,
        }
    }
}

fn p(v: DVec3) -> P {
    v.to_array()
}

fn ids(l: &[VertexId]) -> Vec<u32> {
    l.iter().map(|v| v.0).collect()
}

pub fn encode(m: &Model) -> Vec<u8> {
    let parts = m.to_parts();
    // Each NURBS curve once: edges on the same curve name the same entry.
    let mut shared: Vec<std::sync::Arc<prism_geom::Nurbs>> = Vec::new();
    let mut place = |c: &std::sync::Arc<prism_geom::Nurbs>| -> u32 {
        let at = shared
            .iter()
            .position(|x| std::sync::Arc::ptr_eq(x, c) || x == c)
            .unwrap_or_else(|| {
                shared.push(c.clone());
                shared.len() - 1
            });
        at as u32
    };
    let edges = parts
        .edges
        .iter()
        .map(|(id, e)| EdgeV6 {
            id: id.0,
            a: e.a.0,
            b: e.b.0,
            curve: match &e.curve {
                Curve::Line => CurveV3::Line,
                Curve::Arc {
                    centre,
                    axis,
                    radius,
                } => CurveV3::Arc {
                    centre: p(*centre),
                    axis: p(*axis),
                    radius: *radius,
                },
                Curve::Spline(s) => CurveV3::Spline {
                    curve: place(&s.curve),
                    t0: s.t0,
                    t1: s.t1,
                },
            },
            curve_id: e.curve_id.map(|c| c.0),
            soft: e.soft,
            smooth: e.smooth,
            approx: e.approx,
        })
        .collect();
    let g = GeometryV9 {
        splines: shared
            .iter()
            .map(|c| SplineV9 {
                degree: c.degree() as u32,
                knots: c.knots().to_vec(),
                points: c.points().iter().map(|x| p(*x)).collect(),
                weights: c.weights().to_vec(),
                periodic: c.is_periodic(),
            })
            .collect(),
        patches: Vec::new(),
        edges,
        uv: parts
            .faces
            .iter()
            .filter(|(_, f)| !f.paint.is_default())
            .filter_map(|(id, f)| {
                let m = f.paint.uv?;
                Some((id.0, p(m.origin), p(m.u), p(m.v)))
            })
            .collect(),
        paint: parts
            .faces
            .iter()
            .filter(|(_, f)| !f.paint.is_default())
            .map(|(id, f)| (id.0, f.paint.front.map(|x| x.0), f.paint.back.map(|x| x.0)))
            .collect(),
        vertices: parts.vertices.iter().map(|(id, v)| (id.0, p(*v))).collect(),
        faces: parts
            .faces
            .iter()
            .map(|(id, f)| FaceV2 {
                id: id.0,
                outer: ids(&f.outer),
                holes: f.holes.iter().map(|h| ids(h)).collect(),
                edges: (0..=f.holes.len())
                    .map(|k| f.loop_edge_ids(k).iter().map(|e| e.0).collect())
                    .collect(),
                surface: match f.surface {
                    Surface::Plane(pl) => SurfaceV1::Plane {
                        origin: p(pl.origin),
                        normal: p(pl.normal),
                    },
                    Surface::Cylinder(c) => SurfaceV1::Cylinder {
                        origin: p(c.origin),
                        axis: p(c.axis),
                        radius: c.radius,
                        outward: c.outward,
                    },
                    Surface::Cone(c) => SurfaceV1::Cone {
                        apex: p(c.apex),
                        axis: p(c.axis),
                        half_angle: c.half_angle,
                        outward: c.outward,
                    },
                    Surface::Sphere(x) => SurfaceV1::Sphere {
                        centre: p(x.centre),
                        axis: p(x.axis),
                        radius: x.radius,
                        outward: x.outward,
                    },
                    Surface::Torus(t) => SurfaceV1::Torus {
                        centre: p(t.centre),
                        axis: p(t.axis),
                        major: t.major,
                        minor: t.minor,
                        outward: t.outward,
                    },
                },
            })
            .collect(),
        guides: parts
            .guides
            .iter()
            .map(|(id, g)| {
                (
                    id.0,
                    match *g {
                        Guide::Line { origin, dir } => GuideV1::Line {
                            origin: p(origin),
                            dir: p(dir),
                        },
                        Guide::Point { at } => GuideV1::Point { at: p(at) },
                    },
                )
            })
            .collect(),
        counters: [
            parts.counters.vertex,
            parts.counters.edge,
            parts.counters.face,
            parts.counters.guide,
            parts.counters.curve,
        ],
    };
    postcard::to_allocvec(&g).unwrap_or_default()
}

pub fn decode(bytes: &[u8], version: u32) -> Result<Model, FileError> {
    let bad = |e: postcard::Error| FileError::Damaged(format!("geometry: {e}"));
    let v4 =
        |g: GeometryV4| GeometryV9::from(GeometryV8::from(GeometryV7::from(GeometryV6::from(g))));
    let g: GeometryV9 = match version {
        1 => v4(GeometryV3::from(GeometryV2::from(
            postcard::from_bytes::<GeometryV1>(bytes).map_err(bad)?,
        ))
        .into()),
        2 => v4(GeometryV3::from(postcard::from_bytes::<GeometryV2>(bytes).map_err(bad)?).into()),
        3 => v4(postcard::from_bytes::<GeometryV3>(bytes)
            .map_err(bad)?
            .into()),
        4 | 5 => v4(postcard::from_bytes(bytes).map_err(bad)?),
        6 => GeometryV8::from(GeometryV7::from(
            postcard::from_bytes::<GeometryV6>(bytes).map_err(bad)?,
        ))
        .into(),
        7 => GeometryV8::from(postcard::from_bytes::<GeometryV7>(bytes).map_err(bad)?).into(),
        8 => postcard::from_bytes::<GeometryV8>(bytes)
            .map_err(bad)?
            .into(),
        9 => postcard::from_bytes(bytes).map_err(bad)?,
        _ => {
            return Err(FileError::Damaged(format!(
                "unknown geometry layout {version}"
            )));
        }
    };
    let v = |a: u32| VertexId(a);
    let splines: Vec<std::sync::Arc<prism_geom::Nurbs>> = g
        .splines
        .iter()
        .map(|c| {
            prism_geom::Nurbs::new(
                c.degree as usize,
                c.knots.clone(),
                c.points.iter().map(|x| DVec3::from_array(*x)).collect(),
                c.weights.clone(),
                c.periodic,
            )
            .map(std::sync::Arc::new)
            .ok_or_else(|| {
                FileError::Damaged("geometry: a NURBS curve does not fit together".into())
            })
        })
        .collect::<Result<_, _>>()?;
    let mut bad_curve = false;
    let paint: std::collections::BTreeMap<u32, prism_topo::Paint> = g
        .paint
        .iter()
        .map(|(f, front, back)| {
            (
                *f,
                prism_topo::Paint {
                    front: front.map(MaterialId),
                    back: back.map(MaterialId),
                    uv: None,
                },
            )
        })
        .collect();
    let mut parts = ModelParts {
        vertices: g
            .vertices
            .into_iter()
            .map(|(id, q)| (VertexId(id), DVec3::from_array(q)))
            .collect(),
        edges: g
            .edges
            .into_iter()
            .map(|e| {
                (
                    EdgeId(e.id),
                    Edge {
                        a: v(e.a),
                        b: v(e.b),
                        curve: match e.curve {
                            CurveV3::Line => Curve::Line,
                            CurveV3::Arc {
                                centre,
                                axis,
                                radius,
                            } => Curve::Arc {
                                centre: DVec3::from_array(centre),
                                axis: DVec3::from_array(axis),
                                radius,
                            },
                            CurveV3::Spline { curve, t0, t1 } => {
                                match splines.get(curve as usize) {
                                    Some(c) if t0.is_finite() && t1.is_finite() => {
                                        Curve::Spline(prism_geom::Spline {
                                            curve: c.clone(),
                                            t0,
                                            t1,
                                        })
                                    }
                                    _ => {
                                        bad_curve = true;
                                        Curve::Line
                                    }
                                }
                            }
                        },
                        curve_id: e.curve_id.map(CurveId),
                        soft: e.soft,
                        smooth: e.smooth,
                        approx: e.approx,
                    },
                )
            })
            .collect(),
        faces: g
            .faces
            .into_iter()
            .map(|f| {
                (
                    FaceId(f.id),
                    Face::with_edges(
                        f.outer.into_iter().map(v).collect(),
                        f.holes
                            .into_iter()
                            .map(|h| h.into_iter().map(v).collect())
                            .collect(),
                        match f.surface {
                            SurfaceV1::Plane { origin, normal } => Surface::Plane(Plane {
                                origin: DVec3::from_array(origin),
                                normal: DVec3::from_array(normal),
                            }),
                            SurfaceV1::Cylinder {
                                origin,
                                axis,
                                radius,
                                outward,
                            } => Surface::Cylinder(prism_geom::Cylinder {
                                origin: DVec3::from_array(origin),
                                axis: DVec3::from_array(axis),
                                radius,
                                outward,
                            }),
                            SurfaceV1::Cone {
                                apex,
                                axis,
                                half_angle,
                                outward,
                            } => Surface::Cone(prism_geom::Cone {
                                apex: DVec3::from_array(apex),
                                axis: DVec3::from_array(axis),
                                half_angle,
                                outward,
                            }),
                            SurfaceV1::Sphere {
                                centre,
                                axis,
                                radius,
                                outward,
                            } => Surface::Sphere(prism_geom::Sphere {
                                centre: DVec3::from_array(centre),
                                axis: DVec3::from_array(axis),
                                radius,
                                outward,
                            }),
                            SurfaceV1::Torus {
                                centre,
                                axis,
                                major,
                                minor,
                                outward,
                            } => Surface::Torus(prism_geom::Torus {
                                centre: DVec3::from_array(centre),
                                axis: DVec3::from_array(axis),
                                major,
                                minor,
                                outward,
                            }),
                        },
                        f.edges
                            .into_iter()
                            .map(|l| l.into_iter().map(EdgeId).collect())
                            .collect(),
                    ),
                )
            })
            .collect(),
        guides: g
            .guides
            .into_iter()
            .map(|(id, gd)| {
                (
                    GuideId(id),
                    match gd {
                        GuideV1::Line { origin, dir } => Guide::Line {
                            origin: DVec3::from_array(origin),
                            dir: DVec3::from_array(dir),
                        },
                        GuideV1::Point { at } => Guide::Point {
                            at: DVec3::from_array(at),
                        },
                    },
                )
            })
            .collect(),
        counters: Counters {
            vertex: g.counters[0],
            edge: g.counters[1],
            face: g.counters[2],
            guide: g.counters[3],
            curve: g.counters[4],
        },
    };
    let uv: std::collections::BTreeMap<u32, prism_topo::UvMap> =
        g.uv.iter()
            .map(|(f, o, u, w)| {
                (
                    *f,
                    prism_topo::UvMap {
                        origin: DVec3::from_array(*o),
                        u: DVec3::from_array(*u),
                        v: DVec3::from_array(*w),
                    },
                )
            })
            .collect();
    for (id, f) in &mut parts.faces {
        if let Some(p) = paint.get(&id.0) {
            f.paint = *p;
            // Placement belongs to flat faces only.
            if f.plane().is_some() {
                f.paint.uv = uv.get(&id.0).copied();
            }
        }
    }
    if bad_curve {
        return Err(FileError::Damaged(
            "geometry: an edge names a NURBS curve that is not there".into(),
        ));
    }
    Model::from_parts(parts).map_err(|e| FileError::Damaged(format!("geometry: {e}")))
}
