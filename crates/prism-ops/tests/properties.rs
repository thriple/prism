//! Property tests: random rectangles and lines on a grid, checked against
//! the rules in `docs/acceptance/core-model-acceptance.md`.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::{check, transform};
use prism_topo::Model;
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Op {
    Rect(i32, i32, i32, i32),
    Line(i32, i32, i32, i32),
    /// A circle about a grid point, with a radius of 1 to 3, crossing the
    /// rectangles, lines and other circles.
    Circle(i32, i32, i32),
}

fn rect() -> impl Strategy<Value = Op> {
    (0..8i32, 0..8i32, 1..5i32, 1..5i32).prop_map(|(x, y, w, h)| Op::Rect(x, y, x + w, y + h))
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => rect(),
        1 => (0..10i32, 0..10i32, 1..4i32).prop_map(|(i, j, r)| Op::Circle(i, j, r)),
        1 => (0..10i32, 0..10i32, 0..10i32, 0..10i32).prop_filter("a line has length", |(a, b, c, d)| (a, b) != (c, d)).prop_map(|(a, b, c, d)| Op::Line(a, b, c, d)),
    ]
}

fn apply(m: &mut Model, op: &Op, k: f64) {
    match *op {
        Op::Rect(x0, y0, x1, y1) => {
            prism_ops::add_rect(
                m,
                f64::from(x0) * k,
                f64::from(y0) * k,
                f64::from(x1) * k,
                f64::from(y1) * k,
                0.0,
            );
        }
        Op::Line(a, b, c, d) => {
            let p = DVec3::new(f64::from(a), f64::from(b), 0.0) * k;
            let q = DVec3::new(f64::from(c), f64::from(d), 0.0) * k;
            prism_ops::add_segment(m, p, q);
        }
        Op::Circle(i, j, r) => {
            let c = DVec3::new(f64::from(i), f64::from(j), 0.0) * k;
            prism_ops::add_circle(m, c, DVec3::Z, f64::from(r) * k);
        }
    }
}

fn build(ops: &[Op], k: f64) -> Model {
    let mut m = Model::new();
    for op in ops {
        apply(&mut m, op, k);
        // After every operation, loops name the right edges.
        let p = m.bookkeeping_problems();
        assert!(p.is_empty(), "{p:?} after {op:?}");
    }
    m
}

/// Face areas, sorted, divided by `k²` so scales compare.
fn areas(m: &Model, k: f64) -> Vec<f64> {
    let mut v: Vec<f64> = m.faces().map(|(id, _)| m.face_area(id) / (k * k)).collect();
    v.sort_by(f64::total_cmp);
    v
}

/// The same areas, within rounding (not rounded to a grid, which flips a
/// value lying exactly on a half step, such as 0.2265625).
fn same_areas(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (x - y).abs() <= 1e-9 * x.abs().max(1.0))
}

