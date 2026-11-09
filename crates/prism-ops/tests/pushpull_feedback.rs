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
    m.face(f).expect("face").plane().unwrap().normal
}

#[allow(dead_code)]
fn dump(m: &Model) {
    for (id, f) in m.faces() {
        let pts: Vec<_> = m.points(&f.outer).iter().map(|p| (p.x, p.y, p.z)).collect();
        eprintln!(
            "{id:?} n={:?} outer={pts:?} holes={}",
            f.plane().unwrap().normal,
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
fn two_boxes_of_different_heights_join() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    prism_ops::add_rect(&mut m, 2.0, 0.0, 4.0, 2.0, 0.0);
    let a = prism_ops::face_at(&m, v(1.0, 1.0, 0.0)).expect("a");
    prism_ops::push_pull(&mut m, a, 3.0, false);
    let b = prism_ops::face_at(&m, v(3.0, 1.0, 0.0)).expect("b");
    prism_ops::push_pull(&mut m, b, 1.0, false);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    // (Not checked as closed: a rectangle sharing an edge gets no base when
    // pulled, which is older behaviour.)
    // The boxes join: no face between them below the lower top, and the
    // taller box's wall above it.
    assert!(prism_ops::face_at(&m, v(2.0, 1.0, 0.5)).is_none());
    assert_eq!(normal_at(&m, v(2.0, 1.0, 2.0)), v(1.0, 0.0, 0.0));
}

fn block(m: &mut Model, x0: f64, y0: f64, x1: f64, y1: f64, h: f64) {
    prism_ops::add_rect(m, x0, y0, x1, y1, 0.0);
    let f = prism_ops::face_at(m, v((x0 + x1) / 2.0, (y0 + y1) / 2.0, 0.0)).expect("base");
    prism_ops::push_pull(m, f, h, false);
}

/// The owner's three blocks: B's side pulled until it meets A, then B's
/// front pushed in. Where faces meet back to back they go, as in SketchUp,
/// so nothing is left inside to show up later as a trench.
#[test]
fn pulling_a_face_flush_against_another_joins_the_blocks() {
    let mut m = Model::new();
    block(&mut m, 0.0, 0.0, 2.0, 2.0, 2.0); // A
    block(&mut m, 3.0, 0.5, 5.0, 2.5, 1.5); // B, a gap and an offset away
    let side = prism_ops::face_at(&m, v(3.0, 1.5, 0.75)).expect("B's side");
    prism_ops::push_pull(&mut m, side, 1.0, false);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    closed(&m);
    // Where B meets A there is no face; A's side stays above and beside B.
    assert!(
        prism_ops::face_at(&m, v(2.0, 1.0, 0.75)).is_none(),
        "no face between A and B"
    );
    assert_eq!(normal_at(&m, v(2.0, 1.0, 1.75)), v(1.0, 0.0, 0.0));
    assert_eq!(normal_at(&m, v(2.0, 0.25, 0.75)), v(1.0, 0.0, 0.0));

    // Push B's front in: A's side is uncovered there, and nothing else.
    let front = prism_ops::face_at(&m, v(4.0, 0.5, 0.75)).expect("B's front");
    prism_ops::push_pull(&mut m, front, -1.0, false);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    closed(&m);
    assert_eq!(normal_at(&m, v(2.0, 1.0, 0.75)), v(1.0, 0.0, 0.0));
    assert!(
        prism_ops::face_at(&m, v(2.0, 2.0, 0.75)).is_none(),
        "nothing left inside"
    );
}

/// The solid rule (DECISIONS #31): B's side pulled past A's side, into A.
/// The blocks join; nothing of either is left inside the other.
#[test]
fn pulling_a_block_into_another_joins_them() {
    let mut m = Model::new();
    block(&mut m, 0.0, 0.0, 2.0, 2.0, 2.0); // A
    block(&mut m, 3.0, 0.5, 5.0, 1.5, 1.0); // B
    let side = prism_ops::face_at(&m, v(3.0, 1.0, 0.5)).expect("B's side");
    prism_ops::push_pull(&mut m, side, 1.5, false); // 0.5 into A
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    closed(&m);
    assert!(
        prism_ops::face_at(&m, v(1.5, 1.0, 0.5)).is_none(),
        "B's end is not left inside A"
    );
    assert!(
        prism_ops::face_at(&m, v(1.75, 1.2, 1.0)).is_none(),
        "B's top is not left inside A"
    );
    // A's side has a hole where B goes in, and is still there around it.
    assert!(prism_ops::face_at(&m, v(2.0, 1.0, 0.5)).is_none());
    assert_eq!(normal_at(&m, v(2.0, 1.0, 1.5)), v(1.0, 0.0, 0.0));
}

/// A box's side pulled toward a smaller block stops at it and joins it
/// (owner, 2026-11-09: Push/Pull never passes through a face; this used to
/// swallow the block).
#[test]
fn a_pull_stops_at_a_smaller_block_and_joins_it() {
    let mut m = Model::new();
    block(&mut m, 0.0, 0.0, 2.0, 4.0, 3.0); // big
    block(&mut m, 3.0, 1.0, 4.0, 2.0, 1.0); // small, beside it
    let side = prism_ops::face_at(&m, v(2.0, 3.0, 1.5)).expect("big's side");
    prism_ops::push_pull(&mut m, side, 3.0, false);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    closed(&m);
    // The side stopped at x = 3, flush with the block's near side.
    assert_eq!(normal_at(&m, v(3.0, 3.0, 1.5)), v(1.0, 0.0, 0.0));
    assert!(
        prism_ops::face_at(&m, v(5.0, 3.0, 1.5)).is_none(),
        "not past it"
    );
    // The two joined: no wall left between them, and the block's far side
    // is still there.
    assert!(
        prism_ops::face_at(&m, v(3.0, 1.5, 0.5)).is_none(),
        "no wall between"
    );
    assert_eq!(normal_at(&m, v(4.0, 1.5, 0.5)), v(1.0, 0.0, 0.0));
    // With Alt (new loop) it is not limited.
    let mut m = Model::new();
    block(&mut m, 0.0, 0.0, 2.0, 4.0, 3.0);
    block(&mut m, 3.0, 1.0, 4.0, 2.0, 1.0);
    let side = prism_ops::face_at(&m, v(2.0, 3.0, 1.5)).expect("big's side");
    prism_ops::push_pull(&mut m, side, 3.0, true);
    assert!(
        prism_ops::face_at(&m, v(5.0, 3.0, 1.5)).is_some(),
        "out to x = 5"
    );
}

/// A lone rectangle and an open box are never touched by the rule.
#[test]
fn faces_that_enclose_nothing_stay() {
    let mut m = Model::new();
    block(&mut m, 0.0, 0.0, 2.0, 2.0, 2.0);
    let top = prism_ops::face_at(&m, v(1.0, 1.0, 2.0)).expect("top");
    prism_ops::remove(
        &mut m,
        &prism_ops::Selection {
            faces: [top].into(),
            ..Default::default()
        },
    );
    let n = m.face_count();
    let side = prism_ops::face_at(&m, v(2.0, 1.0, 1.0)).expect("side");
    prism_ops::push_pull(&mut m, side, 1.0, false);
    assert_eq!(m.face_count(), n, "the open box keeps its faces");
}

/// A window pushed in past the back of a box cuts a tunnel: nothing sticks
/// out of the back, and the back has the opening (DECISIONS #31).
#[test]
fn pushing_a_window_out_through_the_back_cuts_a_tunnel() {
    let mut m = Model::new();
    block(&mut m, 0.0, 0.0, 4.0, 4.0, 2.0);
    poly(
        &mut m,
        &[
            v(1.0, 0.0, 0.5),
            v(3.0, 0.0, 0.5),
            v(3.0, 0.0, 1.5),
            v(1.0, 0.0, 1.5),
        ],
    );
    let win = prism_ops::face_at(&m, v(2.0, 0.0, 1.0)).expect("window");
    prism_ops::push_pull(&mut m, win, -6.0, false);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    closed(&m);
    let (_, hi) = m.bbox().expect("box");
    assert!((hi.y - 4.0).abs() < 1e-9, "nothing sticks out of the back");
    assert!(
        prism_ops::face_at(&m, v(2.0, 4.0, 1.0)).is_none(),
        "the back is open"
    );
    assert!(
        prism_ops::face_at(&m, v(2.0, 0.0, 1.0)).is_none(),
        "the front is open"
    );
    assert_eq!(
        normal_at(&m, v(1.0, 2.0, 1.0)),
        v(1.0, 0.0, 0.0),
        "the tunnel's wall faces in"
    );
}

/// A plate far thinner than it is wide (the renderer's thin-plate test:
/// 0.5 mm by 30 m) keeps all six faces: the solid rule's test points stay
/// inside the plate.
#[test]
fn a_very_thin_plate_keeps_its_faces() {
    for s in [1.5, 30.0, 300.0] {
        let mut m = Model::new();
        let t = 0.0005;
        prism_ops::add_rect(&mut m, -s / 2.0, 0.0, s / 2.0, t, 0.0);
        let f = prism_ops::face_at(&m, v(0.0, t / 2.0, 0.0)).expect("base");
        let up = m.face(f).unwrap().plane().unwrap().normal.z > 0.0;
        prism_ops::push_pull(&mut m, f, if up { s } else { -s }, false);
        // And a box behind it, pulled up as a solid of its own.
        prism_ops::add_rect(
            &mut m,
            -s * 0.2,
            t + 0.01,
            s * 0.2,
            t + 0.01 + s * 0.2,
            s * 0.3,
        );
        let g = prism_ops::face_at(&m, v(0.0, t + 0.01 + s * 0.1, s * 0.3)).expect("detail");
        let up = m.face(g).unwrap().plane().unwrap().normal.z > 0.0;
        prism_ops::push_pull(&mut m, g, if up { s * 0.4 } else { -s * 0.4 }, false);
        assert_eq!(m.face_count(), 12, "size {s}");
        closed(&m);
    }
}

/// Found while building Exact Curves 4b: a lone face pushed back (against
/// its normal) left only one face, because its walls faced in and the
/// inside test took them. It makes a box the other way, facing out.
#[test]
fn a_lone_face_pushed_back_makes_a_box() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 1.0, 1.0, 0.0);
    let f = m.faces().next().map(|(id, _)| id).expect("a face");
    prism_ops::push_pull(&mut m, f, -1.0, false);
    assert_eq!(m.face_count(), 6);
    let h = prism_ops::health::check(&m);
    assert_eq!((h.watertight, h.leaky), (1, 0));
    assert!(h.reversed.is_empty(), "{h:?}");
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
}
