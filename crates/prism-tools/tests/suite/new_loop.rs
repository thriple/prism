//! Push/Pull with a new loop (Alt), pushed into a solid (owner's corner
//! test, 2026-11-27): "A new loop push in this instance should have left the
//! same envelope but internally there would be a new face."

use prism_math::DVec3;
use prism_topo::Model;

fn corner() -> Model {
    let c = prism_file::read(include_bytes!("../data/owner-corner-test.prism")).expect("the file");
    c.assembly.root().model.clone()
}

/// The outside of a model: the area on each plane (and on curved faces in
/// all), however it is divided into faces.
fn outside(m: &Model) -> std::collections::BTreeMap<String, i64> {
    let mut out = std::collections::BTreeMap::new();
    for (s, a) in m.faces().map(|(id, f)| {
        let s = match f.plane() {
            Some(p) => {
                // In thousandths, so there is no "-0".
                let r = |x: f64| (x * 1e3).round() as i64;
                let n = p.normal;
                format!("{} {} {} {}", r(n.x), r(n.y), r(n.z), r(n.dot(p.origin)))
            }
            None => "curved".to_string(),
        };
        (s, m.face_area(id))
    }) {
        *out.entry(s).or_insert(0.0) += a;
    }
    out.into_iter()
        .map(|(k, a)| (k, (a * 1e2).round() as i64))
        .collect()
}

#[test]
fn a_new_loop_pushed_through_the_front_keeps_the_outside() {
    let m = corner();
    // The front face, with the arc along its top (y = -0.104, facing -y).
    let y = -0.10369278432345297;
    let front = prism_ops::face_at(&m, DVec3::new(7.0, y, 0.3)).expect("the front face");
    let n = m.face(front).unwrap().plane().unwrap().normal;
    assert!(n.y < -0.99, "{n:?}");
    let mut w = m.clone();
    prism_ops::push_pull(&mut w, front, -1.186, true);
    assert_eq!(prism_ops::check::problems(&w), Vec::<String>::new());
    let h = prism_ops::health::check(&w);
    assert_eq!((h.watertight, h.leaky), (1, 0), "{h:?}");
    // A new face inside, where the loop was pushed to; the only edges with
    // three faces are its own, where it meets the walls around it.
    let inner = prism_ops::face_at(&w, DVec3::new(7.0, y + 1.186, 0.3)).expect("a face inside");
    let own: std::collections::BTreeSet<_> = w.face(inner).unwrap().edge_ids().collect();
    assert!(
        h.non_manifold.edges.iter().all(|e| own.contains(e)),
        "{h:?}"
    );
    let without = {
        let mut x = w.clone();
        x.remove_face(inner);
        x
    };
    assert_eq!(outside(&without), outside(&m), "the same envelope");
}
