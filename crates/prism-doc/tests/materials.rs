//! Materials in the document (Basic Materials, part 1): the table, painting
//! face sides and groups, replacing, and paint kept through edits
//! (DECISIONS #35, #36).
#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_doc::{Command, Document, FaceSide, Material, material};
use prism_math::DVec3;
use prism_ops::Selection;
use prism_topo::{FaceId, MaterialId, Model};
use proptest::prelude::*;

/// A unit cube from (0, 0, 0) to (2, 2, 2), drawn and pulled.
fn cube() -> Document {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    let f = m.faces().next().map(|(id, _)| id).expect("square");
    let up = m
        .face(f)
        .and_then(|x| x.plane())
        .map_or(1.0, |p| p.normal.z);
    prism_ops::push_pull(&mut m, f, 2.0 * up.signum(), false);
    Document::with_model(m)
}

fn add(d: &mut Document, name: &str, c: [u8; 3]) -> MaterialId {
    let before: BTreeSet<MaterialId> = d.assembly().materials().0.keys().copied().collect();
    assert!(d.run(Command::AddMaterial {
        material: Material::colour(name, c),
    }));
    *d.assembly()
        .materials()
        .0
        .keys()
        .find(|k| !before.contains(k))
        .expect("added")
}

fn face_at(d: &Document, p: DVec3) -> FaceId {
    prism_ops::face_at(d.model(), p).expect("a face there")
}

fn front(d: &Document, f: FaceId) -> Option<MaterialId> {
    d.model().face(f).and_then(|x| x.paint.front)
}

fn paint_all(d: &mut Document, m: MaterialId) {
    let sides = d
        .model()
        .faces()
        .map(|(id, _)| (id, FaceSide::Front))
        .collect();
    d.run(Command::Paint {
        sides,
        material: Some(m),
    });
}

#[test]
fn materials_get_unique_names_and_removing_one_unpaints_its_faces() {
    let mut d = cube();
    let a = add(&mut d, "Brick", [178, 34, 34]);
    let b = add(&mut d, "Brick", [100, 0, 0]);
    assert_eq!(
        d.assembly().material(b).map(|m| m.name.as_str()),
        Some("Brick #2")
    );
    let top = face_at(&d, DVec3::new(1.0, 1.0, 2.0));
    d.run(Command::Paint {
        sides: [(top, FaceSide::Front), (top, FaceSide::Back)].into(),
        material: Some(a),
    });
    let f = d.model().face(top).expect("top");
    assert_eq!((f.paint.front, f.paint.back), (Some(a), Some(a)));
    assert_eq!(d.assembly().unused_materials(), [b].into());
    d.run(Command::PurgeMaterials);
    assert!(d.assembly().material(b).is_none());
    d.run(Command::RemoveMaterial { id: a });
    assert!(d.model().face(top).is_some_and(|f| f.paint.is_default()));
    // Undo brings the material and the paint back.
    d.undo();
    assert_eq!(front(&d, top), Some(a));
    // A material the document does not have is not painted with.
    let gone = MaterialId(99);
    assert!(!d.run(Command::Paint {
        sides: [(top, FaceSide::Front)].into(),
        material: Some(gone),
    }));
}

#[test]
fn replace_and_fill_connected() {
    let mut d = cube();
    let red = add(&mut d, "Red", [255, 0, 0]);
    let blue = add(&mut d, "Blue", [0, 0, 255]);
    paint_all(&mut d, red);
    let top = face_at(&d, DVec3::new(1.0, 1.0, 2.0));
    d.run(Command::Paint {
        sides: [(top, FaceSide::Front)].into(),
        material: Some(blue),
    });
    // Fill connected from a side: every red face is reached around the
    // blue top (the cube's sides and bottom are joined).
    let side = face_at(&d, DVec3::new(0.0, 1.0, 1.0));
    let fill = material::connected_same(d.model(), side, FaceSide::Front);
    assert_eq!(fill.len(), 5);
    assert!(!fill.contains(&(top, FaceSide::Front)));
    d.run(Command::ReplaceMaterial {
        from: Some(red),
        to: Some(blue),
        everywhere: true,
    });
    assert!(d.model().faces().all(|(_, f)| f.paint.front == Some(blue)));
}

