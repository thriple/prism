//! The viewport's pure parts: camera, picking, display triangulation and the
//! Select tool's clicks (brief 04, "Tests").

#[path = "../../prism-ops/tests/golden.rs"]
#[allow(dead_code)]
mod golden;

use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3};
use prism_render::camera::{Camera, View, surface_point};
use prism_render::clicks::{ClickSeries, click_select};
use prism_render::pick::{self, Hit};
use prism_render::{Scene, tess};
use prism_topo::Model;

const SCALES: [f64; 3] = [1e-4, 1.0, 1e3];
const W: f64 = 900.0;
const H: f64 = 600.0;

fn home(k: f64) -> Camera {
    let c = Camera::home();
    Camera {
        eye: c.eye * k,
        target: c.target * k,
    }
}

fn under(scene: &Scene, view: &View, s: DVec2, k: f64) -> DVec3 {
    let fb = pick::fallback_distance(scene, view, DVec3::ZERO);
    let _ = k;
    surface_point(view, s, pick::model_point(scene, view, s), fb)
}

fn screen(view: &View, p: DVec3) -> DVec2 {
    view.to_screen(p).expect("in front").0
}

#[test]
fn wheel_zoom_never_reaches_the_surface_and_keeps_the_point_under_the_cursor() {
    for k in SCALES {
        let scene = Scene::build(&golden::cube(k));
        let mut cam = home(k);
        let s = DVec2::new(470.0, 260.0);
        let q0 = under(&scene, &View::new(&cam, W, H), s, k);
        assert!(pick::model_point(&scene, &View::new(&cam, W, H), s).is_some());
        let mut last = cam.eye.distance(q0);
        for _ in 0..400 {
            let view = View::new(&cam, W, H);
            let q = under(&scene, &view, s, k);
            cam = cam.wheel(q, 1.0, scene.tol.dist * 100.0);
            let v2 = View::new(&cam, W, H);
            assert!(
                screen(&v2, q).distance(s) < 0.5,
                "the point moved on screen"
            );
            let d = cam.eye.distance(q0);
            assert!(
                d >= scene.tol.dist * 100.0 * 0.99 && d <= last,
                "zoomed through or backwards"
            );
            assert!((q0 - cam.eye).dot(v2.f) > 0.0, "passed the surface");
            last = d;
        }
        // Zooming back out works too.
        let back = cam.wheel(q0, -3.0, 0.0);
        assert!(back.eye.distance(q0) > cam.eye.distance(q0));
    }
}

#[test]
fn pan_keeps_the_grabbed_point_under_the_cursor() {
    for k in SCALES {
        let scene = Scene::build(&golden::cube(k));
        let cam = home(k);
        let view = View::new(&cam, W, H);
        for s in [
            DVec2::new(450.0, 300.0),
            DVec2::new(100.0, 550.0),
            DVec2::new(800.0, 50.0),
        ] {
            let p = under(&scene, &view, s, k);
            let d = DVec2::new(37.0, -21.0);
            let moved = cam.pan(&view, p, d);
            let got = screen(&View::new(&moved, W, H), p);
            assert!(got.distance(s + d) < 0.5, "at {s:?}: {got:?}");
        }
    }
}

#[test]
fn orbit_keeps_the_pivot_still_and_the_horizon_level() {
    for k in SCALES {
        let scene = Scene::build(&golden::cube(k));
        let mut cam = home(k);
        let s = DVec2::new(420.0, 330.0);
        let pivot = under(&scene, &View::new(&cam, W, H), s, k);
        for (dx, dy) in [
            (40.0, 0.0),
            (0.0, 25.0),
            (-60.0, -30.0),
            (5.0, 300.0),
            (0.0, -900.0),
        ] {
            cam = cam.orbit_drag(pivot, dx, dy);
            let v = View::new(&cam, W, H);
            assert!(
                screen(&v, pivot).distance(s) < 0.5,
                "the pivot moved at {k} after {dx},{dy}: {:?}",
                screen(&v, pivot)
            );
            assert!(v.r.z.abs() < 1e-12, "the horizon tilted");
            assert!(v.f.z.abs() < 0.9951, "looked straight up or down");
        }
    }
}

#[test]
fn zoom_extents_fits_wide_and_tall_windows() {
    for k in SCALES {
        let m = golden::cube(k);
        let (lo, hi) = m.bbox().expect("a model");
        for (w, h) in [(1200.0, 500.0), (400.0, 900.0), (600.0, 600.0)] {
            let cam = home(k).extents(lo, hi, w / h);
            let v = View::new(&cam, w, h);
            let mut min = DVec2::splat(f64::MAX);
            let mut max = DVec2::splat(f64::MIN);
            for i in 0..8 {
                let c = DVec3::new(
                    if i & 1 == 0 { lo.x } else { hi.x },
                    if i & 2 == 0 { lo.y } else { hi.y },
                    if i & 4 == 0 { lo.z } else { hi.z },
                );
                let p = screen(&v, c);
                min = min.min(p);
                max = max.max(p);
            }
            assert!(
                min.x >= 0.0 && min.y >= 0.0 && max.x <= w && max.y <= h,
                "{w}x{h}: {min:?} {max:?}"
            );
            // And it is not tiny.
            assert!((max - min).max_element() > 0.3 * w.min(h));
        }
    }
}

