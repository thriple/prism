//! Section planes (Annotation & Sections part 1): plain document objects.
//! They change no geometry, undo and redo exactly, and only one cuts.

#![cfg(not(target_arch = "wasm32"))]

use prism_doc::{Command, Document};
use prism_math::DVec3;
use proptest::prelude::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn boxed(d: &mut Document) {
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
}

#[test]
fn adding_a_plane_makes_it_the_one_that_cuts() {
    let mut d = Document::new();
    assert!(d.run(Command::AddSection {
        origin: v(0.0, 0.0, 1.0),
        normal: v(0.0, 0.0, 3.0),
    }));
    let (id, s) = d.assembly().active_section().expect("active");
    assert_eq!(s.normal, DVec3::Z, "the normal is made a unit vector");
    assert_eq!(s.name, "Section 1");
    assert!(d.run(Command::AddSection {
        origin: v(1.0, 0.0, 0.0),
        normal: DVec3::X,
    }));
    let (id2, _) = d.assembly().active_section().expect("active");
    assert_ne!(id, id2, "the newest cuts");
    assert_eq!(d.assembly().sections().0.len(), 2);
    // The first can be made the one that cuts again; only one ever does.
    assert!(d.run(Command::SetSectionActive { id, active: true }));
    assert_eq!(d.assembly().active_section().unwrap().0, id);
    assert!(d.run(Command::SetSectionActive { id, active: false }));
    assert!(d.assembly().active_section().is_none());
}

#[test]
fn an_unusable_normal_adds_nothing() {
    let mut d = Document::new();
    assert!(!d.run(Command::AddSection {
        origin: DVec3::ZERO,
        normal: DVec3::ZERO,
    }));
    assert!(!d.run(Command::AddSection {
        origin: v(f64::NAN, 0.0, 0.0),
        normal: DVec3::Z,
    }));
    assert!(d.assembly().sections().0.is_empty());
}

#[test]
fn move_flip_rename_and_delete_undo_exactly() {
    let mut d = Document::new();
    d.run(Command::AddSection {
        origin: DVec3::ZERO,
        normal: DVec3::Z,
    });
    let id = d.assembly().active_section().unwrap().0;
    let steps = [
        Command::MoveSection {
            id,
            origin: v(0.0, 0.0, 1.5),
        },
        Command::FlipSection { id },
        Command::RenameSection {
            id,
            name: "Roof".into(),
        },
        Command::DeleteSection { id },
    ];
    let mut states = vec![d.assembly().clone()];
    for c in steps {
        assert!(d.run(c));
        states.push(d.assembly().clone());
    }
    assert!(d.assembly().sections().0.is_empty());
    assert!(d.assembly().active_section().is_none());
    let s = states[3].section(id).unwrap();
    assert_eq!(s.origin, v(0.0, 0.0, 1.5));
    assert_eq!(s.normal, -DVec3::Z);
    assert_eq!(s.name, "Roof");
    for k in (0..4).rev() {
        assert!(d.undo());
        assert_eq!(d.assembly(), &states[k], "undo step {k}");
    }
    for k in 1..5 {
        assert!(d.redo());
        assert_eq!(d.assembly(), &states[k], "redo step {k}");
    }
}

#[test]
fn a_blank_name_is_refused_and_a_missing_plane_does_nothing() {
    let mut d = Document::new();
    d.run(Command::AddSection {
        origin: DVec3::ZERO,
        normal: DVec3::Z,
    });
    let id = d.assembly().active_section().unwrap().0;
    assert!(!d.run(Command::RenameSection {
        id,
        name: "  ".into()
    }));
    d.run(Command::DeleteSection { id });
    assert!(!d.run(Command::FlipSection { id }));
    assert!(!d.run(Command::MoveSection {
        id,
        origin: DVec3::X
    }));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    /// Planes never change the model, whatever is done to them.
    #[test]
    fn planes_leave_the_geometry_alone(
        nx in -1.0..1.0f64, ny in -1.0..1.0f64, nz in 0.1..1.0f64,
        x in -5.0..5.0f64, y in -5.0..5.0f64, z in -5.0..5.0f64,
    ) {
        let mut d = Document::new();
        boxed(&mut d);
        let model = d.model().clone();
        d.run(Command::AddSection { origin: v(x, y, z), normal: v(nx, ny, nz) });
        let id = d.assembly().active_section().unwrap().0;
        d.run(Command::FlipSection { id });
        d.run(Command::MoveSection { id, origin: v(z, y, x) });
        prop_assert_eq!(d.model(), &model);
        d.run(Command::DeleteSection { id });
        prop_assert_eq!(d.model(), &model);
        d.undo();
        d.undo();
        d.undo();
        d.undo();
        prop_assert_eq!(d.model(), &model);
    }
}
