//! The inference engine against the brief (05, "Tests"), at three scales.

use prism_infer::{Context, Inference, Key, Kind, Lock, Memory, Settings, infer};
use prism_math::{DVec2, DVec3};
use prism_topo::Model;
use prism_view::{Camera, Scene, View};

const SCALES: [f64; 3] = [1e-4, 1.0, 1e3];
const W: f64 = 900.0;
const H: f64 = 600.0;

/// A 4 by 4 square on the ground and a 1 by 1 by 1 box standing on it.
fn model(k: f64) -> Model {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0 * k, 4.0 * k, 0.0);
    prism_ops::add_rect(&mut m, 1.0 * k, 1.0 * k, 2.0 * k, 2.0 * k, 0.0);
    let f = prism_ops::face_at(&m, DVec3::new(1.5, 1.5, 0.0) * k).expect("small square");
    let up = m.face(f).expect("f").plane().unwrap().normal.z > 0.0;
    prism_ops::push_pull(&mut m, f, if up { k } else { -k }, false);
    m
}

struct Setup {
    scene: Scene,
    view: View,
    k: f64,
}

fn setup(k: f64) -> Setup {
    let cam = Camera {
        eye: DVec3::new(2.0, -5.0, 6.0) * k,
        target: DVec3::new(2.0, 2.0, 0.0) * k,
    };
    Setup {
        scene: Scene::build(&model(k)),
        view: View::new(&cam, W, H),
        k,
    }
}

impl Setup {
    fn screen(&self, p: DVec3) -> DVec2 {
        self.view.to_screen(p * self.k).expect("in front").0
    }
    fn at(&self, s: DVec2, ctx: &Context, lock: Lock, mem: &Memory) -> Inference {
        infer(
            &self.scene,
            &self.view,
            s,
            ctx,
            lock,
            mem,
            &Settings::default(),
        )
    }
    fn plain(&self, s: DVec2) -> Inference {
        self.at(s, &Context::default(), Lock::None, &Memory::default())
    }
}

#[test]
fn endpoints_win_inside_their_radius_and_not_outside() {
    for k in SCALES {
        let t = setup(k);
        let c = t.screen(DVec3::new(4.0, 0.0, 0.0));
        let i = t.plain(c + DVec2::new(10.0, 0.0));
        assert_eq!(i.kind, Kind::Endpoint, "at {k}");
        assert!(i.point.distance(DVec3::new(4.0, 0.0, 0.0) * k) < 1e-9 * k);
        // Away along the edge: no longer the endpoint.
        let i = t.plain(
            c + DVec2::new(0.0, 0.0) + (t.screen(DVec3::new(2.0, 0.0, 0.0)) - c).normalize() * 20.0,
        );
        assert_ne!(i.kind, Kind::Endpoint, "at {k}");
        assert_eq!(i.kind, Kind::OnEdge, "at {k}");
    }
}

#[test]
fn midpoints_and_edges() {
    for k in SCALES {
        let t = setup(k);
        let mid = t.screen(DVec3::new(2.0, 0.0, 0.0));
        assert_eq!(
            t.plain(mid + DVec2::new(3.0, 2.0)).kind,
            Kind::Midpoint,
            "at {k}"
        );
        let on = t.screen(DVec3::new(3.2, 0.0, 0.0));
        let i = t.plain(on + DVec2::new(0.0, 4.0));
        assert_eq!(i.kind, Kind::OnEdge, "at {k}");
        assert!(
            i.point.y.abs() < 1e-9 * k && i.point.z.abs() < 1e-9 * k,
            "on the edge itself"
        );
    }
}

#[test]
fn faces_ground_and_nothing() {
    for k in SCALES {
        let t = setup(k);
        assert_eq!(
            t.plain(t.screen(DVec3::new(3.0, 3.0, 0.0))).kind,
            Kind::OnFace,
            "at {k}"
        );
        assert_eq!(
            t.plain(t.screen(DVec3::new(7.0, 3.0, 0.0))).kind,
            Kind::OnGround,
            "at {k}"
        );
        // Up in the sky: free.
        let up = View::new(
            &Camera {
                eye: DVec3::new(2.0, -5.0, 1.0) * k,
                target: DVec3::new(2.0, 2.0, 4.0) * k,
            },
            W,
            H,
        );
        let i = infer(
            &t.scene,
            &up,
            DVec2::new(450.0, 5.0),
            &Context::default(),
            Lock::None,
            &Memory::default(),
            &Settings::default(),
        );
        assert_eq!(i.kind, Kind::Free, "at {k}");
    }
}

