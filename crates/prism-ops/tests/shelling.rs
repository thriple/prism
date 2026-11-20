//! Shell / Thicken (brief 42): flat faces (part 3a) and curved ones (3c).

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::shelling::{self, ALL_OPEN, BRANCHES, CORNER, FREEFORM, MIXED, TOO_THICK, ZERO};
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

    // A face bounded by a freeform curve.
    let p = DVec3::new;
    let mut c = Model::new();
    let curve = prism_geom::Nurbs::open(
        &[
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.5, 0.8, 0.0),
            DVec3::new(1.5, -0.5, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
        ],
        3,
    )
    .unwrap();
    prism_ops::sticky::add_spline(&mut c, &curve);
    poly(
        &mut c,
        &[
            p(2.0, 0.0, 0.0),
            p(2.0, 1.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(0.0, 0.0, 0.0),
        ],
    );
    assert!(c.face_count() >= 1);
    assert_eq!(
        shelling::refusal(&c, &any_face(&c), &none, 0.1),
        Some(FREEFORM)
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

// ---------- Part 3c: curved faces ----------

use std::f64::consts::PI;

fn disc() -> (Model, FaceId) {
    let mut m = Model::new();
    let f = prism_ops::sticky::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0).faces[0];
    (m, f)
}

/// A cylinder of radius 1 and height 1 on the ground, its faces out.
fn cylinder() -> Model {
    let (mut m, f) = disc();
    let up = m.face(f).unwrap().plane().unwrap().normal.z > 0.0;
    prism_ops::pushpull::push_pull(&mut m, f, if up { 1.0 } else { -1.0 }, false);
    sound(&m);
    near_enough(volume(&m), PI);
    m
}

fn near_enough(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} is not {b}");
}

#[test]
fn a_disc_becomes_a_round_slab() {
    for t in [0.2, -0.2] {
        let (mut m, f) = disc();
        shelling::shell(&mut m, &[f].into(), &BTreeSet::new(), t).unwrap();
        sound(&m);
        near_enough(volume(&m), PI * 0.2);
        // The wall is a cylinder, its seams hidden.
        assert!(
            m.faces()
                .any(|(_, f)| matches!(f.surface, prism_geom::Surface::Cylinder(_)))
        );
        assert!(
            m.edges()
                .filter(|(_, e)| !e.curve.is_curved())
                .all(|(_, e)| e.soft)
        );
    }
}

#[test]
fn a_cylinder_is_hollowed_inside_or_grown_outside() {
    for (d, vol) in [(-0.1, PI - PI * 0.81 * 0.8), (0.1, PI * 1.21 * 1.2 - PI)] {
        let mut m = cylinder();
        let all = any_face(&m);
        shelling::shell(&mut m, &all, &BTreeSet::new(), d).unwrap();
        sound(&m);
        near_enough(volume(&m), vol);
    }
    let m = cylinder();
    assert_eq!(
        shelling::refusal(&m, &any_face(&m), &BTreeSet::new(), -1.2),
        Some(TOO_THICK)
    );
}

#[test]
fn a_cylinder_with_its_top_open_becomes_a_cup() {
    let mut m = cylinder();
    let top = face_at(&m, DVec3::Z);
    let all = any_face(&m);
    shelling::shell(&mut m, &all, &[top].into(), -0.1).unwrap();
    sound(&m);
    near_enough(volume(&m), PI - PI * 0.81 * 0.9);
    // The rim is a flat ring at the top.
    assert!(m.faces().any(|(_, f)| {
        f.plane()
            .is_some_and(|p| p.normal.z > 0.99 && p.origin.z > 0.99)
    }));
}

#[test]
fn a_box_with_a_rounded_edge_keeps_its_wall_even() {
    let mut m = cube();
    // The upright edge at x = 1, y = 1.
    let corner = |q: DVec3| (q.x - 1.0).abs() < 1e-9 && (q.y - 1.0).abs() < 1e-9;
    let e = m
        .edges()
        .find(|(_, e)| corner(m.vertex(e.a)) && corner(m.vertex(e.b)))
        .map(|(id, _)| id)
        .unwrap();
    assert!(prism_ops::round::round(&mut m, e, 0.3, true));
    sound(&m);
    let before = volume(&m);
    let all = any_face(&m);
    shelling::shell(&mut m, &all, &BTreeSet::new(), -0.1).unwrap();
    sound(&m);
    // The inside is the same shape 0.1 in: an 0.8 box with an 0.2 round.
    let inner = 0.8 * 0.8 * 0.8 - (0.2 * 0.2 - PI * 0.04 / 4.0) * 0.8;
    near_enough(volume(&m), before - inner);
}

