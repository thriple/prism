//! Construction planes (Annotation & Sections part 4): plain document
//! objects the drawing tools use as their working plane. They change no
//! geometry, and undo and redo exactly.

#![cfg(not(target_arch = "wasm32"))]

use prism_doc::{Command, Document};
use prism_math::DVec3;
use proptest::prelude::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

#[test]
fn adding_a_plane_makes_it_the_working_plane() {
    let mut d = Document::new();
    assert!(d.run(Command::AddWorkPlane {
        origin: v(0.0, 0.0, 1.0),
        normal: v(0.0, 0.0, 3.0),
    }));
    let (id, p) = d.assembly().active_work_plane().expect("active");
    assert_eq!(p.normal, DVec3::Z, "the normal is made a unit vector");
    assert_eq!(p.name, "Plane 1");
    assert!(d.run(Command::AddWorkPlane {
        origin: v(1.0, 0.0, 0.0),
        normal: DVec3::X,
    }));
    let (id2, _) = d.assembly().active_work_plane().expect("active");
    assert_ne!(id, id2, "the newest is the one in use");
    assert_eq!(d.assembly().work_planes().0.len(), 2);
    assert!(d.run(Command::SetWorkPlaneActive { id, active: true }));
    assert_eq!(d.assembly().active_work_plane().unwrap().0, id);
    assert!(d.run(Command::SetWorkPlaneActive { id, active: false }));
    assert!(d.assembly().active_work_plane().is_none());
}

#[test]
fn an_unusable_normal_adds_nothing() {
    let mut d = Document::new();
    assert!(!d.run(Command::AddWorkPlane {
        origin: DVec3::ZERO,
        normal: DVec3::ZERO,
    }));
    assert!(!d.run(Command::AddWorkPlane {
        origin: v(f64::NAN, 0.0, 0.0),
        normal: DVec3::Z,
    }));
    assert!(d.assembly().work_planes().0.is_empty());
}

#[test]
fn move_rename_and_delete_undo_exactly() {
    let mut d = Document::new();
    d.run(Command::AddWorkPlane {
        origin: DVec3::ZERO,
        normal: DVec3::Y,
    });
    let (id, _) = d.assembly().active_work_plane().unwrap();
    let after_add = d.assembly().clone();
    assert!(d.run(Command::MoveWorkPlane {
        id,
        origin: v(0.0, 4.0, 0.0),
    }));
    assert_eq!(
        d.assembly().work_plane(id).unwrap().origin,
        v(0.0, 4.0, 0.0)
    );
    assert!(d.run(Command::RenameWorkPlane {
        id,
        name: "  Wall  ".into(),
    }));
    assert_eq!(d.assembly().work_plane(id).unwrap().name, "Wall");
    assert!(d.run(Command::DeleteWorkPlane { id }));
    assert!(d.assembly().work_planes().0.is_empty());
    assert!(d.assembly().active_work_plane().is_none());
    for _ in 0..3 {
        assert!(d.undo());
    }
    assert_eq!(*d.assembly(), after_add);
    for _ in 0..3 {
        assert!(d.redo());
    }
    assert!(d.assembly().work_planes().0.is_empty());
}

#[test]
fn a_plane_is_not_a_section_plane() {
    let mut d = Document::new();
    d.run(Command::AddWorkPlane {
        origin: DVec3::ZERO,
        normal: DVec3::Z,
    });
    assert!(d.assembly().sections().0.is_empty());
    assert!(d.assembly().active_section().is_none());
}

proptest! {
    /// Planes never change the model, whatever they are and wherever.
    #[test]
    fn planes_never_change_geometry(
        ox in -9.0..9.0f64, oy in -9.0..9.0f64, oz in -9.0..9.0f64,
        nx in -1.0..1.0f64, ny in -1.0..1.0f64, nz in 0.1..1.0f64,
    ) {
        let mut d = Document::new();
        d.run(Command::Rect { x0: 0.0, y0: 0.0, x1: 2.0, y1: 2.0, z: 0.0 });
        let before = d.model().clone();
        d.run(Command::AddWorkPlane { origin: v(ox, oy, oz), normal: v(nx, ny, nz) });
        let (id, _) = d.assembly().active_work_plane().unwrap();
        d.run(Command::SetWorkPlaneActive { id, active: false });
        d.run(Command::DeleteWorkPlane { id });
        prop_assert_eq!(d.model(), &before);
    }
}