/// The area covered by a set of axis-aligned rectangles on the integer grid.
fn union_area(rects: &[(i32, i32, i32, i32)]) -> f64 {
    let mut cells = BTreeSet::new();
    for &(x0, y0, x1, y1) in rects {
        for x in x0..x1 {
            for y in y0..y1 {
                cells.insert((x, y));
            }
        }
    }
    cells.len() as f64
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// Rules 1 to 4: no overlaps, no duplicates, closed bookkeeping, holes inside.
    #[test]
    fn every_model_follows_the_rules(ops in prop::collection::vec(op(), 1..12)) {
        let m = build(&ops, 1.0);
        let p = check::problems(&m);
        prop_assert!(p.is_empty(), "{:?}\nfor {:?}", p, ops);
    }

    /// Rule 5: overlapping never changes the covered area. Every part of every
    /// rectangle stays covered by exactly one face, and the faces cover at
    /// least the rectangles (more only where rectangles enclose an empty
    /// region, which then becomes a face as any closed loop does).
    #[test]
    fn area_is_conserved(rects in prop::collection::vec(rect(), 1..6)) {
        let m = build(&rects, 1.0);
        let r: Vec<_> = rects.iter().map(|o| match o { Op::Rect(a, b, c, d) => (*a, *b, *c, *d), Op::Line(..) | Op::Circle(..) => unreachable!() }).collect();
        for &(x0, y0, x1, y1) in &r {
            for x in x0..x1 {
                for y in y0..y1 {
                    let c = DVec3::new(f64::from(x) + 0.5, f64::from(y) + 0.5, 0.0);
                    prop_assert!(prism_ops::face_at(&m, c).is_some(), "{:?} is not covered", c);
                }
            }
        }
        let total: f64 = m.faces().map(|(id, _)| m.face_area(id)).sum();
        prop_assert!(total + 1e-6 >= union_area(&r), "faces cover {} but the rectangles cover {}", total, union_area(&r));
    }

    /// Rectangles that do not enclose anything cover exactly their union.
    #[test]
    fn overlapping_rectangles_cover_exactly_their_union(a in rect(), b in rect()) {
        let m = build(&[a.clone(), b.clone()], 1.0);
        let r: Vec<_> = [a, b].iter().map(|o| match o { Op::Rect(a, b, c, d) => (*a, *b, *c, *d), Op::Line(..) | Op::Circle(..) => unreachable!() }).collect();
        let total: f64 = m.faces().map(|(id, _)| m.face_area(id)).sum();
        prop_assert!((total - union_area(&r)).abs() < 1e-6, "faces cover {} but the rectangles cover {}", total, union_area(&r));
    }

    /// Rule 6: adding the last rectangle or line again changes nothing.
    #[test]
    fn adding_again_changes_nothing(ops in prop::collection::vec(op(), 1..6)) {
        let mut m = build(&ops, 1.0);
        let before = m.to_json();
        apply(&mut m, ops.last().expect("one op"), 1.0);
        prop_assert_eq!(before, m.to_json());
    }

    /// Rule 7: the same rectangles in another order give the same faces.
    #[test]
    fn order_does_not_matter(rects in prop::collection::vec(rect(), 1..5)) {
        let mut back = rects.clone();
        back.reverse();
        let (a, b) = (areas(&build(&rects, 1.0), 1.0), areas(&build(&back, 1.0), 1.0));
        prop_assert!(same_areas(&a, &b), "{:?} vs {:?}", a, b);
    }

    /// Rule 8: scaling every coordinate gives the same topology.
    #[test]
    fn scale_does_not_matter(ops in prop::collection::vec(op(), 1..6)) {
        let base = build(&ops, 1.0);
        for k in [1e-4, 1e3] {
            let m = build(&ops, k);
            prop_assert_eq!((m.vertex_count(), m.edge_count(), m.face_count()), (base.vertex_count(), base.edge_count(), base.face_count()));
            let (a, b) = (areas(&m, k), areas(&base, 1.0));
            prop_assert!(same_areas(&a, &b), "at {}: {:?} vs {:?}", k, a, b);
        }
    }

    /// Rule 10: the same operations give the same ids and the same saved form.
    #[test]
    fn deterministic(ops in prop::collection::vec(op(), 1..6)) {
        prop_assert_eq!(build(&ops, 1.0).to_json(), build(&ops, 1.0).to_json());
    }

    /// Rule 11: after rotating and scaling, faces stay planar and whole.
    #[test]
    fn faces_stay_planar_after_rotate_and_scale(ops in prop::collection::vec(rect(), 1..5), angle in 0.0..6.2f64) {
        let mut m = build(&ops, 1.0);
        let faces = m.face_count();
        let vs: BTreeSet<_> = m.vertices().map(|(id, _)| id).collect();
        transform::rotate(&mut m, &vs, DVec3::new(3.0, 2.0, 0.0), DVec3::new(1.0, 2.0, 5.0), angle);
        transform::scale(&mut m, &vs, DVec3::ZERO, DVec3::new(2.0, 0.5, 1.5));
        prop_assert_eq!(m.face_count(), faces, "no face was split");
        let p = check::planar(&m, &m.tol());
        prop_assert!(p.is_empty(), "{:?}", p);
    }
}

/// Found by CI on macOS: an area of exactly 0.2265625 used to round
/// differently at different scales.
#[test]
fn scale_does_not_matter_regression_half_step_area() {
    let ops = [
        Op::Line(6, 0, 8, 9),
        Op::Rect(3, 3, 7, 5),
        Op::Line(1, 9, 7, 4),
    ];
    let base = build(&ops, 1.0);
    for k in [1e-4, 1e3] {
        let m = build(&ops, k);
        assert_eq!(m.face_count(), base.face_count());
        assert!(same_areas(&areas(&m, k), &areas(&base, 1.0)), "at {k}");
    }
}

/// Found when circles began to cross things (Exact Curves 2b): two circles
/// touching inside at a seam made a thin face whose sample point fell where
/// the drawn arcs touch.
#[test]
fn circles_touching_inside_regression() {
    let ops = [
        Op::Line(0, 0, 7, 7),
        Op::Circle(6, 5, 2),
        Op::Circle(6, 4, 3),
    ];
    let p = check::problems(&build(&ops, 1.0));
    assert!(p.is_empty(), "{p:?}");
}

/// Found with the one above: the topology differed between scales.
#[test]
fn circles_and_lines_at_every_scale_regression() {
    let ops = [
        Op::Circle(4, 2, 2),
        Op::Line(1, 2, 6, 7),
        Op::Circle(6, 5, 2),
        Op::Rect(4, 2, 5, 5),
    ];
    let base = build(&ops, 1.0);
    assert!(check::problems(&base).is_empty());
    for k in [1e-4, 1e3] {
        let m = build(&ops, k);
        assert_eq!(
            (m.vertex_count(), m.edge_count(), m.face_count()),
            (base.vertex_count(), base.edge_count(), base.face_count()),
            "at {k}"
        );
    }
}