#[test]
fn double_middle_click_recentres() {
    let c = home(1.0);
    let q = DVec3::new(1.0, -1.0, 3.0);
    let v = View::new(&c.recentre(q), W, H);
    assert!(screen(&v, q).distance(DVec2::new(W / 2.0, H / 2.0)) < 1e-6);
}

fn setup(k: f64) -> (Model, Scene, View) {
    let m = golden::cube(k);
    let scene = Scene::build(&m);
    let view = View::new(&home(k), W, H);
    (m, scene, view)
}

#[test]
fn vertices_beat_edges_and_edges_beat_faces() {
    for k in SCALES {
        let (m, scene, view) = setup(k);
        // The top front corner, nearest the eye.
        let corner = prism_ops::vertex_at(&m, DVec3::new(1.0, -1.0, 3.0) * k).expect("corner");
        let cs = screen(&view, m.vertex(corner));
        let p = pick::pick(&scene, &view, cs + DVec2::new(5.0, 3.0), 1.0).expect("a hit");
        assert_eq!(p.hit, Hit::Vertex(corner), "a vertex within reach wins");
        // Halfway along an edge, a little off it: the edge, not the face.
        let other = prism_ops::vertex_at(&m, DVec3::new(-1.0, -1.0, 3.0) * k).expect("corner");
        let edge = m.edge_between(corner, other).expect("edge");
        let mid = screen(&view, m.vertex(corner).lerp(m.vertex(other), 0.5));
        let p = pick::pick(&scene, &view, mid + DVec2::new(0.0, 4.0), 1.0).expect("a hit");
        assert_eq!(p.hit, Hit::Edge(edge));
        // The middle of the top face: the face.
        let top = prism_ops::face_at(&m, DVec3::new(0.0, 0.0, 3.0) * k).expect("top");
        let ts = screen(&view, DVec3::new(0.2, 0.3, 3.0) * k);
        let p = pick::pick(&scene, &view, ts, 1.0).expect("a hit");
        assert_eq!(p.hit, Hit::Face(top));
        // Empty sky: nothing.
        assert!(pick::pick(&scene, &view, DVec2::new(5.0, 5.0), 1.0).is_none());
    }
}

#[test]
fn hidden_things_cannot_be_picked() {
    for k in SCALES {
        let (m, scene, view) = setup(k);
        // The far bottom corner is behind the cube.
        let hidden = prism_ops::vertex_at(&m, DVec3::new(-1.0, 1.0, 1.0) * k).expect("corner");
        let p = m.vertex(hidden);
        assert!(scene.occluded(view.eye, p));
        let got = pick::pick(&scene, &view, screen(&view, p), 1.0).expect("the cube is in the way");
        assert!(!matches!(got.hit, Hit::Vertex(v) if v == hidden));
        assert!(matches!(got.hit, Hit::Face(_)), "{got:?}");
        // Every edge whose middle is hidden is not picked there.
        for e in &scene.edges {
            let mid = e.a.lerp(e.b, 0.5);
            if scene.occluded(view.eye, mid) {
                let got = pick::pick(&scene, &view, screen(&view, mid), 1.0);
                assert!(got.is_none_or(|g| g.hit != Hit::Edge(e.id)));
            }
        }
    }
}

#[test]
fn the_aperture_scales_the_reach() {
    let (m, scene, view) = setup(1.0);
    let corner = prism_ops::vertex_at(&m, DVec3::new(1.0, -1.0, 3.0)).expect("corner");
    let s = screen(&view, m.vertex(corner)) + DVec2::new(0.0, -20.0);
    let near = pick::pick(&scene, &view, s, 1.0);
    assert!(near.is_none_or(|p| p.hit != Hit::Vertex(corner)));
    let far = pick::pick(&scene, &view, s, 2.0).expect("a hit");
    assert_eq!(far.hit, Hit::Vertex(corner));
}

#[test]
fn picking_is_in_logical_pixels_on_any_screen() {
    // A 2x screen has twice the device pixels but the same logical size, so
    // the cursor in logical pixels picks the same thing.
    let (_, scene, _) = setup(1.0);
    let cam = home(1.0);
    for ppp in [1.0, 1.5, 2.0] {
        let device = DVec2::new(W * ppp, H * ppp);
        let view = View::new(&cam, device.x / ppp, device.y / ppp);
        for s in [DVec2::new(470.0, 260.0), DVec2::new(300.0, 200.0)] {
            let a = pick::pick(&scene, &view, s, 1.0).map(|p| p.hit);
            let b = pick::pick(&scene, &View::new(&cam, W, H), s, 1.0).map(|p| p.hit);
            assert_eq!(a, b);
        }
    }
}

