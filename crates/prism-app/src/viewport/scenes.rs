//! Scenes to look at until the drawing tools exist (Help > Test Scenes).

use prism_doc::{Command, Document};
use prism_geom::Surface;
use prism_math::{DVec3, Plane, newell};
use prism_topo::{Face, Model, VertexId};

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
            .is_some_and(|f| f.plane().normal.z > 0.0);
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
            .is_some_and(|f| f.plane().normal.z > 0.0);
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
        m.add_face(Face {
            outer,
            holes: vec![],
            surface: Surface::Plane(Plane::new(centre, newell(&pts))),
        });
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
                f.plane().normal.dot(c - centre) > 0.0,
                "a face points into its box"
            );
        }
    }
}
