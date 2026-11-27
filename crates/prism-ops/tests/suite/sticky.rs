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

/// Session 11 (2026-11-16): a strip of faces with soft (hidden) lines
/// between them, like a loft's rows. Erasing the strip's bottom edge
/// takes the faces, and with them the soft lines that only hid between
/// them: left behind, they could not be seen, yet the next line drawn
/// there was cut where they ended.
#[test]
fn soft_lines_left_with_no_face_go_with_it() {
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    let mut m = Model::new();
    for (a, b) in [
        (p(0.0, 0.0), p(3.0, 0.0)),
        (p(3.0, 0.0), p(3.0, 1.0)),
        (p(3.0, 1.0), p(0.0, 1.0)),
        (p(0.0, 1.0), p(0.0, 0.0)),
    ] {
        add_segment(&mut m, a, b);
    }
    let mut soft = Vec::new();
    for x in [1.0, 2.0] {
        soft.extend(add_segment(&mut m, p(x, 0.0), p(x, 1.0)).edges);
    }
    for e in &soft {
        m.set_soft(*e, true, true);
    }
    assert_eq!(m.face_count(), 3);
    let bottom = |m: &Model| -> Vec<_> {
        m.edges()
            .filter(|(_, e)| m.vertex(e.a).y.abs() < 1e-9 && m.vertex(e.b).y.abs() < 1e-9)
            .map(|(id, _)| id)
            .collect()
    };
    let mut sel = Selection::default();
    sel.edges.extend(bottom(&m));
    assert_eq!(
        sel.edges.len(),
        3,
        "the bottom is cut where the soft lines meet it"
    );
    remove(&mut m, &sel);
    assert_eq!(m.face_count(), 0);
    assert!(
        m.edges().all(|(_, e)| !e.soft),
        "the soft lines went with their faces"
    );

    add_segment(&mut m, p(0.0, 0.0), p(3.0, 0.0));
    assert_eq!(bottom(&m).len(), 1, "the new line is one edge");
    assert_eq!(m.face_count(), 1, "and it closes the strip again");
}