fn check_mesh(scene: &Scene, m: &Model, what: &str) {
    for f in &scene.faces {
        let r = f.tris.start as usize..f.tris.end as usize;
        let idx = &scene.mesh.indices[r];
        let mut area = 0.0;
        for t in idx.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|i| scene.mesh.positions[t[i] as usize]);
            let n = (b - a).cross(c - a);
            area += n.length() * 0.5;
            assert!(n.dot(f.normal) > 0.0, "{what}: a triangle faces backwards");
            // Its middle is inside the face and not in a hole.
            let g = (a + b + c) / 3.0;
            let d = g - f.origin;
            let q = DVec2::new(d.dot(f.u), d.dot(f.v));
            assert_ne!(
                point_in_polygon(q, &f.outer),
                Inside::Outside,
                "{what}: outside the face"
            );
            for h in &f.holes {
                assert_ne!(
                    point_in_polygon(q, h),
                    Inside::Inside,
                    "{what}: covers a hole"
                );
            }
        }
        let want = m.face_area(f.id);
        assert!(
            (area - want).abs() <= want * 1e-9,
            "{what}: triangles cover {area}, the face is {want}"
        );
    }
}

#[test]
fn every_golden_model_triangulates_exactly() {
    let cases = golden::cases();
    let mut n = 0;
    for case in cases["cases"].as_array().expect("cases") {
        for k in SCALES {
            let m = golden::run(case, k);
            let scene = Scene::build(&m);
            check_mesh(&scene, &m, &format!("{} at {k}", case["id"]));
            n += 1;
        }
    }
    assert!(n >= 19 * 3, "ran {n}");
}

#[test]
fn a_face_with_a_thousand_corners_and_holes_is_quick() {
    let n = 1000;
    let outer: Vec<DVec2> = (0..n)
        .map(|i| {
            let a = std::f64::consts::TAU * f64::from(i) / f64::from(n);
            // A wavy outline, with many reflex corners.
            let r = 10.0 + if i % 2 == 0 { 0.0 } else { 0.4 };
            DVec2::new(r * a.cos(), r * a.sin())
        })
        .collect();
    let mut holes = Vec::new();
    for gx in -3..3 {
        for gy in -3..3 {
            let (x, y) = (f64::from(gx) * 2.0 + 0.3, f64::from(gy) * 2.0 + 0.3);
            holes.push(vec![
                DVec2::new(x, y),
                DVec2::new(x, y + 1.0),
                DVec2::new(x + 1.0, y + 1.0),
                DVec2::new(x + 1.0, y),
            ]);
        }
    }
    let start = std::time::Instant::now();
    let tris = tess::triangulate(&outer, &holes);
    let took = start.elapsed();
    let all: Vec<DVec2> = outer
        .iter()
        .chain(holes.iter().flatten())
        .copied()
        .collect();
    let area: f64 = tris
        .iter()
        .map(|t| prism_math::predicates::orient2d(all[t[0]], all[t[1]], all[t[2]]) * 0.5)
        .sum();
    let want = prism_math::predicates::signed_area2(&outer) * 0.5 - holes.len() as f64;
    assert!((area - want).abs() < 1e-9 * want, "{area} vs {want}");
    assert!(took.as_millis() < 2000, "took {took:?}");
    eprintln!(
        "1000-corner face with {} holes: {} triangles in {took:?}",
        holes.len(),
        tris.len()
    );
}

#[test]
fn clicks_count_within_350_ms_and_6_px() {
    let mut c = ClickSeries::default();
    let s = DVec2::new(100.0, 100.0);
    assert_eq!(c.press(0.0, s), 1);
    assert_eq!(c.press(0.2, s + DVec2::new(3.0, 3.0)), 2);
    assert_eq!(c.press(0.5, s), 3);
    assert_eq!(c.press(0.9, s), 1, "too slow");
    assert_eq!(c.press(1.0, s + DVec2::new(6.0, 0.0)), 1, "too far");
    assert_eq!(c.press(1.2, s + DVec2::new(6.0, 0.0)), 2);
}

#[test]
fn single_double_and_triple_click_select_as_in_the_feel_test() {
    let mut m = golden::cube(1.0);
    prism_ops::add_rect(&mut m, 3.0, 3.0, 4.0, 4.0, 0.0);
    let top = prism_ops::face_at(&m, DVec3::new(0.0, 0.0, 3.0)).expect("top");
    let one = click_select(&m, Some(Hit::Face(top)), 1);
    assert_eq!((one.faces.len(), one.edges.len()), (1, 0));
    let two = click_select(&m, Some(Hit::Face(top)), 2);
    assert_eq!(
        (two.faces.len(), two.edges.len()),
        (1, 4),
        "the face and its edges"
    );
    let three = click_select(&m, Some(Hit::Face(top)), 3);
    assert_eq!(
        (three.faces.len(), three.edges.len(), three.vertices.len()),
        (6, 12, 8),
        "the whole cube, not the square"
    );
    let e = m.edges().next().map(|(id, _)| id).expect("an edge");
    assert_eq!(click_select(&m, Some(Hit::Edge(e)), 2).edges.len(), 1);
    assert!(click_select(&m, None, 1).is_empty());
}
