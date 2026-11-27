//! Dimensions (Annotation & Sections part 2): plain document objects that
//! change no geometry and undo exactly.

#![cfg(not(target_arch = "wasm32"))]

use prism_doc::{Command, DimId, Document};
use prism_math::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn add(d: &mut Document, a: DVec3, b: DVec3, offset: DVec3) -> bool {
    d.run(Command::AddDimension { a, b, offset })
}

#[test]
fn a_dimension_measures_the_distance_between_its_points() {
    let mut d = Document::new();
    assert!(add(
        &mut d,
        v(0.0, 0.0, 0.0),
        v(3.0, 4.0, 0.0),
        v(0.0, 0.0, 1.0)
    ));
    let (all, next) = d.assembly().dimensions();
    assert_eq!(next, 1);
    let dim = all[&DimId(0)];
    assert_eq!(dim.length(), 5.0);
    assert_eq!(dim.offset, v(0.0, 0.0, 1.0));
}

#[test]
fn points_that_are_not_a_line_add_nothing() {
    let mut d = Document::new();
    assert!(!add(&mut d, DVec3::X, DVec3::X, DVec3::Z), "same point");
    assert!(!add(&mut d, DVec3::X, v(f64::NAN, 0.0, 0.0), DVec3::Z));
    assert!(!add(&mut d, DVec3::X, DVec3::Y, v(0.0, f64::INFINITY, 0.0)));
    assert!(d.assembly().dimensions().0.is_empty());
}

#[test]
fn move_and_delete_undo_and_redo_exactly() {
    let mut d = Document::new();
    add(&mut d, DVec3::ZERO, DVec3::X, DVec3::Y);
    let id = DimId(0);
    let mut states = vec![d.assembly().clone()];
    assert!(d.run(Command::MoveDimension {
        id,
        offset: v(0.0, 2.0, 0.0)
    }));
    states.push(d.assembly().clone());
    assert!(d.run(Command::DeleteDimension { id }));
    states.push(d.assembly().clone());
    assert!(d.assembly().dimensions().0.is_empty());
    assert!(d.undo());
    assert_eq!(d.assembly(), &states[1]);
    assert!(d.undo());
    assert_eq!(d.assembly(), &states[0]);
    assert!(d.redo() && d.redo());
    assert_eq!(d.assembly(), &states[2]);
    // Gone: nothing to move or delete.
    assert!(!d.run(Command::MoveDimension {
        id,
        offset: DVec3::X
    }));
    assert!(!d.run(Command::DeleteDimension { id }));
    // A new one gets a fresh id, even after undo.
    d.undo();
    d.undo();
    d.undo();
    add(&mut d, DVec3::ZERO, DVec3::Z, DVec3::X);
    assert!(d.assembly().dimensions().0.contains_key(&DimId(1)));
}

#[test]
fn dimensions_leave_the_geometry_alone() {
    let mut d = Document::new();
    d.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 2.0,
        y1: 2.0,
        z: 0.0,
    });
    let model = d.model().clone();
    add(&mut d, DVec3::ZERO, v(2.0, 0.0, 0.0), v(0.0, -1.0, 0.0));
    assert_eq!(d.model(), &model);
    d.undo();
    assert_eq!(d.model(), &model);
}

// ---------- Attached ends follow the model (brief 46 gaps) ----------

use prism_doc::Anchor;

/// A box 2 on each side, standing on the ground at the origin.
fn cube() -> Document {
    let mut d = Document::new();
    d.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 2.0,
        y1: 2.0,
        z: 0.0,
    });
    let face = prism_ops::face_at(d.model(), v(1.0, 1.0, 0.0)).unwrap();
    let up = d.model().face(face).unwrap().plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face,
        distance: if up { 2.0 } else { -2.0 },
        new_loop: false,
    });
    d
}

fn corner(d: &Document, p: DVec3) -> prism_topo::VertexId {
    d.model()
        .vertices()
        .find(|(_, q)| q.distance(p) < 1e-9)
        .map(|(id, _)| id)
        .expect("a vertex there")
}

