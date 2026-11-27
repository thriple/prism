//! Many edges rounded at once through the kernel (Fillet Joins step B,
//! brief 32), checked against volumes worked by hand.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::Surface;
use prism_math::DVec3;
use prism_ops::rounds::{NO_SIZE, refusal, round_edges};
use prism_ops::{add_circle, add_polygon, add_rect, check, face_at, health, push_pull, vertex_at};
use prism_topo::{EdgeId, Model};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn pull(m: &mut Model, p: DVec3, d: f64) {
    let f = face_at(m, p).expect("a face there");
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(m, f, d * up, false);
}

/// A cube of side `l` on the ground.
fn cube(l: f64) -> Model {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, l, l, 0.0);
    pull(&mut m, v(l / 2.0, l / 2.0, 0.0), l);
    m
}

fn edge(m: &Model, p: DVec3, q: DVec3) -> EdgeId {
    let a = vertex_at(m, p).expect("p");
    let b = vertex_at(m, q).expect("q");
    m.edge_between(a, b).expect("an edge")
}

/// The solid's volume as the kernel measures it (exact), checked against
/// Model Health's sum (which draws curved edges as fine segments).
fn volume(m: &Model) -> f64 {
    let seed = m.faces().next().expect("a face").0;
    let exact = prism_ops::bridge::round_trip(&mut m.clone(), seed).expect("a solid");
    let drawn: f64 = m.faces().map(|(id, _)| health::face_volume(m, id)).sum();
    assert!((exact - drawn).abs() < 1e-3 * exact, "{exact} {drawn}");
    exact
}

fn clean(m: &Model) {
    assert_eq!(check::problems(m), Vec::<String>::new());
    let h = health::check(m);
    assert_eq!((h.watertight, h.leaky), (1, 0), "{h:?}");
    assert!(h.reversed.is_empty(), "{h:?}");
}

