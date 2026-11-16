//! Shell / Thicken of flat faces (brief 42, part 3a).

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::shelling::{self, ALL_OPEN, BRANCHES, CORNER, FLAT_ONLY, MIXED, TOO_THICK, ZERO};
use prism_ops::{add_segment, boolean::is_solid, health::face_volume};
use prism_topo::{FaceId, Model};

fn poly(m: &mut Model, pts: &[DVec3]) -> Vec<FaceId> {
    let mut faces = Vec::new();
    for i in 0..pts.len() {
        faces.extend(add_segment(m, pts[i], pts[(i + 1) % pts.len()]).faces);
    }
    faces
}

fn square() -> (Model, FaceId) {
    let mut m = Model::new();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    let f = poly(
        &mut m,
        &[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)],
    );
    (m, f[0])
}

/// A 1 m box at the origin, its faces facing out.
fn cube() -> Model {
    let (mut m, f) = square();
    let d = if m.face(f).unwrap().plane().unwrap().normal.z > 0.0 {
        1.0
    } else {
        -1.0
    };
    prism_ops::pushpull::push_pull(&mut m, f, d, false);
    assert!(is_solid(&m));
    assert!((volume(&m) - 1.0).abs() < 1e-9, "faces out");
    m
}

fn volume(m: &Model) -> f64 {
    m.faces().map(|(id, _)| face_volume(m, id)).sum()
}

fn any_face(m: &Model) -> BTreeSet<FaceId> {
    m.faces().map(|(id, _)| id).take(1).collect()
}

fn face_at(m: &Model, n: DVec3) -> FaceId {
    m.faces()
        .find(|(_, f)| f.plane().is_some_and(|p| p.normal.dot(n) > 0.99))
        .map(|(id, _)| id)
        .unwrap()
}

fn sound(m: &Model) {
    assert!(is_solid(m), "closed");
    let p = prism_ops::check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} is not {b}");
}

#[test]
fn a_face_becomes_a_slab_on_either_side() {
    for t in [0.2, -0.2] {
        let (mut m, f) = square();
        let n = m.face(f).unwrap().plane().unwrap().normal;
        shelling::shell(&mut m, &[f].into(), &BTreeSet::new(), t).unwrap();
        sound(&m);
        assert_eq!(m.face_count(), 6);
        near(volume(&m), 0.2);
        // It grew to the side the thickness says.
        let far = m.vertices().map(|(_, p)| p.dot(n)).fold(
            if t > 0.0 { f64::MIN } else { f64::MAX },
            |a, b| {
                if t > 0.0 { a.max(b) } else { a.min(b) }
            },
        );
        near(far, t);
    }
}

