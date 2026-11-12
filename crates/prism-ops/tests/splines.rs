//! NURBS curves in the model (Freeform NURBS part 1): faces, crossings,
//! splits and transforms.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_geom::Nurbs;
use prism_math::{DAffine3, DVec3};
use prism_ops::{add_circle, add_segment, add_spline, check, transform};
use prism_topo::{Model, VertexId};
use proptest::prelude::*;

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn all(m: &Model) -> BTreeSet<VertexId> {
    m.vertices().map(|(id, _)| id).collect()
}

fn wave() -> Nurbs {
    Nurbs::open(
        &[
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 2.0, 0.0),
            DVec3::new(3.0, -2.0, 0.0),
            DVec3::new(4.0, 0.0, 0.0),
        ],
        3,
    )
    .unwrap()
}

fn blob() -> Nurbs {
    Nurbs::closed(
        &[
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(4.0, 3.0, 0.0),
            DVec3::new(2.0, 5.0, 0.0),
            DVec3::new(0.0, 3.0, 0.0),
        ],
        3,
    )
    .unwrap()
}

fn splines(m: &Model) -> usize {
    m.edges().filter(|(_, e)| e.curve.is_spline()).count()
}

#[test]
fn an_open_curve_is_one_edge() {
    let mut m = Model::new();
    let added = add_spline(&mut m, &wave());
    assert_eq!(added.edges.len(), 1);
    assert_eq!(m.face_count(), 0);
    clean(&m);
    // Drawn again, nothing is added.
    assert!(add_spline(&mut m, &wave()).edges.is_empty());
}

#[test]
fn a_closed_flat_curve_makes_a_face() {
    let mut m = Model::new();
    let added = add_spline(&mut m, &blob());
    assert_eq!(added.edges.len(), 2, "two halves, as a circle");
    assert_eq!(m.face_count(), 1);
    clean(&m);
    // Its area is that of the drawn curve, inside the control polygon's.
    let area = m.face_area(m.faces().next().unwrap().0);
    assert!(area > 8.0 && area < 16.0, "{area}");
}

#[test]
fn a_line_across_a_curve_splits_both() {
    let mut m = Model::new();
    add_spline(&mut m, &wave());
    // The wave crosses the x axis at its middle as well as its ends.
    let added = add_segment(
        &mut m,
        DVec3::new(2.0, -1.0, 0.0),
        DVec3::new(2.0, 1.0, 0.0),
    );
    assert_eq!(splines(&m), 2);
    assert_eq!(added.edges.len(), 2);
    clean(&m);
    // A line joining its ends closes two faces with it.
    add_segment(&mut m, DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0));
    assert_eq!(m.face_count(), 2);
    clean(&m);
}

#[test]
fn a_curve_across_lines_and_a_circle_cuts_them() {
    let mut m = Model::new();
    add_segment(
        &mut m,
        DVec3::new(2.0, -1.0, 0.0),
        DVec3::new(2.0, 1.0, 0.0),
    );
    add_circle(&mut m, DVec3::new(2.0, 0.0, 0.0), DVec3::Z, 0.5);
    let before = m.edge_count();
    add_spline(&mut m, &wave());
    // The wave passes through the circle's centre: it is cut at the line
    // and twice at the circle.
    assert_eq!(splines(&m), 4);
    assert!(m.edge_count() > before + 4);
    clean(&m);
}

#[test]
fn a_curve_moves_turns_and_scales_unevenly_and_stays_a_curve() {
    let mut m = Model::new();
    add_spline(&mut m, &blob());
    let area = m.face_area(m.faces().next().unwrap().0);
    let vs = all(&m);
    transform::translate(&mut m, &vs, DVec3::new(1.0, 2.0, 3.0));
    transform::rotate(&mut m, &vs, DVec3::ZERO, DVec3::X, 0.7);
    clean(&m);
    assert_eq!(splines(&m), 2);
    transform::scale(&mut m, &vs, DVec3::ZERO, DVec3::new(2.0, 1.0, 1.0));
    clean(&m);
    assert_eq!(splines(&m), 2, "an uneven scale keeps a NURBS curve exact");
    let after = m.face_area(m.faces().next().unwrap().0);
    assert!((after - 2.0 * area).abs() < 2e-3 * area, "{after} {area}");
}

#[test]
fn one_end_of_a_curve_cannot_move_alone() {
    let mut m = Model::new();
    add_spline(&mut m, &wave());
    let one: BTreeSet<VertexId> = m.vertices().take(1).map(|(id, _)| id).collect();
    let d = DAffine3::from_translation(DVec3::Z);
    assert_eq!(
        transform::arcs_refusal(&m, &one, &d),
        Some(transform::SPLINE_WOULD_BEND)
    );
}

#[test]
fn the_model_survives_json() {
    let mut m = Model::new();
    add_spline(&mut m, &blob());
    add_segment(
        &mut m,
        DVec3::new(-1.0, 2.0, 0.0),
        DVec3::new(5.0, 2.0, 0.0),
    );
    clean(&m);
    let back = Model::from_json(&m.to_json()).unwrap();
    assert_eq!(back, m);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn random_curves_and_lines_keep_the_model_clean(
        pts in prop::collection::vec((0.0f64..6.0, 0.0f64..6.0), 4..7),
        lines in prop::collection::vec(((0.0f64..6.0, 0.0f64..6.0), (0.0f64..6.0, 0.0f64..6.0)), 0..4),
        closed in any::<bool>(),
    ) {
        let pts: Vec<DVec3> = pts.iter().map(|(x, y)| DVec3::new(*x, *y, 0.0)).collect();
        let curve = if closed { Nurbs::closed(&pts, 3) } else { Nurbs::open(&pts, 3) };
        let Some(curve) = curve else { return Ok(()) };
        // A curve that crosses itself is not cut there yet: leave those out.
        let poly: Vec<DVec3> = (0..=64).map(|i| curve.eval(i as f64 / 64.0)).collect();
        let crosses = (0..poly.len() - 1).any(|i| (i + 2..poly.len() - 1).any(|j| {
            if closed && i == 0 && j == poly.len() - 2 { return false; }
            let (a, b, c, d) = (poly[i], poly[i + 1], poly[j], poly[j + 1]);
            let o = |p: DVec3, q: DVec3, r: DVec3| (q - p).cross(r - p).z;
            o(a, b, c) * o(a, b, d) <= 0.0 && o(c, d, a) * o(c, d, b) <= 0.0
        }));
        prop_assume!(!crosses);
        let mut m = Model::new();
        add_spline(&mut m, &curve);
        for ((x0, y0), (x1, y1)) in lines {
            add_segment(&mut m, DVec3::new(x0, y0, 0.0), DVec3::new(x1, y1, 0.0));
        }
        let p = check::problems(&m);
        prop_assert!(p.is_empty(), "{p:?}");
    }
}