#[test]
fn one_edge_filleted_removes_its_corner() {
    let (l, r) = (2.0, 0.3);
    let mut m = cube(l);
    let e = edge(&m, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    round_edges(&mut m, &BTreeSet::from([e]), r, true).unwrap();
    clean(&m);
    let want = l * l * l - r * r * (1.0 - PI / 4.0) * l;
    assert!((volume(&m) - want).abs() < 1e-7, "{} {want}", volume(&m));
    // The round is a cylinder face, Default paint, joined softly.
    assert_eq!(
        m.faces()
            .filter(|(_, f)| matches!(f.surface, Surface::Cylinder(_)))
            .count(),
        1
    );
    assert_eq!(m.edges().filter(|(_, e)| e.soft && e.smooth).count(), 2);
}

#[test]
fn one_edge_through_the_kernel_matches_the_old_round() {
    let mut a = cube(2.0);
    let mut b = a.clone();
    let e = edge(&a, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    round_edges(&mut a, &BTreeSet::from([e]), 0.3, true).unwrap();
    assert!(prism_ops::round::round(&mut b, e, 0.3, true));
    assert!((volume(&a) - volume(&b)).abs() < 1e-7);
    assert_eq!(a.face_count(), b.face_count());
    let mut a = cube(2.0);
    let mut b = a.clone();
    round_edges(&mut a, &BTreeSet::from([e]), 0.3, false).unwrap();
    assert!(prism_ops::round::round(&mut b, e, 0.3, false));
    assert!((volume(&a) - volume(&b)).abs() < 1e-7);
}

#[test]
fn a_boxs_four_top_edges_round_with_mitred_corners() {
    let (l, r) = (2.0, 0.3);
    let mut m = cube(l);
    let top = face_at(&m, v(1.0, 1.0, 2.0)).unwrap();
    let edges: BTreeSet<EdgeId> = m.face(top).unwrap().edge_ids().collect();
    assert_eq!(edges.len(), 4);
    round_edges(&mut m, &edges, r, true).unwrap();
    clean(&m);
    let a = r * r * (1.0 - PI / 4.0);
    let want = l * l * l - 4.0 * a * (l - 2.0 * r) - 4.0 * r * r * r / 3.0;
    assert!((volume(&m) - want).abs() < 1e-7, "{} {want}", volume(&m));
    // The top face is still there with its id, smaller.
    assert!(m.face(top).is_some());
    // The mitres are exact curves, not approximations.
    assert!(m.edges().all(|(_, e)| !e.approx));
}

#[test]
fn all_twelve_edges_make_a_rounded_cube() {
    let (l, r) = (2.0, 0.3);
    let mut m = cube(l);
    let edges: BTreeSet<EdgeId> = m.edges().map(|(id, _)| id).collect();
    assert_eq!(edges.len(), 12);
    round_edges(&mut m, &edges, r, true).unwrap();
    clean(&m);
    let k = l - 2.0 * r;
    let want = k * k * k + 6.0 * r * k * k + 3.0 * PI * r * r * k + 4.0 / 3.0 * PI * r * r * r;
    assert!((volume(&m) - want).abs() < 1e-7, "{} {want}", volume(&m));
    assert_eq!(
        m.faces()
            .filter(|(_, f)| matches!(f.surface, Surface::Sphere(_)))
            .count(),
        8,
        "a ball at each corner"
    );
}

#[test]
fn a_box_chamfered_on_its_top_edges() {
    let (l, d) = (2.0, 0.2);
    let mut m = cube(l);
    let top = face_at(&m, v(1.0, 1.0, 2.0)).unwrap();
    let edges: BTreeSet<EdgeId> = m.face(top).unwrap().edge_ids().collect();
    round_edges(&mut m, &edges, d, false).unwrap();
    clean(&m);
    // Four prisms of section d²/2 along the straight runs, and at each
    // corner the two half-runs to the mitre: 2 · ∫ (d - y)² dy = 2d³/3.
    let want = l * l * l - 4.0 * (d * d / 2.0) * (l - 2.0 * d) - 4.0 * 2.0 * d * d * d / 3.0;
    assert!((volume(&m) - want).abs() < 1e-7, "{} {want}", volume(&m));
}

#[test]
fn a_sixty_degree_edge_is_filleted() {
    // An equilateral triangle pulled up; its upright edges meet at 60°.
    let (s, h, r) = (2.0, 1.0, 0.2);
    let mut m = Model::new();
    let c = v(s / 2.0, s * 3f64.sqrt() / 2.0, 0.0);
    add_polygon(&mut m, &[v(0.0, 0.0, 0.0), v(s, 0.0, 0.0), c]);
    pull(&mut m, v(1.0, 0.5, 0.0), h);
    let e = edge(&m, v(0.0, 0.0, 0.0), v(0.0, 0.0, h));
    let before = volume(&m);
    round_edges(&mut m, &BTreeSet::from([e]), r, true).unwrap();
    clean(&m);
    let removed = r * r * (3f64.sqrt() - PI / 3.0) * h;
    assert!(
        (before - volume(&m) - removed).abs() < 1e-7,
        "{}",
        before - volume(&m)
    );
}

#[test]
fn the_top_edge_of_a_cylinder_is_filleted() {
    let (big, h, s) = (1.0, 2.0, 0.2);
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, big);
    pull(&mut m, DVec3::ZERO, h);
    let top = face_at(&m, v(0.0, 0.0, h)).unwrap();
    let edges: BTreeSet<EdgeId> = m.face(top).unwrap().edge_ids().collect();
    let before = volume(&m);
    round_edges(&mut m, &edges, s, true).unwrap();
    clean(&m);
    // Pappus: the corner's cross-section turned about the axis.
    let a = s * s * (1.0 - PI / 4.0);
    let xbar = s * (5.0 / 6.0 - PI / 4.0) / (1.0 - PI / 4.0);
    let removed = 2.0 * PI * (big - xbar) * a;
    assert!(
        (before - volume(&m) - removed).abs() < 1e-7,
        "{} {removed}",
        before - volume(&m)
    );
}

#[test]
fn rounds_are_refused_with_a_reason_and_change_nothing() {
    let mut m = cube(2.0);
    let e = edge(&m, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    let before = m.clone();
    assert_eq!(refusal(&m, &BTreeSet::from([e]), 0.0, true), Some(NO_SIZE));
    assert_eq!(
        refusal(&m, &BTreeSet::from([e]), 2.5, true),
        Some(prism_kernel::solid::TOO_LARGE)
    );
    assert!(round_edges(&mut m, &BTreeSet::from([e]), 2.5, true).is_err());
    assert_eq!(m, before);
    // An edge of a loose rectangle beside the cube.
    add_rect(&mut m, 5.0, 0.0, 6.0, 1.0, 0.0);
    let loose = edge(&m, v(5.0, 0.0, 0.0), v(6.0, 0.0, 0.0));
    assert_eq!(
        refusal(&m, &BTreeSet::from([loose]), 0.1, true),
        Some(prism_ops::rounds::ONE_FACE)
    );
    assert_eq!(
        refusal(&m, &BTreeSet::from([e, loose]), 0.1, true),
        Some(prism_ops::rounds::ONE_FACE)
    );
}

/// A line drawn on an end face, and a point where a side edge was split,
/// are not corners of the solid's shape: a round runs past them (owner's
/// session 22: a round stopped at the first of them). The line is drawn
/// back on afterwards, as far as it still lies on the solid.
#[test]
fn a_round_runs_past_lines_drawn_on_the_faces_at_its_ends() {
    let (l, r) = (2.0, 0.3);
    let mut m = cube(l);
    // A line up the end face y = 0, 0.05 from the edge to round.
    prism_ops::add_segment(&mut m, v(1.95, 0.0, 0.0), v(1.95, 0.0, 2.0));
    // The upright edge at that end split 0.03 below the top.
    let tol = m.tol();
    prism_ops::insert_point(&mut m, v(2.0, 0.0, 1.97), &tol);
    let faces = m.face_count();
    let e = edge(&m, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    round_edges(&mut m, &BTreeSet::from([e]), r, true).unwrap();
    clean(&m);
    let want = l * l * l - r * r * (1.0 - PI / 4.0) * l;
    assert!((volume(&m) - want).abs() < 1e-7, "{} {want}", volume(&m));
    // The line is back, from the ground up to the round, and nothing of
    // it is left in the air.
    let up: Vec<(DVec3, DVec3)> = m
        .edges()
        .map(|(_, e)| (m.vertex(e.a), m.vertex(e.b)))
        .filter(|(a, b)| {
            (a.x - 1.95).abs() < 1e-9
                && (b.x - 1.95).abs() < 1e-9
                && a.y.abs() < 1e-9
                && b.y.abs() < 1e-9
        })
        .collect();
    assert!(!up.is_empty(), "the line is back");
    let top = up.iter().map(|(a, b)| a.z.max(b.z)).fold(0.0, f64::max);
    let on_round = 1.7 + (r * r - 0.25f64 * 0.25).sqrt();
    assert!(
        (top - on_round).abs() < 1e-6,
        "ends on the round: {top} {on_round}"
    );
    assert_eq!(health::check(&m).stray.edges.len(), 0);
    // The cube's six, the end face still in two by the line, and the round.
    assert_eq!((faces, m.face_count()), (7, 8));
}

/// A box 2 long whose top edge is bevelled by a face at `deg` degrees
/// to the top (the profile in y, z pulled along x), and the edge between
/// the top and the bevel.
fn bevelled(deg: f64) -> (Model, EdgeId) {
    let mut m = Model::new();
    let drop = deg.to_radians().tan();
    add_polygon(
        &mut m,
        &[
            v(0.0, 0.0, 0.0),
            v(0.0, 2.0, 0.0),
            v(0.0, 2.0, 1.0),
            v(0.0, 1.0, 1.0),
            v(0.0, 0.0, 1.0 - drop),
        ],
    );
    let f = face_at(&m, v(0.0, 1.0, 0.5)).expect("the profile");
    let out = m.face(f).unwrap().plane().unwrap().normal.x;
    push_pull(&mut m, f, 2.0 * out, false);
    let e = edge(&m, v(0.0, 1.0, 1.0), v(2.0, 1.0, 1.0));
    (m, e)
}

/// The kernel refuses a small round on a shallow edge as too large (at
/// 12 degrees every size; at 45 degrees, 5 mm): Prism's own round makes
/// it, a cylinder tangent to both faces (owner's Xby4, 2026-11-26).
#[test]
fn a_shallow_edge_takes_a_small_round() {
    for (deg, r) in [
        (12.0_f64, 0.005),
        (12.0, 0.02),
        (12.0, 0.2),
        (45.0, 0.005),
        (27.0, 0.05),
    ] {
        let (mut m, e) = bevelled(deg);
        let whole = volume(&m);
        round_edges(&mut m, &[e].into(), r, true).unwrap_or_else(|w| panic!("{deg} {r}: {w}"));
        clean(&m);
        // What a fillet takes off: between the two tangent lines and the
        // arc, r²(tan(φ/2) − φ/2) for a turn of φ, along the 2 m edge.
        let phi = deg.to_radians();
        let cut = r * r * ((phi / 2.0).tan() - phi / 2.0) * 2.0;
        assert!(
            (whole - volume(&m) - cut).abs() < 1e-9 + 1e-6 * cut,
            "{deg} {r}: {} {cut}",
            whole - volume(&m)
        );
    }
}

/// A line drawn on a flat face has no corner to round, and says so.
#[test]
fn a_line_on_a_flat_face_is_not_rounded() {
    let mut m = cube(2.0);
    prism_ops::add_segment(&mut m, v(1.0, 0.0, 0.0), v(1.0, 0.0, 2.0));
    let e = edge(&m, v(1.0, 0.0, 0.0), v(1.0, 0.0, 2.0));
    assert_eq!(
        refusal(&m, &[e].into(), 0.1, true),
        Some(prism_ops::rounds::FLAT_LINE)
    );
}

/// A fillet or chamfer needs only the two faces beside the edge, not a
/// closed solid (owner, 2026-11-26): an open box's corner rounds, and so
/// does an edge where two loose faces meet.
#[test]
fn an_open_box_and_two_loose_faces_round() {
    let l = 2.0;
    let mut m = cube(l);
    let top = face_at(&m, v(1.0, 1.0, l)).expect("the top");
    m.remove_face(top);
    let e = edge(&m, v(l, 0.0, 0.0), v(l, 0.0, l));
    for (fillet, r) in [(true, 0.3), (false, 0.3)] {
        let mut w = m.clone();
        round_edges(&mut w, &BTreeSet::from([e]), r, fillet)
            .unwrap_or_else(|why| panic!("{fillet}: {why}"));
        assert_eq!(check::problems(&w), Vec::<String>::new());
        // The bottom keeps its corner cut to match, the top rim is open.
        assert!(face_at(&w, v(l - 0.01, 0.01, 0.0)).is_none(), "{fillet}");
        assert_eq!(w.faces().count(), m.faces().count() + 1);
    }
    // Two loose faces meeting at an edge: an L of two rectangles.
    let mut m = Model::new();
    add_polygon(
        &mut m,
        &[
            v(0.0, 0.0, 0.0),
            v(2.0, 0.0, 0.0),
            v(2.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
        ],
    );
    add_polygon(
        &mut m,
        &[
            v(0.0, 0.0, 0.0),
            v(2.0, 0.0, 0.0),
            v(2.0, 0.0, 1.0),
            v(0.0, 0.0, 1.0),
        ],
    );
    let e = edge(&m, v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0));
    round_edges(&mut m, &BTreeSet::from([e]), 0.2, true).expect("rounded");
    assert_eq!(check::problems(&m), Vec::<String>::new());
    assert_eq!(m.faces().count(), 3);
}

/// An L-shaped block 1 high: its outline (0,0) (3,0) (3,1) (1,1) (1,3)
/// (0,3) pulled up.
fn l_block() -> Model {
    let mut m = Model::new();
    add_polygon(
        &mut m,
        &[
            v(0.0, 0.0, 0.0),
            v(3.0, 0.0, 0.0),
            v(3.0, 1.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(1.0, 3.0, 0.0),
            v(0.0, 3.0, 0.0),
        ],
    );
    pull(&mut m, v(0.5, 0.5, 0.0), 1.0);
    m
}

/// Where an inside round (the L's inner vertical edge) meets outside ones
/// (its top edges) the kernel cannot close the corner with all of them at
/// once; rounded in turn, each later round runs onto the faces the
/// earlier made (DECISIONS #46).
#[test]
fn an_inside_round_meets_outside_ones_at_a_corner() {
    let m = l_block();
    let edges: BTreeSet<EdgeId> = m
        .edges()
        .filter(|(_, e)| {
            let (a, b) = (m.vertex(e.a), m.vertex(e.b));
            (a.z == 1.0 && b.z == 1.0) || (a.x == 1.0 && b.x == 1.0 && a.y == 1.0 && b.y == 1.0)
        })
        .map(|(id, _)| id)
        .collect();
    assert_eq!(edges.len(), 7);
    let whole = volume(&m);
    for (fillet, r) in [(true, 0.1), (true, 0.2), (false, 0.1)] {
        let mut w = m.clone();
        round_edges(&mut w, &edges, r, fillet).unwrap_or_else(|why| panic!("{fillet} {r}: {why}"));
        clean(&w);
        // Less than the block, more than the block less its rounds' edges
        // at full size (the inside one adds, the outside ones take away).
        let after = volume(&w);
        assert!(
            after < whole + 1.0 && (whole - after).abs() < 12.0 * r * r,
            "{after} {whole}"
        );
    }
}

/// Four rounds meeting at a pyramid's apex: the kernel cannot close that
/// corner at all. It is cut off by a small flat face past the rounds'
/// reach first, so each round ends at a plain corner (a step toward a
/// smooth corner patch, DECISIONS #46).
#[test]
fn four_rounds_meet_at_an_apex() {
    let mut py = Model::new();
    let base = [
        v(0.0, 0.0, 0.0),
        v(2.0, 0.0, 0.0),
        v(2.0, 2.0, 0.0),
        v(0.0, 2.0, 0.0),
    ];
    let apex = v(1.0, 1.0, 1.5);
    for i in 0..4 {
        add_polygon(&mut py, &[base[(i + 1) % 4], base[i], apex]);
    }
    add_polygon(&mut py, &[base[3], base[2], base[1], base[0]]);
    health::orient(&mut py, None);
    let slants: BTreeSet<EdgeId> = py
        .edges()
        .filter(|(_, e)| py.vertex(e.a) == apex || py.vertex(e.b) == apex)
        .map(|(id, _)| id)
        .collect();
    for r in [0.05, 0.2] {
        let mut w = py.clone();
        round_edges(&mut w, &slants, r, true).unwrap_or_else(|why| panic!("{r}: {why}"));
        clean(&w);
        assert!(vertex_at(&w, apex).is_none(), "the apex is cut off");
    }
}

/// Edges that meet, off a closed solid (an open box's two upright edges
/// and the bottom edge between them): the opening is closed by a flat
/// face for the kernel and taken away after, so the rounds meet at the
/// corner as on a solid (owner, 2026-11-26: "fillet/chamfer should work
/// along that whole edge, no complaints").
#[test]
fn edges_that_meet_round_off_a_closed_solid() {
    let l = 2.0;
    let mut m = cube(l);
    let top = face_at(&m, v(1.0, 1.0, l)).expect("the top");
    m.remove_face(top);
    let edges = BTreeSet::from([
        edge(&m, v(l, 0.0, 0.0), v(l, 0.0, l)),
        edge(&m, v(0.0, 0.0, 0.0), v(0.0, 0.0, l)),
        edge(&m, v(0.0, 0.0, 0.0), v(l, 0.0, 0.0)),
    ]);
    for fillet in [true, false] {
        let mut w = m.clone();
        round_edges(&mut w, &edges, 0.3, fillet).unwrap_or_else(|why| panic!("{fillet}: {why}"));
        assert_eq!(check::problems(&w), Vec::<String>::new());
        assert!(
            face_at(&w, v(1.0, 1.0, l)).is_none(),
            "{fillet}: still open at the top"
        );
        let h = health::check(&w);
        assert!(h.reversed.is_empty(), "{fillet}: {h:?}");
    }
}

/// A line drawn into a face from its edge, ending inside it, stays on the
/// face when the edges round (it is drawn back if it had to be taken off).
#[test]
fn a_box_with_a_line_into_a_face_rounds_edges_that_meet() {
    let l = 2.0;
    let mut m = cube(l);
    prism_ops::add_segment(&mut m, v(1.0, 0.0, 0.0), v(1.0, 0.0, 0.6));
    // How much of the line is there.
    let slit = |m: &Model| -> f64 {
        m.edges()
            .map(|(_, e)| (m.vertex(e.a), m.vertex(e.b)))
            .filter(|(a, b)| {
                [a, b]
                    .iter()
                    .all(|p| (p.x - 1.0).abs() < 1e-9 && p.y.abs() < 1e-9)
            })
            .map(|(a, b)| a.distance(b))
            .sum()
    };
    assert!((slit(&m) - 0.6).abs() < 1e-9);
    let top: BTreeSet<EdgeId> = [
        edge(&m, v(0.0, 0.0, l), v(l, 0.0, l)),
        edge(&m, v(l, 0.0, l), v(l, l, l)),
        edge(&m, v(l, l, l), v(0.0, l, l)),
        edge(&m, v(0.0, l, l), v(0.0, 0.0, l)),
    ]
    .into();
    for fillet in [true, false] {
        let mut w = m.clone();
        round_edges(&mut w, &top, 0.3, fillet).unwrap_or_else(|why| panic!("{fillet}: {why}"));
        assert_eq!(check::problems(&w), Vec::<String>::new());
        assert!(
            (slit(&w) - 0.6).abs() < 1e-9,
            "{fillet}: the line is drawn back"
        );
        assert_eq!(w.faces().count(), m.faces().count() + 4, "{fillet}");
    }
}

/// A top edge running into a taller block behind it rounds: the round's
/// end lies in the block's front (brief 56 step 2, keep surface).
#[test]
fn a_round_runs_into_a_wall_behind_it() {
    // An L-shaped profile in the plane x = 0, pulled 2 along x: a low
    // block (z up to 1) with a tall one (z up to 2) behind it (y 2 to 3).
    let mut m = Model::new();
    add_polygon(
        &mut m,
        &[
            v(0.0, 0.0, 0.0),
            v(0.0, 3.0, 0.0),
            v(0.0, 3.0, 2.0),
            v(0.0, 2.0, 2.0),
            v(0.0, 2.0, 1.0),
            v(0.0, 0.0, 1.0),
        ],
    );
    let f = face_at(&m, v(0.0, 1.0, 0.5)).expect("profile");
    let nx = m.face(f).unwrap().plane().unwrap().normal.x;
    push_pull(&mut m, f, if nx > 0.0 { 2.0 } else { -2.0 }, false);
    let e = edge(&m, v(2.0, 0.0, 1.0), v(2.0, 2.0, 1.0));
    let along_wall = edge(&m, v(0.0, 2.0, 1.0), v(2.0, 2.0, 1.0));
    for (name, set) in [
        ("edge", BTreeSet::from([e])),
        ("edge and wall foot", BTreeSet::from([e, along_wall])),
    ] {
        for fillet in [true, false] {
            let mut w = m.clone();
            round_edges(&mut w, &set, 0.3, fillet)
                .unwrap_or_else(|why| panic!("{name} {fillet}: {why}"));
            clean(&w);
        }
    }
}

/// The same, the block behind wider than the low one.
#[test]
fn a_round_runs_into_a_wider_wall_behind_it() {
    // A low block (x 0..2, y 0..2, z 0..1) against a taller, wider one
    // (x 0..3, y 2..3, z 0..2): the low top edge at x = 2 runs into the
    // tall one's front, which goes on past it.
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    pull(&mut m, v(1.0, 1.0, 0.0), 1.0);
    add_rect(&mut m, 0.0, 2.0, 3.0, 3.0, 0.0);
    pull(&mut m, v(1.5, 2.5, 0.0), 2.0);
    clean(&m);
    let e = edge(&m, v(2.0, 0.0, 1.0), v(2.0, 2.0, 1.0));
    for fillet in [true, false] {
        let mut w = m.clone();
        round_edges(&mut w, &BTreeSet::from([e]), 0.3, fillet)
            .unwrap_or_else(|why| panic!("{fillet}: {why}"));
        clean(&w);
    }
}

/// Brief 56 step 2, the end cap: the block behind sits on the low one and
/// overhangs its side, so five edges meet at the round's end and the
/// kernel will not round it. The solid is cut square to the edge there,
/// the edge rounded, and the pieces joined: the round ends in a flat cap
/// in the block's front, and the volume taken is the round's corner area
/// along the edge.
#[test]
fn a_round_ends_in_a_cap_under_an_overhang() {
    // A low block (x 0..2, y 0..3, z 0..1) with a tall one sitting on its
    // back (x 0..3, y 2..3, z 1..2), overhanging its side by 1: the low
    // top edge at x = 2 runs into the tall one's front, and the side face
    // under it meets the overhang's underside (session 29's edge 2).
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 2.0, 3.0, 0.0);
    pull(&mut m, v(1.0, 1.0, 0.0), 1.0);
    add_rect(&mut m, 0.0, 2.0, 3.0, 3.0, 1.0);
    for p in [v(1.0, 2.5, 1.0), v(2.5, 2.5, 1.0)] {
        let f = face_at(&m, p).expect("a piece");
        let up = m.face(f).unwrap().plane().unwrap().normal.z;
        push_pull(&mut m, f, if up > 0.0 { 1.0 } else { -1.0 }, false);
    }
    clean(&m);
    let e = edge(&m, v(2.0, 0.0, 1.0), v(2.0, 2.0, 1.0));
    let at = vertex_at(&m, v(2.0, 2.0, 1.0)).unwrap();
    assert_eq!(m.edges().filter(|(_, x)| x.has(at)).count(), 5);
    let r = 0.3;
    for (fillet, area) in [(true, r * r * (1.0 - PI / 4.0)), (false, r * r / 2.0)] {
        let mut w = m.clone();
        round_edges(&mut w, &BTreeSet::from([e]), r, fillet)
            .unwrap_or_else(|why| panic!("{fillet}: {why}"));
        clean(&w);
        let taken = volume(&m) - volume(&w);
        assert!((taken - area * 2.0).abs() < 1e-6, "{fillet}: {taken}");
        // The cap: a flat face in the block's front plane (y = 2) where
        // the round ends.
        assert!(
            face_at(&w, v(1.95, 2.0, 0.95)).is_some(),
            "{fillet}: the cap"
        );
    }
}

/// The owner's shape (2026-11-27, "refusing to go down an arc from a
/// point"): a profile pulled along x, its top flat, dropping at a point
/// into a cove, flat again, then a bullnose to the end. Every edge of the
/// chain along the top rounds alone, and so do pairs that meet smoothly
/// (bullnose and flat, flat and cove). The cove with the flat at the point
/// does not yet: two rounds meeting at a sharp corner, brief 56 step 5.
#[test]
fn the_cove_and_bullnose_chain_rounds_where_it_meets_smoothly() {
    use std::f64::consts::FRAC_PI_2;
    let mut m = Model::new();
    let p = |y: f64, z: f64| v(0.0, y, z);
    prism_ops::add_segment(&mut m, p(0.0, 0.0), p(3.5, 0.0));
    prism_ops::add_segment(&mut m, p(3.5, 0.0), p(3.5, 1.0));
    prism_ops::add_arc(&mut m, p(3.0, 1.0), DVec3::X, 0.5, p(3.5, 1.0), FRAC_PI_2);
    prism_ops::add_segment(&mut m, p(3.0, 1.5), p(2.0, 1.5));
    prism_ops::add_arc(&mut m, p(2.0, 2.0), DVec3::X, 0.5, p(1.5, 2.0), FRAC_PI_2);
    prism_ops::add_segment(&mut m, p(1.5, 2.0), p(0.0, 2.0));
    prism_ops::add_segment(&mut m, p(0.0, 2.0), p(0.0, 0.0));
    let f = face_at(&m, v(0.0, 1.0, 0.5)).expect("profile");
    let nx = m.face(f).unwrap().plane().unwrap().normal.x;
    push_pull(&mut m, f, if nx > 0.0 { 1.5 } else { -1.5 }, false);
    clean(&m);
    let at = |y0: f64, z0: f64, y1: f64, z1: f64| edge(&m, v(1.5, y0, z0), v(1.5, y1, z1));
    let bullnose = at(3.5, 1.0, 3.0, 1.5);
    let flat = at(3.0, 1.5, 2.0, 1.5);
    let cove = at(2.0, 1.5, 1.5, 2.0);
    let top = at(1.5, 2.0, 0.0, 2.0);
    for set in [
        vec![bullnose],
        vec![flat],
        vec![cove],
        vec![top],
        vec![bullnose, flat],
        vec![flat, cove],
        vec![bullnose, flat, cove],
    ] {
        for fillet in [true, false] {
            let mut w = m.clone();
            let set: BTreeSet<EdgeId> = set.iter().copied().collect();
            round_edges(&mut w, &set, 0.1, fillet)
                .unwrap_or_else(|why| panic!("{set:?} {fillet}: {why}"));
            clean(&w);
        }
    }
}

/// Brief 56 step 2 (owner, 2026-11-27: "B for both"): a line drawn up to a
/// rounded edge carries on across the round as a seam, on Prism's own
/// round too (here a shallow edge, which the kernel will not round) as on
/// the kernel's.
#[test]
fn a_line_to_the_edge_carries_on_across_prisms_own_round() {
    for fillet in [true, false] {
        let (mut m, e) = bevelled(12.0);
        prism_ops::add_segment(&mut m, v(1.0, 2.0, 1.0), v(1.0, 1.0, 1.0));
        let e = m
            .edges()
            .find(|(_, x)| {
                let (a, b) = (m.vertex(x.a), m.vertex(x.b));
                (a.y - 1.0).abs() < 1e-9
                    && (b.y - 1.0).abs() < 1e-9
                    && (a.z - 1.0).abs() < 1e-9
                    && (b.z - 1.0).abs() < 1e-9
                    && a.x.max(b.x) > 1.5
            })
            .map(|(id, _)| id)
            .unwrap_or(e);
        let mut run = prism_ops::rounds::straight_run(&m, e);
        run.insert(e);
        round_edges(&mut m, &run, 0.05, fillet).unwrap_or_else(|w| panic!("{fillet}: {w}"));
        clean(&m);
        // Across the round at x = 1: an edge lying in that plane, on the
        // round's own face (not on the top or the bevel).
        let across = m.edges().any(|(_, x)| {
            let (a, b) = (m.vertex(x.a), m.vertex(x.b));
            (a.x - 1.0).abs() < 1e-9
                && (b.x - 1.0).abs() < 1e-9
                && a.distance(b) > 0.001
                && a.y.min(b.y) < 1.0
                && a.z.min(b.z) < 1.0
        });
        assert!(across, "{fillet}: the line carries on across the round");
    }
}

/// Brief 56 step 3: a fault far from the edge (an edge at the other end
/// of the block stored as an approximation, which the kernel will not
/// take) does not stop a round: it is rounded in a box around it, and the
/// far edge is left as it was.
#[test]
fn a_fault_far_away_does_not_stop_a_round() {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 4.0, 1.0, 0.0);
    pull(&mut m, v(2.0, 0.5, 0.0), 1.0);
    let far = edge(&m, v(0.0, 0.0, 1.0), v(0.0, 1.0, 1.0));
    m.set_approx(far, true);
    let whole = volume_drawn(&m);
    let e = edge(&m, v(4.0, 0.0, 1.0), v(4.0, 1.0, 1.0));
    round_edges(&mut m, &[e].into(), 0.2, true).expect("rounded");
    assert_eq!(check::problems(&m), Vec::<String>::new());
    let h = health::check(&m);
    assert_eq!((h.watertight, h.leaky), (1, 0), "{h:?}");
    // The far edge is still there, still approximate.
    let a = vertex_at(&m, v(0.0, 0.0, 1.0)).expect("far corner");
    let b = vertex_at(&m, v(0.0, 1.0, 1.0)).expect("far corner");
    let far = m.edge_between(a, b).expect("the far edge");
    assert!(m.edge(far).unwrap().approx);
    // A fillet takes (1 - pi/4) r^2 along the 1 m edge.
    let taken = whole - volume_drawn(&m);
    let want = (1.0 - PI / 4.0) * 0.04;
    assert!((taken - want).abs() < 1e-3 * want + 1e-6, "{taken} {want}");
}

/// Model Health's volume (curved faces drawn as fine segments).
fn volume_drawn(m: &Model) -> f64 {
    m.faces().map(|(id, _)| health::face_volume(m, id)).sum()
}
