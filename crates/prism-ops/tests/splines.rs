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
    #![proptest_config(ProptestConfig::with_cases(96))]
    #[test]
    fn random_curves_and_lines_keep_the_model_clean(
        pts in prop::collection::vec((0.0f64..6.0, 0.0f64..6.0), 4..7),
        lines in prop::collection::vec(((0.0f64..6.0, 0.0f64..6.0), (0.0f64..6.0, 0.0f64..6.0)), 0..4),
        closed in any::<bool>(),
    ) {
        let pts: Vec<DVec3> = pts.iter().map(|(x, y)| DVec3::new(*x, *y, 0.0)).collect();
        let curve = if closed { Nurbs::closed(&pts, 3) } else { Nurbs::open(&pts, 3) };
        let Some(curve) = curve else { return Ok(()) };
        let mut m = Model::new();
        add_spline(&mut m, &curve);
        for ((x0, y0), (x1, y1)) in lines {
            add_segment(&mut m, DVec3::new(x0, y0, 0.0), DVec3::new(x1, y1, 0.0));
        }
        let p = check::problems(&m);
        prop_assert!(p.is_empty(), "{p:?}");
    }
}

// ---------- Freeform NURBS part 2: freeform faces ----------

#[test]
fn pulling_a_face_with_a_curved_edge_makes_freeform_walls() {
    let mut m = Model::new();
    add_spline(&mut m, &blob());
    let face = m.faces().next().unwrap().0;
    let area = m.face_area(face);
    assert!(prism_ops::pushpull::refusal(&m, face, 2.0).is_none());
    prism_ops::push_pull(&mut m, face, 2.0, false);
    clean(&m);
    let patches: Vec<_> = m
        .faces()
        .filter(|(_, f)| f.surface.patch().is_some())
        .collect();
    assert_eq!(patches.len(), 2, "one wall along each half of the curve");
    assert_eq!(m.face_count(), 4);
    // Both walls lie on one surface, and face out of the solid.
    let (a, b) = (
        patches[0].1.surface.patch().unwrap(),
        patches[1].1.surface.patch().unwrap(),
    );
    assert!(std::sync::Arc::ptr_eq(&a.surface, &b.surface));
    let centre = DVec3::new(2.0, 2.0, 1.0);
    for (_, f) in &patches {
        let p = m.vertex(f.outer[0]);
        let n = f.surface.normal_at(p);
        assert!(n.dot(p - centre) > 0.0, "{n} at {p}");
        assert!(n.z.abs() < 1e-9);
    }
    // The lines between the two halves are soft: the wall is smooth there.
    assert_eq!(m.edges().filter(|(_, e)| e.soft).count(), 2);
    // A closed solid of the face's area times the height.
    let v: f64 = m
        .faces()
        .map(|(id, _)| prism_ops::health::face_volume(&m, id))
        .sum();
    assert!((v - area * 2.0).abs() < 2e-3 * v, "{v} {area}");
    // Moved, turned and scaled unevenly, it stays exact and clean.
    let vs = all(&m);
    transform::rotate(&mut m, &vs, DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0), 0.6);
    transform::scale(&mut m, &vs, DVec3::ZERO, DVec3::new(1.0, 2.0, 0.5));
    clean(&m);
    assert_eq!(
        m.faces()
            .filter(|(_, f)| f.surface.patch().is_some())
            .count(),
        2
    );
    let back = Model::from_json(&m.to_json()).unwrap();
    assert_eq!(back, m);
}

#[test]
fn a_freeform_face_is_not_pushed_or_pulled() {
    let mut m = Model::new();
    add_spline(&mut m, &blob());
    let face = m.faces().next().unwrap().0;
    prism_ops::push_pull(&mut m, face, 2.0, false);
    let wall = m
        .faces()
        .find(|(_, f)| f.surface.patch().is_some())
        .unwrap()
        .0;
    assert_eq!(
        prism_ops::pushpull::refusal(&m, wall, 0.5),
        Some(prism_ops::radius::FREEFORM)
    );
}

/// An open curve that loops over itself, like a written "e".
fn loop_curve() -> Nurbs {
    Nurbs::open(
        &[
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(4.0, 3.0, 0.0),
            DVec3::new(2.0, 3.0, 0.0),
            DVec3::new(2.0, -1.0, 0.0),
            DVec3::new(5.0, -1.0, 0.0),
        ],
        3,
    )
    .unwrap()
}

