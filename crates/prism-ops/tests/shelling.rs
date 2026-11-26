//! Shell / Thicken (brief 42): flat faces (part 3a) and curved ones (3c).

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::shelling::{self, ALL_OPEN, CORNER, TOO_THICK, ZERO};
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
}

/// An upside-down T of three unit squares at one edge (x = 0, y 0 to 1):
/// `f` (x 0 to 1) and `g` (x -1 to 0) flat on z = 0, facing up, and `h`
/// standing on their shared edge (z 0 to 1), facing +x.
fn tee() -> (Model, FaceId, FaceId, FaceId) {
    let p = DVec3::new;
    let (mut m, f) = square();
    let g = poly(
        &mut m,
        &[
            p(0.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(-1.0, 1.0, 0.0),
            p(-1.0, 0.0, 0.0),
        ],
    )[0];
    poly(
        &mut m,
        &[
            p(0.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(0.0, 1.0, 1.0),
            p(0.0, 0.0, 1.0),
        ],
    );
    assert_eq!(m.face_count(), 3);
    let h = m
        .faces()
        .map(|(id, _)| id)
        .find(|id| *id != f && *id != g)
        .unwrap();
    for (x, want) in [(f, DVec3::Z), (g, DVec3::Z), (h, DVec3::X)] {
        if m.face(x).unwrap().plane().unwrap().normal.dot(want) < 0.0 {
            m.edit_face(x, |face| face.reverse());
        }
    }
    (m, f, g, h)
}

/// Three faces at one edge used to be refused (`BRANCHES`). Now the body
/// is parted there into sheets that each have at most two faces at an
/// edge (neighbours round the edge that could be one sheet folded there
/// are joined: here `f` with `h`, an L, mitred inside its fold), each is
/// shelled on its own, and their shells are joined, merged where they
/// overlap. By hand: up, the three slabs (3d) less the square where the
/// L's two overlap at its fold (d^2); down, the three slabs only touch.
#[test]
fn three_faces_at_one_edge_are_shelled_as_sheets_and_joined() {
    let none = BTreeSet::new();
    for (d, want) in [(0.1f64, 0.3 - 0.01), (-0.1, 0.3), (0.02, 0.06 - 0.0004)] {
        let (m, f, g, h) = tee();
        assert_eq!(shelling::refusal(&m, &[f].into(), &none, d), None, "by {d}");
        let built = shelling::slab_flagged(&m, &[f].into(), &none, d, &Default::default()).unwrap();
        sound(&built.model);
        assert_eq!(built.replaced, [f, g, h].into());
        near(volume(&built.model).abs(), want);
        let branched = &built.flags[shelling::BRANCHED];
        assert!(
            !branched.faces.is_empty() && !branched.edges.is_empty(),
            "by {d}"
        );
        // Only what is near the edge is flagged: not the far ends.
        for e in &branched.edges {
            let x = built.model.edge(*e).unwrap();
            for v in [x.a, x.b] {
                assert!(
                    built.model.vertex(v).x.abs() <= 2.0 * d.abs() + 1e-9,
                    "by {d}"
                );
            }
        }
        let mut joined = m.clone();
        shelling::shell(&mut joined, &[f].into(), &none, d).unwrap();
        sound(&joined);
        near(volume(&joined).abs(), want);
    }
}

/// The same upside-down T, of three flat freeform (NURBS) faces. Each
/// sheet shells on its own, but joining the shells is still refused: the
/// end wall of `g`'s shell lies right on the face of `h` it stands
/// against, and the boolean's inside test cannot tell which side of a
/// freeform face a point that close to it is on (every ray is unsure),
/// so it says so rather than guess. Flat faces (`tee`) are placed exactly.
/// When the boolean can place a face lying on a freeform one, this should
/// build (by hand, as `tee`: 3d less d^2 up, 3d down).
#[test]
fn three_freeform_faces_at_one_edge_are_refused_where_their_shells_touch() {
    use prism_geom::{NurbsSurface, Patch, Surface};
    use prism_topo::Face;
    let p = DVec3::new;
    let mut m = Model::new();
    let (o0, o1) = (
        m.add_vertex(p(0.0, 0.0, 0.0)),
        m.add_vertex(p(0.0, 1.0, 0.0)),
    );
    let shared = m.add_edge(o0, o1).unwrap();
    // A flat patch from the shared edge out to `far` (two corners),
    // walking the shared edge from `o0` when `forward`, facing `want`.
    let sheet = |m: &mut Model, far: [DVec3; 2], forward: bool, want: DVec3| {
        let k = vec![0.0, 0.0, 1.0, 1.0];
        let s = NurbsSurface::new(
            [1, 1],
            [k.clone(), k],
            [2, 2],
            vec![p(0.0, 0.0, 0.0), p(0.0, 1.0, 0.0), far[0], far[1]],
            vec![1.0; 4],
            [false, false],
        )
        .unwrap();
        let patch = Patch::whole(s);
        let mid = (far[0] + far[1]) * 0.25 + p(0.0, 0.25, 0.0);
        let patch = if Surface::Nurbs(patch.clone()).normal_at(mid).dot(want) < 0.0 {
            Patch {
                flipped: true,
                ..patch
            }
        } else {
            patch
        };
        let (f0, f1) = (m.add_vertex(far[0]), m.add_vertex(far[1]));
        let (e0, e1, e2) = (
            m.add_edge(o1, f1).unwrap(),
            m.add_edge(f1, f0).unwrap(),
            m.add_edge(f0, o0).unwrap(),
        );
        let (vs, es) = if forward {
            (vec![o0, o1, f1, f0], vec![shared, e0, e1, e2])
        } else {
            (vec![o1, o0, f0, f1], vec![shared, e2, e1, e0])
        };
        m.add_face(Face::with_edges(
            vs,
            vec![],
            Surface::Nurbs(patch),
            vec![es],
        ))
    };
    // As `tee`: f (x 0 to 1) and g (x -1 to 0) up, h standing, facing +x.
    let f = sheet(
        &mut m,
        [p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0)],
        false,
        DVec3::Z,
    );
    let g = sheet(
        &mut m,
        [p(-1.0, 0.0, 0.0), p(-1.0, 1.0, 0.0)],
        true,
        DVec3::Z,
    );
    let h = sheet(&mut m, [p(0.0, 0.0, 1.0), p(0.0, 1.0, 1.0)], true, DVec3::X);
    assert!(
        prism_ops::check::problems(&m).is_empty(),
        "{:?}",
        prism_ops::check::problems(&m)
    );
    let none = BTreeSet::new();
    let _ = (g, h);
    for d in [0.1f64, -0.1] {
        assert_eq!(
            shelling::refusal(&m, &[f].into(), &none, d),
            Some(shelling::BRANCHES),
            "by {d}"
        );
    }
}

/// A box 2 by 1 by 1, its faces facing out, with a wall across its middle
/// (x = 1): each of the wall's four edges has three faces.
fn parted_box() -> Model {
    let p = DVec3::new;
    let mut m = Model::new();
    let f = poly(
        &mut m,
        &[
            p(0.0, 0.0, 0.0),
            p(2.0, 0.0, 0.0),
            p(2.0, 1.0, 0.0),
            p(0.0, 1.0, 0.0),
        ],
    )[0];
    let up = if m.face(f).unwrap().plane().unwrap().normal.z > 0.0 {
        1.0
    } else {
        -1.0
    };
    prism_ops::pushpull::push_pull(&mut m, f, up, false);
    assert!((volume(&m) - 2.0).abs() < 1e-9, "faces out");
    poly(
        &mut m,
        &[
            p(1.0, 0.0, 0.0),
            p(1.0, 1.0, 0.0),
            p(1.0, 1.0, 1.0),
            p(1.0, 0.0, 1.0),
        ],
    );
    assert_eq!(m.face_count(), 11);
    m
}

/// A box with a wall inside it: each half is one closed box (one half
/// and the wall) or an open one (the other half), each shelled on its
/// own, then joined. By hand, `t` the thickness: inward, the two halves'
/// shells only touch at the wall, so it is the closed half hollowed (1 -
/// (1 - 2t)^3) and the open half hollowed (1 - (1 - t)(1 - 2t)^2);
/// outward, they overlap where the wall's offset meets the open half's
/// grown walls round its open end (the ring round it by t).
#[test]
fn a_box_with_a_wall_inside_shells_both_ways() {
    let none = BTreeSet::new();
    for t in [0.1f64, 0.05] {
        let inward = 1.0 - (1.0 - 2.0 * t).powi(3) + 1.0 - (1.0 - t) * (1.0 - 2.0 * t).powi(2);
        let ring = (1.0 + 2.0 * t).powi(2) - 1.0;
        let outward =
            (1.0 + 2.0 * t).powi(3) - 1.0 + (1.0 + t) * (1.0 + 2.0 * t).powi(2) - 1.0 - t * ring;
        for (d, want) in [(-t, inward), (t, outward)] {
            let m = parted_box();
            let all = any_face(&m);
            let built = shelling::slab_flagged(&m, &all, &none, d, &Default::default())
                .unwrap_or_else(|e| panic!("by {d}: {e}"));
            sound(&built.model);
            assert_eq!(built.replaced.len(), 11);
            near(volume(&built.model).abs(), want);
            assert!(built.flags.contains_key(shelling::BRANCHED));
        }
    }
}

/// A flat face bounded by a freeform curve (from x = 0 to 2 along y = 0,
/// bulging out and in) and three straight sides up to y = 1, and its area
/// by hand (the rectangle less the curve's own signed area under y = 0,
/// by the trapezium rule on many samples of it).
fn spline_bounded() -> (Model, FaceId, f64) {
    let p = DVec3::new;
    let mut c = Model::new();
    let curve = prism_geom::Nurbs::open(
        &[
            p(0.0, 0.0, 0.0),
            p(0.5, 0.8, 0.0),
            p(1.5, -0.5, 0.0),
            p(2.0, 0.0, 0.0),
        ],
        3,
    )
    .unwrap();
    prism_ops::sticky::add_spline(&mut c, &curve);
    let corners = [
        p(2.0, 0.0, 0.0),
        p(2.0, 1.0, 0.0),
        p(0.0, 1.0, 0.0),
        p(0.0, 0.0, 0.0),
    ];
    for w in corners.windows(2) {
        add_segment(&mut c, w[0], w[1]);
    }
    assert_eq!(c.face_count(), 1);
    let f = c.faces().map(|(id, _)| id).next().unwrap();
    let n = 20000;
    let mut under = 0.0;
    for i in 0..n {
        let (a, b) = (
            curve.eval(i as f64 / n as f64),
            curve.eval((i + 1) as f64 / n as f64),
        );
        under += (b.x - a.x) * (a.y + b.y) * 0.5;
    }
    (c, f, 2.0 - under)
}

/// A face bounded by a freeform curve used to be refused (`FREEFORM`).
/// Now the curve's offset is fitted through points moved along the face's
/// normal and the wall between them is ruled, as a freeform face's rim
/// is: the slab is the face's area by the thickness, either way.
#[test]
fn a_face_bounded_by_a_freeform_curve_is_thickened() {
    let none = BTreeSet::new();
    for d in [0.1, -0.1, 0.01] {
        let (mut c, f, area) = spline_bounded();
        let seeds: BTreeSet<FaceId> = [f].into();
        assert_eq!(shelling::refusal(&c, &seeds, &none, d), None, "by {d}");
        let (s, replaced) = shelling::slab(&c, &seeds, &none, d).unwrap();
        sound(&s);
        assert_eq!(replaced, seeds);
        assert!(
            (volume(&s).abs() - area * d.abs()).abs() < 1e-6 * d.abs(),
            "by {d}: {} vs {}",
            volume(&s),
            area * d
        );
        // The wall along the curve is ruled between it and its offset.
        assert!(
            s.faces()
                .any(|(_, f)| matches!(f.surface, prism_geom::Surface::Nurbs(_)))
        );
        shelling::shell(&mut c, &seeds, &none, d).unwrap();
        sound(&c);
    }
}

/// The same face with a flat wall standing on its straight far side (y =
/// 1, up to z = 1): the wall's fold with it is mitred as any flat fold is
/// (by hand, as the L of two faces: the faces' area by the thickness, less
/// or more the fold's length by its square), while the freeform rim gets
/// its fitted wall.
#[test]
fn a_freeform_rim_and_a_mitred_fold_together() {
    let p = DVec3::new;
    let none = BTreeSet::new();
    let (c0, f, area) = spline_bounded();
    let mut c = c0.clone();
    for w in [
        p(2.0, 1.0, 0.0),
        p(2.0, 1.0, 1.0),
        p(0.0, 1.0, 1.0),
        p(0.0, 1.0, 0.0),
    ]
    .windows(2)
    {
        add_segment(&mut c, w[0], w[1]);
    }
    assert_eq!(c.face_count(), 2);
    let seeds: BTreeSet<FaceId> = [f].into();
    let mut got = Vec::new();
    for d in [0.1f64, -0.1] {
        let (s, _) = shelling::slab(&c, &seeds, &none, d).unwrap_or_else(|e| panic!("{d}: {e}"));
        sound(&s);
        let v = volume(&s).abs();
        let (less, more) = (
            (area + 2.0) * d.abs() - 2.0 * d * d,
            (area + 2.0) * d.abs() + 2.0 * d * d,
        );
        assert!(
            (v - less).abs() < 1e-6 || (v - more).abs() < 1e-6,
            "by {d}: {v} vs {less} or {more}"
        );
        got.push((v - less).abs() < 1e-6);
    }
    assert_ne!(got[0], got[1], "one way into the fold, the other out of it");
}

/// Two squares side by side (x 0 to 2), the second turned to face the
/// other way from the first, and the two.
fn mixed_pair() -> (Model, FaceId, FaceId) {
    let p = DVec3::new;
    let (mut m, f) = square();
    let g = poly(
        &mut m,
        &[
            p(1.0, 0.0, 0.0),
            p(2.0, 0.0, 0.0),
            p(2.0, 1.0, 0.0),
            p(1.0, 1.0, 0.0),
        ],
    )[0];
    let normal = |m: &Model, x: FaceId| m.face(x).unwrap().plane().unwrap().normal;
    if normal(&m, f).dot(normal(&m, g)) > 0.0 {
        m.edit_face(g, |x| x.reverse());
    }
    (m, f, g)
}

/// Faces that face different ways used to be refused (`MIXED`, "Solid >
/// Heal turns them"). Now they are turned to agree on a copy first, as
/// Heal would, and the shell is the one Heal then Shell would make; the
/// turned face's two copies are flagged.
#[test]
fn faces_that_face_different_ways_are_turned_to_agree_and_flagged() {
    let (m, f, g) = mixed_pair();
    let none = BTreeSet::new();
    assert_eq!(shelling::refusal(&m, &[f].into(), &none, 0.1), None);
    let built = shelling::slab_flagged(&m, &[f].into(), &none, 0.1, &Default::default()).unwrap();
    sound(&built.model);
    near(volume(&built.model).abs(), 0.2);
    assert_eq!(built.replaced, [f, g].into());
    // Heal turns the second square (the fewer), so its copies are flagged.
    let turned = &built.flags[shelling::TURNED];
    assert_eq!(turned.faces.len(), 2, "{:?}", built.flags);
    assert_eq!(built.flags.len(), 1);
    // The same as healing first, then shelling.
    let mut healed = m.clone();
    prism_ops::health::orient(&mut healed, None);
    let (plain, _) = shelling::slab(&healed, &[f].into(), &none, 0.1).unwrap();
    near(volume(&plain), volume(&built.model));
    // Flagged faces are both copies of the turned square: their corners
    // are over it.
    for id in &turned.faces {
        let c = built.model.face(*id).unwrap();
        assert!(
            c.outer
                .iter()
                .all(|v| built.model.vertex(*v).x >= 1.0 - 1e-9)
        );
    }
    // Joined into the model, the flags name faces of the model.
    let mut joined = m.clone();
    let flags =
        shelling::shell_flagged(&mut joined, &[f].into(), &none, 0.1, &Default::default()).unwrap();
    sound(&joined);
    assert_eq!(flags[shelling::TURNED].faces.len(), 2);
    assert!(
        flags[shelling::TURNED]
            .faces
            .iter()
            .all(|x| joined.face(*x).is_some())
    );
    // A body that already agrees is left as it is, with nothing flagged.
    let (sq, one) = square();
    let plain =
        shelling::slab_flagged(&sq, &[one].into(), &none, 0.1, &Default::default()).unwrap();
    assert!(plain.flags.is_empty());
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

// ---------- rim styles (brief 49 part 3) ----------

/// Two flat quads, each a NURBS patch (not simplified to a plane), side by
/// side in the plane y = 0 (x from 0 to 1 and 1 to 2, z from 0 to 1),
/// joined smoothly along x = 1; both face -y. Returns the two bottom rim
/// edges (z = 0) too.
fn flat_pair() -> (Model, FaceId, FaceId, [prism_topo::EdgeId; 2]) {
    use prism_geom::{NurbsSurface, Patch, Surface};
    use prism_topo::Face;

    let square = |x0: f64| {
        let k = vec![0.0, 0.0, 1.0, 1.0];
        NurbsSurface::new(
            [1, 1],
            [k.clone(), k],
            [2, 2],
            vec![
                DVec3::new(x0, 0.0, 0.0),
                DVec3::new(x0, 0.0, 1.0),
                DVec3::new(x0 + 1.0, 0.0, 0.0),
                DVec3::new(x0 + 1.0, 0.0, 1.0),
            ],
            vec![1.0; 4],
            [false, false],
        )
        .unwrap()
    };
    let mut m = Model::new();
    let v = |m: &mut Model, x: f64, z: f64| m.add_vertex(DVec3::new(x, 0.0, z));
    let (a0, a1, a2) = (
        v(&mut m, 0.0, 0.0),
        v(&mut m, 1.0, 0.0),
        v(&mut m, 2.0, 0.0),
    );
    let (b0, b1, b2) = (
        v(&mut m, 0.0, 1.0),
        v(&mut m, 1.0, 1.0),
        v(&mut m, 2.0, 1.0),
    );
    let (bottom_a, bottom_b) = (m.add_edge(a0, a1).unwrap(), m.add_edge(a1, a2).unwrap());
    let (right_b, top_b) = (m.add_edge(a2, b2).unwrap(), m.add_edge(b2, b1).unwrap());
    let (top_a, left_a) = (m.add_edge(b1, b0).unwrap(), m.add_edge(b0, a0).unwrap());
    let shared = m.add_edge(a1, b1).unwrap();
    let fa = m.add_face(Face::with_edges(
        vec![a0, a1, b1, b0],
        vec![],
        Surface::Nurbs(Patch::whole(square(0.0))),
        vec![vec![bottom_a, shared, top_a, left_a]],
    ));
    let fb = m.add_face(Face::with_edges(
        vec![a1, a2, b2, b1],
        vec![],
        Surface::Nurbs(Patch::whole(square(1.0))),
        vec![vec![bottom_b, right_b, top_b, shared]],
    ));
    (m, fa, fb, [bottom_a, bottom_b])
}

/// The plane through the x axis whose wall leaves the sheet's -y side
/// leaning `lean` (along z) for each unit of thickness: 0 is square.
fn leaning_plane(lean: f64) -> prism_math::Plane {
    // The wall's direction in (y, z) is (-1, lean); the plane's normal is
    // square to it and to x.
    prism_math::Plane::new(DVec3::ZERO, DVec3::new(0.0, lean, 1.0))
}

fn flush_both(edges: [prism_topo::EdgeId; 2], plane: prism_math::Plane) -> shelling::RimStyles {
    edges
        .into_iter()
        .map(|e| (e, shelling::RimStyle::Flush(plane)))
        .collect()
}

#[test]
fn a_flush_rim_square_to_the_sheet_is_the_square_rim() {
    let d = 0.1;
    let (m0, a, b, edges) = flat_pair();
    let body: BTreeSet<FaceId> = [a, b].into();
    let mut square = m0.clone();
    shelling::shell(&mut square, &body, &BTreeSet::new(), d).unwrap();
    sound(&square);
    let mut flush = m0.clone();
    shelling::shell_with(
        &mut flush,
        &body,
        &BTreeSet::new(),
        d,
        &flush_both(edges, leaning_plane(0.0)),
    )
    .unwrap();
    sound(&flush);
    near_enough(volume(&flush).abs(), volume(&square).abs());
    near_enough(volume(&flush).abs(), 2.0 * d);
}

#[test]
fn a_flush_rim_that_leans_over_the_sheet_trims_the_offset_back() {
    // The wall leans 0.5 along z for each unit of thickness, up the sheet:
    // by hand, the cross-section loses a triangle of legs d and d/2, over
    // a length of two.
    let d: f64 = 0.1;
    let (mut m, a, b, edges) = flat_pair();
    let body: BTreeSet<FaceId> = [a, b].into();
    let styles = flush_both(edges, leaning_plane(0.5));
    assert_eq!(
        shelling::refusal_with(&m, &body, &BTreeSet::new(), d, &styles),
        None
    );
    shelling::shell_with(&mut m, &body, &BTreeSet::new(), d, &styles).unwrap();
    sound(&m);
    near_enough(volume(&m).abs(), 2.0 * (d - 0.5 * d * d * 0.5));
    // The wall lies in the plane: every vertex of it on the bottom edge
    // line or on the plane.
    let plane = leaning_plane(0.5);
    let on_plane = m
        .faces()
        .filter(|(_, f)| {
            f.plane()
                .is_some_and(|p| p.normal.dot(plane.normal).abs() > 0.999999)
        })
        .count();
    assert_eq!(on_plane, 2, "one flush wall for each bottom edge");
}

#[test]
fn a_flush_rim_that_leans_away_reaches_past_the_sheets_edge() {
    // The wall leans 0.5 along z for each unit of thickness, away from the
    // sheet: the offset surface is carried on past the sheet's own edge to
    // meet the plane. By hand, the cross-section gains a triangle of legs d
    // and d/2, over a length of two; the sheet's ends stay cut square.
    let d: f64 = 0.1;
    let (mut m, a, b, edges) = flat_pair();
    let body: BTreeSet<FaceId> = [a, b].into();
    let styles = flush_both(edges, leaning_plane(-0.5));
    assert_eq!(
        shelling::refusal_with(&m, &body, &BTreeSet::new(), d, &styles),
        None
    );
    shelling::shell_with(&mut m, &body, &BTreeSet::new(), d, &styles).unwrap();
    sound(&m);
    near_enough(volume(&m).abs(), 2.0 * (d + 0.5 * d * d * 0.5));
    // The wall lies in the plane: one flush wall for each bottom edge.
    let plane = leaning_plane(-0.5);
    let on_plane = m
        .faces()
        .filter(|(_, f)| {
            f.plane()
                .is_some_and(|p| p.normal.dot(plane.normal).abs() > 0.999999)
        })
        .count();
    assert_eq!(on_plane, 2, "one flush wall for each bottom edge");
}

#[test]
fn a_flush_rim_reaching_a_long_way_past_the_edge_still_says_so() {
    // A plane almost along the sheet needs the offset carried on past any
    // reasonable distance: refused in the kernel's words.
    let (m, a, b, edges) = flat_pair();
    let body: BTreeSet<FaceId> = [a, b].into();
    let styles = flush_both(edges, leaning_plane(-40.0));
    assert_eq!(
        shelling::refusal_with(&m, &body, &BTreeSet::new(), 0.1, &styles),
        Some(shelling::FLUSH_REACH)
    );
}

/// One flat NURBS quad alone (x 0 to 1, z 0 to 1, in y = 0), and its
/// bottom edge.
fn lone_sheet() -> (Model, FaceId, prism_topo::EdgeId) {
    use prism_geom::{NurbsSurface, Patch, Surface};
    use prism_topo::Face;
    let k = vec![0.0, 0.0, 1.0, 1.0];
    let surface = NurbsSurface::new(
        [1, 1],
        [k.clone(), k],
        [2, 2],
        vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 1.0),
        ],
        vec![1.0; 4],
        [false, false],
    )
    .unwrap();
    let mut m = Model::new();
    let a0 = m.add_vertex(DVec3::new(0.0, 0.0, 0.0));
    let a1 = m.add_vertex(DVec3::new(1.0, 0.0, 0.0));
    let b1 = m.add_vertex(DVec3::new(1.0, 0.0, 1.0));
    let b0 = m.add_vertex(DVec3::new(0.0, 0.0, 1.0));
    let (bottom, right) = (m.add_edge(a0, a1).unwrap(), m.add_edge(a1, b1).unwrap());
    let (top, left) = (m.add_edge(b1, b0).unwrap(), m.add_edge(b0, a0).unwrap());
    let f = m.add_face(Face::with_edges(
        vec![a0, a1, b1, b0],
        vec![],
        Surface::Nurbs(Patch::whole(surface)),
        vec![vec![bottom, right, top, left]],
    ));
    (m, f, bottom)
}

