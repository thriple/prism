//! Test scenes (Help > Test Scenes).

use prism_doc::{Command, Document};
use prism_geom::Surface;
use prism_math::{DVec3, Plane, newell};
use prism_topo::{Face, Model, VertexId};

/// A cylinder of radius `r` and height `h` on the ground, built by hand
/// for tests (Push/Pull makes them from Exact Curves 4b): two discs, two
/// seams, and two cylinder faces facing out.
pub fn cylinder(r: f64, h: f64) -> Model {
    let mut m = Model::new();
    prism_ops::add_circle(&mut m, DVec3::ZERO, DVec3::Z, r);
    prism_ops::add_circle(&mut m, DVec3::Z * h, DVec3::Z, r);
    let at = |m: &Model, z: f64| -> Vec<VertexId> {
        m.vertices()
            .filter(|(_, p)| (p.z - z).abs() < 1e-9)
            .map(|(id, _)| id)
            .collect()
    };
    let (bottom, top) = (at(&m, 0.0), at(&m, h));
    let above = |m: &Model, b: VertexId| -> VertexId {
        let p = m.vertex(b);
        *top.iter()
            .find(|t| m.vertex(**t).truncate().distance(p.truncate()) < 1e-9)
            .expect("a seam")
    };
    let (b0, b1) = (bottom[0], bottom[1]);
    let (t0, t1) = (above(&m, b0), above(&m, b1));
    let seams = [(m.vertex(b0), m.vertex(t0)), (m.vertex(b1), m.vertex(t1))];
    for (p, q) in seams {
        prism_ops::add_segment(&mut m, p, q);
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

pub fn by_action(id: &str) -> Option<Document> {
    Some(match id {
        "help.scene.cube" => cube(),
        "help.scene.pocket" => pocket(),
        "help.scene.plate" => plate(),
        "help.scene.large" => large(50),
        _ => return None,
    })
}

/// Draw a rectangle and pull its face by `height` (upward), as the tools will.
fn pulled_rect(doc: &mut Document, x0: f64, y0: f64, x1: f64, y1: f64, z: f64, height: f64) {
    doc.run(Command::Rect { x0, y0, x1, y1, z });
    let mid = DVec3::new((x0 + x1) / 2.0, (y0 + y1) / 2.0, z);
    if let Some(face) = prism_ops::face_at(doc.model(), mid) {
        let up = doc
            .model()
            .face(face)
            .is_some_and(|f| f.plane().unwrap().normal.z > 0.0);
        doc.run(Command::PushPull {
            face,
            distance: if up { height } else { -height },
            new_loop: false,
        });
    }
}

/// The prototype's starting cube, hovering above the ground: x and y from
/// -1 to 1, z from 1 to 3.
pub fn cube() -> Document {
    let mut doc = Document::new();
    pulled_rect(&mut doc, -1.0, -1.0, 1.0, 1.0, 1.0, 2.0);
    doc
}

/// A 4 by 4 by 2 box with a square pocket 2.4 wide and 1.5 deep.
pub fn pocket() -> Document {
    let mut doc = Document::new();
    pulled_rect(&mut doc, 0.0, 0.0, 4.0, 4.0, 0.0, 2.0);
    pulled_rect(&mut doc, 0.8, 0.8, 3.2, 3.2, 2.0, -1.5);
    doc
}

/// A 1 m plate 0.5 mm thick, with a small box 1 mm behind it: zoom out to
/// a kilometre and nothing behind should show through.
pub fn plate() -> Document {
    let mut doc = Document::new();
    let t = 0.0005;
    // Standing up: 1 m wide in x, t thick in y, 1 m tall.
    doc.run(Command::Rect {
        x0: -0.5,
        y0: -0.2,
        x1: 0.5,
        y1: -0.2 + t,
        z: 0.0,
    });
    if let Some(face) = prism_ops::face_at(doc.model(), DVec3::new(0.0, -0.2 + t / 2.0, 0.0)) {
        let up = doc
            .model()
            .face(face)
            .is_some_and(|f| f.plane().unwrap().normal.z > 0.0);
        doc.run(Command::PushPull {
            face,
            distance: if up { 1.0 } else { -1.0 },
            new_loop: false,
        });
    }
    let y = -0.2 + t + 0.001;
    pulled_rect(&mut doc, -0.2, y, 0.2, y + 0.2, 0.3, 0.4);
    doc
}

/// `n` by `n` separate boxes, for timing drawing and picking.
pub fn large(n: i32) -> Document {
    let mut m = Model::new();
    for i in 0..n {
        for j in 0..n {
            let lo = DVec3::new(f64::from(i) * 1.5, f64::from(j) * 1.5, 0.0);
            let h = 0.5 + f64::from((i * 7 + j * 3) % 5) * 0.3;
            add_box(&mut m, lo, lo + DVec3::new(1.0, 1.0, h));
        }
    }
    Document::with_model(m)
}

/// A closed box straight into the model (no sticky geometry: the boxes in
/// the large scene never touch).
fn add_box(m: &mut Model, lo: DVec3, hi: DVec3) {
    let c: Vec<VertexId> = (0..8)
        .map(|i| {
            m.add_vertex(DVec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            ))
        })
        .collect();
    // Each face counter-clockwise seen from outside.
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
            Surface::Plane(Plane::new(centre, newell(&pts))),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scene_is_a_valid_model() {
        for id in [
            "help.scene.cube",
            "help.scene.pocket",
            "help.scene.plate",
            "help.scene.large",
        ] {
            let doc = if id == "help.scene.large" {
                large(4)
            } else {
                by_action(id).expect("scene")
            };
            let p = prism_ops::check::problems(doc.model());
            assert!(p.is_empty(), "{id}: {p:?}");
            assert!(doc.model().face_count() >= 6, "{id}");
        }
        assert_eq!(cube().model().face_count(), 6);
        assert_eq!(pocket().model().face_count(), 11);
    }

    #[test]
    fn large_boxes_face_outward() {
        let doc = large(3);
        let m = doc.model();
        for (_, f) in m.faces() {
            let pts = m.points(&f.outer);
            let c = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
            let (i, j) = ((c.x / 1.5).floor() as i32, (c.y / 1.5).floor() as i32);
            let h = 0.5 + f64::from((i * 7 + j * 3) % 5) * 0.3;
            let centre = DVec3::new(f64::from(i) * 1.5 + 0.5, f64::from(j) * 1.5 + 0.5, h / 2.0);
            assert!(
                f.plane().unwrap().normal.dot(c - centre) > 0.0,
                "a face points into its box"
            );
        }
    }
}
