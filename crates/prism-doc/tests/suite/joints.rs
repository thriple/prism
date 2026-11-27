//! Hinge and Slide joints (Fit & Mechanisms part 1): plain document
//! objects between two instances. They change no geometry, undo and
//! redo exactly, and are dropped when either part they name is gone.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_doc::{Command, Document, InstanceId, JointKind};
use prism_math::DVec3;
use prism_ops::Selection;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn everything(m: &prism_topo::Model) -> Selection {
    Selection {
        edges: m.edges().map(|(id, _)| id).collect(),
        faces: m.faces().map(|(id, _)| id).collect(),
        ..Default::default()
    }
}

/// A box at `at`, 1x1x1, made into its own group at the root. Returns its
/// instance id.
fn boxed_group(d: &mut Document, at: DVec3) -> InstanceId {
    d.run(Command::Rect {
        x0: at.x,
        y0: at.y,
        x1: at.x + 1.0,
        y1: at.y + 1.0,
        z: at.z,
    });
    let f = prism_ops::face_at(d.model(), at + v(0.5, 0.5, 0.0)).expect("base");
    let up = d.model().face(f).expect("face").plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face: f,
        distance: if up { 1.0 } else { -1.0 },
        new_loop: false,
    });
    let id = d.assembly().next_instance();
    assert!(d.run(Command::MakeGroup {
        selection: everything(d.model()),
        instances: BTreeSet::new(),
    }));
    id
}

#[test]
fn adding_a_hinge_or_a_slide_and_undo_removes_it() {
    let mut d = Document::new();
    let a = boxed_group(&mut d, v(0.0, 0.0, 0.0));
    let b = boxed_group(&mut d, v(2.0, 0.0, 0.0));
    assert!(d.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b,
        origin: v(1.0, 0.0, 0.0),
        dir: v(0.0, 1.0, 0.0),
    }));
    let (joints, _) = d.assembly().joints();
    assert_eq!(joints.len(), 1);
    let (id, j) = joints.iter().next().expect("one");
    assert_eq!(j.kind, JointKind::Hinge);
    assert_eq!((j.a, j.b), (a, b));
    assert_eq!(j.axis_origin, v(1.0, 0.0, 0.0));
    assert_eq!(j.axis_dir, v(0.0, 1.0, 0.0));
    assert!(j.min.is_none() && j.max.is_none() && !j.stopped);
    let id = *id;

    assert!(d.undo());
    assert!(d.assembly().joints().0.is_empty());
    assert!(d.redo());
    assert_eq!(d.assembly().joint(id).expect("back").a, a);

    // A Slide the same way.
    assert!(d.run(Command::AddJoint {
        kind: JointKind::Slide,
        a,
        b,
        origin: v(1.0, 0.0, 0.0),
        dir: v(1.0, 0.0, 0.0),
    }));
    assert_eq!(d.assembly().joints().0.len(), 2);
}

#[test]
fn a_joint_is_refused_for_the_same_instance_or_an_unusable_axis() {
    let mut d = Document::new();
    let a = boxed_group(&mut d, v(0.0, 0.0, 0.0));
    assert!(!d.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b: a,
        origin: v(0.0, 0.0, 0.0),
        dir: v(1.0, 0.0, 0.0),
    }));
    let b = boxed_group(&mut d, v(2.0, 0.0, 0.0));
    assert!(!d.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b,
        origin: v(0.0, 0.0, 0.0),
        dir: v(0.0, 0.0, 0.0),
    }));
    // An instance not in the root context (made up) is refused too.
    assert!(!d.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b: InstanceId(9999),
        origin: v(0.0, 0.0, 0.0),
        dir: v(1.0, 0.0, 0.0),
    }));
    assert!(d.assembly().joints().0.is_empty());
}

