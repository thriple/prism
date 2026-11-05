//! Screenshot tests on the GPU (a software renderer in CI). They check
//! colours, not exact pixels, so they do not depend on the GPU.

mod common;
#[path = "../../prism-ops/tests/golden.rs"]
#[allow(dead_code)]
mod golden;

use common::Gpu;
use prism_render::gpu::Highlight;
use prism_render::{Camera, Style};

fn gpu() -> Option<Gpu> {
    let g = Gpu::new();
    if g.is_none() {
        eprintln!("no GPU adapter: skipping the screenshot test");
        assert!(
            std::env::var_os("CI").is_none(),
            "CI must have a (software) GPU"
        );
    }
    g
}

#[test]
fn cube_draws() {
    let Some(mut g) = gpu() else { return };
    let style = Style::default();
    let m = golden::cube(1.0);
    let img = g.shot(
        &m,
        Camera::home(),
        640,
        400,
        1.0,
        &style,
        &Highlight::default(),
    );
    img.save("cube");
    // Nothing magenta-ish or missing: the sky, the ground and the cube are there.
    assert!(img.count_near([0xf8, 0xf7, 0xf4], 40) > 1000, "front faces");

    // Selection and hover.
    let top = prism_ops::face_at(&m, DVec3::new(0.0, 0.0, 3.0)).expect("top");
    let front = prism_ops::face_at(&m, DVec3::new(0.0, -1.0, 2.0)).expect("front");
    let corner = prism_ops::vertex_at(&m, DVec3::new(1.0, -1.0, 1.0)).expect("corner");
    let edge = m.edges().next().map(|(id, _)| id).expect("edge");
    let mut sel = prism_ops::select::Selection::default();
    sel.faces.insert(top);
    sel.vertices.insert(corner);
    sel.edges.insert(edge);
    let h = Highlight {
        hover: Some(prism_render::Hit::Face(front)),
        selection: sel,
    };
    let img = g.shot(&m, Camera::home(), 640, 400, 2.0, &style, &h);
    img.save("cube-selected");
    let blue = style.selected.0;
    assert!(
        img.count_near([blue[0], blue[1], blue[2]], 30) > 50,
        "selection blue"
    );
    let orange = img
        .px
        .iter()
        .filter(|p| p[0] > 200 && p[1] > 150 && p[2] < 190 && p[0] > p[2] + 40)
        .count();
    assert!(orange > 1000, "hover tint ({orange})");

    // Profile lines and dashed hidden lines.
    let s2 = Style {
        profile_width: 3.0,
        hidden_lines: prism_render::style::HiddenLines::Dashed,
        ..Style::default()
    };
    g.shot(
        &m,
        Camera::home(),
        640,
        400,
        1.0,
        &s2,
        &Highlight::default(),
    )
    .save("cube-profile-hidden");
}

use prism_math::DVec3;
use prism_render::style::Rgba;
use prism_topo::Model;

/// A style that paints back faces pure magenta, so any that show are easy
/// to count.
fn tell_tale() -> Style {
    Style {
        face_back: Rgba::rgb(255, 0, 255),
        ..Style::default()
    }
}

/// Pixels that are clearly the magenta back colour (shaded or not).
fn back_pixels(img: &common::Image) -> usize {
    img.px
        .iter()
        .filter(|p| p[1] < 40 && p[0] > 110 && p[2] > 110)
        .count()
}

fn looking_at(target: DVec3, dist: f64, az_deg: f64, el_deg: f64) -> Camera {
    let (a, e) = (az_deg.to_radians(), el_deg.to_radians());
    Camera {
        eye: target + DVec3::new(e.cos() * a.cos(), e.cos() * a.sin(), e.sin()) * dist,
        target,
    }
}

/// A 4 by 4 by 2 box with a square pocket: `ratio` of the width, `depth` deep.
fn pocket_box(ratio: f64, depth: f64) -> Model {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    let base = prism_ops::face_at(&m, DVec3::new(2.0, 2.0, 0.0)).expect("base");
    let up = if m.face(base).expect("face").plane().normal.z > 0.0 {
        2.0
    } else {
        -2.0
    };
    prism_ops::push_pull(&mut m, base, up, false);
    let (a, b) = (2.0 - 2.0 * ratio, 2.0 + 2.0 * ratio);
    prism_ops::add_rect(&mut m, a, a, b, b, 2.0);
    let inner = prism_ops::face_at(&m, DVec3::new(2.0, 2.0, 2.0)).expect("pocket face");
    let down = if m.face(inner).expect("face").plane().normal.z > 0.0 {
        -depth
    } else {
        depth
    };
    prism_ops::push_pull(&mut m, inner, down, false);
    assert!(
        prism_ops::check::problems(&m).is_empty(),
        "{:?}",
        prism_ops::check::problems(&m)
    );
    m
}