#[test]
fn a_freeform_face_on_its_own_becomes_a_slab() {
    let mut m = Model::new();
    let curve = |y: f64| {
        prism_geom::Nurbs::open(
            &[
                DVec3::new(0.0, y, 0.0),
                DVec3::new(0.4, y, 0.5),
                DVec3::new(0.8, y, -0.3),
                DVec3::new(1.2, y, 0.0),
            ],
            3,
        )
        .unwrap()
    };
    prism_ops::sticky::add_spline(&mut m, &curve(0.0));
    prism_ops::sticky::add_spline(&mut m, &curve(1.0));
    let edges: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    prism_ops::loft::loft(&mut m, &edges).unwrap();
    let f = m
        .faces()
        .find(|(_, f)| f.surface.patch().is_some())
        .map(|(id, _)| id)
        .expect("a freeform face");
    assert_eq!(m.face_count(), 1);
    shelling::shell(&mut m, &[f].into(), &BTreeSet::new(), 0.1).unwrap();
    assert!(is_solid(&m));
    assert!(volume(&m).abs() > 0.1, "{}", volume(&m));
}

/// A loft-like sheet of three smooth NURBS faces (brief 49 part 1): four
/// open curves lofted give one surface cut into three faces, sharing the
/// middle two curves as smooth interior joins.
#[test]
fn a_loft_like_sheet_of_smooth_freeform_faces_thickens_to_a_valid_solid() {
    let mut m = Model::new();
    let wave = |z: f64, bump: f64| {
        prism_geom::Nurbs::open(
            &[
                DVec3::new(0.0, 0.0, z),
                DVec3::new(1.0, bump, z),
                DVec3::new(3.0, -bump, z),
                DVec3::new(4.0, 0.0, z),
            ],
            3,
        )
        .unwrap()
    };
    for (z, bump) in [(0.0, 2.0), (2.0, 0.5), (4.0, 1.0), (6.0, 0.5)] {
        prism_ops::sticky::add_spline(&mut m, &wave(z, bump));
    }
    let edges: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    let faces = prism_ops::loft::loft(&mut m, &edges).unwrap();
    assert_eq!(faces.len(), 3, "one face between each pair of curves");
    let body: BTreeSet<FaceId> = faces.into_iter().collect();
    let area: f64 = body.iter().map(|f| m.face_area(*f)).sum();
    let d = 0.3;
    assert!(shelling::refusal(&m, &body, &BTreeSet::new(), d).is_none());
    shelling::shell(&mut m, &body, &BTreeSet::new(), d).unwrap();
    assert!(is_solid(&m));
    let v = volume(&m);
    assert!((v - area * d).abs() < 0.05 * v, "{v} {area}");
}

#[test]
fn a_sphere_is_hollowed() {
    // A sphere of radius 1, by Follow Me of a half disc round a circle far
    // above (which stays as it is).
    let mut m = Model::new();
    add_segment(
        &mut m,
        DVec3::new(0.0, 0.0, -1.0),
        DVec3::new(0.0, 0.0, 1.0),
    );
    prism_ops::sticky::add_arc(
        &mut m,
        DVec3::ZERO,
        -DVec3::Y,
        1.0,
        DVec3::new(0.0, 0.0, -1.0),
        PI,
    );
    let prof = m.faces().next().map(|(id, _)| id).unwrap();
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    prism_ops::sticky::add_circle(&mut m, DVec3::new(0.0, 0.0, 3.0), DVec3::Z, 0.5);
    let path: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert!(prism_ops::follow::follow(&mut m, &path, prof));
    let ball = m
        .faces()
        .find(|(_, f)| matches!(f.surface, prism_geom::Surface::Sphere(_)))
        .map(|(id, _)| id)
        .unwrap();
    let v0 = volume(&m);
    shelling::shell(&mut m, &[ball].into(), &BTreeSet::new(), -0.1).unwrap();
    assert!(prism_ops::check::problems(&m).is_empty());
    near_enough(volume(&m) - v0, -4.0 / 3.0 * PI * 0.9f64.powi(3));
}