#[test]
fn hidden_points_are_not_snapped() {
    for k in SCALES {
        let t = setup(k);
        // The box's bottom back corner is behind the box.
        let p = DVec3::new(1.0, 2.0, 0.0);
        assert!(t.scene.occluded(t.view.eye, p * k));
        let i = t.plain(t.screen(p));
        assert!(
            !(i.kind == Kind::Endpoint && i.point.distance(p * k) < 1e-9 * k),
            "at {k}: {i:?}"
        );
    }
}

#[test]
fn hysteresis_holds_to_1_6_times_the_radius() {
    for k in SCALES {
        let t = setup(k);
        let c = t.screen(DVec3::new(0.0, 0.0, 0.0));
        let away = DVec2::new(-1.0, 0.0); // off the model, so nothing else competes
        let mut mem = Memory::default();
        let first = t.at(c + away * 10.0, &Context::default(), Lock::None, &mem);
        assert_eq!(first.kind, Kind::Endpoint);
        // At 18 px a fresh look does not find it...
        let fresh = t.at(c + away * 18.0, &Context::default(), Lock::None, &mem);
        assert_ne!(fresh.kind, Kind::Endpoint, "at {k}");
        // ...but a held snap stays until 1.6 x 14 = 22.4 px.
        mem.observe(&first, 0.0, &Settings::default());
        assert_eq!(
            t.at(c + away * 18.0, &Context::default(), Lock::None, &mem)
                .kind,
            Kind::Endpoint,
            "at {k}"
        );
        assert_eq!(
            t.at(c + away * 22.0, &Context::default(), Lock::None, &mem)
                .kind,
            Kind::Endpoint,
            "at {k}"
        );
        assert_ne!(
            t.at(c + away * 23.0, &Context::default(), Lock::None, &mem)
                .kind,
            Kind::Endpoint,
            "at {k}"
        );
    }
}

#[test]
fn the_start_point_casts_axis_guides() {
    for k in SCALES {
        let t = setup(k);
        let start = DVec3::new(6.0, 1.0, 0.0) * k;
        let ctx = Context {
            start: Some(start),
            ..Default::default()
        };
        // A point on the ground straight along red from the start, a little off.
        let s = t.screen(DVec3::new(8.0, 1.0, 0.0)) + DVec2::new(0.0, 4.0);
        let i = t.at(s, &ctx, Lock::None, &Memory::default());
        assert_eq!(i.kind, Kind::Guide, "at {k}");
        assert_eq!(i.axis, Some(0));
        assert_eq!(i.label, "On Red Axis");
        assert!((i.point.y - start.y).abs() < 1e-9 * k && i.point.z.abs() < 1e-9 * k);
    }
}

fn wake(t: &Setup, mem: &mut Memory, p: DVec3, now: f64) {
    let s = t.screen(p);
    let i = t.at(s, &Context::default(), Lock::None, mem);
    assert!(i.kind.wakes(), "{:?}", i.kind);
    mem.observe(&i, now, &Settings::default());
    assert!(!mem.tick(now + 0.1, &Settings::default()), "too soon");
    assert!(mem.tick(now + 0.25, &Settings::default()), "should wake");
}

#[test]
fn woken_points_cast_guides_that_cross() {
    for k in SCALES {
        let t = setup(k);
        let mut mem = Memory::default();
        // Wake the square's far right corner.
        wake(&t, &mut mem, DVec3::new(4.0, 4.0, 0.0), 0.0);
        assert_eq!(mem.woken.len(), 1);
        // Draw from a start on the ground in front: where "on green from the
        // start" crosses "red from the woken corner" is a guide cross.
        let start = DVec3::new(6.0, -1.0, 0.0) * k;
        let ctx = Context {
            start: Some(start),
            ..Default::default()
        };
        let cross = DVec3::new(6.0, 4.0, 0.0);
        let i = t.at(
            t.screen(cross) + DVec2::new(3.0, -2.0),
            &ctx,
            Lock::None,
            &mem,
        );
        assert_eq!(i.kind, Kind::GuideCross, "at {k}: {i:?}");
        assert!(i.point.distance(cross * k) < 1e-6 * k);
        assert_eq!(i.guides.len(), 2);
        // Red from the woken point alone, away from the start's guides.
        let i = t.at(
            t.screen(DVec3::new(7.0, 4.0, 0.0)) + DVec2::new(0.0, 3.0),
            &Context::default(),
            Lock::None,
            &mem,
        );
        assert_eq!(i.label, "Red from Point", "at {k}: {i:?}");
    }
}