/// NOTES, "Drawing order bug found in the prototype": a box with a pocket,
/// seen from every 6 degrees around and several heights, never shows the
/// back of a face from outside.
#[test]
fn a_pocket_never_shows_a_back_face_from_outside() {
    let Some(mut g) = gpu() else { return };
    let style = tell_tale();
    let mut shots = 0;
    for ratio in [0.3, 0.6, 0.9] {
        for depth in [1.2, 1.5, 1.9] {
            let m = pocket_box(ratio, depth);
            for el in [8.0, 25.0, 45.0, 70.0, 85.0] {
                for k in 0..60 {
                    let az = f64::from(k) * 6.0;
                    let cam = looking_at(DVec3::new(2.0, 2.0, 1.0), 11.0, az, el);
                    let img = g.shot(&m, cam, 200, 150, 1.0, &style, &Highlight::default());
                    let bad = back_pixels(&img);
                    if bad > 0 {
                        img.save(&format!("pocket-fail-{ratio}-{depth}-{el}-{az}"));
                    }
                    assert_eq!(
                        bad, 0,
                        "ratio {ratio}, depth {depth}, elevation {el}, azimuth {az}"
                    );
                    shots += 1;
                }
            }
            // One picture per pocket for looking at by hand.
            let cam = looking_at(DVec3::new(2.0, 2.0, 1.0), 11.0, 30.0, 45.0);
            g.shot(
                &m,
                cam,
                480,
                320,
                1.0,
                &Style::default(),
                &Highlight::default(),
            )
            .save(&format!("pocket-{ratio}-{depth}"));
        }
    }
    eprintln!("{shots} pocket views, no back faces");
}

/// PLAN 4: a 0.5 mm plate with detail just behind it, seen from 5 m, 100 m
/// and 1 km, face on and at an angle. Nothing behind it shows through, and
/// its own edges stay drawn.
#[test]
fn nothing_shows_through_a_thin_plate_far_away() {
    let Some(mut g) = gpu() else { return };
    let style = tell_tale();
    for dist in [5.0, 100.0, 1000.0] {
        let s = dist * 0.3;
        let t = 0.0005;
        let mut m = Model::new();
        // The plate: s by s in x and z, t thick in y, in front of the blue axis.
        let y0 = -s * 0.05;
        prism_ops::add_rect(&mut m, -s / 2.0, y0, s / 2.0, y0 + t, 0.0);
        let f = prism_ops::face_at(&m, DVec3::new(0.0, y0 + t / 2.0, 0.0)).expect("plate base");
        let up = if m.face(f).expect("f").plane().normal.z > 0.0 {
            s
        } else {
            -s
        };
        prism_ops::push_pull(&mut m, f, up, false);
        // The detail: a box 1 mm behind the plate, with its own edges.
        let (d0, d1) = (y0 + t + 0.001, y0 + t + 0.001 + s * 0.2);
        prism_ops::add_rect(&mut m, -s * 0.2, d0, s * 0.2, d1, s * 0.3);
        let f =
            prism_ops::face_at(&m, DVec3::new(0.0, (d0 + d1) / 2.0, s * 0.3)).expect("detail base");
        let up = if m.face(f).expect("f").plane().normal.z > 0.0 {
            s * 0.4
        } else {
            -s * 0.4
        };
        prism_ops::push_pull(&mut m, f, up, false);
        for az in [-90.0, -60.0, -115.0] {
            for el in [0.0, 20.0] {
                let target = DVec3::new(0.0, 0.0, s / 2.0);
                let cam = looking_at(target, dist, az, el);
                let img = g.shot(&m, cam, 300, 300, 1.0, &style, &Highlight::default());
                let name = format!("plate-{dist}-{az}-{el}");
                // The middle of the plate on screen is plain face colour: no
                // edges of the detail, and no back faces.
                let mut dark = 0;
                for y in 120..180 {
                    for x in 120..180 {
                        let p = img.at(x, y);
                        if p[0] < 120 || (p[1] < 40 && p[0] > 110) {
                            dark += 1;
                        }
                    }
                }
                if dark > 0 || back_pixels(&img) > 0 {
                    img.save(&name);
                    let ids = g.ids(300, 300);
                    for y in (100..200).step_by(4) {
                        let row: Vec<String> = (140..160)
                            .step_by(4)
                            .map(|x| {
                                let v = ids[(y * 300 + x) as usize];
                                format!("{}:{:.7}", v[0], f32::from_bits(v[1]))
                            })
                            .collect();
                        let dk: Vec<bool> = (140..160)
                            .step_by(4)
                            .map(|x| img.at(x, y)[0] < 120)
                            .collect();
                        eprintln!("y{y}: {row:?} {dk:?}");
                    }
                }
                assert_eq!(dark, 0, "{name}: something behind shows through");
                assert_eq!(back_pixels(&img), 0, "{name}: a back face shows");
                // The plate's outline is drawn: dark pixels exist in the picture.
                let edges = img.px.iter().filter(|p| p[0] < 60 && p[1] < 60).count();
                assert!(
                    edges > 100,
                    "{name}: the plate's edges are missing ({edges})"
                );
            }
        }
    }
}

