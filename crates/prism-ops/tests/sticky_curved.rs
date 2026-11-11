//! Sticky geometry on curved faces (Exact Curves 6b-2): what is drawn on a
//! curved face splits it, crossings that meet in a curved face's middle are
//! split where they meet, and Follow Me's result sticks to what it meets.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::Surface;
use prism_math::DVec3;
use prism_ops::follow;
use prism_ops::intersect::intersect_faces;
use prism_ops::pushpull::{Pulled, push_pull};
use prism_ops::{add_circle, add_rect, add_segment, check, health::face_volume};
use prism_topo::{FaceId, Model};

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn sides(m: &Model) -> Vec<FaceId> {
    m.faces()
        .filter(|(_, f)| matches!(f.surface, Surface::Cylinder(_)))
        .map(|(id, _)| id)
        .collect()
}

fn curved_volume(m: &Model) -> f64 {
    m.faces()
        .filter(|(_, f)| f.is_curved())
        .map(|(id, _)| face_volume(m, id))
        .sum()
}

/// A cylinder of radius 1 from z = 0 to 2; its seams are at x = 0.
fn cylinder() -> Model {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let disc = m.faces().next().map(|(id, _)| id).expect("a disc");
    assert!(!matches!(
        push_pull(&mut m, disc, 2.0, false),
        Pulled::Nothing
    ));
    m
}

fn at(deg: f64, z: f64) -> DVec3 {
    let t = deg.to_radians();
    DVec3::new(t.cos(), t.sin(), z)
}

#[test]
fn a_line_drawn_along_a_cylinder_splits_it() {
    let mut m = cylinder();
    let before = curved_volume(&m);
    let r = add_segment(&mut m, at(60.0, 0.0), at(60.0, 2.0));
    assert_eq!(r.edges.len(), 1);
    assert_eq!(sides(&m).len(), 3, "the half it lies on, in two");
    clean(&m);
    assert!((curved_volume(&m) - before).abs() < 1e-9);
    // A second line on the other half, then one across a piece's middle
    // is not on the surface: nothing more is split by that.
    add_segment(&mut m, at(-60.0, 0.0), at(-60.0, 2.0));
    assert_eq!(sides(&m).len(), 4);
    add_segment(&mut m, at(10.0, 0.5), at(50.0, 1.5));
    assert_eq!(sides(&m).len(), 4);
    clean(&m);
}

#[test]
fn a_circle_drawn_round_a_cylinder_splits_both_halves() {
    let mut m = cylinder();
    let before = curved_volume(&m);
    add_circle(&mut m, DVec3::new(0.0, 0.0, 0.8), DVec3::Z, 1.0);
    assert_eq!(sides(&m).len(), 4);
    clean(&m);
    assert!((curved_volume(&m) - before).abs() < 1e-9);
    // A line along the side now crosses it: it is split there, and both
    // pieces it runs over are split.
    add_segment(&mut m, at(30.0, 0.0), at(30.0, 2.0));
    assert_eq!(sides(&m).len(), 6);
    clean(&m);
}

#[test]
fn crossings_that_meet_in_a_curved_face_are_split_where_they_meet() {
    let mut m = cylinder();
    let before = curved_volume(&m);
    // A square across at z = 1, and an upright one through the axis at
    // 45°: on the cylinder, the circle and each line cross mid-face.
    add_rect(&mut m, -2.0, -2.0, 2.0, 2.0, 1.0);
    let d = DVec3::new(1.0, 1.0, 0.0).normalize() * 2.0;
    for (p, q) in [
        (-d - DVec3::Z, d - DVec3::Z),
        (d - DVec3::Z, d + DVec3::Z * 3.0),
        (d + DVec3::Z * 3.0, -d + DVec3::Z * 3.0),
        (-d + DVec3::Z * 3.0, -d - DVec3::Z),
    ] {
        add_segment(&mut m, p, q);
    }
    intersect_faces(&mut m, None);
    clean(&m);
    // Each half: the circle across it and a line down it make four.
    assert_eq!(sides(&m).len(), 8);
    assert!((curved_volume(&m) - before).abs() < 1e-9);
    // The meeting points are exact: on the circle and on the line.
    for deg in [45.0, 225.0] {
        let p = at(deg, 1.0);
        assert!(
            m.vertices().any(|(_, v)| v.distance(p) < 1e-12),
            "a vertex at {p}"
        );
    }
}

#[test]
fn follow_me_sticks_to_a_wall_it_ends_on() {
    let mut m = Model::new();
    // A wall in the plane x = 2.
    for (p, q) in [
        (DVec3::new(2.0, -2.0, -1.0), DVec3::new(2.0, 2.0, -1.0)),
        (DVec3::new(2.0, 2.0, -1.0), DVec3::new(2.0, 2.0, 2.0)),
        (DVec3::new(2.0, 2.0, 2.0), DVec3::new(2.0, -2.0, 2.0)),
        (DVec3::new(2.0, -2.0, 2.0), DVec3::new(2.0, -2.0, -1.0)),
    ] {
        add_segment(&mut m, p, q);
    }
    // A disc standing in the plane x = 0, swept along x to the wall.
    add_circle(&mut m, DVec3::new(0.0, 0.0, 0.5), DVec3::X, 0.5);
    let prof = m
        .faces()
        .find(|(_, f)| f.plane().is_some_and(|p| p.origin.x.abs() < 1e-9))
        .map(|(id, _)| id)
        .expect("the disc");
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    add_segment(&mut m, DVec3::new(0.0, 0.0, 0.5), DVec3::new(2.0, 0.0, 0.5));
    let path: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert!(follow::follow(&mut m, &path, prof));
    clean(&m);
    // The wall now has the rod's end as a hole: its circle is shared.
    let wall = m
        .faces()
        .filter(|(_, f)| f.plane().is_some_and(|p| (p.origin.x - 2.0).abs() < 1e-9))
        .map(|(id, f)| (id, f.holes.len()))
        .collect::<Vec<_>>();
    assert!(wall.iter().any(|(_, h)| *h == 1), "{wall:?}");
    assert_eq!(sides(&m).len(), 2);
    let v: f64 = sides(&m).iter().map(|f| face_volume(&m, *f)).sum();
    // The rod's sides' share of its volume: area × r / 3 = 2πr·L·r/3.
    assert!((v - 2.0 * PI * 0.25 * 2.0 / 3.0).abs() < 1e-9, "{v}");
}