#[test]
fn an_l_of_two_faces_keeps_its_thickness_round_the_fold() {
    let mut m = Model::new();
    let p = DVec3::new;
    let a = poly(
        &mut m,
        &[
            p(2.0, 0.0, 0.0),
            p(0.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(2.0, 1.0, 0.0),
        ],
    );
    let b = poly(
        &mut m,
        &[
            p(0.0, 0.0, 0.0),
            p(0.0, 0.0, 2.0),
            p(0.0, 1.0, 2.0),
            p(0.0, 1.0, 0.0),
        ],
    );
    let (a, b) = (a[0], b[0]);
    // Both face into the fold, so the thickness goes the same way for both.
    for (f, inward) in [(a, DVec3::Z), (b, DVec3::X)] {
        if m.face(f).unwrap().plane().unwrap().normal.dot(inward) < 0.0 {
            m.edit_face(f, |x| x.reverse());
        }
    }
    let t = 0.1;
    for (d, vol) in [(t, 4.0 * t - t * t), (-t, 4.0 * t + t * t)] {
        let mut m = m.clone();
        shelling::shell(&mut m, &[a].into(), &BTreeSet::new(), d).unwrap();
        sound(&m);
        near(volume(&m).abs(), vol);
        // The moved faces lie the thickness from the old ones.
        let zs: BTreeSet<i64> = m
            .vertices()
            .map(|(_, q)| (q.z * 1e6).round() as i64)
            .collect();
        assert!(zs.contains(&((d * 1e6).round() as i64)), "{zs:?}");
    }
}

#[test]
fn a_box_is_hollowed_inside_or_grown_outside() {
    let t = 0.1;
    for (d, vol) in [(-t, 1.0 - 0.8f64.powi(3)), (t, 1.2f64.powi(3) - 1.0)] {
        let mut m = cube();
        let all = any_face(&m);
        shelling::shell(&mut m, &all, &BTreeSet::new(), d).unwrap();
        sound(&m);
        assert_eq!(m.face_count(), 12, "two shells");
        near(volume(&m), vol);
    }
}

#[test]
fn a_box_with_its_top_left_open_becomes_a_tray() {
    let mut m = cube();
    let top = face_at(&m, DVec3::Z);
    let all = any_face(&m);
    shelling::shell(&mut m, &all, &[top].into(), -0.1).unwrap();
    sound(&m);
    assert_eq!(m.face_count(), 5 + 5 + 4, "outside, inside and the rim");
    near(volume(&m), 1.0 - 0.8 * 0.8 * 0.9);
    // The rim is flat, at the top.
    let rim = m
        .faces()
        .filter(|(_, f)| {
            f.plane()
                .is_some_and(|p| p.normal.z > 0.99 && p.origin.z > 0.99)
        })
        .count();
    assert_eq!(rim, 4);
}

#[test]
fn shelling_is_refused_with_a_reason() {
    let (m, f) = square();
    let one: BTreeSet<FaceId> = [f].into();
    let none = BTreeSet::new();
    assert_eq!(shelling::refusal(&m, &one, &none, 0.0), Some(ZERO));
    assert_eq!(shelling::refusal(&m, &one, &one, 0.1), Some(ALL_OPEN));

    let m = cube();
    assert_eq!(
        shelling::refusal(&m, &any_face(&m), &none, -0.6),
        Some(TOO_THICK)
    );

    // A circle.
    let mut c = Model::new();
    prism_ops::sticky::add_circle(&mut c, DVec3::ZERO, DVec3::Z, 1.0);
    assert_eq!(
        shelling::refusal(&c, &any_face(&c), &none, 0.1),
        Some(FLAT_ONLY)
    );

    // A lopsided pyramid: four faces meet at its tip, and their moved
    // copies do not meet at one point.
    let mut py = Model::new();
    let p = DVec3::new;
    let base = [
        p(0.0, 0.0, 0.0),
        p(1.0, 0.0, 0.0),
        p(1.0, 1.0, 0.0),
        p(0.0, 1.0, 0.0),
    ];
    poly(&mut py, &base);
    for q in base {
        add_segment(&mut py, q, p(0.3, 0.6, 1.0));
    }
    assert_eq!(py.face_count(), 5);
    prism_ops::health::orient(&mut py, None);
    assert_eq!(
        shelling::refusal(&py, &any_face(&py), &none, -0.05),
        Some(CORNER)
    );

    // Two faces that face different ways.
    let (mut m2, f) = square();
    let g = poly(
        &mut m2,
        &[
            p(1.0, 0.0, 0.0),
            p(2.0, 0.0, 0.0),
            p(2.0, 1.0, 0.0),
            p(1.0, 1.0, 0.0),
        ],
    )[0];
    let same = |m: &Model| {
        m.face(f)
            .unwrap()
            .plane()
            .unwrap()
            .normal
            .dot(m.face(g).unwrap().plane().unwrap().normal)
            > 0.0
    };
    if same(&m2) {
        m2.edit_face(g, |x| x.reverse());
    }
    assert_eq!(shelling::refusal(&m2, &[f].into(), &none, 0.1), Some(MIXED));

    // Three faces at one edge.
    let (mut m3, f) = square();
    poly(
        &mut m3,
        &[
            p(0.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(-1.0, 1.0, 0.0),
            p(-1.0, 0.0, 0.0),
        ],
    );
    poly(
        &mut m3,
        &[
            p(0.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(0.0, 1.0, 1.0),
            p(0.0, 0.0, 1.0),
        ],
    );
    assert_eq!(m3.face_count(), 3);
    assert_eq!(
        shelling::refusal(&m3, &[f].into(), &none, 0.1),
        Some(BRANCHES)
    );
}

#[test]
fn a_face_with_a_hole_gets_a_wall_round_the_hole_too() {
    let (mut m, f) = square();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    poly(
        &mut m,
        &[p(0.25, 0.25), p(0.75, 0.25), p(0.75, 0.75), p(0.25, 0.75)],
    );
    let inner = m
        .faces()
        .map(|(id, _)| id)
        .find(|id| *id != f && m.face(*id).unwrap().holes.is_empty() && m.face_area(*id) < 0.3)
        .unwrap();
    prism_ops::remove(
        &mut m,
        &prism_ops::Selection {
            faces: [inner].into(),
            ..Default::default()
        },
    );
    let ring = any_face(&m);
    assert_eq!(m.face(*ring.first().unwrap()).unwrap().holes.len(), 1);
    shelling::shell(&mut m, &ring, &BTreeSet::new(), 0.1).unwrap();
    sound(&m);
    assert_eq!(m.face_count(), 2 + 4 + 4);
    near(volume(&m), 0.75 * 0.1);
}