#[test]
fn at_most_five_points_stay_woken() {
    let t = setup(1.0);
    let mut mem = Memory::default();
    let pts = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(4.0, 0.0, 0.0),
        DVec3::new(4.0, 4.0, 0.0),
        DVec3::new(0.0, 4.0, 0.0),
        DVec3::new(2.0, 0.0, 0.0),
        DVec3::new(2.0, 1.0, 1.0),
    ];
    for (n, p) in pts.iter().enumerate() {
        wake(&t, &mut mem, *p, n as f64);
    }
    assert_eq!(mem.woken.len(), 5);
    assert!(
        mem.woken.iter().all(|w| w.point != DVec3::ZERO),
        "the oldest went first"
    );
}

#[test]
fn a_guide_crossing_an_edge_is_an_intersection() {
    for k in SCALES {
        let t = setup(k);
        // From a start out on the ground, green runs into the square's front edge.
        let start = DVec3::new(3.0, -3.0, 0.0) * k;
        let ctx = Context {
            start: Some(start),
            ..Default::default()
        };
        let hit = DVec3::new(3.0, 0.0, 0.0);
        let i = t.at(
            t.screen(hit) + DVec2::new(2.0, 2.0),
            &ctx,
            Lock::None,
            &Memory::default(),
        );
        assert_eq!(i.kind, Kind::Intersection, "at {k}: {i:?}");
        assert!(i.point.distance(hit * k) < 1e-6 * k);
    }
}

#[test]
fn a_lock_projects_points_onto_the_axis() {
    for k in SCALES {
        let t = setup(k);
        let start = DVec3::new(6.0, -1.0, 0.0) * k;
        let ctx = Context {
            start: Some(start),
            ..Default::default()
        };
        // Locked to green, hovering the far corner gives its green position.
        let i = t.at(
            t.screen(DVec3::new(4.0, 4.0, 0.0)),
            &ctx,
            Lock::Axis(1),
            &Memory::default(),
        );
        assert!(i.locked);
        assert_eq!(i.key, Some(Key::Lock));
        assert!(
            i.point.distance(DVec3::new(6.0, 4.0, 0.0) * k) < 1e-9 * k,
            "at {k}: {:?}",
            i.point
        );
        // Rectangle-style tools ignore the lock.
        let r = Context {
            no_lock: true,
            ..ctx.clone()
        };
        assert!(
            !t.at(
                t.screen(DVec3::new(4.0, 4.0, 0.0)),
                &r,
                Lock::Axis(1),
                &Memory::default()
            )
            .locked
        );
    }
}

#[test]
fn excluded_vertices_are_not_snapped() {
    let t = setup(1.0);
    let m = model(1.0);
    let v = prism_ops::vertex_at(&m, DVec3::new(4.0, 0.0, 0.0)).expect("corner");
    let ctx = Context {
        exclude: [v].into(),
        ..Default::default()
    };
    let i = t.at(
        t.screen(DVec3::new(4.0, 0.0, 0.0)),
        &ctx,
        Lock::None,
        &Memory::default(),
    );
    assert_ne!(i.kind, Kind::Endpoint);
    assert_ne!(i.kind, Kind::Midpoint);
}

#[cfg(not(target_arch = "wasm32"))]
mod stability {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 400, ..ProptestConfig::default() })]

        /// No flicker: a snap that changes as the cursor moves under a pixel
        /// does not change back when it moves back.
        #[test]
        fn jitter_never_flickers(x in 100.0..800.0f64, y in 100.0..500.0f64, dx in -0.9..0.9f64, dy in -0.9..0.9f64) {
            let t = setup(1.0);
            let mut mem = Memory::default();
            let s = DVec2::new(x, y);
            let a = t.at(s, &Context::default(), Lock::None, &mem);
            mem.observe(&a, 0.0, &Settings::default());
            let b = t.at(s + DVec2::new(dx, dy), &Context::default(), Lock::None, &mem);
            mem.observe(&b, 0.0, &Settings::default());
            let c = t.at(s, &Context::default(), Lock::None, &mem);
            if b.kind.priority().1 > 0.0 {
                prop_assert_eq!(b.key, c.key, "{:?} -> {:?} -> {:?}", a.kind, b.kind, c.kind);
            }
        }
    }
}

/// The setup, with guides in the model: a line along y = 3 (crossing the
/// square's edges x = 0 and x = 4), a line along x = 3.5, and a point.
fn with_guides(k: f64) -> Setup {
    use prism_topo::Guide;
    let mut m = model(k);
    m.add_guide(Guide::Line {
        origin: DVec3::new(0.5, 3.0, 0.0) * k,
        dir: DVec3::X,
    });
    m.add_guide(Guide::Line {
        origin: DVec3::new(3.5, 0.5, 0.0) * k,
        dir: DVec3::Y,
    });
    m.add_guide(Guide::Point {
        at: DVec3::new(3.0, 0.5, 0.0) * k,
    });
    let mut t = setup(k);
    t.scene = Scene::build(&m);
    t
}

