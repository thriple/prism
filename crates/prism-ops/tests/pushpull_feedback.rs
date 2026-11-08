//! Push/Pull cases from the owner's testing (2026-11-08).

use prism_math::DVec3;
use prism_ops::check;
use prism_topo::Model;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn poly(m: &mut Model, pts: &[DVec3]) {
    for i in 0..pts.len() {
        prism_ops::add_segment(m, pts[i], pts[(i + 1) % pts.len()]);
    }
}

fn cube() -> Model {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    let f = prism_ops::face_at(&m, v(2.0, 2.0, 0.0)).expect("base");
    prism_ops::push_pull(&mut m, f, 4.0, false);
    m
}

/// Every edge has exactly two faces: a closed solid with nothing missing.
fn closed(m: &Model) {
    let ef = m.edge_faces();
    for (id, e) in m.edges() {
        let n = ef.get(&id).map_or(0, |v| v.len());
        assert_eq!(
            n,
            2,
            "edge {:?}-{:?} has {n} faces",
            m.vertex(e.a),
            m.vertex(e.b)
        );
    }
}

fn normal_at(m: &Model, p: DVec3) -> DVec3 {
    let f = prism_ops::face_at(m, p).unwrap_or_else(|| panic!("a face at {p:?}"));
    m.face(f).expect("face").plane().normal
}

#[allow(dead_code)]
fn dump(m: &Model) {
    for (id, f) in m.faces() {
        let pts: Vec<_> = m.points(&f.outer).iter().map(|p| (p.x, p.y, p.z)).collect();
        eprintln!(
            "{id:?} n={:?} outer={pts:?} holes={}",
            f.plane().normal,
            f.holes.len()
        );
    }
}

#[test]
fn pushing_a_top_down_past_a_box_pulled_from_a_side() {
    let mut m = cube();
    // A rectangle on the side x = 4, reaching above half height, pulled out.
    poly(
        &mut m,
        &[
            v(4.0, 1.0, 1.0),
            v(4.0, 3.0, 1.0),
            v(4.0, 3.0, 3.5),
            v(4.0, 1.0, 3.5),
        ],
    );
    let r = prism_ops::face_at(&m, v(4.0, 2.0, 2.0)).expect("rect");
    prism_ops::push_pull(&mut m, r, 2.0, false);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    let top = prism_ops::face_at(&m, v(2.0, 2.0, 4.0)).expect("top");
    prism_ops::push_pull(&mut m, top, -2.0, false);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    closed(&m);
    // The pulled box's back, above the lowered top, is now closed, facing
    // into the cube's side (outward from the pulled box).
    assert_eq!(normal_at(&m, v(4.0, 2.0, 3.0)), v(-1.0, 0.0, 0.0));
    // Where the two still touch, there is no face between them.
    assert!(prism_ops::face_at(&m, v(4.0, 2.0, 1.5)).is_none());
    // The side around it is still there, facing out.
    assert_eq!(normal_at(&m, v(4.0, 0.5, 1.0)), v(1.0, 0.0, 0.0));
}

#[test]
fn pushing_a_ring_back_takes_its_walls_along() {
    let mut m = cube();
    let top = prism_ops::face_at(&m, v(2.0, 2.0, 4.0)).expect("top");
    prism_ops::offset::offset(&mut m, top, 1.0);
    let mid = prism_ops::face_at(&m, v(2.0, 2.0, 4.0)).expect("middle");
    prism_ops::push_pull(&mut m, mid, 2.0, false);
    let top2 = prism_ops::face_at(&m, v(2.0, 2.0, 6.0)).expect("top of pulled box");
    prism_ops::offset::offset(&mut m, top2, 0.5);
    let ring = prism_ops::face_at(&m, v(1.2, 2.0, 6.0)).expect("ring");
    prism_ops::push_pull(&mut m, ring, -1.0, false);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    closed(&m);
    // The middle stays up, on four new walls; the outer walls got shorter.
    assert!(
        prism_ops::face_at(&m, v(2.0, 2.0, 6.0)).is_some(),
        "the middle stays"
    );
    assert_eq!(normal_at(&m, v(1.0, 2.0, 4.5)), v(-1.0, 0.0, 0.0));
    assert!(
        prism_ops::face_at(&m, v(1.0, 2.0, 5.5)).is_none(),
        "no wall above the ring"
    );
    assert!(
        prism_ops::face_at(&m, v(1.2, 2.0, 5.0)).is_some(),
        "the ring moved down"
    );
    assert!(
        prism_ops::face_at(&m, v(1.2, 2.0, 6.0)).is_none(),
        "nothing left at the old height"
    );
}

#[test]
fn pushing_a_ring_all_the_way_back_leaves_no_slivers() {
    let mut m = cube();
    let top = prism_ops::face_at(&m, v(2.0, 2.0, 4.0)).expect("top");
    prism_ops::offset::offset(&mut m, top, 1.0);
    let mid = prism_ops::face_at(&m, v(2.0, 2.0, 4.0)).expect("middle");
    prism_ops::push_pull(&mut m, mid, 2.0, false);
    let top2 = prism_ops::face_at(&m, v(2.0, 2.0, 6.0)).expect("top of pulled box");
    prism_ops::offset::offset(&mut m, top2, 0.5);
    let ring = prism_ops::face_at(&m, v(1.2, 2.0, 6.0)).expect("ring");
    prism_ops::push_pull(&mut m, ring, -2.0, false);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    closed(&m);
    assert_eq!(normal_at(&m, v(1.2, 2.0, 4.0)), v(0.0, 0.0, 1.0));
    assert!(
        prism_ops::face_at(&m, v(2.0, 2.0, 6.0)).is_some(),
        "the middle stays"
    );
}

#[test]
fn two_boxes_of_different_heights_share_one_wall() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    prism_ops::add_rect(&mut m, 2.0, 0.0, 4.0, 2.0, 0.0);
    let a = prism_ops::face_at(&m, v(1.0, 1.0, 0.0)).expect("a");
    prism_ops::push_pull(&mut m, a, 3.0, false);
    let b = prism_ops::face_at(&m, v(3.0, 1.0, 0.0)).expect("b");
    prism_ops::push_pull(&mut m, b, 1.0, false);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    // One face between the boxes below the lower top, and the taller box's
    // wall above it.
    assert!(prism_ops::face_at(&m, v(2.0, 1.0, 0.5)).is_some());
    assert_eq!(normal_at(&m, v(2.0, 1.0, 2.0)), v(1.0, 0.0, 0.0));
}