// ---------- Part 3d: corners of four or more faces ----------

/// A lopsided pyramid on a polygon base (faces out), its tip at `tip`.
fn pyramid(base: &[DVec3], tip: DVec3) -> Model {
    let mut py = Model::new();
    poly(&mut py, base);
    for q in base {
        add_segment(&mut py, *q, tip);
    }
    assert_eq!(py.face_count(), base.len() + 1);
    prism_ops::health::orient(&mut py, None);
    assert!(volume(&py) > 0.0, "faces out");
    py
}

fn square_base() -> Vec<DVec3> {
    let p = DVec3::new;
    vec![
        p(0.0, 0.0, 0.0),
        p(1.0, 0.0, 0.0),
        p(1.0, 1.0, 0.0),
        p(0.0, 1.0, 0.0),
    ]
}

#[test]
fn a_tip_of_four_faces_gets_a_small_face_and_walls_stay_even() {
    for t in [-0.05, 0.05] {
        let mut m = pyramid(&square_base(), DVec3::new(0.3, 0.6, 1.0));
        let v0 = volume(&m);
        let planes: Vec<_> = m.faces().map(|(_, f)| *f.plane().unwrap()).collect();
        let all = any_face(&m);
        shelling::shell(&mut m, &all, &BTreeSet::new(), t).unwrap();
        sound(&m);
        // Two shells: the old one and the moved one, with a small face at
        // the tip.
        assert_eq!(m.face_count(), 5 + 5 + 1);
        let v = volume(&m);
        assert!(
            v > 0.0 && (if t < 0.0 { v < v0 } else { v > 0.0 }),
            "{v} {v0}"
        );
        // Every old face has a moved copy the thickness from it.
        for pl in &planes {
            let moved = m.faces().any(|(_, f)| {
                f.plane().is_some_and(|q| {
                    q.normal.dot(pl.normal).abs() > 1.0 - 1e-9
                        && ((q.origin - pl.origin).dot(pl.normal) - t).abs() < 1e-9
                })
            });
            assert!(moved, "a face moved by {t}");
        }
    }
}

#[test]
fn a_rim_corner_of_four_faces_is_still_refused() {
    // A five-sided pyramid with one side open: four faces meet at the tip,
    // which is on the rim.
    let p = DVec3::new;
    let base = [
        p(0.0, 0.0, 0.0),
        p(1.0, 0.0, 0.0),
        p(1.4, 0.8, 0.0),
        p(0.5, 1.3, 0.0),
        p(-0.3, 0.7, 0.0),
    ];
    let m = pyramid(&base, p(0.3, 0.5, 1.0));
    let side = m
        .faces()
        .find(|(_, f)| f.outer.len() == 3)
        .map(|(id, _)| id)
        .unwrap();
    assert_eq!(
        shelling::refusal(&m, &any_face(&m), &[side].into(), -0.05),
        Some(CORNER)
    );
}

