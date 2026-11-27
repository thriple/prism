//! Two boxes touching along a corner line (owner feedback, 2026-11-09).

#![cfg(not(target_arch = "wasm32"))]

use prism_math::DVec3;
use prism_topo::Model;

/// Boxes 2 m wide, touching along the line x = y = 2; the second `h2` tall.
fn touching(h2: f64) -> Model {
    let mut m = Model::new();
    for (x, h) in [(0.0, 2.0), (2.0, h2)] {
        prism_ops::add_rect(&mut m, x, x, x + 2.0, x + 2.0, 0.0);
        let f = prism_ops::face_at(&m, DVec3::new(x + 1.0, x + 1.0, 0.0)).expect("base");
        let up = m.face(f).expect("f").plane().unwrap().normal.z > 0.0;
        prism_ops::push_pull(&mut m, f, if up { h } else { -h }, false);
    }
    m
}

/// The edges on the corner line, as (bottom, top, faces).
fn corner_edges(m: &Model) -> Vec<(f64, f64, usize)> {
    let ef = m.edge_faces();
    let mut out: Vec<(f64, f64, usize)> = m
        .edges()
        .filter_map(|(id, e)| {
            let (a, b) = (m.vertex(e.a), m.vertex(e.b));
            let on = |p: DVec3| (p.x - 2.0).abs() < 1e-9 && (p.y - 2.0).abs() < 1e-9;
            (on(a) && on(b)).then(|| (a.z.min(b.z), a.z.max(b.z), ef.get(&id).map_or(0, Vec::len)))
        })
        .collect();
    out.sort_by(|x, y| x.0.total_cmp(&y.0));
    out
}

/// One shared corner line with four faces (the renderer draws it whole:
/// see `lines_on_a_face_seen_edge_on_are_drawn_whole` in prism-render).
#[test]
fn touching_boxes_share_one_corner_line() {
    let m = touching(2.0);
    assert_eq!(corner_edges(&m), [(0.0, 2.0, 4)]);
}

/// When one box is taller, the corner line splits where the lower one
/// stops: shared below, the taller box's own above.
#[test]
fn a_taller_box_has_its_own_corner_line_above() {
    let m = touching(3.0);
    assert_eq!(corner_edges(&m), [(0.0, 2.0, 4), (2.0, 3.0, 2)]);
}

/// From one shared line, pulling one box up or pushing it down splits the
/// line where the two now differ.
#[test]
fn pulling_one_box_up_or_down_splits_the_shared_line() {
    for (d, want) in [
        (1.0, [(0.0, 2.0, 4), (2.0, 3.0, 2)]),
        (-1.0, [(0.0, 1.0, 4), (1.0, 2.0, 2)]),
    ] {
        let mut m = touching(2.0);
        let top = prism_ops::face_at(&m, DVec3::new(3.0, 3.0, 2.0)).expect("top");
        let up = m.face(top).expect("f").plane().unwrap().normal.z > 0.0;
        prism_ops::push_pull(&mut m, top, if up { d } else { -d }, false);
        assert_eq!(corner_edges(&m), want, "pulled by {d}");
        assert!(prism_ops::check::problems(&m).is_empty());
    }
}
