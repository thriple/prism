//! Offset says why it does nothing (owner's session 12: the deck face would
//! not offset, with no word as to why).

use prism_geom::Nurbs;
use prism_math::DVec3;
use prism_ops::offset::{SPLINE_OFFSET, refusal};
use prism_ops::{add_segment, add_spline};
use prism_topo::Model;

#[test]
fn a_face_with_a_spline_edge_says_so() {
    let mut m = Model::new();
    let curve = Nurbs::through(
        &[
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.6, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
        ],
        false,
    )
    .expect("a curve");
    add_spline(&mut m, &curve);
    add_segment(&mut m, DVec3::new(2.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 0.0));
    let (face, _) = m
        .faces()
        .next()
        .expect("the curve and the line close a face");
    assert_eq!(refusal(&m, face, 0.1), Some(SPLINE_OFFSET));
    assert_eq!(refusal(&m, face, 0.0), Some(SPLINE_OFFSET));
}

#[test]
fn a_plain_face_has_no_refusal() {
    let mut m = Model::new();
    let c = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)].map(|(x, y)| DVec3::new(x, y, 0.0));
    for i in 0..4 {
        add_segment(&mut m, c[i], c[(i + 1) % 4]);
    }
    let (face, _) = m.faces().next().expect("a square");
    assert_eq!(refusal(&m, face, 0.3), None);
    assert_eq!(refusal(&m, face, -0.3), None);
}