/// A 1.5 point edge is 1.5 points wide at 1x and 2x.
#[test]
fn lines_keep_their_width_on_any_screen() {
    let Some(mut g) = gpu() else { return };
    let style = Style::default();
    // One loose edge across the view, seen from above.
    let mut m = Model::new();
    prism_ops::add_segment(
        &mut m,
        DVec3::new(-2.0, 0.0, 0.5),
        DVec3::new(2.0, 0.0, 0.5),
    );
    let cam = Camera {
        eye: DVec3::new(0.0, -0.001, 10.0),
        target: DVec3::new(0.0, 0.0, 0.0),
    };
    for ppp in [1.0f32, 2.0] {
        let (w, h) = ((200.0 * ppp) as u32, (200.0 * ppp) as u32);
        let img = g.shot(&m, cam, w, h, ppp, &style, &Highlight::default());
        // Coverage down the middle column: sum of darkness relative to the ground.
        let x = w / 2;
        let bg = img.at(x, 5);
        let mut cover = 0.0;
        for y in 0..h {
            let p = img.at(x, y);
            let line = style.edge.0;
            let t = (f32::from(bg[0]) - f32::from(p[0])) / (f32::from(bg[0]) - f32::from(line[0]));
            if t > 0.02 {
                cover += t.clamp(0.0, 1.0);
            }
        }
        let width_pt = cover / ppp;
        img.save(&format!("line-{ppp}x"));
        assert!(
            (width_pt - style.edge_width).abs() <= 0.5,
            "at {ppp}x the line is {width_pt} points wide"
        );
    }
}

/// Timings for NOTES (run by hand: `cargo test --release -p prism-render
/// --test screens timing -- --ignored --nocapture`).
#[test]
#[ignore = "prints timings"]
fn timing_large_scene() {
    use prism_math::DVec2;
    let Some(mut g) = gpu() else { return };
    let n = 50;
    let m = prism_render_large(n);
    let t = std::time::Instant::now();
    let scene = prism_render::Scene::build(&m);
    let build = t.elapsed();
    let style = Style::default();
    let target = DVec3::new(37.0, 37.0, 0.0);
    let cam = looking_at(target, 60.0, -60.0, 35.0);
    let view = prism_render::View::new(&cam, 1200.0, 800.0);
    let t = std::time::Instant::now();
    let mut hits = 0;
    for k in 0..20 {
        let s = DVec2::new(300.0 + f64::from(k) * 30.0, 400.0);
        hits += usize::from(prism_render::pick::pick(&scene, &view, s, 1.0).is_some());
    }
    let pick = t.elapsed() / 20;
    let _ = g.shot(&m, cam, 1200, 800, 1.0, &style, &Highlight::default());
    let t = std::time::Instant::now();
    for k in 0..10 {
        let c = looking_at(target, 60.0, -60.0 + f64::from(k) * 3.0, 35.0);
        let _ = g.draw(c, 1200, 800, 1.0, &style, scene.bounds);
    }
    let frame = t.elapsed() / 10;
    eprintln!(
        "{} faces, {} edges: scene built in {build:?}, pick {pick:?} ({hits}/20 hit), orbit frame (draw + read back) {frame:?}",
        m.face_count(),
        m.edge_count()
    );
}

fn prism_render_large(n: i32) -> Model {
    use prism_geom::Surface;
    use prism_math::{Plane, newell};
    use prism_topo::{Face, VertexId};
    let mut m = Model::new();
    for i in 0..n {
        for j in 0..n {
            let lo = DVec3::new(f64::from(i) * 1.5, f64::from(j) * 1.5, 0.0);
            let hi = lo + DVec3::new(1.0, 1.0, 0.5 + f64::from((i * 7 + j * 3) % 5) * 0.3);
            let c: Vec<VertexId> = (0..8)
                .map(|k| {
                    m.add_vertex(DVec3::new(
                        if k & 1 == 0 { lo.x } else { hi.x },
                        if k & 2 == 0 { lo.y } else { hi.y },
                        if k & 4 == 0 { lo.z } else { hi.z },
                    ))
                })
                .collect();
            for l in [
                [0, 2, 3, 1],
                [4, 5, 7, 6],
                [0, 1, 5, 4],
                [2, 6, 7, 3],
                [0, 4, 6, 2],
                [1, 3, 7, 5],
            ] {
                let outer: Vec<VertexId> = l.iter().map(|k| c[*k]).collect();
                for k in 0..4 {
                    m.add_edge(outer[k], outer[(k + 1) % 4]);
                }
                let pts = m.points(&outer);
                let centre = pts.iter().copied().sum::<DVec3>() / 4.0;
                m.add_face(Face {
                    outer,
                    holes: vec![],
                    surface: Surface::Plane(Plane::new(centre, newell(&pts))),
                });
            }
        }
    }
    m
}
