//! Outline lines (Exact Curves 6d): where a smooth curved face turns away
//! from the eye, its outline is found from the drawn triangles.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_math::DVec3;
use prism_topo::Model;
use prism_view::Scene;
use prism_view::outline::Outlines;

/// A ball of radius 1 at the origin, by Follow Me of a half disc.
fn ball() -> Model {
    let mut m = Model::new();
    prism_ops::add_segment(&mut m, -DVec3::Z, DVec3::Z);
    prism_ops::add_arc(&mut m, DVec3::ZERO, -DVec3::Y, 1.0, -DVec3::Z, PI);
    let prof = prism_ops::face_at(&m, DVec3::new(0.5, 0.0, 0.0)).expect("half disc");
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    prism_ops::add_circle(&mut m, DVec3::new(0.0, 0.0, 10.0), DVec3::Z, 0.5);
    let path: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert!(prism_ops::follow::follow(&mut m, &path, prof));
    m
}

#[test]
fn a_ball_seen_from_afar_has_a_round_outline() {
    let s = Scene::build(&ball());
    let o = Outlines::of(&s);
    let eye = DVec3::new(100.0, 0.0, 0.0);
    let lines = o.lines(eye);
    assert!(!lines.is_empty());
    // Where the line of sight grazes the ball: |p|² = p · eye, so x = 0.01,
    // on a circle of radius √(1 - 0.0001).
    let r = (1.0f64 - 1e-4).sqrt();
    let mut len = 0.0;
    for l in &lines {
        for p in [l.a, l.b] {
            assert!((p.x - 0.01).abs() < 2e-3, "{p}");
            assert!((p.length() - 1.0).abs() < 2e-3, "{p}");
        }
        len += l.a.distance(l.b);
    }
    assert!((len - 2.0 * PI * r).abs() < 0.01 * 2.0 * PI, "{len}");
    // Turned with the view: from above, the outline is the equator.
    for l in o.lines(DVec3::new(0.0, 0.0, 100.0)) {
        assert!((l.a.z - 0.01).abs() < 2e-3 && (l.b.z - 0.01).abs() < 2e-3);
    }
}

#[test]
fn a_cylinder_has_side_outlines_but_none_seen_down_its_axis() {
    let mut m = Model::new();
    prism_ops::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let f = m.faces().next().map(|(id, _)| id).expect("a disc");
    let up = m
        .face(f)
        .and_then(|x| x.plane())
        .map_or(1.0, |p| p.normal.z);
    prism_ops::push_pull(&mut m, f, 2.0 * up, false);
    let o = Outlines::of(&Scene::build(&m));
    // From the side: two lines up the sides, at y = ±1 seen from far +x.
    let lines = o.lines(DVec3::new(1000.0, 0.0, 1.0));
    assert!(!lines.is_empty());
    let mut height = 0.0;
    for l in &lines {
        assert!((l.a.y.abs() - 1.0).abs() < 2e-3, "{}", l.a);
        height += (l.b.z - l.a.z).abs();
    }
    assert!((height - 4.0).abs() < 1e-6, "{height}");
    // Down the axis: the outline is the caps' edges, which are edges.
    assert!(o.lines(DVec3::new(0.0, 0.0, 50.0)).is_empty());
}

#[test]
fn flat_faces_have_no_outline_lines() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 1.0, 1.0, 0.0);
    let o = Outlines::of(&Scene::build(&m));
    assert!(o.is_empty());
}
