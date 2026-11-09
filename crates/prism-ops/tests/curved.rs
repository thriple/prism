//! Curved faces in the core (Exact Curves 4a): a cylinder face keeps the
//! model check clean, moves whole and exactly, and the operations that
//! need flat faces refuse or skip it.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::{Curve, Cylinder, Surface};
use prism_math::{DAffine3, DVec3};
use prism_ops::transform::{ARC_WOULD_BEND, CURVED_WOULD_BEND, arcs_refusal, turn};
use prism_ops::{add_circle, add_segment, check};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

/// A cylinder of radius `r` and height `h` on the ground, built by hand
/// (Push/Pull makes them from Exact Curves 4b): two discs, two seams, and
/// two cylinder faces facing out.
fn cylinder(r: f64, h: f64) -> Model {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, r);
    add_circle(&mut m, DVec3::Z * h, DVec3::Z, r);
    let at = |m: &Model, z: f64| -> Vec<VertexId> {
        m.vertices()
            .filter(|(_, p)| (p.z - z).abs() < 1e-9)
            .map(|(id, _)| id)
            .collect()
    };
    let (bottom, top) = (at(&m, 0.0), at(&m, h));
    let above = |m: &Model, b: VertexId| -> VertexId {
        let p = m.vertex(b);
        *top.iter()
            .find(|t| m.vertex(**t).truncate().distance(p.truncate()) < 1e-9)
            .expect("a seam")
    };
    let (b0, b1) = (bottom[0], bottom[1]);
    let (t0, t1) = (above(&m, b0), above(&m, b1));
    let seams = [(m.vertex(b0), m.vertex(t0)), (m.vertex(b1), m.vertex(t1))];
    for (p, q) in seams {
        add_segment(&mut m, p, q);
    }
    // The arc between `a` and `b` that runs counter-clockwise about +Z
    // from `a`.
    let arc = |m: &Model, a: VertexId, b: VertexId| -> EdgeId {
        m.edges()
            .find(|(_, e)| {
                e.has(a) && e.has(b) && {
                    let Curve::Arc { axis, .. } = e.curve else {
                        return false;
                    };
                    (if e.a == a { axis } else { -axis }).z > 0.0
                }
            })
            .map(|(id, _)| id)
            .expect("an arc")
    };
    let line = |m: &Model, a: VertexId, b: VertexId| -> EdgeId {
        m.edges()
            .find(|(_, e)| e.has(a) && e.has(b) && !e.curve.is_arc())
            .map(|(id, _)| id)
            .expect("a seam")
    };
    let side = Surface::Cylinder(Cylinder {
        origin: DVec3::ZERO,
        axis: DVec3::Z,
        radius: r,
        outward: true,
    });
    for (a, b, ta, tb) in [(b0, b1, t0, t1), (b1, b0, t1, t0)] {
        // Round the bottom, up a seam, back round the top, down.
        let edges = vec![
            arc(&m, a, b),
            line(&m, b, tb),
            arc(&m, ta, tb),
            line(&m, ta, a),
        ];
        m.add_face(Face::with_edges(
            vec![a, b, tb, ta],
            vec![],
            side,
            vec![edges],
        ));
    }
    m
}

fn sides(m: &Model) -> Vec<FaceId> {
    m.faces()
        .filter(|(_, f)| f.is_curved())
        .map(|(id, _)| id)
        .collect()
}

fn all(m: &Model) -> BTreeSet<VertexId> {
    m.vertices().map(|(id, _)| id).collect()
}

fn cylinder_of(m: &Model, f: FaceId) -> Cylinder {
    match m.face(f).expect("face").surface {
        Surface::Cylinder(c) => c,
        Surface::Plane(_) => panic!("flat"),
    }
}

#[test]
fn a_cylinder_built_by_hand_is_clean_with_exact_areas() {
    let m = cylinder(1.5, 2.0);
    assert_eq!(check::problems(&m), Vec::<String>::new());
    let s = sides(&m);
    assert_eq!(s.len(), 2);
    for f in s {
        let a = m.face_area(f);
        assert!((a - PI * 1.5 * 2.0).abs() < 1e-12, "half the side: {a}");
    }
    let r = prism_ops::health::check(&m);
    assert!(r.off_circle.is_empty());
    // The check can fail: a side facing in, against its loop.
    let mut bad = m.clone();
    let f = sides(&bad)[0];
    bad.edit_face(f, |x| x.surface = x.surface.flipped());
    let p = check::problems(&bad);
    assert!(p.iter().any(|x| x.contains("winds against")), "{p:?}");
}

