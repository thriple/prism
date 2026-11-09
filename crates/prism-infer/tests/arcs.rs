//! Snapping to arcs (brief 23, step 2c), at three scales.

use prism_infer::{Context, Inference, Kind, Lock, Memory, Settings, infer};
use prism_math::{DVec2, DVec3};
use prism_topo::Model;
use prism_view::{Camera, Scene, View};

const SCALES: [f64; 3] = [1e-4, 1.0, 1e3];
const C: DVec3 = DVec3::new(2.0, 2.0, 0.0);

struct Setup {
    scene: Scene,
    view: View,
    k: f64,
}

/// A circle of radius 1 about (2, 2) on the ground; `cut` adds a line
/// through it on the diagonal, so its arcs are not halves.
fn setup(k: f64, cut: bool) -> Setup {
    let mut m = Model::new();
    prism_ops::add_circle(&mut m, C * k, DVec3::Z, k);
    if cut {
        prism_ops::add_segment(
            &mut m,
            DVec3::new(1.0, 1.0, 0.0) * k,
            DVec3::new(3.0, 3.0, 0.0) * k,
        );
    }
    let cam = Camera {
        eye: DVec3::new(2.0, -2.0, 6.0) * k,
        target: C * k,
    };
    Setup {
        scene: Scene::build(&m),
        view: View::new(&cam, 900.0, 600.0),
        k,
    }
}

impl Setup {
    fn screen(&self, p: DVec3) -> DVec2 {
        self.view.to_screen(p * self.k).expect("in front").0
    }
    fn at(&self, s: DVec2, ctx: &Context, mem: &Memory) -> Inference {
        infer(
            &self.scene,
            &self.view,
            s,
            ctx,
            Lock::None,
            mem,
            &Settings::default(),
        )
    }
    /// How far `p` is from the circle, in model units at scale 1.
    fn off_circle(&self, p: DVec3) -> f64 {
        let q = p / self.k - C;
        (q.z.abs() + (q.truncate().length() - 1.0).abs()).max(0.0)
    }
}

fn on_circle(deg: f64) -> DVec3 {
    let t = deg.to_radians();
    C + DVec3::new(t.cos(), t.sin(), 0.0)
}

#[test]
fn an_arc_has_a_midpoint_on_the_curve() {
    for k in SCALES {
        let t = setup(k, false);
        // The halves meet at (2, 1) and (2, 3); their midpoints are at
        // (1, 2) and (3, 2), on the circle (not on the chord).
        let i = t.at(
            t.screen(on_circle(0.0)) + DVec2::new(3.0, 2.0),
            &Context::default(),
            &Memory::default(),
        );
        assert_eq!(i.kind, Kind::Midpoint, "at {k}");
        assert!(t.off_circle(i.point) < 1e-9, "at {k}: {:?}", i.point);
        assert!((i.point / k).distance(on_circle(0.0)) < 1e-9, "at {k}");
    }
}

#[test]
fn on_arc_lands_exactly_on_the_circle() {
    for k in SCALES {
        let t = setup(k, false);
        for deg in [30.0, 115.0, 200.0, 333.0] {
            let i = t.at(
                t.screen(on_circle(deg)) + DVec2::new(2.0, 3.0),
                &Context::default(),
                &Memory::default(),
            );
            assert_eq!(i.kind, Kind::OnArc, "at {k}, {deg}°");
            assert_eq!(i.label, "On Arc");
            assert!(
                t.off_circle(i.point) < 1e-9,
                "at {k}, {deg}°: {:?}",
                i.point
            );
        }
    }
}

#[test]
fn quadrants_where_they_are_not_already_ends_or_midpoints() {
    for k in SCALES {
        let t = setup(k, true);
        for deg in [0.0, 180.0] {
            let i = t.at(
                t.screen(on_circle(deg)) + DVec2::new(2.0, -2.0),
                &Context::default(),
                &Memory::default(),
            );
            assert_eq!(i.kind, Kind::Quadrant, "at {k}, {deg}°");
            assert!((i.point / k).distance(on_circle(deg)) < 1e-9, "at {k}");
        }
    }
}

#[test]
fn the_centre_snaps_once_the_arc_is_hovered() {
    for k in SCALES {
        let t = setup(k, false);
        let mut mem = Memory::default();
        let centre = t.screen(C);
        let i = t.at(centre, &Context::default(), &mem);
        assert_ne!(
            i.kind,
            Kind::Centre,
            "not before the arc is hovered, at {k}"
        );
        // Hover the arc long enough to wake it.
        let i = t.at(t.screen(on_circle(60.0)), &Context::default(), &mem);
        assert_eq!(i.kind, Kind::OnArc);
        mem.observe(&i, 0.0, &Settings::default());
        assert!(mem.tick(0.3, &Settings::default()), "the arc wakes");
        let i = t.at(centre + DVec2::new(4.0, 0.0), &Context::default(), &mem);
        assert_eq!(i.kind, Kind::Centre, "at {k}");
        assert!((i.point / k).distance(C) < 1e-9, "at {k}");
        assert!(i.guides.is_empty(), "a woken arc casts no guides");
    }
}

#[test]
fn a_guide_meets_an_arc_on_the_circle_not_its_chord() {
    for k in SCALES {
        let t = setup(k, false);
        // Green from a start at (2.5, -1) crosses the circle at
        // y = 2 - √0.75.
        let ctx = Context {
            start: Some(DVec3::new(2.5, -1.0, 0.0) * k),
            ..Default::default()
        };
        let want = DVec3::new(2.5, 2.0 - 0.75f64.sqrt(), 0.0);
        let i = t.at(
            t.screen(want) + DVec2::new(2.0, 2.0),
            &ctx,
            &Memory::default(),
        );
        assert_eq!(i.kind, Kind::Intersection, "at {k}");
        assert!((i.point / k).distance(want) < 1e-9, "at {k}: {:?}", i.point);
    }
}

#[test]
fn a_tangent_guide_runs_from_an_arc_end() {
    for k in SCALES {
        // An eighth of a turn about (2, 2), from (3, 2) to 45°.
        let mut m = Model::new();
        prism_ops::add_arc(
            &mut m,
            C * k,
            DVec3::Z,
            k,
            DVec3::new(3.0, 2.0, 0.0) * k,
            std::f64::consts::FRAC_PI_4,
        );
        let cam = Camera {
            eye: DVec3::new(2.0, -2.0, 6.0) * k,
            target: C * k,
        };
        let t = Setup {
            scene: Scene::build(&m),
            view: View::new(&cam, 900.0, 600.0),
            k,
        };
        let end = on_circle(45.0);
        let dir = DVec3::new(-1.0, 1.0, 0.0).normalize();
        let ctx = Context {
            start: Some(end * k),
            ..Default::default()
        };
        let want = end + dir * 0.8;
        let i = t.at(
            t.screen(want) + DVec2::new(2.0, 1.0),
            &ctx,
            &Memory::default(),
        );
        assert_eq!(i.kind, Kind::Tangent, "at {k}");
        assert_eq!(i.label, "Tangent");
        let off = i.point / k - end;
        assert!(
            off.cross(dir).length() < 1e-9,
            "on the tangent, at {k}: {off:?}"
        );
    }
}
