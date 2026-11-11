//! Screenshot tests on the GPU (a software renderer in CI). They check
//! colours, not exact pixels, so they do not depend on the GPU.

mod common;
#[path = "../../prism-ops/tests/golden.rs"]
#[allow(dead_code)]
mod golden;

use common::Gpu;
use prism_render::gpu::Highlight;
use prism_render::{Camera, Style};

/// The GPU, one test at a time: Mesa's software renderer in CI crashed
/// (SIGSEGV) once with several devices made at the same time. Keep the
/// returned guard alive for the whole test.
fn gpu() -> Option<(std::sync::MutexGuard<'static, ()>, Gpu)> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // A test that failed while holding the lock must not stop the rest.
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let g = Gpu::new().map(|gpu| (turn, gpu));
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
    let Some((_turn, mut g)) = gpu() else { return };
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
        ..Highlight::default()
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
    let up = if m.face(base).expect("face").plane().unwrap().normal.z > 0.0 {
        2.0
    } else {
        -2.0
    };
    prism_ops::push_pull(&mut m, base, up, false);
    let (a, b) = (2.0 - 2.0 * ratio, 2.0 + 2.0 * ratio);
    prism_ops::add_rect(&mut m, a, a, b, b, 2.0);
    let inner = prism_ops::face_at(&m, DVec3::new(2.0, 2.0, 2.0)).expect("pocket face");
    let down = if m.face(inner).expect("face").plane().unwrap().normal.z > 0.0 {
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
    let Some((_turn, mut g)) = gpu() else { return };
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
    let Some((_turn, mut g)) = gpu() else { return };
    let style = tell_tale();
    for dist in [5.0, 100.0, 1000.0] {
        let s = dist * 0.3;
        let t = 0.0005;
        let mut m = Model::new();
        // The plate: s by s in x and z, t thick in y, in front of the blue axis.
        let y0 = -s * 0.05;
        prism_ops::add_rect(&mut m, -s / 2.0, y0, s / 2.0, y0 + t, 0.0);
        let f = prism_ops::face_at(&m, DVec3::new(0.0, y0 + t / 2.0, 0.0)).expect("plate base");
        let up = if m.face(f).expect("f").plane().unwrap().normal.z > 0.0 {
            s
        } else {
            -s
        };
        prism_ops::push_pull(&mut m, f, up, false);
        // The detail: a box behind the plate, with its own edges: 1 mm
        // behind, or 1e-5 of the distance if that is more (1 cm at 1 km).
        // 32-bit depth resolves about 1e-7 of the distance, but real GPUs
        // round line depths by several steps more (seen on Windows and macOS
        // CI: a 1.5 mm gap at 1 km is only 13 steps), so 1e-5 is the margin
        // that holds everywhere.
        let gap = (dist * 1e-5).max(0.001);
        let (d0, d1) = (y0 + t + gap, y0 + t + gap + s * 0.2);
        prism_ops::add_rect(&mut m, -s * 0.2, d0, s * 0.2, d1, s * 0.3);
        let f =
            prism_ops::face_at(&m, DVec3::new(0.0, (d0 + d1) / 2.0, s * 0.3)).expect("detail base");
        let up = if m.face(f).expect("f").plane().unwrap().normal.z > 0.0 {
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
                    // Diagnostics: at leaking pixels, what was stored and the
                    // exact depths of the plate's front and the detail's front.
                    let ids = g.ids(300, 300);
                    let view = prism_render::View::new(&cam, 300.0, 300.0);
                    let mut boxes = vec![
                        (DVec3::new(-30.0, -30.0, 0.0), DVec3::new(30.0, 30.0, 0.0)),
                        (DVec3::new(-60.0, 0.0, 0.0), DVec3::new(60.0, 0.0, 0.0)),
                        (DVec3::new(0.0, -60.0, 0.0), DVec3::new(0.0, 60.0, 0.0)),
                        (DVec3::ZERO, DVec3::new(0.0, 0.0, 60.0)),
                    ];
                    boxes.extend(m.bbox());
                    let (near, _) = prism_render::gpu::fit_depth(&view, &boxes);
                    let depth_on = |x: u32, y: u32, plane_y: f64| {
                        let (o, d) = view.ray(prism_math::DVec2::new(
                            f64::from(x) + 0.5,
                            f64::from(y) + 0.5,
                        ));
                        near / ((plane_y - o.y) / d.y * d.dot(view.f))
                    };
                    let mut shown = 0;
                    for y in 120..180u32 {
                        for x in 120..180u32 {
                            if img.at(x, y)[0] < 120 && shown < 12 {
                                shown += 1;
                                let v = ids[(y * 300 + x) as usize];
                                eprintln!(
                                    "LEAK ({x},{y}) rgb {:?} face {} stored {:.9} plate {:.9} detail {:.9}",
                                    &img.at(x, y)[..3],
                                    v[0],
                                    f32::from_bits(v[1]),
                                    depth_on(x, y, y0),
                                    depth_on(x, y, d0)
                                );
                            }
                        }
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
    let Some((_turn, mut g)) = gpu() else { return };
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
    let Some((_turn, mut g)) = gpu() else { return };
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
                m.add_face(Face::new(
                    outer,
                    vec![],
                    Surface::Plane(Plane::new(centre, newell(&pts))),
                ));
            }
        }
    }
    m
}

/// The tools' feedback: every glyph shape, a dashed guide and a preview.
#[test]
fn tool_feedback_draws() {
    use prism_view::overlay::{Line, Mark, Overlay, Shape, Tri};
    let Some((_turn, mut g)) = gpu() else { return };
    let style = Style::default();
    let fb = style.feedback;
    let m = golden::cube(1.0);
    let scene = prism_render::Scene::build(&m);
    let mut o = Overlay::default();
    let shapes = [
        (Shape::Circle, fb.endpoint),
        (Shape::Triangle, fb.midpoint),
        (Shape::Square, fb.on_edge),
        (Shape::Diamond, fb.on_face),
        (Shape::Cross, fb.guide_cross),
        (Shape::Ring, fb.endpoint),
        (Shape::DiamondOutline, fb.woken),
    ];
    for (k, (shape, color)) in shapes.into_iter().enumerate() {
        o.marks.push(Mark {
            at: DVec3::new(-3.0 + k as f64, -2.0, 0.0),
            shape,
            color,
            size: fb.glyph_size,
        });
    }
    o.lines.push(Line {
        a: DVec3::new(-4.0, 2.0, 0.0),
        b: DVec3::new(4.0, 2.0, 0.0),
        color: style.axes[0],
        width: fb.guide_width,
        dash: fb.guide_dash,
    });
    o.tris.push(Tri {
        p: [
            DVec3::new(2.0, 3.0, 0.0),
            DVec3::new(4.0, 3.0, 0.0),
            DVec3::new(4.0, 5.0, 0.0),
        ],
        color: fb.preview_fill,
    });
    g.renderer.resize(&g.device, 640, 400);
    g.renderer.set_scene(&g.device, &scene, &style);
    g.renderer
        .set_highlight(&g.device, &scene, &Highlight::default(), &style);
    g.renderer.set_overlay(&g.device, &o);
    let img = g.draw(Camera::home(), 640, 400, 1.0, &style, scene.bounds);
    img.save("feedback");
    let green = fb.endpoint.0;
    assert!(
        img.count_near([green[0], green[1], green[2]], 20) > 20,
        "the endpoint glyph is drawn"
    );
}

/// Guides stored in the model are hidden by faces in front of them; other
/// tool feedback draws on top.
#[test]
fn faces_hide_guides() {
    use prism_view::overlay::{Line, Overlay};
    let Some((_turn, mut g)) = gpu() else { return };
    let style = Style::default();
    let m = golden::cube(1.0);
    let scene = prism_render::Scene::build(&m);
    let color = Rgba::rgb(255, 0, 255);
    // A guide through the middle of the cube, along red.
    let guide = Line {
        a: DVec3::new(-6.0, 0.0, 2.0),
        b: DVec3::new(6.0, 0.0, 2.0),
        color,
        width: 3.0,
        dash: [0.0, 0.0],
    };
    let mut count = |o: &Overlay| {
        g.renderer.resize(&g.device, 640, 400);
        g.renderer.set_scene(&g.device, &scene, &style);
        g.renderer
            .set_highlight(&g.device, &scene, &Highlight::default(), &style);
        g.renderer.set_overlay(&g.device, o);
        let img = g.draw(Camera::home(), 640, 400, 1.0, &style, scene.bounds);
        img.save(if o.tested.is_empty() {
            "guide-on-top"
        } else {
            "guide-tested"
        });
        img.count_near([255, 0, 255], 40)
    };
    let on_top = count(&Overlay {
        lines: vec![guide],
        ..Overlay::default()
    });
    let tested = count(&Overlay {
        tested: vec![guide],
        ..Overlay::default()
    });
    assert!(on_top > 200, "the guide is drawn: {on_top}");
    assert!(tested > 100, "the guide shows outside the cube: {tested}");
    assert!(
        (tested as f64) < on_top as f64 * 0.9,
        "the part inside the cube is hidden: {tested} of {on_top}"
    );
}

/// An open group (PLAN 1.2): the rest of the model is drawn faded, and a
/// dashed box shows the group.
#[test]
fn an_open_group_fades_the_rest() {
    use prism_doc::{Command, Document};
    let Some((_turn, mut g)) = gpu() else { return };
    let style = Style::default();
    let mut doc = Document::new();
    for x in [0.0, 3.0] {
        doc.run(Command::Rect {
            x0: x,
            y0: 0.0,
            x1: x + 2.0,
            y1: 2.0,
            z: 0.0,
        });
        let f = prism_ops::face_at(doc.model(), DVec3::new(x + 1.0, 1.0, 0.0)).expect("base");
        let up = doc.model().face(f).expect("f").plane().unwrap().normal.z > 0.0;
        doc.run(Command::PushPull {
            face: f,
            distance: if up { 2.0 } else { -2.0 },
            new_loop: false,
        });
    }
    // The left box becomes a group; the right one stays loose.
    let left = prism_ops::Selection {
        faces: doc
            .model()
            .faces()
            .filter(|(id, _)| doc.model().face_area(*id) > 0.0)
            .filter(|(_, f)| doc.model().points(&f.outer).iter().all(|p| p.x < 2.5))
            .map(|(id, _)| id)
            .collect(),
        ..Default::default()
    };
    let id = doc.assembly().next_instance();
    doc.run(Command::MakeGroup {
        selection: left,
        instances: Default::default(),
    });
    let mut s = prism_tools::Session::new(doc);
    s.open_group(id);
    let scene = s.display_scene().clone();
    assert!(
        scene.faces.iter().any(|f| f.faded),
        "the loose box is faded"
    );
    let cam = Camera {
        eye: DVec3::new(2.5, -9.0, 6.0),
        target: DVec3::new(2.5, 1.0, 1.0),
    };
    g.renderer.resize(&g.device, 640, 400);
    g.renderer.set_scene(&g.device, &scene, &style);
    g.renderer
        .set_highlight(&g.device, &scene, &Highlight::default(), &style);
    g.renderer.set_overlay(&g.device, &s.overlay());
    let img = g.draw(cam, 640, 400, 1.0, &style, scene.bounds);
    img.save("open-group");
    // The faded box is lighter than the open group's faces.
    let lum = |x: u32, y: u32| {
        let p = img.at(x, y);
        u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2])
    };
    let open = (cam_px(&cam, DVec3::new(1.0, 0.0, 1.0)), "open group");
    let faded = (cam_px(&cam, DVec3::new(4.0, 0.0, 1.0)), "faded box");
    assert!(
        lum(faded.0.0, faded.0.1) > lum(open.0.0, open.0.1) + 30,
        "the box outside is faded: {} vs {}",
        lum(faded.0.0, faded.0.1),
        lum(open.0.0, open.0.1)
    );
    let o = style.open_box.0;
    assert!(
        img.count_near([o[0], o[1], o[2]], 12) > 40,
        "the dashed box is drawn"
    );
}

