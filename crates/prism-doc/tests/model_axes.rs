//! The model's own axes (Annotation & Sections part 4): everything moves
//! so the new axes are the world's, in one undo step.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_doc::{Command, Document};
use prism_math::{DAffine3, DMat3, DVec3};
use prism_ops::Selection;
use proptest::prelude::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A box with a group beside it, a guide, a section plane and a dimension.
fn scene() -> Document {
    let mut d = Document::new();
    d.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 2.0,
        y1: 2.0,
        z: 0.0,
    });
    let f = prism_ops::face_at(d.model(), v(1.0, 1.0, 0.0)).expect("base");
    let up = d.model().face(f).expect("face").plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face: f,
        distance: if up { 2.0 } else { -2.0 },
        new_loop: false,
    });
    d.run(Command::Rect {
        x0: 5.0,
        y0: 0.0,
        x1: 6.0,
        y1: 1.0,
        z: 0.0,
    });
    let m = d.model();
    let selection = Selection {
        vertices: m
            .vertices()
            .filter(|(_, p)| p.x > 4.0)
            .map(|(id, _)| id)
            .collect(),
        edges: m
            .edges()
            .filter(|(id, _)| {
                m.edge(*id)
                    .is_some_and(|e| [e.a, e.b].iter().all(|v| m.vertex(*v).x > 4.0))
            })
            .map(|(id, _)| id)
            .collect(),
        faces: m
            .faces()
            .filter(|(id, _)| {
                m.loop_points(m.face(*id).unwrap(), 0)
                    .iter()
                    .all(|p| p.x > 4.0)
            })
            .map(|(id, _)| id)
            .collect(),
        guides: BTreeSet::new(),
    };
    d.run(Command::MakeGroup {
        selection,
        instances: BTreeSet::new(),
    });
    d.run(Command::AddGuide {
        guide: prism_topo::Guide::Point {
            at: v(1.0, 3.0, 0.0),
        },
    });
    d.run(Command::AddSection {
        origin: v(1.0, 1.0, 1.0),
        normal: DVec3::X,
    });
    d.run(Command::AddDimension {
        a: v(0.0, -1.0, 0.0),
        b: v(2.0, -1.0, 0.0),
        offset: v(0.0, -1.0, 0.0),
    });
    d
}

fn frame(origin: DVec3, yaw: f64) -> DAffine3 {
    DAffine3::from_cols(
        v(yaw.cos(), yaw.sin(), 0.0),
        v(-yaw.sin(), yaw.cos(), 0.0),
        DVec3::Z,
        origin,
    )
}

#[test]
fn everything_moves_by_the_inverse_and_undo_is_exact() {
    let mut d = scene();
    let before = d.assembly().clone();
    let axes = frame(v(1.0, 2.0, 0.5), 0.6);
    let back = axes.inverse();
    assert!(d.run(Command::ChangeModelAxes { axes }));
    let asm = d.assembly();
    // Loose geometry.
    let want: Vec<DVec3> = before
        .root()
        .model
        .vertices()
        .map(|(_, p)| back.transform_point3(p))
        .collect();
    let got: Vec<DVec3> = asm.root().model.vertices().map(|(_, p)| p).collect();
    assert_eq!(want.len(), got.len());
    for p in &want {
        assert!(got.iter().any(|q| q.distance(*p) < 1e-9));
    }
    // A group at the top level: placed by the inverse; its inside untouched.
    let (id, inst) = before.root().instances.iter().next().expect("group");
    let moved = asm.root().instances[id];
    assert_eq!(moved.def, inst.def);
    assert!(moved.transform.abs_diff_eq(back * inst.transform, 1e-9));
    assert_eq!(asm.def(inst.def), before.def(inst.def));
    // Guides, planes and dimensions.
    let (_, g) = asm.root().model.guides().next().expect("guide");
    assert!(
        matches!(g, prism_topo::Guide::Point { at } if at.distance(back.transform_point3(v(1.0, 3.0, 0.0))) < 1e-9)
    );
    let (_, s) = asm.sections().0.iter().next().expect("plane");
    assert!(s.origin.distance(back.transform_point3(v(1.0, 1.0, 1.0))) < 1e-9);
    assert!(s.normal.distance(back.transform_vector3(DVec3::X)) < 1e-9);
    let (_, dim) = asm.dimensions().0.iter().next().expect("dimension");
    assert!((dim.length() - 2.0).abs() < 1e-9, "lengths do not change");
    // One undo step.
    assert!(d.undo());
    // (The counters that hand out new ids only ever go up, so compare the
    // contents.)
    let same = |a: &prism_doc::Assembly, b: &prism_doc::Assembly| {
        a.root().model.vertices().collect::<Vec<_>>()
            == b.root().model.vertices().collect::<Vec<_>>()
            && a.root().model.guides().map(|(_, g)| *g).collect::<Vec<_>>()
                == b.root().model.guides().map(|(_, g)| *g).collect::<Vec<_>>()
            && a.root().instances == b.root().instances
            && a.sections() == b.sections()
            && a.dimensions() == b.dimensions()
    };
    assert!(same(d.assembly(), &before));
    assert!(d.redo());
    assert!(
        d.assembly()
            .sections()
            .0
            .iter()
            .next()
            .unwrap()
            .1
            .origin
            .distance(back.transform_point3(v(1.0, 1.0, 1.0)))
            < 1e-9
    );
}

#[test]
fn axes_that_change_nothing_or_are_not_rigid_are_refused() {
    let mut d = scene();
    let before = d.assembly().clone();
    assert!(!d.run(Command::ChangeModelAxes {
        axes: DAffine3::IDENTITY
    }));
    // A scaled frame, a mirrored one, and one that is not finite.
    let scaled =
        DAffine3::from_mat3_translation(DMat3::from_diagonal(v(2.0, 1.0, 1.0)), DVec3::ZERO);
    let mirrored = DAffine3::from_cols(v(-1.0, 0.0, 0.0), DVec3::Y, DVec3::Z, DVec3::ZERO);
    let nan = DAffine3::from_translation(v(f64::NAN, 0.0, 0.0));
    for axes in [scaled, mirrored, nan] {
        assert!(!d.run(Command::ChangeModelAxes { axes }), "{axes:?}");
        assert_eq!(*d.assembly(), before);
    }
}

proptest! {
    /// Moving the axes keeps every distance: lengths, areas and the shape
    /// of the model do not change, and moving them back restores it.
    #[test]
    fn model_axes_keep_shapes(
        x in -5.0..5.0f64, y in -5.0..5.0f64, z in -5.0..5.0f64, yaw in -3.0..3.0f64
    ) {
        let mut d = scene();
        let axes = frame(v(x, y, z), yaw);
        prop_assume!(axes != DAffine3::IDENTITY);
        let area = |d: &Document| -> f64 {
            let m = d.assembly().flattened();
            m.faces().map(|(id, _)| m.face_area(id)).sum()
        };
        let a0 = area(&d);
        d.run(Command::ChangeModelAxes { axes });
        prop_assert!((area(&d) - a0).abs() < 1e-6);
        d.run(Command::ChangeModelAxes { axes: axes.inverse() });
        prop_assert!((area(&d) - a0).abs() < 1e-6);
    }
}
