//! Booleans of groups (Solids & CAD part 2, brief 39): each one step,
//! the result a group placed at the cut solid's axes, the inputs gone (Trim
//! keeps the cutter), undo bringing them back, locked groups refusing.

use std::collections::BTreeSet;

use prism_doc::{Command, Document, InstanceId, SolidOp};
use prism_math::DVec3;
use prism_ops::Selection;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A `w` wide, `h` tall box at `at`, made into a group. Returns it.
fn boxed_group(d: &mut Document, at: DVec3, w: f64, h: f64) -> InstanceId {
    d.run(Command::Rect {
        x0: at.x,
        y0: at.y,
        x1: at.x + w,
        y1: at.y + w,
        z: at.z,
    });
    let f = prism_ops::face_at(d.model(), at + v(w / 2.0, w / 2.0, 0.0)).expect("base");
    let up = d.model().face(f).expect("face").plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face: f,
        distance: if up { h } else { -h },
        new_loop: false,
    });
    let m = d.model();
    let selection = Selection {
        vertices: m.vertices().map(|(id, _)| id).collect(),
        edges: m.edges().map(|(id, _)| id).collect(),
        faces: m.faces().map(|(id, _)| id).collect(),
        guides: BTreeSet::new(),
    };
    let id = d.assembly().next_instance();
    assert!(d.run(Command::MakeGroup {
        selection,
        instances: BTreeSet::new(),
    }));
    id
}

/// Two 2 m boxes, the second moved by (1, 1): they share a 1 x 1 x 2 block.
fn two() -> (Document, InstanceId, InstanceId) {
    let mut d = Document::new();
    let a = boxed_group(&mut d, v(0.0, 0.0, 0.0), 2.0, 2.0);
    let b = boxed_group(&mut d, v(1.0, 1.0, 0.0), 2.0, 2.0);
    (d, a, b)
}