/// Two flat quads, each stored as a NURBS patch (not simplified to a
/// plane), folded at a right angle along their shared edge.
fn folded_patches() -> (Model, FaceId, FaceId) {
    use prism_geom::{NurbsSurface, Patch, Surface};
    use prism_topo::Face;
    use std::sync::Arc;

    let square = |p00: DVec3, p01: DVec3, p10: DVec3, p11: DVec3| {
        let k = vec![0.0, 0.0, 1.0, 1.0];
        Arc::new(
            NurbsSurface::new(
                [1, 1],
                [k.clone(), k],
                [2, 2],
                vec![p00, p01, p10, p11],
                vec![1.0; 4],
                [false, false],
            )
            .unwrap(),
        )
    };
    // The shared edge: the line x = 0, y = 0, z from 0 to 1.
    let (p, q) = (DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 1.0));
    let sa = square(p, q, DVec3::new(1.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 1.0));
    let sb = square(p, q, DVec3::new(0.0, 1.0, 0.0), DVec3::new(0.0, 1.0, 1.0));

    let mut m = Model::new();
    let vp = m.add_vertex(p);
    let vq = m.add_vertex(q);
    let shared = m.add_edge(vp, vq).unwrap();

    let ra = m.add_vertex(DVec3::new(1.0, 0.0, 0.0));
    let sa_v = m.add_vertex(DVec3::new(1.0, 0.0, 1.0));
    let e1 = m.add_edge(vp, ra).unwrap();
    let e2 = m.add_edge(ra, sa_v).unwrap();
    let e3 = m.add_edge(sa_v, vq).unwrap();
    let a = m.add_face(Face::with_edges(
        vec![vp, ra, sa_v, vq],
        vec![],
        Surface::Nurbs(Patch::whole(
            Arc::try_unwrap(sa).unwrap_or_else(|a| (*a).clone()),
        )),
        vec![vec![e1, e2, e3, shared]],
    ));

    let rb = m.add_vertex(DVec3::new(0.0, 1.0, 0.0));
    let sb_v = m.add_vertex(DVec3::new(0.0, 1.0, 1.0));
    let e4 = m.add_edge(vq, sb_v).unwrap();
    let e5 = m.add_edge(sb_v, rb).unwrap();
    let e6 = m.add_edge(rb, vp).unwrap();
    let b = m.add_face(Face::with_edges(
        vec![vq, sb_v, rb, vp],
        vec![],
        Surface::Nurbs(Patch {
            flipped: true,
            ..Patch::whole(Arc::try_unwrap(sb).unwrap_or_else(|a| (*a).clone()))
        }),
        vec![vec![e4, e5, e6, shared]],
    ));
    (m, a, b)
}

/// Three faces meeting at one vertex (part 2c): a NURBS patch `a` (as
/// `folded_patches`) and two flat faces `b`, `c`, each a 90 degree fold
/// from `a` (so `a`'s crease with each of them can be an outside
/// corner, same as `folded_patches`), but `b` and `c` meet each other
/// smoothly... no: at a plain inside corner (both flat, so never
/// "outside" themselves). The shared vertex `q` therefore has `a` as
/// the one face an outside corner's mitre there can reach past, while
/// `b` and `c` keep the one shared corner between themselves.
fn three_face_corner() -> (Model, FaceId, FaceId, FaceId) {
    use prism_geom::{NurbsSurface, Patch, Surface};
    use prism_math::Plane;
    use prism_topo::Face;
    use std::sync::Arc;

    let p = DVec3::new(0.0, 0.0, 0.0);
    let q = DVec3::new(0.0, 0.0, 1.0);
    let ra = DVec3::new(1.0, 0.0, 0.0);
    let sa_v = DVec3::new(1.0, 0.0, 1.0);
    let rb = DVec3::new(0.0, 1.0, 0.0);
    let sb_v = DVec3::new(0.0, 1.0, 1.0);
    let cc = DVec3::new(1.0, 1.0, 1.0);

    let k = vec![0.0, 0.0, 1.0, 1.0];
    let sa = Arc::new(
        NurbsSurface::new(
            [1, 1],
            [k.clone(), k],
            [2, 2],
            vec![p, q, ra, sa_v],
            vec![1.0; 4],
            [false, false],
        )
        .unwrap(),
    );

    let mut m = Model::new();
    let vp = m.add_vertex(p);
    let vq = m.add_vertex(q);
    let shared_ab = m.add_edge(vp, vq).unwrap();

    let v_ra = m.add_vertex(ra);
    let v_sa = m.add_vertex(sa_v);
    let e1 = m.add_edge(vp, v_ra).unwrap();
    let e2 = m.add_edge(v_ra, v_sa).unwrap();
    let e3 = m.add_edge(v_sa, vq).unwrap();
    let a = m.add_face(Face::with_edges(
        vec![vp, v_ra, v_sa, vq],
        vec![],
        Surface::Nurbs(Patch::whole(
            Arc::try_unwrap(sa).unwrap_or_else(|a| (*a).clone()),
        )),
        vec![vec![e1, e2, e3, shared_ab]],
    ));

    let v_rb = m.add_vertex(rb);
    let v_sb = m.add_vertex(sb_v);
    let shared_bc = m.add_edge(vq, v_sb).unwrap();
    let e5 = m.add_edge(v_sb, v_rb).unwrap();
    let e6 = m.add_edge(v_rb, vp).unwrap();
    let b = m.add_face(Face::with_edges(
        vec![vq, v_sb, v_rb, vp],
        vec![],
        Surface::Plane(Plane::new(p, DVec3::new(-1.0, 0.0, 0.0))),
        vec![vec![shared_bc, e5, e6, shared_ab]],
    ));

    let v_cc = m.add_vertex(cc);
    let e_ac = m.add_edge(v_sa, v_cc).unwrap();
    let e_cb = m.add_edge(v_cc, v_sb).unwrap();
    let c = m.add_face(Face::with_edges(
        vec![vq, v_sa, v_cc, v_sb],
        vec![],
        Surface::Plane(Plane::new(q, DVec3::new(0.0, 0.0, 1.0))),
        vec![vec![e3, e_ac, e_cb, shared_bc]],
    ));
    (m, a, b, c)
}

