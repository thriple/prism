//! Push/Pull with a new loop (Alt), pushed into a solid (owner's corner
//! test, 2026-11-27): "A new loop push in this instance should have left the
//! same envelope but internally there would be a new face."

use prism_math::DVec3;
use prism_topo::Model;

fn corner() -> Model {
    let c = prism_file::read(include_bytes!("data/owner-corner-test.prism")).expect("the file");
    c.assembly.root().model.clone()
}

/// The outside of a model: each face's plane (or surface) and area, sorted,
/// for faces that are not inside the solid.
fn outside(m: &Model) -> Vec<(String, i64)> {
    let h = prism_ops::health::check(m);
    let mut out: Vec<(String, i64)> = m
        .faces()
        .filter(|(id, _)| !h.interior.faces.contains(id))
        .map(|(id, f)| {
            let s = match f.plane() {
                Some(p) => format!(
                    "{:.3} {:.3} {:.3} {:.3}",
                    p.normal.x,
                    p.normal.y,
                    p.normal.z,
                    p.normal.dot(p.origin)
                ),
                None => "curved".to_string(),
            };
            (s, (m.face_area(id) * 1e4).round() as i64)
        })
        .collect();
    out.sort();
    out
}

#[test]
#[ignore = "not fixed yet: the new loop eats into the solid"]
fn a_new_loop_pushed_through_the_front_keeps_the_outside() {
    let m = corner();
    // The front face, with the arc along its top (y = -0.104, facing -y).
    let front = prism_ops::face_at(&m, DVec3::new(7.0, -0.104, 0.3)).expect("the front face");
    let n = m.face(front).unwrap().plane().unwrap().normal;
    assert!(n.y < -0.99, "{n:?}");
    let mut w = m.clone();
    prism_ops::push_pull(&mut w, front, -1.186, true);
    assert_eq!(prism_ops::check::problems(&w), Vec::<String>::new());
    let h = prism_ops::health::check(&w);
    eprintln!("{h:?}");
    assert!(h.non_manifold.edges.is_empty(), "{h:?}");
    assert_eq!(outside(&w), outside(&m), "the same envelope");
    assert!(
        prism_ops::face_at(&w, DVec3::new(7.0, -0.104 + 1.186, 0.3)).is_some(),
        "a new face inside, where the loop was pushed to"
    );
}
