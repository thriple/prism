//! Offset says why it does nothing (owner's session 12: the deck face would
//! not offset, with no word as to why).

use prism_geom::Nurbs;
use prism_math::DVec3;
use prism_ops::offset::{offset, refusal};
use prism_ops::{add_segment, add_spline};
use prism_topo::Model;

/// A face under an arch: a spline from (0,0) over (1,0.6) to (2,0) and the
/// line back along the ground.
fn arch() -> (Model, prism_topo::FaceId, Nurbs) {
    let mut m = Model::new();
    let curve = Nurbs::through(
        &[
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.5, 0.45, 0.0),
            DVec3::new(1.0, 0.6, 0.0),
            DVec3::new(1.5, 0.45, 0.0),
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
    (m, face, curve)
}

#[test]
fn a_face_with_a_spline_edge_offsets_with_a_spline_the_distance_away() {
    for d in [0.1, -0.1] {
        let (mut m, face, curve) = arch();
        assert_eq!(refusal(&m, face, d), None);
        let used = offset(&mut m, face, d).expect("offsets");
        assert!((used - d).abs() < 1e-12);
        let p = prism_ops::check::problems(&m);
        assert!(p.is_empty(), "{p:?}");
        // The new spline lies `|d|` from the old one all along.
        let new = m
            .edges()
            .filter_map(|(_, e)| e.curve.spline().cloned())
            .find(|sp| sp.curve.eval(sp.at(0.5)).distance(curve.eval(0.5)) > 1e-6)
            .expect("a second spline");
        // Away from the ends, where a mitre makes it longer.
        for i in 5..16 {
            let p = new.point(f64::from(i) / 20.0);
            let near = (0..=400)
                .map(|k| curve.eval(f64::from(k) / 400.0).distance(p))
                .fold(f64::INFINITY, f64::min);
            assert!((near - d.abs()).abs() < 2e-3, "d {d}: {near}");
        }
    }
}

#[test]
fn a_spline_offset_goes_the_whole_way_and_stays_valid() {
    let (mut m, face, _) = arch();
    // Past where the arch's two sides meet the ground.
    let _ = offset(&mut m, face, 0.35);
    let p = prism_ops::check::problems(&m);
    assert!(p.is_empty(), "{p:?}");
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