/// Three faces at a corner (part 2c): `a`'s mitre with each of `b` and
/// `c` is its own outside corner (as `an_outside_corner_past_a_freeform_edge_is_bridged`),
/// but `b` and `c` meet each other at a plain inside corner, and are
/// themselves flat (no edge of their own to miss), so only `a` ever
/// splits away; wherever the shared corner is still truly on `b` or
/// `c`, it keeps it, exactly as the three faces' plain, unbounded
/// corner would. By hand: the three faces are the faces of a unit
/// cube's own corner, so thickening it outward by `d` grows it to a
/// cube of side `1 + d` less the original, `(1 + d)^3 - 1`.
#[test]
fn three_faces_at_a_corner_bridge_two_outside_creases_and_mitre_the_third() {
    let d: f64 = 0.3;
    let vol = (1.0 + d).powi(3) - 1.0;
    let (mut m, a, b, c) = three_face_corner();
    let body: BTreeSet<FaceId> = [a, b, c].into();
    assert_eq!(shelling::refusal(&m, &body, &BTreeSet::new(), d), None);
    shelling::shell(&mut m, &body, &BTreeSet::new(), d).unwrap();
    sound(&m);
    near_enough(volume(&m), vol);
}

/// Two NURBS patches sharing an edge, creased along most of it but
/// exactly tangent (the same normal) at one end: a real boat hull has
/// a join like this, fading out to flat at one end of a chine line.
fn tapering_fold() -> (Model, FaceId, FaceId) {
    use prism_geom::{NurbsSurface, Patch, Surface};
    use prism_topo::Face;
    use std::sync::Arc;

    let k = vec![0.0, 0.0, 1.0, 1.0];
    let (p, q) = (DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 1.0));
    let sa = Arc::new(
        NurbsSurface::new(
            [1, 1],
            [k.clone(), k.clone()],
            [2, 2],
            vec![p, q, DVec3::new(1.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 1.0)],
            vec![1.0; 4],
            [false, false],
        )
        .unwrap(),
    );
    // Tangent to A (the same normal) at p, a 90 degree crease at q.
    let sb = Arc::new(
        NurbsSurface::new(
            [1, 1],
            [k.clone(), k],
            [2, 2],
            vec![p, q, DVec3::new(1.0, 0.0, 1.0), DVec3::new(0.0, 1.0, 1.0)],
            vec![1.0; 4],
            [false, false],
        )
        .unwrap(),
    );

    let mut m = Model::new();
    let vp = m.add_vertex(p);
    let vq = m.add_vertex(q);
    let shared = m.add_edge(vp, vq).unwrap();

    let ra = m.add_vertex(DVec3::new(1.0, 0.0, 0.0));
    let sa_v = m.add_vertex(DVec3::new(1.0, 0.0, 1.0));
    let e1 = m.add_edge(vp, ra).unwrap();
    let e2 = m.add_edge(ra, sa_v).unwrap();
    let e3 = m.add_edge(sa_v, vq).unwrap();
    let a = m.add_face(Face::with_edges(
        vec![vp, ra, sa_v, vq],
        vec![],
        Surface::Nurbs(Patch::whole(
            Arc::try_unwrap(sa).unwrap_or_else(|a| (*a).clone()),
        )),
        vec![vec![e1, e2, e3, shared]],
    ));

    let rb = m.add_vertex(DVec3::new(1.0, 0.0, 1.0));
    let sb_v = m.add_vertex(DVec3::new(0.0, 1.0, 1.0));
    let e4 = m.add_edge(vq, sb_v).unwrap();
    let e5 = m.add_edge(sb_v, rb).unwrap();
    let e6 = m.add_edge(rb, vp).unwrap();
    let b = m.add_face(Face::with_edges(
        vec![vq, sb_v, rb, vp],
        vec![],
        Surface::Nurbs(Patch::whole(
            Arc::try_unwrap(sb).unwrap_or_else(|a| (*a).clone()),
        )),
        vec![vec![e4, e5, e6, shared]],
    ));
    (m, a, b)
}