/// Where a point lands in a 640 x 400 picture from `cam`.
fn cam_px(cam: &Camera, p: DVec3) -> (u32, u32) {
    let view = prism_render::View::new(cam, 640.0, 400.0);
    let (q, _) = view.to_screen(p).expect("in view");
    (q.x as u32, q.y as u32)
}

/// Two boxes touching along a corner line share one edge with four faces.
/// None of them hides it, so it is drawn whole, not in dashes (owner
/// feedback, 2026-11-09: the shared corner line z-fought).
#[test]
fn an_edge_shared_by_two_touching_boxes_is_drawn_whole() {
    let Some((_turn, mut g)) = gpu() else { return };
    let style = Style::default();
    let mut m = Model::new();
    for (x, y) in [(0.0, 0.0), (2.0, 2.0)] {
        prism_ops::add_rect(&mut m, x, y, x + 2.0, y + 2.0, 0.0);
        let f = prism_ops::face_at(&m, DVec3::new(x + 1.0, y + 1.0, 0.0)).expect("base");
        let up = m.face(f).expect("f").plane().unwrap().normal.z > 0.0;
        prism_ops::push_pull(&mut m, f, if up { 2.0 } else { -2.0 }, false);
    }
    let corner = m
        .edges()
        .filter(|(_, e)| {
            let (a, b) = (m.vertex(e.a), m.vertex(e.b));
            a.truncate().distance(DVec3::new(2.0, 2.0, 0.0).truncate()) < 1e-9
                && b.truncate().distance(a.truncate()) < 1e-9
        })
        .count();
    assert_eq!(corner, 1, "one shared corner edge");
    // From both open sides: each sees a different pair of the four walls.
    for (k, eye) in [DVec3::new(6.0, -2.0, 3.0), DVec3::new(-2.0, 6.0, 3.0)]
        .into_iter()
        .enumerate()
    {
        let cam = Camera {
            eye,
            target: DVec3::new(2.0, 2.0, 1.0),
        };
        let img = g.shot(&m, cam, 640, 400, 1.0, &style, &Highlight::default());
        img.save(&format!("corner-edge-{k}"));
        let line = style.edge.0;
        let mut missing = Vec::new();
        for k in 1..20 {
            let z = 2.0 * f64::from(k) / 20.0;
            let (x, y) = cam_px(&cam, DVec3::new(2.0, 2.0, z));
            let drawn = (x.saturating_sub(1)..=x + 1)
                .flat_map(|x| (y.saturating_sub(1)..=y + 1).map(move |y| (x, y)))
                // Anti-aliased: well darker than the walls on either side.
                .any(|(x, y)| img.at(x, y)[0] < line[0].saturating_add(80));
            if !drawn {
                missing.push(z);
            }
        }
        assert!(
            missing.is_empty(),
            "from {eye}: the corner edge is missing at z = {missing:?}"
        );
    }
}