#[test]
fn a_lone_freeform_face_takes_a_chosen_flush_rim_both_ways() {
    // Leaning over the sheet trims the wall back; leaning away carries the
    // offset on past the sheet's edge: by hand, cross-sections of
    // d - lean d^2/2 and d + lean d^2/2, over a length of one.
    let d: f64 = 0.1;
    for (lean, sign) in [(0.5, -1.0), (-0.5, 1.0)] {
        let (mut m, f, bottom) = lone_sheet();
        let body: BTreeSet<FaceId> = [f].into();
        let styles: shelling::RimStyles =
            [(bottom, shelling::RimStyle::Flush(leaning_plane(lean)))].into();
        shelling::shell_with(&mut m, &body, &BTreeSet::new(), d, &styles).unwrap();
        sound(&m);
        near_enough(volume(&m).abs(), d + sign * 0.5 * d * d * 0.5);
    }
    // The program picks nothing for a lone face: the sheet's bottom edge
    // lies in the ground plane and stays square.
    let (m, f, _) = lone_sheet();
    let body: BTreeSet<FaceId> = [f].into();
    assert!(shelling::default_rims(&m, &body, &BTreeSet::new(), d).is_empty());
}

/// A flush plane that cannot end its rim (it misses the edge, or lies
/// along the face) used to refuse the whole shell (`FLUSH_OFF`,
/// `FLUSH_PARALLEL`). Now that rim is cut square instead, flagged, and
/// the shell is the square one.
#[test]
fn a_flush_plane_that_cannot_end_its_rim_is_cut_square_and_flagged() {
    let (m, a, b, edges) = flat_pair();
    let body: BTreeSet<FaceId> = [a, b].into();
    let none = BTreeSet::new();
    let (square, _) = shelling::slab(&m, &body, &none, 0.1).unwrap();
    let off = prism_math::Plane::new(DVec3::new(0.0, 0.0, 0.2), DVec3::new(0.0, 0.5, 1.0));
    let along = prism_math::Plane::new(DVec3::ZERO, DVec3::Y);
    for (plane, why) in [
        (off, shelling::SQUARED_OFF),
        (along, shelling::SQUARED_ALONG),
    ] {
        let styles = flush_both(edges, plane);
        assert_eq!(shelling::refusal_with(&m, &body, &none, 0.1, &styles), None);
        let built = shelling::slab_flagged(&m, &body, &none, 0.1, &styles).unwrap();
        sound(&built.model);
        near_enough(volume(&built.model).abs(), volume(&square).abs());
        assert_eq!(built.flags.len(), 1, "{:?}", built.flags);
        // One wall for each of the two bottom edges, with its edges.
        let found = &built.flags[why];
        assert_eq!(found.faces.len(), 2, "{found:?}");
        assert!(found.edges.len() >= 4, "{found:?}");
        // The walls are the square ones: square to the sheet (in y = 0).
        for f in &found.faces {
            let wall = built.model.face(*f).unwrap();
            assert!(
                wall.outer
                    .iter()
                    .all(|v| built.model.vertex(*v).z.abs() < 1e-9)
            );
        }
    }
    // The program never picks a plane that would be cut square.
    let picked = shelling::default_rims(&m, &body, &none, 0.1);
    assert!(picked.values().all(|s| match s {
        shelling::RimStyle::Flush(p) => !p.normal.abs().abs_diff_eq(DVec3::Y, 1e-9),
        shelling::RimStyle::Square => true,
    }));
}

