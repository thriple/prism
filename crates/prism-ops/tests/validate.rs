//! Solid > Validate and Heal (Solids & CAD part 1, brief 37): each piece
//! is a valid solid with its exact volume, or says why not; Heal fixes
//! holes, reversed faces, near misses and inside walls, and changes
//! nothing in a model that needs nothing.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_math::DVec3;
use prism_ops::validate::{SolidCheck, heal, validate};
use prism_ops::{add_arc, add_circle, add_rect, add_segment, check, face_at, follow, push_pull};
use prism_topo::Model;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A 2 by 3 by 4 box on the ground.
fn boxed() -> Model {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 2.0, 3.0, 0.0);
    let f = face_at(&m, v(1.0, 1.0, 0.0)).unwrap();
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(&mut m, f, 4.0 * up, false);
    m
}

fn one(m: &Model) -> SolidCheck {
    let all = validate(m);
    assert_eq!(all.len(), 1, "{all:?}");
    all.into_iter().next().unwrap()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

fn valid(c: &SolidCheck, volume: f64) {
    assert!(c.valid, "{c:?}");
    close(c.volume.expect("closed"), volume);
}

#[test]
fn a_box_is_a_valid_solid_with_its_volume() {
    let c = one(&boxed());
    valid(&c, 24.0);
    assert_eq!(c.kernel_skipped, None, "the kernel checked it");
}

#[test]
fn a_box_with_a_face_erased_has_a_hole_that_heal_fills() {
    let mut m = boxed();
    let top = face_at(&m, v(1.0, 1.0, 4.0)).unwrap();
    m.remove_face(top);
    let c = one(&m);
    assert!(!c.valid);
    assert_eq!(c.volume, None);
    assert_eq!(c.reasons, vec!["4 open edges (holes)".to_owned()]);
    let h = heal(&mut m, None);
    assert_eq!(h.filled, 1, "{h:?}");
    assert_eq!(h.words(), "Filled 1 hole");
    valid(&one(&m), 24.0);
    assert_eq!(check::problems(&m), Vec::<String>::new());
}

#[test]
fn heal_turns_a_reversed_face_and_welds_a_near_miss() {
    let mut m = boxed();
    let top = face_at(&m, v(1.0, 1.0, 4.0)).unwrap();
    prism_ops::health::reverse(&mut m, &[top].into());
    let c = one(&m);
    assert_eq!(c.reasons, vec!["1 face pointing inward".to_owned()]);
    assert_eq!(heal(&mut m, None).turned, 1);
    valid(&one(&m), 24.0);

    // A side's corner pulled away from the box by five tolerances: the
    // side no longer shares it.
    let mut m = boxed();
    let tol = m.tol().dist;
    let corner = m
        .vertices()
        .find(|(_, p)| p.distance(v(2.0, 3.0, 4.0)) < 1e-9)
        .map(|(id, _)| id)
        .unwrap();
    let side = face_at(&m, v(2.0, 1.5, 2.0)).unwrap();
    let extra = m.add_vertex(v(2.0, 3.0, 4.0 + 5.0 * tol));
    let mut outer = m.face(side).unwrap().outer.clone();
    for x in &mut outer {
        if *x == corner {
            *x = extra;
        }
    }
    let plane = *m.face(side).unwrap().plane().unwrap();
    m.remove_face(side);
    for w in 0..outer.len() {
        m.add_edge(outer[w], outer[(w + 1) % outer.len()]);
    }
    m.add_face(prism_topo::Face::new(
        outer,
        vec![],
        prism_geom::Surface::Plane(plane),
    ));
    let c = one(&m);
    assert!(!c.valid);
    assert!(
        c.reasons.iter().any(|r| r.contains("almost touching")),
        "{c:?}"
    );
    let h = heal(&mut m, None);
    assert_eq!(h.welded, 1, "{h:?}");
    valid(&one(&m), 24.0);
}

#[test]
fn heal_removes_a_wall_left_inside() {
    let mut m = boxed();
    // A wall across the middle, at y = 1.5.
    add_segment(&mut m, v(0.0, 1.5, 0.0), v(2.0, 1.5, 0.0));
    add_segment(&mut m, v(2.0, 1.5, 0.0), v(2.0, 1.5, 4.0));
    add_segment(&mut m, v(2.0, 1.5, 4.0), v(0.0, 1.5, 4.0));
    add_segment(&mut m, v(0.0, 1.5, 4.0), v(0.0, 1.5, 0.0));
    let c = one(&m);
    assert!(!c.valid, "{c:?}");
    let h = heal(&mut m, None);
    assert!(h.removed >= 1, "{h:?}");
    valid(&one(&m), 24.0);
}

#[test]
fn a_valid_solid_needs_no_healing() {
    let mut m = boxed();
    let before = m.clone();
    let h = heal(&mut m, None);
    assert!(h.is_empty(), "{h:?}");
    assert_eq!(h.words(), "Nothing needed healing");
    assert_eq!(m, before);
}

#[test]
fn a_cylinder_is_valid_through_the_kernel_and_a_ball_by_prisms_rules() {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let f = face_at(&m, v(0.0, 0.0, 0.0)).unwrap();
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(&mut m, f, 2.0 * up, false);
    let c = one(&m);
    valid(&c, PI * 2.0);
    assert_eq!(c.kernel_skipped, None);

    // A ball: a half disc turned round the z axis (and the path's disc
    // left beside it, open).
    let mut m = Model::new();
    add_segment(&mut m, v(0.0, 0.0, -1.0), v(0.0, 0.0, 1.0));
    add_arc(&mut m, DVec3::ZERO, -DVec3::Y, 1.0, v(0.0, 0.0, -1.0), PI);
    let prof = face_at(&m, v(0.5, 0.0, 0.0)).unwrap();
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    add_circle(&mut m, v(0.0, 0.0, 10.0), DVec3::Z, 0.5);
    let path: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert!(follow::follow(&mut m, &path, prof));
    let all = validate(&m);
    let ball = all
        .iter()
        .find(|c| c.valid)
        .unwrap_or_else(|| panic!("{all:?}"));
    // Its poles cannot go to the kernel yet (part 3).
    assert_eq!(
        ball.kernel_skipped,
        Some(prism_ops::validate::KERNEL_NOT_YET)
    );
    close(ball.volume.unwrap(), 4.0 / 3.0 * PI);
    // Its area is exact too (it was 0, so Model Health called every
    // turned face "of no size").
    close(ball.faces.iter().map(|f| m.face_area(*f)).sum(), 4.0 * PI);
    assert_eq!(all.len(), 2);
}
