//! The model-wide rules (sticky geometry, one face per region of a plane,
//! points and edges that land on others become one) with circles and
//! arcs, through the commands every Draw and Edit tool sends. The owner's
//! session 7 found arcs not following them (2026-11-25).

use std::collections::BTreeSet;

use prism_doc::{Command, Document, Motion};
use prism_math::DVec3;
use prism_ops::Selection;
use prism_topo::Model;

fn clean(m: &Model) {
    assert_eq!(prism_ops::check::problems(m), Vec::<String>::new());
    // No arc twice (same ends, same middle).
    let arcs: Vec<_> = m.edges().filter(|(_, e)| e.curve.is_arc()).collect();
    for (i, (ia, a)) in arcs.iter().enumerate() {
        for (ib, b) in &arcs[i + 1..] {
            if (a.a, a.b) == (b.a, b.b) || (a.a, a.b) == (b.b, b.a) {
                let ma = a.curve.point(m.vertex(a.a), m.vertex(a.b), 0.5);
                let mb = b.curve.point(m.vertex(b.a), m.vertex(b.b), 0.5);
                assert!(
                    ma.distance(mb) > 1e-6,
                    "{ia:?} and {ib:?} are one arc twice"
                );
            }
        }
    }
}

fn circle(d: &mut Document, x: f64, y: f64, z: f64, r: f64) {
    assert!(d.run(Command::Circle {
        centre: DVec3::new(x, y, z),
        normal: DVec3::Z,
        radius: r,
    }));
}

fn face(d: &Document, p: DVec3) -> prism_topo::FaceId {
    prism_ops::sticky::face_at(d.model(), p).unwrap_or_else(|| panic!("no face at {p}"))
}

fn arcs(d: &Document) -> usize {
    d.model().edges().filter(|(_, e)| e.curve.is_arc()).count()
}

fn rect(d: &mut Document, x0: f64, y0: f64, x1: f64, y1: f64, z: f64) {
    assert!(d.run(Command::Rect { x0, y0, x1, y1, z }));
}

#[test]
fn two_overlapping_circles_make_three_regions() {
    let mut d = Document::new();
    circle(&mut d, 0.0, 0.0, 0.0, 1.0);
    circle(&mut d, 1.0, 0.0, 0.0, 1.0);
    assert_eq!(d.model().face_count(), 3);
    clean(d.model());
}

#[test]
fn the_same_circle_drawn_twice_is_one_circle() {
    let mut d = Document::new();
    circle(&mut d, 0.0, 0.0, 0.0, 1.0);
    let (f, e) = (d.model().face_count(), d.model().edges().count());
    d.run(Command::Circle {
        centre: DVec3::ZERO,
        normal: DVec3::Z,
        radius: 1.0,
    });
    assert_eq!((d.model().face_count(), d.model().edges().count()), (f, e));
    clean(d.model());
}

#[test]
fn a_circle_on_a_box_top_cuts_it_and_pulls_through_as_a_hole() {
    let mut d = Document::new();
    rect(&mut d, 0.0, 0.0, 4.0, 4.0, 0.0);
    let base = face(&d, DVec3::new(2.0, 2.0, 0.0));
    let up = d.model().face(base).unwrap().plane().unwrap().normal.z;
    assert!(d.run(Command::PushPull {
        face: base,
        distance: 2.0 * up,
        new_loop: false
    }));
    circle(&mut d, 2.0, 2.0, 2.0, 1.0);
    assert_eq!(d.model().face_count(), 7, "the top has a disc and a ring");
    clean(d.model());
    // Down through the bottom: a hole.
    let disc = face(&d, DVec3::new(2.0, 2.0, 2.0));
    let n = d.model().face(disc).unwrap().plane().unwrap().normal.z;
    assert!(d.run(Command::PushPull {
        face: disc,
        distance: -2.0 * n,
        new_loop: false
    }));
    clean(d.model());
    assert!(
        prism_ops::sticky::face_at(d.model(), DVec3::new(2.0, 2.0, 0.0)).is_none(),
        "through"
    );
    assert!(prism_ops::validate::validate(d.model())[0].valid);
}