/// A face never hides a line lying in its own plane, at any angle: lines
/// drawn on a big floor seen from just above it are whole, not broken up
/// (the general rule behind the owner's z-fighting corner line, 2026-11-09).
/// The ground grid still hides under the floor.
#[test]
fn lines_on_a_face_seen_edge_on_are_drawn_whole() {
    let Some((_turn, mut g)) = gpu() else { return };
    let style = Style::default();
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, -200.0, -200.0, 200.0, 200.0, 0.0);
    let cam = Camera {
        eye: DVec3::new(0.0, -10.0, 0.3),
        target: DVec3::new(0.0, 40.0, 0.0),
    };
    let scene = prism_view::Scene::build(&m);
    g.renderer.resize(&g.device, 640, 400);
    g.renderer.set_scene(&g.device, &scene, &style);
    g.renderer
        .set_highlight(&g.device, &scene, &Highlight::default(), &style);
    let line = |x: f64| prism_view::overlay::Line {
        a: DVec3::new(x, 0.0, 0.0),
        b: DVec3::new(x, 150.0, 0.0),
        color: style.edge,
        width: 2.0,
        dash: [0.0, 0.0],
    };
    let o = prism_view::overlay::Overlay {
        tested: vec![line(-3.0), line(3.0)],
        ..Default::default()
    };
    g.renderer.set_overlay(&g.device, &o);
    let img = g.draw(cam, 640, 400, 1.0, &style, scene.bounds);
    img.save("lines-on-floor");
    for x in [-3.0, 3.0] {
        let mut missing = Vec::new();
        for k in 0..30 {
            let y = 0.5 + f64::from(k) * 0.5;
            let (px, py) = cam_px(&cam, DVec3::new(x, y, 0.0));
            let drawn = (px.saturating_sub(1)..=px + 1)
                .flat_map(|px| (py.saturating_sub(1)..=py + 1).map(move |py| (px, py)))
                .any(|(px, py)| img.at(px, py)[0] < 120);
            if !drawn {
                missing.push(y);
            }
        }
        assert!(missing.is_empty(), "line x = {x} broken at y = {missing:?}");
    }
    // The grid's own lines stay under the floor: away from the two lines,
    // the floor in the foreground is plain.
    let (px, py) = cam_px(&cam, DVec3::new(-1.5, 2.0, 0.0));
    let floor = img.at(px, py);
    assert!(floor[0] > 200, "the floor is plain there: {floor:?}");
}

