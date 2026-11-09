//! Drawing over edges already there (session 5, 2026-11-09).

use prism_math::DVec3;
use prism_ops::{Selection, add_segment, remove};
use prism_topo::Model;

/// A flat loop whose face was erased gets it back when one of its sides
/// is drawn again, as in SketchUp.
#[test]
fn drawing_over_a_side_of_a_loop_with_no_face_fills_it() {
    let (a, b, c) = (
        DVec3::new(0.0, 2.0, 0.0),
        DVec3::new(0.0, 2.0, 2.0),
        DVec3::new(3.0, 2.0, 0.0),
    );
    let mut m = Model::new();
    for (p, q) in [(a, b), (b, c), (c, a)] {
        add_segment(&mut m, p, q);
    }
    let f = m.faces().next().map(|(id, _)| id).expect("the triangle");
    let mut sel = Selection::default();
    sel.faces.insert(f);
    remove(&mut m, &sel);
    assert_eq!((m.face_count(), m.edges().count()), (0, 3));
    add_segment(&mut m, c, a);
    assert_eq!((m.face_count(), m.edges().count()), (1, 3), "filled");
    // With a face there already, no second one (the edge is only split
    // where the line ends, as drawing always does).
    add_segment(&mut m, a, a.lerp(b, 0.5));
    let f = m.faces().next().map(|(id, _)| id).expect("the triangle");
    assert_eq!(m.face_count(), 1);
    assert!((m.face_area(f) - 3.0).abs() < 1e-12);
    assert!(prism_ops::check::problems(&m).is_empty());
}