/// A rim style the shape cannot take at all (a flush rim on flat faces
/// alone) used to refuse the whole shell (`RIM_STYLE`). Now that rim is
/// cut square instead, flagged, and the rest is built.
#[test]
fn a_rim_style_on_a_shape_that_does_not_support_it_is_cut_square_and_flagged() {
    let (m, f) = square();
    let none = BTreeSet::new();
    // The bottom edge (y = 0), and a plane through it leaning across it.
    let bottom = m
        .edges()
        .find(|(_, e)| m.vertex(e.a).y.abs() < 1e-9 && m.vertex(e.b).y.abs() < 1e-9)
        .map(|(id, _)| id)
        .unwrap();
    let plane = prism_math::Plane::new(DVec3::ZERO, DVec3::new(0.0, 1.0, 0.5));
    let styles: shelling::RimStyles = [(bottom, shelling::RimStyle::Flush(plane))].into();
    assert_eq!(
        shelling::refusal_with(&m, &[f].into(), &none, 0.1, &styles),
        None
    );
    let built = shelling::slab_flagged(&m, &[f].into(), &none, 0.1, &styles).unwrap();
    sound(&built.model);
    near(volume(&built.model).abs(), 0.1);
    let found = &built.flags[shelling::SQUARED];
    assert_eq!(found.faces.len(), 1, "{:?}", built.flags);
    // Every edge of the square named with a plane that lies along it: each
    // is cut square, named for that reason instead.
    let along: shelling::RimStyles = m
        .edges()
        .map(|(e, _)| (e, shelling::RimStyle::Flush(leaning_plane(0.0))))
        .collect();
    let built = shelling::slab_flagged(&m, &[f].into(), &none, 0.1, &along).unwrap();
    assert_eq!(built.flags[shelling::SQUARED_ALONG].faces.len(), 4);
    // A thickness that cannot be built at all is still refused for that.
    let cube = cube();
    let all = any_face(&cube);
    let any_edge: shelling::RimStyles = cube
        .edges()
        .map(|(e, _)| (e, shelling::RimStyle::Flush(plane)))
        .collect();
    assert_eq!(
        shelling::refusal_with(&cube, &all, &none, -0.6, &any_edge),
        Some(TOO_THICK)
    );
}