#[test]
fn a_curve_that_crosses_itself_is_cut_there_and_its_loop_is_a_face() {
    let mut m = Model::new();
    let added = add_spline(&mut m, &loop_curve());
    assert_eq!(
        added.edges.len(),
        4,
        "lead in, the loop in two halves, lead out"
    );
    assert_eq!(m.face_count(), 1, "the loop closes a face");
    clean(&m);
    // One vertex where it crosses, with four edge ends.
    let crossing = m
        .vertices()
        .find(|(v, _)| m.edges().filter(|(_, e)| e.has(*v)).count() == 4)
        .map(|(_, p)| p);
    assert!(crossing.is_some(), "the crossing joins four edges");
}

#[test]
fn a_figure_eight_is_two_faces() {
    let mut m = Model::new();
    let eight = Nurbs::closed(
        &[
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(2.0, 2.0, 0.0),
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(2.0, -2.0, 0.0),
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(-2.0, 2.0, 0.0),
            DVec3::new(-4.0, 0.0, 0.0),
            DVec3::new(-2.0, -2.0, 0.0),
        ],
        3,
    )
    .unwrap();
    add_spline(&mut m, &eight);
    assert_eq!(m.face_count(), 2);
    clean(&m);
    let areas: Vec<f64> = m.faces().map(|(f, _)| m.face_area(f)).collect();
    assert!((areas[0] - areas[1]).abs() < 1e-3 * areas[0], "{areas:?}");
}

#[test]
fn a_curve_that_doubles_back_is_not_cut_at_its_tip() {
    // Found by the property test: a sharp turn back gave false crossings.
    let pts: Vec<DVec3> = [
        (5.641681983196217, 4.258456509234495),
        (5.267743374674798, 1.8535312219342872),
        (5.090840079789922, 3.8417318288890674),
        (5.424635215974237, 0.3492715404018569),
        (0.0, 0.0),
    ]
    .iter()
    .map(|(x, y)| DVec3::new(*x, *y, 0.0))
    .collect();
    let mut m = Model::new();
    add_spline(&mut m, &Nurbs::closed(&pts, 3).unwrap());
    clean(&m);
}

#[test]
fn a_sharp_turn_back_on_a_closed_curve_is_not_a_crossing() {
    // Found by the property test: the strands at the turn run almost on
    // top of each other.
    let pts: Vec<DVec3> = [
        (3.392148257792721, 5.539123292483001),
        (3.932910641202735, 3.099152644216482),
        (0.4217332197798381, 5.53300821673034),
        (4.3164393451727525, 0.10829164701344554),
    ]
    .iter()
    .map(|(x, y)| DVec3::new(*x, *y, 0.0))
    .collect();
    let mut m = Model::new();
    add_spline(&mut m, &Nurbs::closed(&pts, 3).unwrap());
    clean(&m);
}

#[test]
fn a_line_across_a_hook_meets_it_every_time() {
    // Found by the property test: the curve comes back close to itself,
    // and the line's third crossing was put on the wrong strand and lost,
    // leaving two faces that overlapped.
    let pts: Vec<DVec3> = [
        (0.0, 0.0),
        (0.7450582569539217, 1.823509793294491),
        (4.37404391589213, 3.1550472947039374),
        (0.5433801233376531, 2.7731089001236833),
        (5.826578634349477, 2.9409573947427026),
    ]
    .iter()
    .map(|(x, y)| DVec3::new(*x, *y, 0.0))
    .collect();
    let mut m = Model::new();
    add_spline(&mut m, &Nurbs::open(&pts, 3).unwrap());
    let added = add_segment(
        &mut m,
        DVec3::new(3.794457242205567, 0.0, 0.0),
        DVec3::new(1.0784649099617725, 5.474750455019208, 0.0),
    );
    assert_eq!(added.edges.len(), 4, "three crossings cut the line in four");
    add_segment(
        &mut m,
        DVec3::new(0.0, 4.392495139090876, 0.0),
        DVec3::new(4.344417584688826, 1.9257203142311268, 0.0),
    );
    clean(&m);
}

#[test]
fn a_shallow_crossing_near_a_tip_is_still_cut() {
    // Found by the property test: a thin leaf whose strands cross at about
    // one degree near its tip. Left uncut, a line across the tip made two
    // faces that overlapped.
    let pts: Vec<DVec3> = [
        (3.10773848195406, 1.6282056731001124),
        (1.335075253210589, 4.765843326131168),
        (0.0, 4.614389251866482),
        (1.7030284650494323, 4.148455401783976),
    ]
    .iter()
    .map(|(x, y)| DVec3::new(*x, *y, 0.0))
    .collect();
    let mut m = Model::new();
    add_spline(&mut m, &Nurbs::closed(&pts, 3).unwrap());
    assert_eq!(m.face_count(), 2, "the leaf and the small loop at its tip");
    add_segment(
        &mut m,
        DVec3::new(0.0, 2.8738185762044717, 0.0),
        DVec3::new(5.72154296034267, 2.28137092736813, 0.0),
    );
    clean(&m);
}