/// View Settings (PLAN 1.14): profiles, Wireframe, Hidden Line, X-ray and
/// colour by axis, on one box.
#[test]
fn view_styles_draw_as_described() {
    use prism_view::ViewStyle;
    let Some((_turn, mut g)) = gpu() else { return };
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    let f = prism_ops::face_at(&m, DVec3::new(1.0, 1.0, 0.0)).expect("base");
    let up = m.face(f).expect("f").plane().unwrap().normal.z > 0.0;
    prism_ops::push_pull(&mut m, f, if up { 2.0 } else { -2.0 }, false);
    let cam = Camera {
        eye: DVec3::new(5.0, -4.0, 4.0),
        target: DVec3::new(1.0, 1.0, 1.0),
    };
    let shot = |g: &mut common::Gpu, v: &ViewStyle, name: &str| {
        let style = v.apply(&Style::default());
        let img = g.shot(&m, cam, 640, 400, 1.0, &style, &Highlight::default());
        img.save(name);
        img
    };
    // Dark pixels across a vertical edge, along the row through `p`.
    let width = |img: &common::Image, p: DVec3| {
        let (x, y) = cam_px(&cam, p);
        (x.saturating_sub(10)..x + 10)
            .filter(|x| img.at(*x, y)[0] < 150)
            .count()
    };
    let top = cam_px(&cam, DVec3::new(1.0, 1.0, 2.0));

    let default = shot(&mut g, &ViewStyle::default(), "style-default");
    // The left edge outlines the box (a profile); the front corner is inside.
    let (outline, inner) = (
        width(&default, DVec3::new(0.0, 0.0, 1.0)),
        width(&default, DVec3::new(2.0, 0.0, 1.0)),
    );
    assert!(outline > inner, "profile {outline} px vs plain {inner} px");
    let face = default.at(top.0, top.1);
    assert!(face[0] > 200, "the top is drawn: {face:?}");

    let wire = shot(
        &mut g,
        &ViewStyle::preset("Wireframe").unwrap(),
        "style-wireframe",
    );
    let mut empty = Model::new();
    prism_ops::add_segment(
        &mut empty,
        DVec3::new(50.0, 50.0, 0.0),
        DVec3::new(51.0, 50.0, 0.0),
    );
    let bg = g
        .shot(
            &empty,
            cam,
            640,
            400,
            1.0,
            &Style::default(),
            &Highlight::default(),
        )
        .at(top.0, top.1);
    let w = wire.at(top.0, top.1);
    assert!(
        (0..3).all(|i| w[i].abs_diff(bg[i]) < 12),
        "no face in Wireframe: {w:?} vs background {bg:?}"
    );

    let hidden = shot(
        &mut g,
        &ViewStyle::preset("Hidden Line").unwrap(),
        "style-hidden-line",
    );
    let h = hidden.at(top.0, top.1);
    assert!(
        h[..3].iter().all(|c| *c > 250),
        "Hidden Line faces are paper: {h:?}"
    );

    // The back corner (x = 0, y = 2) is behind the box.
    let back = DVec3::new(0.0, 2.0, 0.5);
    let xray = shot(&mut g, &ViewStyle::preset("X-ray").unwrap(), "style-xray");
    assert!(width(&xray, back) > 0, "X-ray shows the back corner");
    assert_eq!(width(&default, back), 0, "shaded hides it");

    let by_axis = ViewStyle {
        line_color: prism_view::view_style::LineColor::ByAxis,
        ..ViewStyle::default()
    };
    let axis = shot(&mut g, &by_axis, "style-by-axis");
    let (x, y) = cam_px(&cam, DVec3::new(1.0, 0.0, 0.0));
    let red = (x - 1..=x + 1)
        .flat_map(|x| (y - 1..=y + 1).map(move |y| (x, y)))
        .any(|(x, y)| {
            let p = axis.at(x, y);
            i32::from(p[0]) > i32::from(p[1]) + 60
        });
    assert!(red, "an edge along red is red");
}

