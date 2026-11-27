//! Push/Pull as a boolean with arcs and curved faces (Exact Curves 6c):
//! the inside test counts curved faces exactly, so holes, slots and
//! notches come out as with flat faces, with exact volumes.

use std::f64::consts::PI;

use prism_math::DVec3;
use prism_ops::pushpull::{Pulled, push_pull};
use prism_ops::{add_circle, add_rect, check, face_at, health::face_volume};
use prism_topo::{FaceId, Model};

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn volume(m: &Model) -> f64 {
    m.faces().map(|(id, _)| face_volume(m, id)).sum()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

fn pull(m: &mut Model, f: FaceId, d: f64) {
    assert!(!matches!(push_pull(m, f, d, false), Pulled::Nothing));
}

/// A 4 by 4 by 2 box on the ground, centred on the z axis.
fn block() -> Model {
    let mut m = Model::new();
    add_rect(&mut m, -2.0, -2.0, 2.0, 2.0, 0.0);
    let f = m.faces().next().map(|(id, _)| id).expect("a square");
    // Faces up or down, the box ends up above the ground.
    let up = m
        .face(f)
        .and_then(|x| x.plane())
        .map(|p| p.normal.z)
        .unwrap_or(1.0);
    pull(&mut m, f, 2.0 * up.signum());
    close(volume(&m), 32.0);
    m
}

fn top_at(m: &Model, x: f64, y: f64) -> FaceId {
    face_at(m, DVec3::new(x, y, 2.0)).expect("a face on top")
}

#[test]
fn a_disc_pushed_into_a_box_makes_a_blind_hole_then_a_through_hole() {
    let mut m = block();
    add_circle(&mut m, DVec3::new(0.0, 0.0, 2.0), DVec3::Z, 0.5);
    let disc = top_at(&m, 0.0, 0.0);
    pull(&mut m, disc, -1.0);
    clean(&m);
    close(volume(&m), 32.0 - PI * 0.25);
    // On down to the bottom: a hole right through.
    let bottom = face_at(&m, DVec3::new(0.0, 0.0, 1.0)).expect("the hole's floor");
    pull(&mut m, bottom, -1.0);
    clean(&m);
    close(volume(&m), 32.0 - PI * 0.25 * 2.0);
    assert!(
        face_at(&m, DVec3::new(0.0, 0.0, 0.0)).is_none(),
        "open below"
    );
}

#[test]
fn a_slot_pushed_across_a_hole_cuts_its_wall() {
    let mut m = block();
    add_circle(&mut m, DVec3::new(0.0, 0.0, 2.0), DVec3::Z, 0.5);
    let disc = top_at(&m, 0.0, 0.0);
    pull(&mut m, disc, -2.0);
    clean(&m);
    // A slot from the hole out to the edge, 0.5 wide, 1 deep: its two
    // sides drawn from the hole's rim to the box's edge (a rectangle's
    // end across the open hole would put faces over it).
    let x0 = (0.25f64 - 0.0625).sqrt();
    for y in [-0.25, 0.25] {
        prism_ops::add_segment(&mut m, DVec3::new(x0, y, 2.0), DVec3::new(2.0, y, 2.0));
    }
    let slot = top_at(&m, 1.0, 0.0);
    pull(&mut m, slot, -1.0);
    clean(&m);
    // The slot's area outside the hole: 1 less the hole's part of it.
    let y = 0.25f64;
    let inside = 2.0 * (y / 2.0 * (0.25 - y * y).sqrt() + 0.25 / 2.0 * (2.0 * y).asin());
    close(volume(&m), 32.0 - PI * 0.25 * 2.0 - (1.0 - inside));
}

#[test]
fn a_notch_pushed_into_a_cylinders_rim() {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let disc = m.faces().next().map(|(id, _)| id).expect("a disc");
    pull(&mut m, disc, 2.0);
    let z = if face_at(&m, DVec3::new(0.0, 0.0, 2.0)).is_some() {
        2.0
    } else {
        0.0
    };
    close(volume(&m), 2.0 * PI);
    // A line across the cap at x = 0.5: the part beyond it pushed down
    // takes a notch out of the rim.
    let h = 0.75f64.sqrt();
    prism_ops::add_segment(&mut m, DVec3::new(0.5, -h, z), DVec3::new(0.5, h, z));
    let part = face_at(&m, DVec3::new(0.75, 0.0, z)).expect("the cap's part");
    let n = m
        .face(part)
        .and_then(|f| f.plane())
        .map(|p| p.normal.z)
        .unwrap();
    // Into the solid: against the cap's outward normal.
    let into = if (z > 1.0) == (n > 0.0) { -1.0 } else { 1.0 };
    pull(&mut m, part, into);
    clean(&m);
    let seg = (0.5f64).acos() - 0.5 * 0.75f64.sqrt();
    close(volume(&m), 2.0 * PI - seg);
}

#[test]
fn a_disc_pushed_into_a_cylinders_cap_makes_a_cup() {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let disc = m.faces().next().map(|(id, _)| id).expect("a disc");
    pull(&mut m, disc, 2.0);
    let z = if face_at(&m, DVec3::new(0.0, 0.0, 2.0)).is_some() {
        2.0
    } else {
        0.0
    };
    add_circle(&mut m, DVec3::new(0.0, 0.0, z), DVec3::Z, 0.6);
    let inner = face_at(&m, DVec3::new(0.0, 0.0, z)).expect("the inner disc");
    let n = m
        .face(inner)
        .and_then(|f| f.plane())
        .map(|p| p.normal.z)
        .unwrap();
    // Into the solid: against the face's outward normal.
    let into = if (z > 1.0) == (n > 0.0) { -1.5 } else { 1.5 };
    pull(&mut m, inner, into);
    clean(&m);
    close(volume(&m), 2.0 * PI - PI * 0.36 * 1.5);
}

#[test]
fn a_hole_pushed_up_into_a_cone() {
    use std::collections::BTreeSet;
    // A cone of radius 1 and height 2 on the ground (Follow Me of a
    // triangle round a circle about z); the path's disc is left above.
    let mut m = Model::new();
    for (p, q) in [
        (DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0)),
        (DVec3::new(1.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 2.0)),
        (DVec3::new(0.0, 0.0, 2.0), DVec3::ZERO),
    ] {
        prism_ops::add_segment(&mut m, p, q);
    }
    let prof = m.faces().next().map(|(id, _)| id).expect("a triangle");
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    add_circle(&mut m, DVec3::new(0.0, 0.0, 3.0), DVec3::Z, 0.5);
    let path: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert!(prism_ops::follow::follow(&mut m, &path, prof));
    let lid = face_at(&m, DVec3::new(0.0, 0.0, 3.0)).expect("the path's disc");
    let cone = |m: &Model| volume(m) - face_volume(m, lid);
    close(cone(&m), 2.0 * PI / 3.0);
    // A small disc on the base, off its seam (the base's halves meet
    // along the y axis), pushed up into the cone.
    add_circle(&mut m, DVec3::new(0.5, 0.0, 0.0), DVec3::Z, 0.2);
    let hole = face_at(&m, DVec3::new(0.5, 0.0, 0.0)).expect("the small disc");
    let n = m
        .face(hole)
        .and_then(|f| f.plane())
        .map(|p| p.normal.z)
        .unwrap();
    pull(&mut m, hole, if n < 0.0 { -0.5 } else { 0.5 });
    clean(&m);
    close(cone(&m), 2.0 * PI / 3.0 - PI * 0.04 * 0.5);
}

/// Owner, 2026-11-11 (sessions 6 and 7): a circle on a box's side pulled
/// out joins a post to the box (the side gets a hole where they join), and
/// pushed right through makes a through hole (both sides get a hole).
#[test]
fn a_circle_on_a_side_pulled_out_joins_and_pushed_through_makes_a_hole() {
    for (d, vol) in [(1.0, 32.0 + PI * 0.25), (-4.0, 32.0 - PI * 0.25 * 4.0)] {
        let mut m = block();
        add_circle(&mut m, DVec3::new(2.0, 0.0, 1.0), DVec3::X, 0.5);
        let disc = face_at(&m, DVec3::new(2.0, 0.0, 1.0)).expect("the disc");
        let n = m
            .face(disc)
            .and_then(|f| f.plane())
            .map_or(1.0, |p| p.normal.x);
        pull(&mut m, disc, d * n.signum());
        clean(&m);
        close(volume(&m), vol);
        assert!(
            face_at(&m, DVec3::new(2.0, 0.0, 1.0)).is_none(),
            "{d}: open where the circle was"
        );
        if d < 0.0 {
            assert!(
                face_at(&m, DVec3::new(-2.0, 0.0, 1.0)).is_none(),
                "open at the far side"
            );
        }
    }
}
