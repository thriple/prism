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