#[test]
fn a_circle_moved_onto_an_equal_circle_becomes_one() {
    let mut d = Document::new();
    circle(&mut d, 0.0, 0.0, 0.0, 1.0);
    circle(&mut d, 3.0, 0.0, 0.0, 1.0);
    let f = face(&d, DVec3::new(0.0, 0.0, 0.0));
    let vertices: BTreeSet<_> = d.model().face(f).unwrap().outer.iter().copied().collect();
    assert!(d.run(Command::Translate {
        vertices,
        by: DVec3::new(3.0, 0.0, 0.0)
    }));
    assert_eq!(d.model().face_count(), 1);
    assert_eq!(arcs(&d), 2);
    clean(d.model());
}

#[test]
fn a_circle_copied_onto_an_equal_circle_and_half_over_another_splits_right() {
    let mut d = Document::new();
    circle(&mut d, 0.0, 0.0, 0.0, 1.0);
    circle(&mut d, 3.0, 0.0, 0.0, 1.0);
    let f = face(&d, DVec3::new(0.0, 0.0, 0.0));
    let sel = |d: &Document, f| Selection {
        faces: [f].into(),
        edges: d.model().face(f).unwrap().edge_ids().collect(),
        vertices: d.model().face(f).unwrap().outer.iter().copied().collect(),
        ..Default::default()
    };
    // Exactly onto the other: still two circles.
    let s = sel(&d, f);
    assert!(d.run(Command::Copy {
        selection: s,
        step: Motion::Translate {
            by: DVec3::new(3.0, 0.0, 0.0)
        },
        count: 1
    }));
    assert_eq!(d.model().face_count(), 2);
    assert_eq!(arcs(&d), 4);
    clean(d.model());
    // Half over it: three regions there.
    let s = sel(&d, f);
    assert!(d.run(Command::Copy {
        selection: s,
        step: Motion::Translate {
            by: DVec3::new(2.0, 0.0, 0.0)
        },
        count: 1
    }));
    assert_eq!(d.model().face_count(), 4);
    clean(d.model());
}

#[test]
fn a_circle_turned_copy_about_a_point_on_it_overlaps_cleanly() {
    let mut d = Document::new();
    circle(&mut d, 0.0, 0.0, 0.0, 1.0);
    let f = face(&d, DVec3::ZERO);
    let s = Selection {
        faces: [f].into(),
        edges: d.model().face(f).unwrap().edge_ids().collect(),
        vertices: d.model().face(f).unwrap().outer.iter().copied().collect(),
        ..Default::default()
    };
    assert!(d.run(Command::Copy {
        selection: s,
        step: Motion::Rotate {
            centre: DVec3::new(1.0, 0.0, 0.0),
            axis: DVec3::Z,
            angle: 1.0
        },
        count: 1
    }));
    assert_eq!(d.model().face_count(), 3);
    clean(d.model());
}

#[test]
fn an_offset_circle_pulls_as_a_ring() {
    let mut d = Document::new();
    circle(&mut d, 0.0, 0.0, 0.0, 1.0);
    let f = face(&d, DVec3::ZERO);
    assert!(d.run(Command::Offset {
        face: f,
        distance: -0.25
    }));
    assert_eq!(d.model().face_count(), 2);
    clean(d.model());
    let ring = face(&d, DVec3::new(0.9, 0.0, 0.0));
    assert!(d.run(Command::PushPull {
        face: ring,
        distance: 0.5,
        new_loop: false
    }));
    clean(d.model());
}

#[test]
fn erasing_a_circles_arcs_merges_the_face_back() {
    let mut d = Document::new();
    rect(&mut d, 0.0, 0.0, 6.0, 4.0, 0.0);
    circle(&mut d, 3.0, 2.0, 0.0, 1.0);
    assert_eq!(d.model().face_count(), 2);
    let edges: BTreeSet<_> = d
        .model()
        .edges()
        .filter(|(_, e)| e.curve.is_arc())
        .map(|(id, _)| id)
        .collect();
    assert!(d.run(Command::Erase {
        selection: Selection {
            edges,
            ..Default::default()
        }
    }));
    assert_eq!(d.model().face_count(), 1);
    clean(d.model());
}

#[test]
fn a_circle_crossing_a_rectangles_edge_cuts_both() {
    let mut d = Document::new();
    rect(&mut d, 0.0, 0.0, 4.0, 4.0, 0.0);
    circle(&mut d, 4.0, 2.0, 0.0, 1.0);
    assert_eq!(
        d.model().face_count(),
        3,
        "square less a bite, the bite, the outside half"
    );
    clean(d.model());
}