#[test]
fn a_crease_tangent_at_one_end_is_not_mistaken_for_a_fold_back() {
    let (m, a, b) = tapering_fold();
    let body: BTreeSet<FaceId> = [a, b].into();
    let tol = m.tol();
    let fa = m.face(a).unwrap();
    let fb = m.face(b).unwrap();
    // The fixture's own premise: exactly tangent at one end of the
    // shared edge (p = origin), a real crease at the other (q).
    let (p, q) = (DVec3::ZERO, DVec3::new(0.0, 0.0, 1.0));
    let (na0, nb0) = (fa.surface.normal_at(p), fb.surface.normal_at(p));
    assert!(tol.parallel(na0, nb0) && na0.dot(nb0) > 0.0, "tangent at p");
    let (na1, nb1) = (fa.surface.normal_at(q), fb.surface.normal_at(q));
    assert!(na1.dot(nb1).abs() < 0.1, "a real crease at q");

    // Neither sign is "too sharp" (the bug): a tangent end is not a
    // fold back. Whether either is an outside corner (extends, brief
    // 49's own separate, open part, given this crease's own shape) is
    // not what this test is about.
    for d in [0.1, -0.1] {
        assert_ne!(
            shelling::refusal(&m, &body, &BTreeSet::new(), d),
            Some(shelling::SHARP),
            "d={d}"
        );
    }
}

/// Two unit squares at a right angle (`folded_patches`) thicken to a
/// sharp mitre on an inside corner (trimmed, by hand: the two offset
/// rectangles are short of a `d` by `d` square where they meet, so
/// `area = 2|d| - d^2`, times the 1 m fold gives the volume).
#[test]
fn a_crease_between_freeform_faces_gets_a_sharp_mitre() {
    let d: f64 = -0.3;
    let area = 2.0 * d.abs() - d * d;
    let (mut m, a, b) = folded_patches();
    let body: BTreeSet<FaceId> = [a, b].into();
    assert_eq!(shelling::refusal(&m, &body, &BTreeSet::new(), d), None);
    shelling::shell(&mut m, &body, &BTreeSet::new(), d).unwrap();
    sound(&m);
    near_enough(volume(&m).abs(), area);
}

/// An outside corner (the other sign of `d`, brief 49 part 2b): the
/// mitre would reach past each freeform face's own edge, which a NURBS
/// surface cannot extend to meet, so each face keeps its own straight
/// offset edge, bridged by a small flat face at the gap. By hand: the
/// two offset rectangles now fall short of each other (no overlap to
/// trim away), and the gap between their near edges is a right-angled
/// triangle of legs `d`, so `area = 2|d| + d^2 / 2`.
#[test]
fn an_outside_corner_past_a_freeform_edge_is_bridged() {
    let d: f64 = 0.3;
    let area = 2.0 * d.abs() + d * d / 2.0;
    let (mut m, a, b) = folded_patches();
    let body: BTreeSet<FaceId> = [a, b].into();
    assert_eq!(shelling::refusal(&m, &body, &BTreeSet::new(), d), None);
    shelling::shell(&mut m, &body, &BTreeSet::new(), d).unwrap();
    sound(&m);
    near_enough(volume(&m).abs(), area);
}