/// With the sun, a box shadows the plate it stands on (View Settings part
/// 2): a spot in its shadow is darker than open plate, and is lit again
/// with shadows off. At night faces sit at the dark level.
#[test]
fn the_sun_casts_shadows_on_faces() {
    use prism_view::ViewStyle;
    let Some((_turn, mut g)) = gpu() else { return };
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, -6.0, -6.0, 6.0, 6.0, 0.0);
    prism_ops::add_rect(&mut m, -1.0, -1.0, 1.0, 1.0, 0.0);
    let f = prism_ops::face_at(&m, DVec3::new(0.0, 0.0, 0.0)).expect("square");
    let up = m.face(f).expect("f").plane().unwrap().normal.z > 0.0;
    prism_ops::push_pull(&mut m, f, if up { 3.0 } else { -3.0 }, false);
    let sun = ViewStyle {
        sun: true,
        ..ViewStyle::default()
    };
    // Where the shadow falls: from the box's middle, away from the sun,
    // onto the plate.
    let d = sun.sun_direction();
    assert!(d.z > 0.2, "the sun is up: {d}");
    let away = DVec3::new(-d.x, -d.y, 0.0).normalize();
    let in_shadow = away * 1.6;
    let open = -away * 3.0;
    let cam = Camera {
        eye: DVec3::new(0.0, -3.0, 18.0),
        target: DVec3::ZERO,
    };
    let lum = |img: &common::Image, p: DVec3| {
        let (x, y) = cam_px(&cam, p);
        let c = img.at(x, y);
        u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2])
    };
    let shot = |g: &mut common::Gpu, v: &ViewStyle, name: &str| {
        let img = g.shot(
            &m,
            cam,
            640,
            400,
            1.0,
            &v.apply(&Style::default()),
            &Highlight::default(),
        );
        img.save(name);
        img
    };
    let lit = shot(&mut g, &sun, "sun-shadows");
    assert!(
        lum(&lit, in_shadow) + 30 < lum(&lit, open),
        "shadow {} vs open {}",
        lum(&lit, in_shadow),
        lum(&lit, open)
    );
    let off = shot(
        &mut g,
        &ViewStyle {
            shadows: false,
            ..sun.clone()
        },
        "sun-no-shadows",
    );
    assert!(
        lum(&off, in_shadow).abs_diff(lum(&off, open)) < 6,
        "no shadow: {} vs {}",
        lum(&off, in_shadow),
        lum(&off, open)
    );
    let night = shot(
        &mut g,
        &ViewStyle {
            hours: 23.0,
            ..sun.clone()
        },
        "sun-night",
    );
    assert!(lum(&night, open) < lum(&off, open), "night is darker");
}