/// A box, and a dimension across its bottom edge, attached at both ends.
fn box_with_dimension() -> (Document, prism_topo::VertexId, prism_topo::VertexId) {
    let mut d = cube();
    let (a, b) = (corner(&d, v(0.0, 0.0, 0.0)), corner(&d, v(2.0, 0.0, 0.0)));
    let (pa, pb) = (d.model().vertex(a), d.model().vertex(b));
    d.run(Command::Batch {
        commands: vec![
            Command::AddDimension {
                a: pa,
                b: pb,
                offset: v(0.0, -1.0, 0.0),
            },
            Command::AnchorDimension {
                id: DimId(0),
                a: Some(Anchor {
                    path: Vec::new(),
                    vertex: a,
                }),
                b: Some(Anchor {
                    path: Vec::new(),
                    vertex: b,
                }),
            },
        ],
    });
    (d, a, b)
}

#[test]
fn an_attached_dimension_follows_its_vertex_when_it_is_moved() {
    let (mut d, _, b) = box_with_dimension();
    let before = d.assembly().dimensions().0[&DimId(0)];
    assert_eq!(before.length(), 2.0);
    // Move one corner out along x: the dimension grows with it.
    d.run(Command::Translate {
        vertices: [b].into(),
        by: v(1.0, 0.0, 0.0),
    });
    let dim = d.assembly().dimensions().0[&DimId(0)];
    assert!((dim.length() - 3.0).abs() < 1e-9, "{dim:?}");
    assert_eq!(dim.offset, v(0.0, -1.0, 0.0), "same side, same distance");
    // One undo takes the move and the follow back together.
    d.undo();
    assert_eq!(d.assembly().dimensions().0[&DimId(0)], before);
}

#[test]
fn a_dimension_end_lets_go_when_its_vertex_is_erased() {
    let (mut d, a, b) = box_with_dimension();
    let sel = prism_ops::Selection {
        vertices: [a, b].into(),
        ..Default::default()
    };
    d.run(Command::Erase { selection: sel });
    // Nothing to follow now: the dimension stays where it was, unattached.
    assert_eq!(d.assembly().dimensions().0[&DimId(0)].length(), 2.0);
    assert!(d.assembly().dim_anchors().is_empty());
    // And a free dimension is not moved by later edits.
    d.run(Command::Segment {
        p: v(5.0, 5.0, 5.0),
        q: v(6.0, 5.0, 5.0),
    });
    assert_eq!(d.assembly().dimensions().0[&DimId(0)].length(), 2.0);
}

#[test]
fn a_dimension_on_a_group_follows_the_group_and_deleting_it_forgets_the_anchor() {
    let mut d = cube();
    let sel = prism_ops::Selection {
        vertices: d.model().vertices().map(|(i, _)| i).collect(),
        edges: d.model().edges().map(|(i, _)| i).collect(),
        faces: d.model().faces().map(|(i, _)| i).collect(),
        guides: Default::default(),
    };
    let g = d.assembly().next_instance();
    d.run(Command::MakeGroup {
        selection: sel,
        instances: Default::default(),
    });
    let def = d.assembly().root().instances[&g].def;
    let model = &d.assembly().def(def).unwrap().model;
    let a = model
        .vertices()
        .find(|(_, q)| q.distance(v(0.0, 0.0, 0.0)) < 1e-9)
        .unwrap()
        .0;
    let b = model
        .vertices()
        .find(|(_, q)| q.distance(v(2.0, 0.0, 0.0)) < 1e-9)
        .unwrap()
        .0;
    d.run(Command::Batch {
        commands: vec![
            Command::AddDimension {
                a: v(0.0, 0.0, 0.0),
                b: v(2.0, 0.0, 0.0),
                offset: v(0.0, -1.0, 0.0),
            },
            Command::AnchorDimension {
                id: DimId(0),
                a: Some(Anchor {
                    path: vec![g],
                    vertex: a,
                }),
                b: Some(Anchor {
                    path: vec![g],
                    vertex: b,
                }),
            },
        ],
    });
    // Move the group 5 along y: both ends go with it.
    d.run(Command::Place {
        instances: [g].into(),
        change: prism_doc::Change::Translate {
            by: v(0.0, 5.0, 0.0),
        },
    });
    let dim = d.assembly().dimensions().0[&DimId(0)];
    assert_eq!(dim.a, v(0.0, 5.0, 0.0));
    assert_eq!(dim.b, v(2.0, 5.0, 0.0));
    d.run(Command::DeleteDimension { id: DimId(0) });
    assert!(d.assembly().dim_anchors().is_empty());
}
