//! The owner's recorded test (2026-11-14): a face lofted between four
//! curves, then the flat back face beside it pulled out. It was refused
//! ("would bend a curved face") with and without Alt's new loop.

use prism_geom::Surface;
use prism_ops::pushpull::{Pulled, push_pull, refusal_with};
use prism_ops::{check, health};
use prism_topo::{FaceId, Model};

fn lofted() -> (Model, FaceId) {
    let m = Model::from_json(include_str!("data/lofted-back.json")).expect("the model");
    // The flat face facing -y (the back).
    let back = m
        .faces()
        .find(|(_, f)| f.plane().is_some_and(|p| p.normal.y < -0.9))
        .map(|(id, _)| id)
        .expect("the back face");
    (m, back)
}

fn clean(m: &Model) {
    assert_eq!(check::problems(m), Vec::<String>::new());
    assert!(health::check(m).reversed.is_empty());
}

#[test]
fn the_back_of_a_lofted_shape_pulls_out_with_a_new_loop() {
    let (mut m, back) = lofted();
    assert_eq!(refusal_with(&m, back, 1.0, true), None);
    let before = m.face_count();
    assert!(matches!(
        push_pull(&mut m, back, 1.0, true),
        Pulled::Extruded { .. }
    ));
    assert!(m.face_count() > before);
    // The lofted face is still there, and the sides swept from the two
    // curved edges are NURBS faces too.
    let nurbs = m
        .faces()
        .filter(|(_, f)| matches!(f.surface, Surface::Nurbs(_)))
        .count();
    assert_eq!(nurbs, 3);
    // (The face left behind faces into the new block, as with any new
    // loop on a box, so Model Health's reversed list is not checked.)
    assert_eq!(check::problems(&m), Vec::<String>::new());
}

#[test]
fn the_back_of_a_lofted_shape_pulls_out_without_a_new_loop() {
    let (mut m, back) = lofted();
    assert_eq!(refusal_with(&m, back, 1.0, false), None);
    let r = push_pull(&mut m, back, 1.0, false);
    assert!(!matches!(r, Pulled::Nothing), "{r:?}");
    clean(&m);
}