/// Exact Curves 5b: a sphere, a cone and a torus are shaded round, and no
/// back face shows from outside (every grid triangle faces out).
#[test]
fn cones_spheres_and_tori_shade_round() {
    let Some((_turn, mut g)) = gpu() else { return };
    let mut m = prism_tools::scenes::sphere(DVec3::new(-4.0, 0.0, 2.0), 2.0);
    m.append(&prism_tools::scenes::cone(1.5, 3.0));
    let mut t = prism_tools::scenes::torus(1.5, 0.5);
    prism_ops::transform::affine(
        &mut t,
        &prism_math::DAffine3::from_translation(DVec3::new(4.0, 0.0, 0.5)),
    );
    m.append(&t);
    for (name, cam) in [
        (
            "turned",
            looking_at(DVec3::new(0.0, 0.0, 1.0), 14.0, -70.0, 25.0),
        ),
        (
            "turned-above",
            looking_at(DVec3::new(0.0, 0.0, 1.0), 14.0, -60.0, 70.0),
        ),
    ] {
        let img = g.shot(&m, cam, 640, 400, 1.0, &tell_tale(), &Highlight::default());
        img.save(name);
        assert_eq!(back_pixels(&img), 0, "{name}: no back face shows");
    }
}

/// Exact Curves 6d: a smooth ball shows its outline as a line, found
/// again as the view turns; not in Wireframe, where faces are not drawn.
#[test]
fn a_smooth_ball_shows_its_outline() {
    let Some((_turn, mut g)) = gpu() else { return };
    let mut m = prism_tools::scenes::sphere(DVec3::new(0.0, 0.0, 2.0), 2.0);
    // Its seams soft and smooth: no edges drawn on it at all.
    let seams: Vec<_> = m.edges().map(|(id, _)| id).collect();
    for e in seams {
        m.set_soft(e, true, true);
    }
    let red = Style {
        edge: Rgba::rgb(255, 0, 0),
        show_ground: false,
        show_grid: false,
        show_axes: false,
        ..Style::default()
    };
    let scene = prism_render::Scene::build(&m);
    let outlines = prism_view::outline::Outlines::of(&scene);
    for (name, cam) in [
        (
            "outline-side",
            looking_at(DVec3::new(0.0, 0.0, 2.0), 12.0, -60.0, 10.0),
        ),
        (
            "outline-above",
            looking_at(DVec3::new(0.0, 0.0, 2.0), 12.0, 30.0, 60.0),
        ),
    ] {
        let img = g.shot(&m, cam, 640, 400, 1.0, &red, &Highlight::default());
        img.save(name);
        let lines = outlines.lines(cam.eye);
        assert!(!lines.is_empty(), "{name}");
        // Every few pieces along the outline, the line is drawn there.
        let mut hits = 0;
        let mut tried = 0;
        for l in lines.iter().step_by(7) {
            let (x, y) = cam_px(&cam, (l.a + l.b) * 0.5);
            tried += 1;
            let found = (x.saturating_sub(2)..=(x + 2).min(639)).any(|i| {
                (y.saturating_sub(2)..=(y + 2).min(399))
                    .any(|j| common::near(img.at(i, j), [255, 0, 0], 60))
            });
            hits += usize::from(found);
        }
        assert!(hits * 10 >= tried * 9, "{name}: {hits} of {tried}");
    }
    // Wireframe: no faces, so no outline.
    let wire = Style {
        faces: prism_view::style::FaceDraw::None,
        ..red
    };
    let cam = looking_at(DVec3::new(0.0, 0.0, 2.0), 12.0, -60.0, 10.0);
    let img = g.shot(&m, cam, 640, 400, 1.0, &wire, &Highlight::default());
    assert_eq!(img.count_near([255, 0, 0], 60), 0);
}