#[test]
fn edits_keep_paint() {
    let mut d = cube();
    let red = add(&mut d, "Red", [255, 0, 0]);
    let blue = add(&mut d, "Blue", [0, 0, 255]);
    paint_all(&mut d, red);
    // A line across the top splits it: both pieces stay red.
    d.run(Command::Segment {
        p: DVec3::new(0.5, 0.0, 2.0),
        q: DVec3::new(0.5, 2.0, 2.0),
    });
    let small = face_at(&d, DVec3::new(0.25, 1.0, 2.0));
    let big = face_at(&d, DVec3::new(1.25, 1.0, 2.0));
    assert_ne!(small, big);
    assert_eq!((front(&d, small), front(&d, big)), (Some(red), Some(red)));
    // Paint the small piece blue, then erase the line: the merged face
    // takes the larger piece's red.
    d.run(Command::Paint {
        sides: [(small, FaceSide::Front)].into(),
        material: Some(blue),
    });
    let line = d
        .model()
        .edges()
        .find(|(_, e)| {
            let (a, b) = (d.model().vertex(e.a), d.model().vertex(e.b));
            (a.x - 0.5).abs() < 1e-9 && (b.x - 0.5).abs() < 1e-9 && a.z > 1.9 && b.z > 1.9
        })
        .map(|(id, _)| id)
        .expect("the line");
    d.run(Command::Erase {
        selection: Selection::edge(line),
    });
    let top = face_at(&d, DVec3::new(1.0, 1.0, 2.0));
    assert_eq!(front(&d, top), Some(red));
    // Push/Pull: the moved top keeps red; new walls are Default.
    let n = d
        .model()
        .face(top)
        .and_then(|f| f.plane())
        .map(|p| p.normal.z)
        .unwrap_or(1.0);
    d.run(Command::PushPull {
        face: top,
        distance: n.signum(),
        new_loop: false,
    });
    let moved = face_at(&d, DVec3::new(1.0, 1.0, 3.0));
    assert_eq!(front(&d, moved), Some(red));
    // The box's sides stretch (they run along the pull), so they stay red.
    let side = face_at(&d, DVec3::new(1.0, 0.0, 2.5));
    assert_eq!(
        front(&d, side),
        Some(red),
        "a stretched side keeps its paint"
    );
    // Reverse swaps front and back.
    d.run(Command::ReverseFaces {
        faces: [moved].into(),
    });
    let f = d.model().face(moved).expect("moved");
    assert_eq!((f.paint.front, f.paint.back), (None, Some(red)));
}

#[test]
fn a_pulled_rectangle_keeps_its_paint_and_its_new_walls_are_default() {
    // Painted on either side, pulled either way: the moved face and the
    // base left behind both show the colour outside (owner, 2026-11-12).
    for side in [FaceSide::Front, FaceSide::Back] {
        for up in [true, false] {
            let mut m = Model::new();
            prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
            let mut d = Document::with_model(m);
            let red = add(&mut d, "Red", [255, 0, 0]);
            let f = face_at(&d, DVec3::new(1.0, 1.0, 0.0));
            d.run(Command::Paint {
                sides: [(f, side)].into(),
                material: Some(red),
            });
            let n = d
                .model()
                .face(f)
                .and_then(|x| x.plane())
                .map_or(1.0, |p| p.normal.z);
            let dz = if up { 1.0 } else { -1.0 };
            d.run(Command::PushPull {
                face: f,
                distance: dz * n.signum(),
                new_loop: false,
            });
            assert_eq!(d.model().face_count(), 6, "{side:?} {up}");
            // Seen from outside: the front of every face of the box.
            let outside: Vec<_> = d
                .model()
                .faces()
                .filter(|(_, x)| x.paint.front == Some(red))
                .map(|(_, x)| x.plane().map(|p| p.normal.z.round()))
                .collect();
            assert_eq!(outside.len(), 2, "top and base ({side:?}, up {up})");
            assert!(outside.contains(&Some(1.0)) && outside.contains(&Some(-1.0)));
            let walls = d
                .model()
                .faces()
                .filter(|(_, x)| x.plane().is_some_and(|p| p.normal.z.abs() < 0.5))
                .all(|(_, x)| x.paint.is_default());
            assert!(walls, "new walls are Default");
        }
    }
}

