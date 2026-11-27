//! Group Axes (brief 36): Change Axes moves nothing in the world, for a
//! group and for every copy of a component; Reset Axes and Axes to Centre.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_doc::{Change, Command, Document, InstanceId};
use prism_math::{DAffine3, DQuat, DVec3};
use prism_ops::Selection;
use prism_topo::Model;
use proptest::prelude::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn boxed(d: &mut Document, at: DVec3, w: f64, h: f64) {
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
}

fn everything(m: &Model) -> Selection {
    Selection {
        vertices: m.vertices().map(|(id, _)| id).collect(),
        edges: m.edges().map(|(id, _)| id).collect(),
        faces: m.faces().map(|(id, _)| id).collect(),
        guides: BTreeSet::new(),
    }
}

fn group_all(d: &mut Document, name: Option<&str>) -> InstanceId {
    let id = d.assembly().next_instance();
    let selection = everything(d.model());
    let c = match name {
        None => Command::MakeGroup {
            selection,
            instances: BTreeSet::new(),
        },
        Some(n) => Command::MakeComponent {
            selection,
            instances: BTreeSet::new(),
            name: n.into(),
        },
    };
    assert!(d.run(c));
    id
}

/// Every vertex in the world.
fn world(d: &Document) -> Vec<[f64; 3]> {
    let m = d.assembly().flattened();
    m.vertices().map(|(_, p)| p.to_array()).collect()
}

fn same(a: &[[f64; 3]], b: &[[f64; 3]]) {
    assert_eq!(a.len(), b.len());
    // Nearest match: rounding can change a sorted order.
    for p in a {
        let p = DVec3::from_array(*p);
        let d = b
            .iter()
            .map(|q| p.distance(DVec3::from_array(*q)))
            .fold(f64::INFINITY, f64::min);
        assert!(d < 1e-9, "{p} moved by {d}");
    }
}

fn transform_of(d: &Document, i: InstanceId) -> DAffine3 {
    d.assembly().find_instance(i).expect("instance").1.transform
}

fn turned(angle: f64, at: DVec3) -> DAffine3 {
    DAffine3::from_rotation_translation(DQuat::from_rotation_z(angle), at)
}

#[test]
fn change_axes_moves_nothing_and_undoes() {
    let mut d = Document::new();
    boxed(&mut d, v(1.0, 2.0, 0.0), 2.0, 3.0);
    let g = group_all(&mut d, None);
    let before = world(&d);
    let t0 = transform_of(&d, g);
    let axes = turned(0.6, v(1.0, 1.0, 1.5));
    assert!(d.run(Command::ChangeAxes { instance: g, axes }));
    same(&before, &world(&d));
    let t1 = transform_of(&d, g);
    assert!((t1.translation - (t0 * axes).translation).length() < 1e-12);
    assert!(t1.matrix3.abs_diff_eq((t0 * axes).matrix3, 1e-12));
    assert!(d.undo());
    assert_eq!(transform_of(&d, g), t0, "undo puts it back exactly");
    same(&before, &world(&d));
}

#[test]
fn every_copy_of_a_component_keeps_its_place() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 2.0);
    let a = group_all(&mut d, Some("Leg"));
    d.run(Command::CopyInstances {
        instances: [a].into(),
        step: Change::Rotate {
            centre: v(5.0, 0.0, 0.0),
            axis: DVec3::Z,
            angle: 1.0,
        },
        count: 2,
    });
    let before = world(&d);
    assert!(d.run(Command::ChangeAxes {
        instance: a,
        axes: turned(-0.3, v(1.0, 0.0, 2.0)),
    }));
    same(&before, &world(&d));
}

#[test]
fn a_group_inside_a_group_keeps_its_place() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 1.0, 1.0);
    let inner = group_all(&mut d, None);
    boxed(&mut d, v(3.0, 0.0, 0.0), 1.0, 2.0);
    let outer = d.assembly().next_instance();
    let selection = everything(d.model());
    d.run(Command::MakeGroup {
        selection,
        instances: [inner].into(),
    });
    let before = world(&d);
    assert!(d.run(Command::ChangeAxes {
        instance: outer,
        axes: turned(0.25, v(2.0, 0.5, 0.0)),
    }));
    same(&before, &world(&d));
}

#[test]
fn reset_axes_squares_the_axes_at_the_low_corner() {
    let mut d = Document::new();
    boxed(&mut d, v(1.0, 2.0, 0.0), 2.0, 3.0);
    let g = group_all(&mut d, None);
    d.run(Command::Place {
        instances: [g].into(),
        change: Change::Rotate {
            centre: v(0.0, 0.0, 0.0),
            axis: DVec3::Z,
            angle: 0.7,
        },
    });
    let before = world(&d);
    let axes = d.assembly().reset_axes(g).expect("axes");
    assert!(d.run(Command::ChangeAxes { instance: g, axes }));
    same(&before, &world(&d));
    let t = transform_of(&d, g);
    assert!(
        t.matrix3.abs_diff_eq(prism_math::DMat3::IDENTITY, 1e-9),
        "no turn left: {t:?}"
    );
    let lo = before
        .iter()
        .map(|p| DVec3::from_array(*p))
        .reduce(DVec3::min)
        .unwrap();
    assert!(t.translation.distance(lo) < 1e-9, "{t:?} vs {lo}");
}

#[test]
fn axes_to_centre_puts_the_origin_in_the_middle() {
    let mut d = Document::new();
    boxed(&mut d, v(1.0, 2.0, 0.0), 2.0, 4.0);
    let g = group_all(&mut d, None);
    let (_, inst) = d.assembly().find_instance(g).unwrap();
    let axes = d.assembly().centre_axes(inst.def).expect("axes");
    assert!(d.run(Command::ChangeAxes { instance: g, axes }));
    assert!(transform_of(&d, g).translation.distance(v(2.0, 3.0, 2.0)) < 1e-12);
}

#[test]
fn a_locked_group_refuses() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 1.0, 1.0);
    let g = group_all(&mut d, None);
    d.run(Command::SetLocked {
        instances: [g].into(),
        locked: true,
    });
    assert!(!d.run(Command::ChangeAxes {
        instance: g,
        axes: turned(0.5, v(1.0, 0.0, 0.0)),
    }));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn random_rigid_axes_keep_every_vertex(
        ax in -1.0..1.0f64, ay in -1.0..1.0f64, az in 0.1..1.0f64,
        angle in -3.0..3.0f64,
        x in -5.0..5.0f64, y in -5.0..5.0f64, z in -5.0..5.0f64,
    ) {
        let mut d = Document::new();
        boxed(&mut d, v(1.0, 1.0, 0.0), 2.0, 1.5);
        let g = group_all(&mut d, Some("Part"));
        d.run(Command::CopyInstances {
            instances: [g].into(),
            step: Change::Translate { by: v(4.0, 0.0, 0.0) },
            count: 1,
        });
        let before = world(&d);
        let q = DQuat::from_axis_angle(v(ax, ay, az).normalize(), angle);
        d.run(Command::ChangeAxes {
            instance: g,
            axes: DAffine3::from_rotation_translation(q, v(x, y, z)),
        });
        same(&before, &world(&d));
    }
}