#[test]
fn moves_turns_mirrors_and_even_scales_keep_cylinders_exact() {
    use prism_ops::transform::{rotate, scale, scaling, translate};
    type Run = fn(&mut Model, &BTreeSet<VertexId>);
    let (c, axis, k) = (DVec3::new(0.3, 0.0, 0.0), DVec3::new(1.0, 1.0, 0.2), 2.5);
    let moves: [(DAffine3, Run); 4] = [
        (
            DAffine3::from_translation(DVec3::new(1.0, -2.0, 0.5)),
            |m, vs| translate(m, vs, DVec3::new(1.0, -2.0, 0.5)),
        ),
        (turn(c, axis, 0.7), |m, vs| {
            rotate(
                m,
                vs,
                DVec3::new(0.3, 0.0, 0.0),
                DVec3::new(1.0, 1.0, 0.2),
                0.7,
            )
        }),
        (
            scaling(DVec3::new(0.5, 0.5, 0.0), DVec3::splat(k)),
            |m, vs| scale(m, vs, DVec3::new(0.5, 0.5, 0.0), DVec3::splat(2.5)),
        ),
        (scaling(DVec3::ZERO, DVec3::new(-1.0, 1.0, 1.0)), |m, vs| {
            scale(m, vs, DVec3::ZERO, DVec3::new(-1.0, 1.0, 1.0))
        }),
    ];
    for (m, run) in moves {
        let mut model = cylinder(1.0, 3.0);
        let vs = all(&model);
        let before = cylinder_of(&model, sides(&model)[0]);
        assert_eq!(arcs_refusal(&model, &vs, &m), None);
        run(&mut model, &vs);
        assert_eq!(check::problems(&model), Vec::<String>::new(), "{m:?}");
        let after = cylinder_of(&model, sides(&model)[0]);
        let k = prism_geom::even_scale(&m).expect("even");
        assert!((after.radius - before.radius * k).abs() < 1e-12);
        assert!(after.axis.cross(m.transform_vector3(before.axis)).length() < 1e-12);
    }
}

#[test]
fn whole_models_placed_by_a_mirror_keep_cylinders_clean() {
    let mut model = cylinder(1.0, 2.0);
    prism_ops::transform::affine(
        &mut model,
        &prism_ops::transform::scaling(DVec3::ZERO, DVec3::new(1.0, -1.0, 1.0)),
    );
    assert_eq!(check::problems(&model), Vec::<String>::new());
}

#[test]
fn bending_a_cylinder_is_refused() {
    let model = cylinder(1.0, 2.0);
    let top: BTreeSet<VertexId> = model
        .vertices()
        .filter(|(_, p)| p.z > 1.0)
        .map(|(id, _)| id)
        .collect();
    let up = DAffine3::from_translation(DVec3::Z);
    assert_eq!(arcs_refusal(&model, &top, &up), Some(CURVED_WOULD_BEND));
    let squash = prism_ops::transform::scaling(DVec3::ZERO, DVec3::new(2.0, 1.0, 1.0));
    assert_eq!(
        arcs_refusal(&model, &all(&model), &squash),
        Some(ARC_WOULD_BEND)
    );
}

#[test]
fn push_pull_and_offset_refuse_curved_faces_but_not_their_caps() {
    let mut model = cylinder(1.0, 2.0);
    let side = sides(&model)[0];
    let cap = model
        .faces()
        .find(|(_, f)| !f.is_curved())
        .map(|(id, _)| id)
        .expect("a cap");
    use prism_ops::pushpull::{CURVED_PULL, Pulled, push_pull, refusal};
    assert_eq!(refusal(&model, side), Some(CURVED_PULL));
    // A cap pulls along its sides: they only stretch (4b).
    assert_eq!(refusal(&model, cap), None);
    let before = model.clone();
    assert!(matches!(
        push_pull(&mut model, side, 0.5, false),
        Pulled::Nothing
    ));
    assert_eq!(prism_ops::offset::offset(&mut model, side, 0.1), None);
    assert_eq!(model.to_json(), before.to_json());
}

#[test]
fn intersect_faces_skips_curved_faces() {
    let mut model = cylinder(1.0, 2.0);
    // An upright square through the cylinder.
    for (p, q) in [
        (DVec3::new(-2.0, 0.3, -1.0), DVec3::new(2.0, 0.3, -1.0)),
        (DVec3::new(2.0, 0.3, -1.0), DVec3::new(2.0, 0.3, 3.0)),
        (DVec3::new(2.0, 0.3, 3.0), DVec3::new(-2.0, 0.3, 3.0)),
        (DVec3::new(-2.0, 0.3, 3.0), DVec3::new(-2.0, 0.3, -1.0)),
    ] {
        add_segment(&mut model, p, q);
    }
    prism_ops::intersect::intersect_faces(&mut model, None);
    assert_eq!(sides(&model).len(), 2, "the sides are not cut");
    assert!(model.bookkeeping_problems().is_empty());
}

