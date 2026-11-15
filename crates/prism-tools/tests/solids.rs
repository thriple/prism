//! Solid > Validate and Heal in the session (Solids & CAD part 1, brief
//! 37): rows per solid, Heal as one undo step, and none when nothing
//! needs it.

use prism_doc::Document;
use prism_tools::session::SolidOf;
use prism_tools::{Event, Input, Session};

fn act(s: &mut Session, id: &str) {
    s.handle(&Event {
        t: 0.0,
        input: Input::Action { id: id.into() },
    });
}

/// The test cube (2 by 2 by 2), with no history.
fn cube() -> Document {
    Document::with_model(prism_tools::scenes::cube().model().clone())
}

#[test]
fn a_cube_is_one_valid_solid_with_its_volume() {
    let s = Session::new(cube());
    let rows = s.solid_rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(rows[0].valid, "{rows:?}");
    assert_eq!(rows[0].name, "Solid 1");
    assert_eq!(rows[0].volume.as_deref(), Some("8.000 m³"));
    assert!(matches!(rows[0].of, SolidOf::Faces(ref f) if f.len() == 6));
}

#[test]
fn heal_fills_a_missing_top_in_one_undo_step() {
    let mut m = prism_tools::scenes::cube().model().clone();
    let top = m
        .faces()
        .find(|(_, f)| f.plane().is_some_and(|p| p.normal.z > 0.9))
        .map(|(id, _)| id)
        .expect("the top");
    m.remove_face(top);
    let mut s = Session::new(Document::with_model(m));
    let rows = s.solid_rows();
    assert!(!rows[0].valid);
    assert_eq!(rows[0].reasons, vec!["4 open edges (holes)".to_owned()]);
    assert_eq!(s.heal_solids(), "Filled 1 hole");
    assert_eq!(s.document().undo_name(), Some("Heal"));
    assert!(s.solid_rows()[0].valid);
    act(&mut s, "edit.undo");
    assert!(!s.solid_rows()[0].valid, "undone");
    assert!(!s.document().can_undo());
}

#[test]
fn heal_on_a_valid_cube_adds_no_undo_step() {
    let mut s = Session::new(cube());
    assert_eq!(s.heal_solids(), "Nothing needed healing");
    assert!(!s.document().can_undo());
}

#[test]
fn selecting_a_row_selects_its_faces() {
    let mut s = Session::new(cube());
    let rows = s.solid_rows();
    s.select_solid(&rows[0].of);
    assert_eq!(s.selection().faces.len(), 6);
}
