//! Live features (Inspector & Features part 2): a round inside a group
//! stays editable, direct edits run underneath it, and Bake makes it plain.

use std::collections::BTreeSet;

use prism_doc::{Command, DefId, Document, InstanceId};
use prism_math::DVec3;
use prism_ops::Selection;
use prism_topo::{EdgeId, FaceId, Model};

/// A 2 m cube as a group. Returns the group and its definition.
fn grouped_cube() -> (Document, InstanceId, DefId) {
    let mut d = Document::new();
    d.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 2.0,
        y1: 2.0,
        z: 0.0,
    });
    let f = d.model().faces().next().unwrap().0;
    let up = d.model().face(f).unwrap().plane().unwrap().normal.z;
    d.run(Command::PushPull {
        face: f,
        distance: 2.0 * up,
        new_loop: false,
    });
    let id = d.assembly().next_instance();
    let m = d.model();
    let sel = Selection {
        vertices: m.vertices().map(|(i, _)| i).collect(),
        edges: m.edges().map(|(i, _)| i).collect(),
        faces: m.faces().map(|(i, _)| i).collect(),
        guides: BTreeSet::new(),
    };
    assert!(d.run(Command::MakeGroup {
        selection: sel,
        instances: BTreeSet::new(),
    }));
    let def = d.assembly().find_instance(id).unwrap().1.def;
    (d, id, def)
}

fn model(d: &Document, def: DefId) -> &Model {
    &d.assembly().def(def).unwrap().model
}

/// The vertical edge nearest the group's low corner.
fn corner_edge(m: &Model) -> EdgeId {
    m.edges()
        .filter(|(_, e)| (m.vertex(e.a).z - m.vertex(e.b).z).abs() > 1.0)
        .min_by(|(_, a), (_, b)| {
            let s = |e: &prism_topo::Edge| m.vertex(e.a).x + m.vertex(e.a).y;
            s(a).total_cmp(&s(b))
        })
        .unwrap()
        .0
}

/// The face of `m` whose normal is `n`.
fn face_facing(m: &Model, n: DVec3) -> FaceId {
    m.faces()
        .find(|(id, _)| {
            m.face(*id)
                .and_then(|f| f.plane())
                .is_some_and(|p| p.normal.dot(n) > 0.99)
        })
        .unwrap()
        .0
}

fn round(d: &mut Document, inst: InstanceId, def: DefId, size: f64) {
    let e = corner_edge(model(d, def));
    assert!(d.run_in(
        &[inst],
        Command::RoundEdges {
            edges: [e].into(),
            size,
            fillet: true,
        }
    ));
}

fn max_x(m: &Model) -> f64 {
    m.vertices().map(|(_, p)| p.x).fold(f64::MIN, f64::max)
}

#[test]
fn a_round_in_a_group_stays_live() {
    let (mut d, inst, def) = grouped_cube();
    let plain = model(&d, def).geometry_json();
    round(&mut d, inst, def, 0.25);
    let stack = d.assembly().def(def).unwrap().stack.as_ref().unwrap();
    assert_eq!(stack.base.face_count(), 6, "the base is the cube");
    assert_eq!(stack.features.len(), 1);
    assert!(!stack.features[0].is_broken());
    assert_eq!(model(&d, def).face_count(), 7, "the cube and its round");
    // One undo takes it all back, stack too.
    d.undo();
    assert!(d.assembly().def(def).unwrap().stack.is_none());
    assert_eq!(model(&d, def).geometry_json(), plain);
    d.redo();
    assert!(d.assembly().def(def).unwrap().stack.is_some());
}

#[test]
fn loose_geometry_is_rounded_outright() {
    let mut d = Document::new();
    d.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 2.0,
        y1: 2.0,
        z: 0.0,
    });
    let f = d.model().faces().next().unwrap().0;
    let up = d.model().face(f).unwrap().plane().unwrap().normal.z;
    d.run(Command::PushPull {
        face: f,
        distance: 2.0 * up,
        new_loop: false,
    });
    let e = corner_edge(d.model());
    assert!(d.run(Command::RoundEdges {
        edges: [e].into(),
        size: 0.25,
        fillet: true,
    }));
    assert!(d.assembly().root().stack.is_none());
}