#[test]
fn model_health_finds_and_repairs_points_off_a_cylinder() {
    let mut m = cylinder(1.0, 2.0);
    let (v, p) = m.vertices().find(|(_, p)| p.z > 1.0).expect("a top point");
    m.set_vertex(v, p * DVec3::new(1.01, 1.01, 1.0));
    let r = prism_ops::health::check(&m);
    assert_eq!(r.off_circle.faces.len(), 2, "both sides");
    assert!(prism_ops::health::repair_arcs(&mut m) > 0);
    assert!(prism_ops::health::check(&m).off_circle.is_empty());
    assert_eq!(check::problems(&m), Vec::<String>::new());
}

// ---------- Cylinders from Push/Pull (Exact Curves 4b) ----------

/// The volume inside the model's faces, by the divergence theorem: a flat
/// face adds area × (normal · a point) / 3; a cylinder face whose axis
/// passes through the origin adds area × radius / 3 (facing out).
fn volume(m: &Model) -> f64 {
    m.faces()
        .map(|(id, f)| {
            let a = m.face_area(id);
            match f.surface {
                Surface::Plane(p) => a * p.normal.dot(p.origin) / 3.0,
                Surface::Cylinder(c) => {
                    assert!(c.origin.cross(c.axis).length() < 1e-12, "axis through 0");
                    let s = if c.outward { 1.0 } else { -1.0 };
                    s * a * c.radius / 3.0
                }
            }
        })
        .sum()
}

fn disc(r: f64) -> (Model, FaceId) {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, r);
    let f = m.faces().next().map(|(id, _)| id).expect("a disc");
    (m, f)
}

fn clean(m: &Model) {
    assert_eq!(check::problems(m), Vec::<String>::new());
    let r = prism_ops::health::check(m);
    assert!(r.off_circle.is_empty(), "{r:?}");
}

#[test]
fn a_disc_pulled_is_a_closed_cylinder() {
    use prism_ops::pushpull::{Pulled, push_pull};
    let (r, h) = (1.5, 2.0);
    for d in [h, -h] {
        let (mut m, f) = disc(r);
        let pulled = push_pull(&mut m, f, d, false);
        assert!(matches!(pulled, Pulled::Extruded { .. }), "{pulled:?}");
        clean(&m);
        assert_eq!(sides(&m).len(), 2, "two half cylinders");
        assert_eq!(m.face_count(), 4);
        for s in sides(&m) {
            assert!((m.face_area(s) - PI * r * h).abs() < 1e-9);
            let c = cylinder_of(&m, s);
            assert!((c.radius - r).abs() < 1e-12 && c.axis.cross(DVec3::Z).length() < 1e-12);
        }
        let health = prism_ops::health::check(&m);
        assert_eq!((health.watertight, health.leaky), (1, 0));
        assert!(health.reversed.is_empty(), "{health:?}");
        let v = volume(&m);
        assert!((v - PI * r * r * h).abs() < 1e-9, "{v}");
    }
}

#[test]
fn pulling_a_cylinders_cap_again_lengthens_it() {
    use prism_ops::pushpull::{Pulled, push_pull, refusal};
    let (mut m, f) = disc(1.0);
    let d = if m.face(f).unwrap().plane().unwrap().normal.z > 0.0 {
        1.0
    } else {
        -1.0
    };
    push_pull(&mut m, f, d, false);
    let cap = f;
    assert_eq!(refusal(&m, cap), None, "the sides run along the pull");
    let more = d * 0.5;
    assert_eq!(push_pull(&mut m, cap, more, false), Pulled::Stretched);
    clean(&m);
    assert_eq!(m.face_count(), 4, "the same faces, longer");
    for s in sides(&m) {
        assert!((m.face_area(s) - PI * 1.5).abs() < 1e-9);
    }
    assert!((volume(&m).abs() - PI * 1.5).abs() < 1e-9);
}

#[test]
fn alt_pulls_a_new_loop_from_a_cylinders_cap() {
    use prism_ops::pushpull::push_pull;
    let (mut m, f) = disc(1.0);
    let d = if m.face(f).unwrap().plane().unwrap().normal.z > 0.0 {
        1.0
    } else {
        -1.0
    };
    push_pull(&mut m, f, d, false);
    push_pull(&mut m, f, d, true);
    clean(&m);
    assert_eq!(sides(&m).len(), 4, "two rings of sides");
}

#[test]
fn a_rounded_rectangle_pulled_has_flat_and_curved_sides() {
    use prism_ops::pushpull::push_pull;
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 2.0, 0.0);
    for c in [
        DVec3::ZERO,
        DVec3::new(4.0, 0.0, 0.0),
        DVec3::new(4.0, 2.0, 0.0),
    ] {
        assert!(prism_ops::fillet::fillet(&mut m, c, 0.5));
    }
    let f = m.faces().next().map(|(id, _)| id).expect("the face");
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(&mut m, f, up, false);
    clean(&m);
    assert_eq!(sides(&m).len(), 3, "one per rounded corner");
    assert_eq!(m.face_count(), 2 + 3 + 4);
    let h = prism_ops::health::check(&m);
    assert_eq!((h.watertight, h.leaky), (1, 0));
}