/// The groups at the root and each one's volume in the world.
fn volumes(d: &Document) -> Vec<f64> {
    let asm = d.assembly();
    asm.root()
        .instances
        .values()
        .map(|i| {
            let mut m = asm.def(i.def).unwrap().model.clone();
            prism_ops::transform::affine(&mut m, &i.transform);
            let checks = prism_ops::validate::validate(&m);
            assert!(checks.iter().all(|c| c.valid), "{checks:?}");
            checks.iter().map(|c| c.volume.unwrap()).sum()
        })
        .collect()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

#[test]
fn union_makes_one_group_and_undo_brings_both_back() {
    let (mut d, a, b) = two();
    let before = d.assembly().root().clone();
    let world = d.assembly().flattened();
    assert!(d.run(Command::Boolean {
        op: SolidOp::Union,
        first: a,
        second: b,
    }));
    let v = volumes(&d);
    assert_eq!(v.len(), 1);
    close(v[0], 14.0);
    assert_eq!(d.undo_name(), Some("Union"));
    // At the second's axes.
    let (_, inst) = d.assembly().root().instances.iter().next().unwrap();
    close(inst.transform.translation.x, 1.0);
    assert!(d.undo());
    // The same groups and geometry (only the id counters move on).
    assert_eq!(d.assembly().root().instances, before.instances);
    assert_eq!(d.assembly().flattened(), world);
}

#[test]
fn subtract_trim_intersect_and_split() {
    for (op, want) in [
        (SolidOp::Subtract, vec![6.0]),
        (SolidOp::Intersect, vec![2.0]),
        (SolidOp::Trim, vec![6.0, 8.0]),
        (SolidOp::Split, vec![2.0, 6.0, 6.0]),
    ] {
        let (mut d, a, b) = two();
        assert!(d.run(Command::Boolean {
            op,
            first: a,
            second: b,
        }));
        let mut got = volumes(&d);
        got.sort_by(f64::total_cmp);
        assert_eq!(got.len(), want.len(), "{op:?}: {got:?}");
        for (g, w) in got.iter().zip(&want) {
            close(*g, *w);
        }
    }
}

#[test]
fn outer_shell_joins_three() {
    let (mut d, a, b) = two();
    let c = boxed_group(&mut d, v(2.0, 2.0, 0.0), 2.0, 2.0);
    assert!(d.run(Command::OuterShell {
        instances: [a, b, c].into(),
    }));
    let v = volumes(&d);
    assert_eq!(v.len(), 1);
    close(v[0], 8.0 * 3.0 - 2.0 - 2.0);
}

#[test]
fn a_locked_group_refuses() {
    let (mut d, a, b) = two();
    d.run(Command::SetLocked {
        instances: [a].into(),
        locked: true,
    });
    let before = d.assembly().clone();
    d.run(Command::Boolean {
        op: SolidOp::Union,
        first: a,
        second: b,
    });
    assert_eq!(*d.assembly(), before);
}

// ---- Live booleans (Inspector & Features part 3a) ----

fn the_group(d: &Document) -> (InstanceId, prism_doc::DefId) {
    let (id, inst) = d
        .assembly()
        .root()
        .instances
        .iter()
        .next()
        .expect("a group");
    (*id, inst.def)
}

fn body_volume(d: &Document, def: prism_doc::DefId) -> f64 {
    let m = &d.assembly().def(def).unwrap().model;
    prism_ops::validate::validate(m)
        .iter()
        .map(|c| c.volume.unwrap())
        .sum()
}

#[test]
fn a_subtract_stays_live_and_can_be_removed_or_baked() {
    let (mut d, a, b) = two();
    assert!(d.run(Command::Boolean {
        op: SolidOp::Subtract,
        first: a,
        second: b,
    }));
    let (_, def) = the_group(&d);
    let stack = d.assembly().def(def).unwrap().stack.as_ref().expect("live");
    assert_eq!(stack.features.len(), 1);
    assert_eq!(stack.features[0].name(), "Subtract");
    close(body_volume(&d, def), 6.0);
    // Taking the feature off brings the whole box back.
    assert!(d.run(Command::RemoveFeature { def, index: 0 }));
    close(body_volume(&d, def), 8.0);
    assert!(d.assembly().def(def).unwrap().stack.is_none());
    d.undo();
    close(body_volume(&d, def), 6.0);
    // Bake keeps the cut and drops the stack.
    assert!(d.run(Command::BakeFeatures { def }));
    assert!(d.assembly().def(def).unwrap().stack.is_none());
    close(body_volume(&d, def), 6.0);
}

#[test]
fn union_and_intersect_are_live_too() {
    for (op, name, volume) in [
        (SolidOp::Union, "Union", 14.0),
        (SolidOp::Intersect, "Intersect", 2.0),
    ] {
        let (mut d, a, b) = two();
        assert!(d.run(Command::Boolean {
            op,
            first: a,
            second: b,
        }));
        let (_, def) = the_group(&d);
        let stack = d.assembly().def(def).unwrap().stack.as_ref().expect("live");
        assert_eq!(stack.features[0].name(), name);
        close(body_volume(&d, def), volume);
    }
}

#[test]
fn trim_and_split_are_not_live() {
    for op in [SolidOp::Trim, SolidOp::Split] {
        let (mut d, a, b) = two();
        assert!(d.run(Command::Boolean {
            op,
            first: a,
            second: b,
        }));
        for inst in d.assembly().root().instances.values() {
            assert!(d.assembly().def(inst.def).unwrap().stack.is_none());
        }
    }
}

#[test]
fn a_pull_on_the_body_runs_under_the_cut() {
    let (mut d, a, b) = two();
    assert!(d.run(Command::Boolean {
        op: SolidOp::Subtract,
        first: a,
        second: b,
    }));
    let (inst, def) = the_group(&d);
    // The far wall, which the cut does not touch, pulled out a metre: the
    // cut still goes through the part that was there.
    let m = &d.assembly().def(def).unwrap().model;
    let far = m.vertices().map(|(_, p)| p.x).fold(f64::MIN, f64::max);
    let face = m
        .faces()
        .find(|(_, f)| {
            f.plane()
                .is_some_and(|p| p.normal.x > 0.99 && (p.origin.x - far).abs() < 1e-9)
        })
        .expect("the far wall")
        .0;
    assert!(d.run_in(
        &[inst],
        Command::PushPull {
            face,
            distance: 1.0,
            new_loop: false,
        }
    ));
    let stack = d
        .assembly()
        .def(def)
        .unwrap()
        .stack
        .as_ref()
        .expect("still live");
    assert_eq!(stack.features.len(), 1);
    assert!(!stack.features[0].is_broken());
    close(body_volume(&d, def), 12.0 - 2.0);
}

#[test]
fn a_second_boolean_adds_another_feature() {
    let (mut d, a, b) = two();
    let c = boxed_group(&mut d, v(0.0, 2.5, 0.0), 1.0, 1.0);
    assert!(d.run(Command::Boolean {
        op: SolidOp::Subtract,
        first: a,
        second: b,
    }));
    let (made, _) = d
        .assembly()
        .root()
        .instances
        .iter()
        .map(|(i, x)| (*i, x.def))
        .find(|(i, _)| *i != c)
        .expect("the result");
    assert!(d.run(Command::Boolean {
        op: SolidOp::Union,
        first: c,
        second: made,
    }));
    let id = d
        .assembly()
        .root()
        .instances
        .iter()
        .next()
        .map(|(_, i)| i.def)
        .unwrap();
    let stack = d.assembly().def(id).unwrap().stack.as_ref().expect("live");
    let names: Vec<_> = stack.features.iter().map(|f| f.name()).collect();
    assert_eq!(names, ["Subtract", "Union"]);
}
