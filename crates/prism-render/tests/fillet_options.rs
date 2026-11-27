//! Pictures for the owner's two open questions in brief 56 (end caps, and a
//! line drawn across a round). Not a check: run by hand with
//! `cargo test --release -p prism-render --test fillet_options -- --ignored`
//! and look in `target/viewport-screens`.

mod common;

use std::collections::BTreeSet;

use common::Gpu;
use prism_math::DVec3;
use prism_ops::boolean::{Op, boolean};
use prism_render::gpu::Highlight;
use prism_render::{Camera, Style};
use prism_topo::{EdgeId, Model};

const R: f64 = 0.5;

/// A block 2 deep and 2 high, seen from the front (-y); its end at +x is
/// slanted when `top` is less than `bottom`.
fn block(x0: f64, bottom: f64, top: f64) -> Model {
    let mut m = Model::new();
    prism_ops::add_polygon(
        &mut m,
        &[
            DVec3::new(x0, 0.0, 0.0),
            DVec3::new(bottom, 0.0, 0.0),
            DVec3::new(top, 0.0, 2.0),
            DVec3::new(x0, 0.0, 2.0),
        ],
    );
    let f = prism_ops::face_at(&m, DVec3::new((x0 + top) / 2.0, 0.0, 1.0)).expect("side");
    let n = m.face(f).unwrap().plane().unwrap().normal.y;
    prism_ops::push_pull(&mut m, f, if n > 0.0 { 2.0 } else { -2.0 }, false);
    assert!(prism_ops::check::problems(&m).is_empty());
    m
}

/// The edges along the top front corner (y = 0, z = 2).
fn top_front(m: &Model) -> BTreeSet<EdgeId> {
    m.edges()
        .filter(|(_, e)| {
            [e.a, e.b].iter().all(|v| {
                let p = m.vertex(*v);
                p.y.abs() < 1e-9 && (p.z - 2.0).abs() < 1e-9
            })
        })
        .map(|(id, _)| id)
        .collect()
}

fn rounded(mut m: Model) -> Model {
    let e = top_front(&m);
    assert!(!e.is_empty());
    prism_ops::rounds::round_edges(&mut m, &e, R, true).expect("round");
    m
}

fn look(target: DVec3, dist: f64, az: f64, el: f64) -> Camera {
    let (a, e) = (az.to_radians(), el.to_radians());
    Camera {
        eye: target + DVec3::new(e.cos() * a.cos(), e.cos() * a.sin(), e.sin()) * dist,
        target,
    }
}

fn shoot(g: &mut Gpu, m: &Model, name: &str, cams: &[(&str, Camera)]) {
    eprintln!(
        "{name}: {} faces, problems {:?}",
        m.faces().count(),
        prism_ops::check::problems(m)
    );
    for (tag, cam) in cams {
        let img = g.shot(
            m,
            *cam,
            1200,
            800,
            1.0,
            &Style::default(),
            &Highlight::default(),
        );
        img.save(&format!("{name}-{tag}"));
    }
}

#[test]
#[ignore]
fn end_caps_and_lines_across_a_round() {
    let Some(mut g) = Gpu::new() else {
        eprintln!("no GPU");
        return;
    };
    let end = DVec3::new(2.8, 0.4, 1.6);
    let cams = [
        ("near", look(end, 4.5, -40.0, 28.0)),
        ("back", look(end, 3.5, -130.0, 35.0)),
        ("whole", look(DVec3::new(2.0, 1.0, 1.0), 9.0, -55.0, 25.0)),
    ];

    // Keep surface: the round runs on until it meets the slanted end, and
    // its end lies in that face. Prism's round on the slanted block if it
    // takes it; else the same shape from a long rounded bar cut by it.
    let slanted = block(0.0, 4.0, 2.5);
    let mut keep_surface = slanted.clone();
    let e = top_front(&keep_surface);
    match prism_ops::rounds::round_edges(&mut keep_surface, &e, R, true) {
        Ok(_) => eprintln!("keep surface: Prism's round"),
        Err(why) => {
            eprintln!("keep surface: round refused ({why}); built by Intersect");
            let bar = rounded(block(-1.0, 5.0, 5.0));
            keep_surface = boolean(&bar, &slanted, Op::Intersect).expect("intersect");
        }
    }
    shoot(&mut g, &keep_surface, "cap-keep-surface", &cams);

    // Keep edge: the round stops square to the edge where its setback
    // first meets the slanted end (x = 3, on the top face), a flat cap
    // closes it, and the corner beyond stays as it was.
    let short = rounded(block(0.0, 2.5, 2.5));
    let wedge = block(2.5, 4.0, 2.5001);
    let keep_edge = match boolean(&short, &wedge, Op::Union) {
        Ok(m) => m,
        Err(why) => panic!("union: {why}"),
    };
    shoot(&mut g, &keep_edge, "cap-keep-edge", &cams);

    // A line drawn across the top face up to the edge. Stops: the edge
    // rounded whole, the line drawn back to where the round begins (built
    // by hand: what the fallback round does). Across: Prism's round today,
    // which parts the round at the line's end, so a seam carries it on.
    let mut stops = rounded(block(0.0, 4.0, 4.0));
    prism_ops::add_segment(
        &mut stops,
        DVec3::new(2.0, 2.0, 2.0),
        DVec3::new(2.0, R, 2.0),
    );
    let mut lined = block(0.0, 4.0, 4.0);
    prism_ops::add_segment(
        &mut lined,
        DVec3::new(2.0, 2.0, 2.0),
        DVec3::new(2.0, 0.0, 2.0),
    );
    let across = rounded(lined);
    let mid = DVec3::new(2.0, 0.6, 1.4);
    let lcams = [
        ("near", look(mid, 4.5, -60.0, 30.0)),
        ("whole", look(DVec3::new(2.0, 1.0, 1.0), 9.0, -60.0, 25.0)),
    ];
    shoot(&mut g, &stops, "line-stops", &lcams);
    shoot(&mut g, &across, "line-across", &lcams);
}