#[test]
fn a_group_is_painted_as_a_whole_and_copies_keep_it() {
    let mut d = cube();
    let red = add(&mut d, "Red", [255, 0, 0]);
    let g = d.assembly().next_instance();
    let m = d.model();
    d.run(Command::MakeGroup {
        selection: Selection {
            vertices: m.vertices().map(|(id, _)| id).collect(),
            edges: m.edges().map(|(id, _)| id).collect(),
            faces: m.faces().map(|(id, _)| id).collect(),
            guides: Default::default(),
        },
        instances: Default::default(),
    });
    d.run(Command::PaintInstances {
        instances: [g].into(),
        material: Some(red),
    });
    assert_eq!(d.assembly().painted_with(g), Some(red));
    // Nothing is copied onto its faces.
    let inner = d.assembly().root().instances[&g].def;
    assert!(
        d.assembly()
            .def(inner)
            .expect("def")
            .model
            .faces()
            .all(|(_, f)| f.paint.is_default())
    );
    let copy = d.assembly().next_instance();
    d.run(Command::CopyInstances {
        instances: [g].into(),
        step: prism_doc::Change::Translate {
            by: DVec3::new(3.0, 0.0, 0.0),
        },
        count: 1,
    });
    assert_eq!(d.assembly().painted_with(copy), Some(red));
    d.run(Command::EraseInstances {
        instances: [g].into(),
    });
    assert!(!d.assembly().painted().contains_key(&g));
    assert!(
        d.assembly().unused_materials().is_empty(),
        "the copy uses it"
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// Lines across a painted cube's faces, lines erased again, and the
    /// whole moved: every face stays painted, with a material the document
    /// has.
    #[test]
    fn splitting_and_merging_never_loses_paint(
        cuts in prop::collection::vec((0usize..3, 0.1f64..1.9, 0.1f64..1.9, 0.1f64..1.9, 0.1f64..1.9), 1..6),
        erase in prop::collection::vec(0usize..40, 0..4),
        shift in -3.0f64..3.0,
    ) {
        let mut d = cube();
        let red = add(&mut d, "Red", [255, 0, 0]);
        paint_all(&mut d, red);
        for (side, a, b, c, e) in cuts {
            // A line across one face, edge to edge.
            let (p, q) = match side {
                0 => (DVec3::new(a, 0.0, 2.0), DVec3::new(b, 2.0, 2.0)),
                1 => (DVec3::new(0.0, a, c), DVec3::new(0.0, b, e)),
                _ => (DVec3::new(a, 0.0, c), DVec3::new(2.0, b, 2.0)),
            };
            if side == 2 {
                // From the front face's middle to the right side's top
                // edge: across two faces at once.
                d.run(Command::Segment { p: DVec3::new(a, 0.0, c), q: DVec3::new(a, 0.0, 2.0) });
                d.run(Command::Segment { p: DVec3::new(2.0, b, 0.0), q: DVec3::new(2.0, b, 2.0) });
            } else {
                d.run(Command::Segment { p, q });
            }
        }
        let edges: Vec<_> = d.model().edges().map(|(id, _)| id).collect();
        for k in erase {
            if let Some(e) = edges.get(k % edges.len().max(1)) {
                d.run(Command::Erase { selection: Selection::edge(*e) });
            }
        }
        let all: BTreeSet<_> = d.model().vertices().map(|(id, _)| id).collect();
        d.run(Command::Translate { vertices: all, by: DVec3::new(shift, 0.0, 0.0) });
        let have: BTreeSet<MaterialId> = d.assembly().materials().0.keys().copied().collect();
        for (id, f) in d.model().faces() {
            prop_assert!(f.paint.front.is_some(), "face {id:?} lost its paint");
            prop_assert!(f.paint.front.iter().chain(f.paint.back.iter()).all(|m| have.contains(m)));
        }
    }
}

#[test]
fn follow_me_walls_take_the_profile_paint() {
    let mut m = Model::new();
    let c =
        [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)].map(|(y, z)| DVec3::new(0.0, y, z));
    for i in 0..4 {
        prism_ops::add_segment(&mut m, c[i], c[(i + 1) % 4]);
    }
    prism_ops::add_segment(&mut m, DVec3::ZERO, DVec3::new(3.0, 0.0, 0.0));
    let mut d = Document::with_model(m);
    let red = add(&mut d, "Red", [255, 0, 0]);
    paint_all(&mut d, red);
    let profile = face_at(&d, DVec3::new(0.0, 0.2, 0.2));
    let path: BTreeSet<_> = d
        .model()
        .edges()
        .filter(|(_, e)| d.model().vertex(e.b).x.abs() > 1.0 || d.model().vertex(e.a).x.abs() > 1.0)
        .map(|(id, _)| id)
        .collect();
    assert!(d.run(Command::FollowMe { path, profile }));
    assert_eq!(d.model().face_count(), 6);
    assert!(d.model().faces().all(|(_, f)| f.paint.front == Some(red)));
}

/// The edge whose middle is `p`.
fn edge_at(d: &Document, p: DVec3) -> Option<prism_topo::EdgeId> {
    let m = d.model();
    m.edges()
        .find(|(_, e)| ((m.vertex(e.a) + m.vertex(e.b)) / 2.0).distance(p) < 1e-9)
        .map(|(id, _)| id)
}

/// A fillet or chamfer takes its faces' colour (as SketchUp gives it a
/// neighbour's material); with one face painted, that one's.
#[test]
fn a_rounded_edge_takes_its_faces_colour() {
    for fillet in [true, false] {
        let mut d = cube();
        let red = add(&mut d, "Red", [255, 0, 0]);
        paint_all(&mut d, red);
        let edge = edge_at(&d, DVec3::new(1.0, 0.0, 2.0)).expect("top front edge");
        assert!(d.run(Command::RoundEdge {
            edge,
            size: 0.3,
            fillet,
        }));
        assert!(
            d.model().faces().all(|(_, f)| f.paint.front == Some(red)),
            "every face red, fillet {fillet}"
        );
        // Only the top painted: the new face takes the top's.
        let mut d = cube();
        let blue = add(&mut d, "Blue", [0, 0, 255]);
        let top = face_at(&d, DVec3::new(1.0, 1.0, 2.0));
        d.run(Command::Paint {
            sides: [(top, FaceSide::Front)].into(),
            material: Some(blue),
        });
        let edge = edge_at(&d, DVec3::new(1.0, 0.0, 2.0)).expect("edge");
        assert!(d.run(Command::RoundEdge {
            edge,
            size: 0.3,
            fillet,
        }));
        let blue_faces = d
            .model()
            .faces()
            .filter(|(_, f)| f.paint.front == Some(blue))
            .count();
        assert_eq!(blue_faces, 2, "the top and the new face, fillet {fillet}");
    }
}