#[test]
fn a_feature_changes_size_and_goes_back_with_undo() {
    let (mut d, inst, def) = grouped_cube();
    round(&mut d, inst, def, 0.25);
    let small = model(&d, def).to_json();
    assert!(d.run(Command::SetFeatureSize {
        instance: inst,
        index: 0,
        size: 0.5,
    }));
    assert_ne!(model(&d, def).to_json(), small);
    let stack = d.assembly().def(def).unwrap().stack.as_ref().unwrap();
    assert_eq!(stack.base.face_count(), 6, "the base did not change");
    assert!(
        !d.run(Command::SetFeatureSize {
            instance: inst,
            index: 0,
            size: 0.5,
        }),
        "the same size changes nothing"
    );
    d.undo();
    assert_eq!(model(&d, def).to_json(), small);
}

#[test]
fn a_direct_edit_runs_underneath_the_round() {
    let (mut d, inst, def) = grouped_cube();
    round(&mut d, inst, def, 0.25);
    // Pull the far wall out a metre.
    let wall = face_facing(model(&d, def), DVec3::X);
    assert!(d.run_in(
        &[inst],
        Command::PushPull {
            face: wall,
            distance: 1.0,
            new_loop: false,
        }
    ));
    let m = model(&d, def);
    assert!((max_x(m) - 3.0).abs() < 1e-9, "the wall moved");
    assert_eq!(m.face_count(), 7, "and the round is still there");
    let stack = d.assembly().def(def).unwrap().stack.as_ref().unwrap();
    assert!(!stack.features[0].is_broken());
    assert!((max_x(&stack.base) - 3.0).abs() < 1e-9, "the base moved");
}

#[test]
fn an_edit_that_stops_the_round_marks_it_broken() {
    let (mut d, inst, def) = grouped_cube();
    round(&mut d, inst, def, 0.25);
    // Take a wall at the rounded corner away: the body is open.
    let wall = face_facing(model(&d, def), -DVec3::X);
    assert!(d.run_in(
        &[inst],
        Command::Erase {
            selection: Selection {
                faces: [wall].into(),
                ..Selection::default()
            },
        }
    ));
    let stack = d.assembly().def(def).unwrap().stack.as_ref().unwrap();
    assert!(stack.features[0].is_broken(), "said so, not dropped");
    assert_eq!(model(&d, def).face_count(), 5, "the plain base shows");
}

#[test]
fn bake_makes_the_round_plain_geometry() {
    let (mut d, inst, def) = grouped_cube();
    round(&mut d, inst, def, 0.25);
    let shown = model(&d, def).to_json();
    assert!(d.run(Command::BakeFeatures { instance: inst }));
    assert!(d.assembly().def(def).unwrap().stack.is_none());
    assert_eq!(model(&d, def).to_json(), shown, "the shape is the same");
    assert!(!d.run(Command::BakeFeatures { instance: inst }));
    d.undo();
    assert!(d.assembly().def(def).unwrap().stack.is_some());
}

#[test]
fn a_feature_is_taken_off_and_the_cube_is_plain_again() {
    let (mut d, inst, def) = grouped_cube();
    let plain = model(&d, def).to_json();
    round(&mut d, inst, def, 0.25);
    assert!(d.run(Command::RemoveFeature {
        instance: inst,
        index: 0,
    }));
    assert!(d.assembly().def(def).unwrap().stack.is_none());
    assert_eq!(model(&d, def).face_count(), 6);
    assert_eq!(model(&d, def).geometry_json(), {
        let m = Model::from_json(&plain).unwrap();
        m.geometry_json()
    });
}