#[test]
fn each_bound_is_set_and_cleared_on_its_own() {
    let mut d = Document::new();
    let a = boxed_group(&mut d, v(0.0, 0.0, 0.0));
    let b = boxed_group(&mut d, v(2.0, 0.0, 0.0));
    d.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b,
        origin: v(1.0, 0.0, 0.0),
        dir: v(0.0, 1.0, 0.0),
    });
    let id = *d.assembly().joints().0.keys().next().expect("one");

    assert!(d.run(Command::SetJointBound {
        id,
        max: true,
        value: Some(90.0),
    }));
    assert_eq!(d.assembly().joint(id).unwrap().max, Some(90.0));
    assert_eq!(d.assembly().joint(id).unwrap().min, None);

    assert!(d.run(Command::SetJointBound {
        id,
        max: false,
        value: Some(-30.0),
    }));
    assert_eq!(d.assembly().joint(id).unwrap().min, Some(-30.0));
    assert_eq!(d.assembly().joint(id).unwrap().max, Some(90.0));

    // Clearing one bound leaves the other.
    assert!(d.run(Command::SetJointBound {
        id,
        max: true,
        value: None,
    }));
    assert_eq!(d.assembly().joint(id).unwrap().max, None);
    assert_eq!(d.assembly().joint(id).unwrap().min, Some(-30.0));
}

#[test]
fn a_bound_dragged_past_the_other_is_refused_and_the_other_stays() {
    let mut d = Document::new();
    let a = boxed_group(&mut d, v(0.0, 0.0, 0.0));
    let b = boxed_group(&mut d, v(2.0, 0.0, 0.0));
    d.run(Command::AddJoint {
        kind: JointKind::Slide,
        a,
        b,
        origin: v(1.0, 0.0, 0.0),
        dir: v(1.0, 0.0, 0.0),
    });
    let id = *d.assembly().joints().0.keys().next().expect("one");
    d.run(Command::SetJointBound {
        id,
        max: true,
        value: Some(5.0),
    });
    // Trying to push min past the existing max leaves min unset, and max
    // as it was.
    d.run(Command::SetJointBound {
        id,
        max: false,
        value: Some(10.0),
    });
    let j = d.assembly().joint(id).unwrap();
    assert_eq!(j.max, Some(5.0));
    assert_eq!(j.min, None);
}

#[test]
fn stopped_toggles_and_the_joint_can_be_removed() {
    let mut d = Document::new();
    let a = boxed_group(&mut d, v(0.0, 0.0, 0.0));
    let b = boxed_group(&mut d, v(2.0, 0.0, 0.0));
    d.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b,
        origin: v(1.0, 0.0, 0.0),
        dir: v(0.0, 1.0, 0.0),
    });
    let id = *d.assembly().joints().0.keys().next().expect("one");
    assert!(d.run(Command::SetJointStopped { id, stopped: true }));
    assert!(d.assembly().joint(id).unwrap().stopped);
    assert!(d.run(Command::SetJointStopped { id, stopped: false }));
    assert!(!d.assembly().joint(id).unwrap().stopped);

    assert!(d.run(Command::DeleteJoint { id }));
    assert!(d.assembly().joint(id).is_none());
}

#[test]
fn joint_of_finds_it_from_either_part_and_erasing_one_drops_it() {
    let mut d = Document::new();
    let a = boxed_group(&mut d, v(0.0, 0.0, 0.0));
    let b = boxed_group(&mut d, v(2.0, 0.0, 0.0));
    d.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b,
        origin: v(1.0, 0.0, 0.0),
        dir: v(0.0, 1.0, 0.0),
    });
    assert_eq!(d.assembly().joint_of(a).expect("found via a").1.b, b);
    assert_eq!(d.assembly().joint_of(b).expect("found via b").1.a, a);

    d.run(Command::EraseInstances {
        instances: [a].into(),
    });
    assert!(d.assembly().joints().0.is_empty());
}

#[test]
fn changing_the_models_axes_moves_a_joints_axis_too() {
    let mut d = Document::new();
    let a = boxed_group(&mut d, v(0.0, 0.0, 0.0));
    let b = boxed_group(&mut d, v(2.0, 0.0, 0.0));
    d.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b,
        origin: v(1.0, 0.0, 0.0),
        dir: v(0.0, 1.0, 0.0),
    });
    let id = *d.assembly().joints().0.keys().next().expect("one");
    assert!(d.run(Command::ChangeModelAxes {
        axes: prism_math::DAffine3::from_translation(v(1.0, 0.0, 0.0)),
    }));
    let j = d.assembly().joint(id).unwrap();
    assert!((j.axis_origin - v(0.0, 0.0, 0.0)).length() < 1e-9);
    assert!((j.axis_dir - v(0.0, 1.0, 0.0)).length() < 1e-9);
}