/// The owner's stepped block (2026-11-27): the top front edge and the
/// slope's front edge rounded together. The chain ends where the slope
/// meets the lower step, which is where an end cap shows.
#[test]
#[ignore]
fn a_stepped_block_rounded_down_its_slope() {
    let Some(mut g) = Gpu::new() else {
        eprintln!("no GPU");
        return;
    };
    let pts = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(7.0, 0.0, 0.0),
        DVec3::new(7.0, 0.0, 1.2),
        DVec3::new(4.5, 0.0, 1.2),
        DVec3::new(3.0, 0.0, 3.0),
        DVec3::new(0.0, 0.0, 3.0),
    ];
    let mut m = Model::new();
    prism_ops::add_polygon(&mut m, &pts);
    let f = prism_ops::face_at(&m, DVec3::new(1.0, 0.0, 1.0)).expect("side");
    let n = m.face(f).unwrap().plane().unwrap().normal.y;
    prism_ops::push_pull(&mut m, f, if n > 0.0 { 2.5 } else { -2.5 }, false);
    let on = |p: DVec3, a: DVec3, b: DVec3| {
        let t = (p - a).dot(b - a) / (b - a).length_squared();
        (-1e-9..=1.0 + 1e-9).contains(&t) && (a + (b - a) * t).distance(p) < 1e-9
    };
    let chosen: BTreeSet<EdgeId> = m
        .edges()
        .filter(|(_, e)| {
            [(pts[5], pts[4]), (pts[4], pts[3])]
                .iter()
                .any(|&(a, b)| on(m.vertex(e.a), a, b) && on(m.vertex(e.b), a, b))
        })
        .map(|(id, _)| id)
        .collect();
    assert_eq!(chosen.len(), 2);
    for size in [0.5, 0.3] {
        let mut r = m.clone();
        match prism_ops::rounds::round_edges(&mut r, &chosen, size, true) {
            Ok(_) => eprintln!("rounded at {size}"),
            Err(why) => eprintln!("refused at {size}: {why}"),
        }
        let foot = DVec3::new(4.5, 0.3, 1.2);
        shoot(
            &mut g,
            &r,
            &format!("step-{size}"),
            &[
                ("front", look(DVec3::new(3.5, 1.2, 1.5), 10.0, -70.0, 22.0)),
                ("foot", look(foot, 3.0, -55.0, 35.0)),
                ("above", look(foot, 3.0, -100.0, 60.0)),
            ],
        );
    }
}

/// The owner's corner test (2026-11-27): the curve where a sloping wall
/// cuts a curved one, and the floor's edges, rounded.
#[test]
#[ignore]
fn the_owners_corner_test() {
    let Some(mut g) = Gpu::new() else {
        eprintln!("no GPU");
        return;
    };
    let c = prism_file::read(include_bytes!(
        "../../prism-tools/tests/data/owner-corner-test.prism"
    ))
    .expect("the file");
    let m = c.assembly.root().model.clone();
    let curve: BTreeSet<EdgeId> = (171..=186).map(EdgeId).collect();
    let floor: BTreeSet<EdgeId> = [EdgeId(53), EdgeId(54)].into();
    let cam = look(DVec3::new(6.4, 1.0, 1.0), 6.0, -60.0, 35.0);
    shoot(&mut g, &m, "corner-before", &[("view", cam)]);
    for (name, set, fillet) in [
        ("corner-curve-fillet", &curve, true),
        ("corner-floor-chamfer", &floor, false),
    ] {
        let mut w = m.clone();
        let got = prism_ops::rounds::round_edges(&mut w, set, 0.15, fillet);
        eprintln!("{name}: {:?}", got.map(|f| f.len()));
        shoot(&mut g, &w, name, &[("view", cam)]);
    }
}

/// Brief 56 step 2: a round ending in a flat cap under an overhang.
#[test]
#[ignore]
fn a_round_ending_in_a_cap() {
    let Some(mut g) = Gpu::new() else {
        eprintln!("no GPU");
        return;
    };
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 3.0, 0.0);
    let pull = |m: &mut Model, p: DVec3, d: f64| {
        let f = prism_ops::face_at(m, p).expect("a face");
        let up = m.face(f).unwrap().plane().unwrap().normal.z;
        prism_ops::push_pull(m, f, if up > 0.0 { d } else { -d }, false);
    };
    pull(&mut m, DVec3::new(1.0, 1.0, 0.0), 1.0);
    prism_ops::add_rect(&mut m, 0.0, 2.0, 3.0, 3.0, 1.0);
    pull(&mut m, DVec3::new(1.0, 2.5, 1.0), 1.0);
    pull(&mut m, DVec3::new(2.5, 2.5, 1.0), 1.0);
    let a = prism_ops::vertex_at(&m, DVec3::new(2.0, 0.0, 1.0)).unwrap();
    let b = prism_ops::vertex_at(&m, DVec3::new(2.0, 2.0, 1.0)).unwrap();
    let e = m.edge_between(a, b).expect("the edge");
    let cams = [
        ("near", look(DVec3::new(2.0, 1.6, 0.8), 3.5, -30.0, 25.0)),
        ("whole", look(DVec3::new(1.5, 1.5, 1.0), 7.0, -40.0, 30.0)),
    ];
    shoot(&mut g, &m, "cap-before", &cams);
    let mut w = m.clone();
    prism_ops::rounds::round_edges(&mut w, &[e].into(), 0.4, true).expect("rounded");
    shoot(&mut g, &w, "cap-after", &cams);
}