#[test]
fn a_half_circle_on_a_boxs_side_pulls_in_and_the_side_pushes_onto_it() {
    // The owner's wheel well, simplified: a half disc on a box's side,
    // pulled in, then the whole side pushed (new face) to the same plane.
    let mut d = Document::new();
    rect(&mut d, 0.0, 0.0, 4.0, 2.0, 0.0);
    let base = face(&d, DVec3::new(2.0, 1.0, 0.0));
    let up = d.model().face(base).unwrap().plane().unwrap().normal.z;
    assert!(d.run(Command::PushPull {
        face: base,
        distance: 2.0 * up,
        new_loop: false
    }));
    // The side at y = 0 gets a half circle standing on the ground.
    assert!(d.run(Command::Arc {
        centre: DVec3::new(2.0, 0.0, 0.0),
        normal: DVec3::Y,
        radius: 1.0,
        start: DVec3::new(3.0, 0.0, 0.0),
        sweep: std::f64::consts::PI,
        pie: false,
    }));
    clean(d.model());
    let half = face(&d, DVec3::new(2.0, 0.0, 0.5));
    let n = d.model().face(half).unwrap().plane().unwrap().normal.y;
    // In by half a metre.
    assert!(d.run(Command::PushPull {
        face: half,
        distance: 0.5 * -n.signum() * n.abs(),
        new_loop: false
    }));
    clean(d.model());
}

/// Curved faces get the plane's rule (owner, 2026-11-25): where two lie
/// on one surface and overlap, they become one set of faces.
#[test]
fn a_cylinder_copied_half_onto_itself_shares_one_wall_where_they_overlap() {
    let mut d = Document::new();
    circle(&mut d, 0.0, 0.0, 0.0, 1.0);
    let f = face(&d, DVec3::ZERO);
    let up = d.model().face(f).unwrap().plane().unwrap().normal.z;
    assert!(d.run(Command::PushPull {
        face: f,
        distance: 2.0 * up,
        new_loop: false
    }));
    let m = d.model();
    let all = Selection {
        vertices: m.vertices().map(|(i, _)| i).collect(),
        edges: m.edges().map(|(i, _)| i).collect(),
        faces: m.faces().map(|(i, _)| i).collect(),
        ..Default::default()
    };
    assert!(d.run(Command::Copy {
        selection: all,
        step: Motion::Translate {
            by: DVec3::new(0.0, 0.0, 1.0)
        },
        count: 1
    }));
    let m = d.model();
    clean(m);
    let walls_in = |lo: f64, hi: f64| {
        m.faces()
            .filter(|(_, f)| f.is_curved())
            .filter(|(_, f)| {
                let z: Vec<f64> = m.loop_points(f, 0).iter().map(|p| p.z).collect();
                let (a, b) = z
                    .iter()
                    .fold((f64::MAX, f64::MIN), |(a, b), z| (a.min(*z), b.max(*z)));
                (a - lo).abs() < 1e-6 && (b - hi).abs() < 1e-6
            })
            .count()
    };
    let (low, mid, top) = (walls_in(0.0, 1.0), walls_in(1.0, 2.0), walls_in(2.0, 3.0));
    assert!(
        low > 0 && low == mid && mid == top,
        "one wall per band: {low} {mid} {top}"
    );
    let r = prism_ops::health::check(m);
    assert!(r.crossing.faces.is_empty(), "nothing crosses");
}

/// A circle drawn round a cylinder's wall cuts it, as a line cuts a face.
#[test]
fn a_circle_round_a_cylinders_wall_cuts_it_in_two() {
    let mut d = Document::new();
    circle(&mut d, 0.0, 0.0, 0.0, 1.0);
    let f = face(&d, DVec3::ZERO);
    let up = d.model().face(f).unwrap().plane().unwrap().normal.z;
    assert!(d.run(Command::PushPull {
        face: f,
        distance: 2.0 * up,
        new_loop: false
    }));
    let walls = |d: &Document| d.model().faces().filter(|(_, f)| f.is_curved()).count();
    let before = walls(&d);
    circle(&mut d, 0.0, 0.0, 1.0, 1.0);
    assert_eq!(walls(&d), 2 * before, "each wall cut round its middle");
    clean(d.model());
}
