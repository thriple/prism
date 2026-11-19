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
fn loose_geometry_stays_live_too() {
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
    let stack = d.assembly().root().stack.as_ref().expect("live at the top");
    assert_eq!(stack.features.len(), 1);
    assert_eq!(d.model().face_count(), 7);
    // Pulled out a metre, the round follows; baked, it is plain.
    let wall = face_facing(d.model(), DVec3::X);
    assert!(d.run(Command::PushPull {
        face: wall,
        distance: 1.0,
        new_loop: false,
    }));
    assert!(d.assembly().root().stack.is_some());
    assert!(d.run(Command::BakeFeatures {
        def: prism_doc::ROOT,
    }));
    assert!(d.assembly().root().stack.is_none());
    assert_eq!(d.model().face_count(), 7);
}

#[test]
fn a_feature_changes_size_and_goes_back_with_undo() {
    let (mut d, inst, def) = grouped_cube();
    round(&mut d, inst, def, 0.25);
    let small = model(&d, def).to_json();
    assert!(d.run(Command::SetFeatureSize {
        def,
        index: 0,
        size: 0.5,
    }));
    assert_ne!(model(&d, def).to_json(), small);
    let stack = d.assembly().def(def).unwrap().stack.as_ref().unwrap();
    assert_eq!(stack.base.face_count(), 6, "the base did not change");
    assert!(
        !d.run(Command::SetFeatureSize {
            def,
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
fn an_edit_the_features_cannot_follow_bakes_them() {
    let (mut d, inst, def) = grouped_cube();
    round(&mut d, inst, def, 0.25);
    // Take a wall at the rounded corner away: the body is open, so the
    // round could not run again. It is baked, not lost.
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
    assert!(d.assembly().def(def).unwrap().stack.is_none());
    assert_eq!(
        model(&d, def).face_count(),
        6,
        "the round stays, the wall goes"
    );
}

#[test]
fn bake_makes_the_round_plain_geometry() {
    let (mut d, inst, def) = grouped_cube();
    round(&mut d, inst, def, 0.25);
    let shown = model(&d, def).to_json();
    assert!(d.run(Command::BakeFeatures { def }));
    assert!(d.assembly().def(def).unwrap().stack.is_none());
    assert_eq!(model(&d, def).to_json(), shown, "the shape is the same");
    assert!(!d.run(Command::BakeFeatures { def }));
    d.undo();
    assert!(d.assembly().def(def).unwrap().stack.is_some());
}

#[test]
fn a_feature_is_taken_off_and_the_cube_is_plain_again() {
    let (mut d, inst, def) = grouped_cube();
    let plain = model(&d, def).to_json();
    round(&mut d, inst, def, 0.25);
    assert!(d.run(Command::RemoveFeature { def, index: 0 }));
    assert!(d.assembly().def(def).unwrap().stack.is_none());
    assert_eq!(model(&d, def).face_count(), 6);
    assert_eq!(model(&d, def).geometry_json(), {
        let m = Model::from_json(&plain).unwrap();
        m.geometry_json()
    });
}

/// Faces the features made: in the drawn model, not in the base.
fn made_faces(d: &Document, def: DefId) -> Vec<FaceId> {
    let def = d.assembly().def(def).unwrap();
    let base = &def.stack.as_ref().unwrap().base;
    def.model
        .faces()
        .map(|(f, _)| f)
        .filter(|f| base.face(*f).is_none())
        .collect()
}

#[test]
fn an_edit_of_the_round_itself_bakes_it_first() {
    let (mut d, inst, def) = grouped_cube();
    round(&mut d, inst, def, 0.25);
    let made = made_faces(&d, def);
    assert_eq!(made.len(), 1, "the round");
    // Take the round's own face away: the base has no such face.
    assert!(d.run_in(
        &[inst],
        Command::Erase {
            selection: Selection {
                faces: made.into_iter().collect(),
                ..Selection::default()
            },
        }
    ));
    assert!(
        d.assembly().def(def).unwrap().stack.is_none(),
        "baked to make the edit"
    );
    assert_eq!(model(&d, def).face_count(), 6, "the face is gone");
    d.undo();
    assert!(
        d.assembly().def(def).unwrap().stack.is_some(),
        "one undo step"
    );
}

#[test]
fn only_the_features_up_to_the_one_touched_are_baked() {
    let (mut d, inst, def) = grouped_cube();
    round(&mut d, inst, def, 0.25);
    // A second round at the far corner.
    let far = {
        let m = model(&d, def);
        m.edges()
            .filter(|(_, e)| (m.vertex(e.a).z - m.vertex(e.b).z).abs() > 1.0)
            .max_by(|(_, a), (_, b)| {
                let s = |e: &prism_topo::Edge| m.vertex(e.a).x + m.vertex(e.a).y;
                s(a).total_cmp(&s(b))
            })
            .unwrap()
            .0
    };
    assert!(d.run_in(
        &[inst],
        Command::RoundEdges {
            edges: [far].into(),
            size: 0.25,
            fillet: true,
        }
    ));
    assert_eq!(
        d.assembly()
            .def(def)
            .unwrap()
            .stack
            .as_ref()
            .unwrap()
            .features
            .len(),
        2
    );
    // An edge of the first round (one the second round did not make).
    let first_round_edge = {
        let def = d.assembly().def(def).unwrap();
        let stack = def.stack.as_ref().unwrap();
        let mut one = prism_doc::Stack {
            base: stack.base.clone(),
            features: stack.features[..1].to_vec(),
        };
        let after_first = one.evaluate();
        after_first
            .edges()
            .map(|(e, _)| e)
            .find(|e| stack.base.edge(*e).is_none())
            .unwrap()
    };
    assert!(d.run_in(
        &[inst],
        Command::Soften {
            edges: [first_round_edge].into(),
            soft: true,
        }
    ));
    let stack = d.assembly().def(def).unwrap().stack.as_ref().unwrap();
    assert_eq!(stack.features.len(), 1, "only the first round was baked");
    assert!(!stack.features[0].is_broken());
}

// ---------- loose geometry, and features that go with their solid ----------

/// A box `w` wide and `h` tall with its low corner at `at`, at the root.
fn boxed(d: &mut Document, at: DVec3, w: f64, h: f64) {
    d.run(Command::Rect {
        x0: at.x,
        y0: at.y,
        x1: at.x + w,
        y1: at.y + w,
        z: at.z,
    });
    let f = prism_ops::face_at(d.model(), at + DVec3::new(w / 2.0, w / 2.0, 0.0)).expect("base");
    let up = d.model().face(f).unwrap().plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face: f,
        distance: if up { h } else { -h },
        new_loop: false,
    });
}

/// The vertical edge of the model nearest `near`.
fn upright_edge_near(m: &Model, near: DVec3) -> EdgeId {
    m.edges()
        .filter(|(_, e)| (m.vertex(e.a).z - m.vertex(e.b).z).abs() > 0.5)
        .min_by(|(_, a), (_, b)| {
            let d = |e: &prism_topo::Edge| (m.vertex(e.a) - near).length();
            d(a).total_cmp(&d(b))
        })
        .unwrap()
        .0
}

/// What lies inside the box `lo`..`hi` at the root, as a selection.
fn select_in(m: &Model, lo: DVec3, hi: DVec3) -> Selection {
    let inside =
        |p: DVec3| p.cmpge(lo - DVec3::splat(0.5)).all() && p.cmple(hi + DVec3::splat(0.5)).all();
    let vertices: BTreeSet<_> = m
        .vertices()
        .filter(|(_, p)| inside(*p))
        .map(|(v, _)| v)
        .collect();
    Selection {
        edges: m
            .edges()
            .filter(|(_, e)| vertices.contains(&e.a) && vertices.contains(&e.b))
            .map(|(id, _)| id)
            .collect(),
        faces: m
            .faces()
            .filter(|(_, f)| f.loops().flatten().all(|v| vertices.contains(v)))
            .map(|(id, _)| id)
            .collect(),
        vertices,
        guides: BTreeSet::new(),
    }
}

/// Two loose boxes, the one at x 0 rounded at its far-left corner.
fn two_boxes_one_rounded() -> Document {
    let mut d = Document::new();
    boxed(&mut d, DVec3::ZERO, 2.0, 2.0);
    boxed(&mut d, DVec3::new(6.0, 0.0, 0.0), 2.0, 2.0);
    let e = upright_edge_near(d.model(), DVec3::new(0.0, 0.0, 1.0));
    assert!(d.run(Command::RoundEdges {
        edges: [e].into(),
        size: 0.25,
        fillet: true,
    }));
    assert!(d.assembly().root().stack.is_some(), "live at the top");
    d
}

fn new_group(d: &mut Document, sel: Selection) -> (InstanceId, DefId) {
    let id = d.assembly().next_instance();
    assert!(d.run(Command::MakeGroup {
        selection: sel,
        instances: BTreeSet::new(),
    }));
    (id, d.assembly().find_instance(id).unwrap().1.def)
}

#[test]
fn grouping_the_rounded_box_takes_its_round_with_it() {
    let mut d = two_boxes_one_rounded();
    let sel = select_in(d.model(), DVec3::ZERO, DVec3::new(2.0, 2.0, 2.0));
    let (_, def) = new_group(&mut d, sel);
    let group = d.assembly().def(def).unwrap();
    let stack = group.stack.as_ref().expect("the round went with its box");
    assert_eq!(stack.features.len(), 1);
    assert!(!stack.features[0].is_broken());
    assert_eq!(group.model.face_count(), 7, "the box and its round");
    // The other box is plain and nothing is live at the top any more.
    assert!(d.assembly().root().stack.is_none());
    assert_eq!(d.model().face_count(), 6);
    // And the round can be changed in its new home.
    assert!(d.run(Command::SetFeatureSize {
        def,
        index: 0,
        size: 0.5,
    }));
    assert_eq!(d.assembly().def(def).unwrap().model.face_count(), 7);
    d.undo();
    d.undo();
    assert!(
        d.assembly().root().stack.is_some(),
        "undo puts it back at the top"
    );
}

#[test]
fn grouping_the_other_box_leaves_the_round_live_at_the_top() {
    let mut d = two_boxes_one_rounded();
    let sel = select_in(
        d.model(),
        DVec3::new(6.0, 0.0, 0.0),
        DVec3::new(8.0, 2.0, 2.0),
    );
    let (_, def) = new_group(&mut d, sel);
    assert!(d.assembly().def(def).unwrap().stack.is_none());
    let top = d.assembly().root().stack.as_ref().expect("still live");
    assert_eq!(top.features.len(), 1);
    assert!(!top.features[0].is_broken());
    assert_eq!(d.model().face_count(), 7, "the rounded box");
}

#[test]
fn grouping_part_of_a_rounded_box_bakes_its_round() {
    let mut d = two_boxes_one_rounded();
    // The top face of the rounded box alone.
    let top = face_facing(d.model(), DVec3::Z);
    let sel = Selection {
        faces: [top].into(),
        ..Selection::default()
    };
    let (_, def) = new_group(&mut d, sel);
    assert!(
        d.assembly().root().stack.is_none(),
        "baked, not half-carried"
    );
    assert!(d.assembly().def(def).unwrap().stack.is_none());
}

#[test]
fn exploding_a_group_gives_its_round_back_to_the_top() {
    let mut d = two_boxes_one_rounded();
    let sel = select_in(d.model(), DVec3::ZERO, DVec3::new(2.0, 2.0, 2.0));
    let shown = d.model().face_count();
    let (id, _) = new_group(&mut d, sel);
    assert!(d.run(Command::Explode { instance: id }));
    let top = d.assembly().root().stack.as_ref().expect("live again");
    assert_eq!(top.features.len(), 1);
    assert!(!top.features[0].is_broken());
    assert_eq!(d.model().face_count(), shown, "the same two boxes");
}

#[test]
fn a_group_made_beside_a_round_leaves_it_live() {
    let mut d = two_boxes_one_rounded();
    // A third box made after, then grouped: the top-level round stays.
    boxed(&mut d, DVec3::new(12.0, 0.0, 0.0), 2.0, 2.0);
    let sel = select_in(
        d.model(),
        DVec3::new(12.0, 0.0, 0.0),
        DVec3::new(14.0, 2.0, 2.0),
    );
    let _ = new_group(&mut d, sel);
    assert!(d.assembly().root().stack.is_some());
}

#[test]
fn a_feature_gives_the_edge_and_faces_its_size_is_measured_across() {
    let (mut d, inst, def) = grouped_cube();
    round(&mut d, inst, def, 0.25);
    let stack = d.assembly().def(def).unwrap().stack.as_ref().unwrap();
    let g = stack
        .size_guide(0)
        .expect("a straight edge between flat faces");
    // The cube's low upright corner, where the round was made.
    assert!((g.a.x + g.a.y).abs() < 1e-9 && (g.b.x + g.b.y).abs() < 1e-9);
    assert!((g.a - g.b).length() > 1.9);
    assert_eq!(g.sides.len(), 2, "the two walls beside the corner");
    for s in &g.sides {
        // Across a wall from the edge: along the wall, away from the edge,
        // square to it.
        assert!((s.across.length() - 1.0).abs() < 1e-9);
        assert!(s.across.dot(g.b - g.a).abs() < 1e-9);
        assert!(s.across.dot(s.normal).abs() < 1e-9);
        assert!(s.across.x + s.across.y > 0.9, "into the wall, not away");
    }
    assert!(stack.size_guide(1).is_none(), "no such feature");
}

/// A round whose edges are gone is broken; pointing it at edges that are
/// there brings it back (Inspector part 4b).
#[test]
fn a_broken_round_is_given_new_edges_and_runs_again() {
    use prism_doc::{EdgeRef, Feature, Stack};
    let (d, _, def) = grouped_cube();
    let base = model(&d, def).clone();
    let corner = corner_edge(&base);
    let mut stack = Stack::new(base.clone());
    stack.features.push(Feature::Round {
        edges: vec![EdgeRef::of(&base, corner).unwrap()],
        size: 0.3,
        fillet: true,
        broken: false,
    });
    // The edge it points at is moved away: nothing is there any more.
    if let Feature::Round { edges, .. } = &mut stack.features[0] {
        edges[0].id = EdgeId(9999);
        edges[0].a += DVec3::new(9.0, 9.0, 9.0);
        edges[0].b += DVec3::new(9.0, 9.0, 9.0);
    }
    let plain = stack.evaluate();
    assert!(stack.features[0].is_broken());
    assert_eq!(
        plain.face_count(),
        base.face_count(),
        "a broken round does nothing"
    );
    // The far corner's edge, picked as it looks now, is the one rounded.
    let far = base
        .edges()
        .filter(|(_, e)| (base.vertex(e.a).z - base.vertex(e.b).z).abs() > 1.0)
        .max_by(|(_, a), (_, b)| {
            let s = |e: &prism_topo::Edge| base.vertex(e.a).x + base.vertex(e.a).y;
            s(a).total_cmp(&s(b))
        })
        .unwrap()
        .0;
    assert!(stack.set_edges(0, &[EdgeRef::of(&plain, far).unwrap()]));
    let rounded = stack.evaluate();
    assert!(!stack.features[0].is_broken());
    assert!(rounded.face_count() > base.face_count());
    // Edges that are not there, or none, change nothing.
    let was = stack.clone();
    let nowhere = EdgeRef {
        id: far,
        a: DVec3::new(50.0, 0.0, 0.0),
        b: DVec3::new(51.0, 0.0, 0.0),
    };
    assert!(!stack.set_edges(0, &[nowhere]));
    assert!(!stack.set_edges(0, &[]));
    assert!(!stack.set_edges(3, &[EdgeRef::of(&plain, far).unwrap()]));
    assert_eq!(stack, was);
}