/// Owner, 2026-11-11: a cylinder's bottom rim showed through its front
/// wall. Each half of the side wraps round, so the far rim is on the same
/// face as the wall in front of it; a curved face now hides its own edges.
#[test]
fn a_cylinders_far_rim_is_hidden_by_its_near_wall() {
    let Some((_turn, mut g)) = gpu() else { return };
    let mut m = Model::new();
    prism_ops::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let f = m.faces().next().map(|(id, _)| id).expect("disc");
    let up = m
        .face(f)
        .and_then(|x| x.plane())
        .map_or(1.0, |p| p.normal.z);
    prism_ops::push_pull(&mut m, f, 2.0 * up, false);
    let cam = Camera {
        eye: DVec3::new(0.0, -6.0, 4.0),
        target: DVec3::new(0.0, 0.0, 1.0),
    };
    let style = Style::default();
    let img = g.shot(&m, cam, 640, 400, 1.0, &style, &Highlight::default());
    img.save("cylinder-far-rim");
    let edge = [style.edge.0[0], style.edge.0[1], style.edge.0[2]];
    let dark_near = |p: DVec3| {
        let (x, y) = cam_px(&cam, p);
        (x.saturating_sub(2)..=x + 2)
            .flat_map(|i| (y.saturating_sub(2)..=y + 2).map(move |j| (i, j)))
            .filter(|(i, j)| common::near(img.at(*i, *j), edge, 40))
            .count()
    };
    // The far rim (behind the wall) is hidden; the near rim shows.
    for a in [60.0f64, 90.0, 120.0] {
        let (s, c) = a.to_radians().sin_cos();
        assert_eq!(dark_near(DVec3::new(c, s, 0.0)), 0, "far rim at {a}°");
        assert!(dark_near(DVec3::new(c, -s, 0.0)) > 0, "near rim at {a}°");
    }
}