#[test]
fn boundaries_are_runs_between_corners_and_a_straight_one_has_no_plane() {
    let (m, a, b, [bottom_a, bottom_b]) = flat_pair();
    let body: BTreeSet<FaceId> = [a, b].into();
    let runs = shelling::boundaries(&m, &body, &BTreeSet::new());
    // The sheet's rim: the bottom (two edges in a line), the right side,
    // the top (two edges) and the left side.
    assert_eq!(runs.len(), 4, "{runs:?}");
    let bottom = runs.iter().find(|r| r.contains(&bottom_a)).unwrap();
    assert!(bottom.contains(&bottom_b), "{bottom:?}");
    let tol = m.tol();
    assert_eq!(shelling::boundary_plane(&m, bottom, &tol), None);
    // Open one face: only the kept face's own edges are rims, the one it
    // shares with the open face included.
    let open: BTreeSet<FaceId> = [b].into();
    let runs = shelling::boundaries(&m, &body, &open);
    assert_eq!(runs.iter().flatten().count(), 4, "{runs:?}");
    assert!(runs.iter().flatten().all(|e| *e != bottom_b));
}

/// A sheet folded right back on itself (a hairpin): `a` flat (x 0 to 1,
/// in z = 0, facing up, +z), and `b` leaving their shared edge (x = 0, y 0
/// to 1) along +x too, tangent to `a` there, then curving up to z = 1 at
/// x = 1 (a quadratic). They face opposite ways at the fold (a sheet
/// folded back keeps its one side facing into the fold), so the fold is
/// as sharp as a fold can be: no mitre exists there.
fn hairpin() -> (Model, FaceId, FaceId) {
    use prism_geom::{NurbsSurface, Patch, Surface};
    use prism_topo::Face;
    let p = DVec3::new;
    let k1 = vec![0.0, 0.0, 1.0, 1.0];
    let k2 = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let sa = NurbsSurface::new(
        [1, 1],
        [k1.clone(), k1.clone()],
        [2, 2],
        vec![
            p(0.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(1.0, 1.0, 0.0),
        ],
        vec![1.0; 4],
        [false, false],
    )
    .unwrap();
    let sb = NurbsSurface::new(
        [2, 1],
        [k2, k1],
        [3, 2],
        vec![
            p(0.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(0.5, 0.0, 0.0),
            p(0.5, 1.0, 0.0),
            p(1.0, 0.0, 1.0),
            p(1.0, 1.0, 1.0),
        ],
        vec![1.0; 6],
        [false, false],
    )
    .unwrap();
    // Each patch turned to face the way its loop goes round.
    let facing = |s: NurbsSurface, at: DVec3, want: DVec3| {
        let patch = Patch::whole(s);
        if Surface::Nurbs(patch.clone()).normal_at(at).dot(want) < 0.0 {
            Patch {
                flipped: !patch.flipped,
                ..patch
            }
        } else {
            patch
        }
    };
    // Fine precision (0.001 mm), the precision the fold cases were written
    // at: at Standard, thickening 0.02 into the fold is not recognised as
    // one and is bridged instead (tolerance stage 2, NOTES 2026-11-26).
    let mut m = Model::with_tol(prism_math::Tol::new(prism_math::tol::FINE));
    let (p0, p1) = (
        m.add_vertex(p(0.0, 0.0, 0.0)),
        m.add_vertex(p(0.0, 1.0, 0.0)),
    );
    let (q0, q1) = (
        m.add_vertex(p(1.0, 0.0, 0.0)),
        m.add_vertex(p(1.0, 1.0, 0.0)),
    );
    let (r0, r1) = (
        m.add_vertex(p(1.0, 0.0, 1.0)),
        m.add_vertex(p(1.0, 1.0, 1.0)),
    );
    let fold = m.add_edge(p1, p0).unwrap();
    let (a0, a1, a2) = (
        m.add_edge(p0, q0).unwrap(),
        m.add_edge(q0, q1).unwrap(),
        m.add_edge(q1, p1).unwrap(),
    );
    // Round +z: p0, q0, q1, p1 (and back along the fold).
    let a = m.add_face(Face::with_edges(
        vec![p0, q0, q1, p1],
        vec![],
        Surface::Nurbs(facing(sa, p(0.5, 0.5, 0.0), DVec3::Z)),
        vec![vec![a0, a1, a2, fold]],
    ));
    let b_curve = |m: &mut Model, from: prism_topo::VertexId, to: prism_topo::VertexId, y: f64| {
        let c =
            prism_geom::Nurbs::open(&[p(0.0, y, 0.0), p(0.5, y, 0.0), p(1.0, y, 1.0)], 2).unwrap();
        let c = if from == p0 || from == p1 {
            c
        } else {
            c.reversed()
        };
        m.add_curve_edge(
            from,
            to,
            prism_geom::Curve::Spline(prism_geom::Spline::whole(c)),
            None,
        )
        .unwrap()
    };
    // The other way along the fold: p0, p1, r1, r0.
    let b1 = b_curve(&mut m, p1, r1, 1.0);
    let b2 = m.add_edge(r1, r0).unwrap();
    let b3 = b_curve(&mut m, r0, p0, 0.0);
    let b = m.add_face(Face::with_edges(
        vec![p0, p1, r1, r0],
        vec![],
        Surface::Nurbs(facing(sb, p(0.05, 0.5, 0.0), -DVec3::Z)),
        vec![vec![fold, b1, b2, b3]],
    ));
    (m, a, b)
}

/// Every edge of `s` walked once each way by the two faces that use it.
fn walked_both_ways(s: &Model) {
    let mut from: std::collections::BTreeMap<prism_topo::EdgeId, Vec<prism_topo::VertexId>> =
        Default::default();
    for (_, f) in s.faces() {
        for side in f.sides() {
            from.entry(side.edge).or_default().push(side.from);
        }
    }
    for (e, ends) in from {
        assert!(
            ends.len() != 2 || ends[0] != ends[1],
            "{e:?} walked one way twice"
        );
    }
}

/// A sheet folded right back on itself used to be refused at the fold's
/// ends (`CORNER`: no corner both faces' offsets share) or along it
/// (`SHARP`, before even trying). Thickened away from the fold, the two
/// offsets are parallel at the fold and never meet: a gap, bridged as an
/// outside corner's is, but out round the fold through a middle line a
/// thickness past it (two strips, so the bridge keeps a thickness at the
/// fold instead of passing through it). By hand: the flat face's slab
/// (its area by the thickness), the curved face's (its length, the
/// parabola z = x^2 to x = 1, by the thickness, less its turning, atan 2,
/// by half the thickness squared: it moves towards its own centre of
/// curvature), and the bridge's triangle (two thicknesses across, one
/// out) along the fold's length of 1.
#[test]
fn a_sheet_folded_right_back_is_bridged_round_the_fold() {
    let (m, a, b) = hairpin();
    let body: BTreeSet<FaceId> = [a, b].into();
    let none = BTreeSet::new();
    let length = 0.5 * 5f64.sqrt() + 0.25 * (2.0 + 5f64.sqrt()).ln();
    for t in [0.1f64, 0.02] {
        let d = -t;
        assert_eq!(shelling::refusal(&m, &body, &none, d), None, "by {d}");
        let built = shelling::slab_flagged(&m, &body, &none, d, &Default::default()).unwrap();
        sound(&built.model);
        walked_both_ways(&built.model);
        let want = t + (length * t - 2f64.atan() * t * t * 0.5) + t * t;
        near_enough(volume(&built.model).abs(), want);
        // The two strips and the two triangles closing each end of them.
        let bridged = &built.flags[shelling::BRIDGED];
        assert_eq!(bridged.faces.len(), 6, "{:?}", built.flags);
        assert_eq!(built.flags.len(), 1);
        let mut joined = m.clone();
        shelling::shell(&mut joined, &body, &none, d).unwrap();
        sound(&joined);
    }
    // Into the fold, the offsets cross well away from it, where there is no
    // straight line up from the fold inside the solid to part the two
    // faces' end walls at: still refused, by name.
    for d in [0.1, 0.02] {
        assert_eq!(
            shelling::refusal(&m, &body, &none, d),
            Some(shelling::SHARP),
            "by {d}"
        );
    }
}

/// Owner, 2026-11-26: "just give everything a warning". What cannot be
/// shelled as one piece is still built, face by face, and flagged with why;
/// only nothing to build is refused.
#[test]
fn what_cannot_be_shelled_as_one_piece_is_built_face_by_face_with_a_warning() {
    let none = BTreeSet::new();
    // Walls thicker than half the cube: they cross (once refused, too
    // thick). Face by face, the six slabs fill the cube.
    let m = cube();
    let built = shelling::slab_flagged(&m, &any_face(&m), &none, -0.6, &Default::default())
        .expect("built anyway");
    assert!(
        built.flags.contains_key(shelling::APART_THICK),
        "{:?}",
        built.flags.keys()
    );
    assert!(prism_ops::boolean::is_solid(&built.model));
    // Four faces at a rim corner (once refused, `CORNER`).
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
    let mut joined = m.clone();
    let flags = shelling::shell_flagged(
        &mut joined,
        &any_face(&m),
        &[side].into(),
        -0.05,
        &Default::default(),
    )
    .expect("built anyway");
    assert!(
        flags.contains_key(shelling::APART_CORNER),
        "{:?}",
        flags.keys()
    );
    assert!(!flags[shelling::APART_CORNER].faces.is_empty(), "marked");
    // Nothing to build is still refused.
    let (m, f) = square();
    let one: BTreeSet<FaceId> = [f].into();
    assert_eq!(shelling::slab(&m, &one, &none, 0.0).err(), Some(ZERO));
    assert_eq!(shelling::slab(&m, &one, &one, 0.1).err(), Some(ALL_OPEN));
}
