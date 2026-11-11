//! The model's binary entry: its own versioned layout, in postcard.

use prism_geom::{Curve, Surface};
use prism_math::{DVec3, Plane};
use prism_topo::{
    Counters, CurveId, Edge, EdgeId, Face, FaceId, Guide, GuideId, Model, ModelParts, VertexId,
};
use serde::{Deserialize, Serialize};

use crate::FileError;

/// The geometry layout this version writes. Layout 2 (Exact Curves part 1)
/// adds each loop's edges; a layout 1 file finds them when read. Layout 3
/// (part 2) adds arcs and curve ids. Layout 4 (part 4) adds soft and
/// smooth edges. Layout 5 (part 5b) adds cone, sphere and torus faces; it
/// is layout 4 with more kinds of surface, so a layout 4 entry reads as it
/// is. Layout 6 (part 6a) adds approximate edges (DECISIONS #32).
pub const VERSION: u32 = 6;

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
    Arc { centre: P, axis: P, radius: f64 },
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
    let g = GeometryV6 {
        vertices: parts.vertices.iter().map(|(id, v)| (id.0, p(*v))).collect(),
        edges: parts
            .edges
            .iter()
            .map(|(id, e)| EdgeV6 {
                id: id.0,
                a: e.a.0,
                b: e.b.0,
                curve: match e.curve {
                    Curve::Line => CurveV3::Line,
                    Curve::Arc {
                        centre,
                        axis,
                        radius,
                    } => CurveV3::Arc {
                        centre: p(centre),
                        axis: p(axis),
                        radius,
                    },
                },
                curve_id: e.curve_id.map(|c| c.0),
                soft: e.soft,
                smooth: e.smooth,
                approx: e.approx,
            })
            .collect(),
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
    let v4 = |g: GeometryV4| GeometryV6::from(g);
    let g: GeometryV6 = match version {
        1 => v4(GeometryV3::from(GeometryV2::from(
            postcard::from_bytes::<GeometryV1>(bytes).map_err(bad)?,
        ))
        .into()),
        2 => v4(GeometryV3::from(postcard::from_bytes::<GeometryV2>(bytes).map_err(bad)?).into()),
        3 => v4(postcard::from_bytes::<GeometryV3>(bytes)
            .map_err(bad)?
            .into()),
        4 | 5 => v4(postcard::from_bytes(bytes).map_err(bad)?),
        6 => postcard::from_bytes(bytes).map_err(bad)?,
        _ => {
            return Err(FileError::Damaged(format!(
                "unknown geometry layout {version}"
            )));
        }
    };
    let v = |a: u32| VertexId(a);
    let parts = ModelParts {
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
    Model::from_parts(parts).map_err(|e| FileError::Damaged(format!("geometry: {e}")))
}