#[test]
fn stored_guides_snap_cross_each_other_and_edges() {
    for k in SCALES {
        let t = with_guides(k);
        let i = t.plain(t.screen(DVec3::new(2.5, 3.0, 0.0)) + DVec2::new(0.0, 3.0));
        assert_eq!(i.label, "On Guide", "at {k}");
        assert!((i.point.y - 3.0 * k).abs() < 1e-9 * k);
        let i = t.plain(t.screen(DVec3::new(3.5, 3.0, 0.0)) + DVec2::new(4.0, 0.0));
        assert_eq!(i.kind, Kind::GuideCross, "at {k}");
        assert!(i.point.distance(DVec3::new(3.5, 3.0, 0.0) * k) < 1e-9 * k);
        let i = t.plain(t.screen(DVec3::new(4.0, 3.0, 0.0)) + DVec2::new(3.0, 3.0));
        assert_eq!(i.kind, Kind::Intersection, "at {k}");
        assert!(i.point.distance(DVec3::new(4.0, 3.0, 0.0) * k) < 1e-9 * k);
        let i = t.plain(t.screen(DVec3::new(3.0, 0.5, 0.0)) + DVec2::new(5.0, 0.0));
        assert_eq!(i.kind, Kind::GuidePoint, "at {k}");
        assert!(i.point.distance(DVec3::new(3.0, 0.5, 0.0) * k) < 1e-9 * k);
    }
}

#[test]
fn hidden_guides_cost_nothing_and_do_not_snap() {
    let k = 1.0;
    let t = with_guides(k);
    let mut m = model(k);
    m.add_guide(prism_topo::Guide::Point {
        at: DVec3::new(3.0, 0.5, 0.0),
    });
    let hidden = Scene::build_with(&m, false);
    assert!(hidden.guides.is_empty());
    let s = t.screen(DVec3::new(3.0, 0.5, 0.0));
    let i = infer(
        &hidden,
        &t.view,
        s,
        &Context::default(),
        Lock::None,
        &Memory::default(),
        &Settings::default(),
    );
    assert_ne!(i.kind, Kind::GuidePoint);
}

/// DECISIONS #29: over a face another box passes through, where they cross
/// is a guide, and its corner is a snap.
#[test]
fn crossings_of_the_hovered_face_snap() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    let f = prism_ops::face_at(&m, DVec3::new(2.0, 2.0, 0.0)).unwrap();
    let up = m.face(f).unwrap().plane().unwrap().normal.z > 0.0;
    prism_ops::push_pull(&mut m, f, if up { 2.0 } else { -2.0 }, false);
    // A 2 x 1 bar through the box from y = -1 to 6, moved in (a move does
    // not cut, so the crossings stay for inference to find).
    let before: std::collections::BTreeSet<_> = m.vertices().map(|(id, _)| id).collect();
    prism_ops::add_rect(&mut m, 21.0, -1.0, 23.0, 6.0, 0.5);
    let bar = prism_ops::face_at(&m, DVec3::new(22.0, 2.0, 0.5)).unwrap();
    let up = m.face(bar).unwrap().plane().unwrap().normal.z > 0.0;
    prism_ops::push_pull(&mut m, bar, if up { 1.0 } else { -1.0 }, false);
    let vs = m
        .vertices()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    prism_ops::transform::translate(&mut m, &vs, DVec3::new(-20.0, 0.0, 0.0));
    // Look at the back face (y = 4) from behind and below the tube, so the
    // cursor is over the back face itself.
    let cam = Camera {
        eye: DVec3::new(2.5, 12.0, -1.5),
        target: DVec3::new(2.0, 4.0, 1.0),
    };
    let scene = Scene::build(&m);
    let view = View::new(&cam, W, H);
    let at = |p: DVec3, nudge: DVec2| {
        let s = view.to_screen(p).expect("in front").0 + nudge;
        infer(
            &scene,
            &view,
            s,
            &Context::default(),
            Lock::None,
            &Memory::default(),
            &Settings::default(),
        )
    };
    let i = at(DVec3::new(2.0, 4.0, 0.5), DVec2::new(0.0, 2.0));
    assert_eq!(i.label, "Face Intersection");
    assert!(
        (i.point.z - 0.5).abs() < 1e-9 && (i.point.y - 4.0).abs() < 1e-9,
        "{:?}",
        i.point
    );
    let c = at(DVec3::new(1.0, 4.0, 0.5), DVec2::new(3.0, 3.0));
    assert!(
        matches!(c.kind, Kind::GuideCross | Kind::Intersection),
        "{:?}",
        c.kind
    );
    assert!(
        c.point.distance(DVec3::new(1.0, 4.0, 0.5)) < 1e-9,
        "{:?}",
        c.point
    );
}
