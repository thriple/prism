//! The owner's file `owner-cylinder-1.prism` (2026-11-25): a box pushed
//! through a cylinder and intersected with it. Where the box's top meets
//! the wall, the arc is shared by four faces: the top outside the
//! cylinder and inside it, the wall and the piece of wall inside the box.
//! The arc was drawn against the two flat ones as its own, so the wall,
//! drawn as flat facets that stray in front of its exact edge, hid it in
//! places: it drew dashed.

mod common;

use common::Gpu;
use prism_geom::Curve;
use prism_math::DVec3;
use prism_render::gpu::Highlight;
use prism_render::{Camera, Style};
use prism_topo::EdgeId;

fn looking_at(target: DVec3, dist: f64, az_deg: f64, el_deg: f64) -> Camera {
    let (a, e) = (az_deg.to_radians(), el_deg.to_radians());
    Camera {
        eye: target + DVec3::new(e.cos() * a.cos(), e.cos() * a.sin(), e.sin()) * dist,
        target,
    }
}

#[test]
fn an_arc_where_a_box_top_meets_a_cylinder_wall_draws_whole() {
    let c = prism_file::read(include_bytes!(
        "../../prism-tools/tests/data/owner-cylinder-1.prism"
    ))
    .expect("the file");
    let m = c
        .assembly
        .def(prism_doc::DefId(1))
        .expect("the component")
        .model
        .clone();
    let mut scene = prism_view::Scene::build(&m);
    // The arcs on the box tops, each between two flat and two curved faces.
    for id in [EdgeId(53), EdgeId(65)] {
        let e = scene.edge(id).expect("arc");
        assert!(matches!(e.curve, Curve::Arc { .. }), "{id:?} is an arc");
        for f in e.own_faces() {
            let f = f.expect("two own faces");
            assert!(m.face(f).expect("face").is_curved(), "{id:?}: {f:?}");
        }
    }
    let Some(mut g) = Gpu::new() else {
        return;
    };
    scene.drop_soft();
    let style = Style::default();
    let (w, h) = (900, 700);
    g.renderer.resize(&g.device, w, h);
    g.renderer.set_scene(&g.device, &g.queue, &scene, &style);
    g.renderer
        .set_highlight(&g.device, &scene, &Highlight::default(), &style);
    let (lo, hi) = scene.bounds.expect("bounds");
    let mid = (lo + hi) / 2.0;
    let d = (hi - lo).length() * 1.5;
    // Each arc from the side it faces.
    for (id, az) in [(EdgeId(53), -60.0), (EdgeId(65), 200.0)] {
        let cam = looking_at(mid, d, az, 25.0);
        let img = g.draw(cam, w, h, 2.0, &style, scene.bounds);
        img.save(&format!("cut_arc_{}", id.0));
        let view = prism_render::View::new(&cam, f64::from(w), f64::from(h));
        let e = m.edge(id).expect("edge");
        let (pa, pb) = (m.vertex(e.a), m.vertex(e.b));
        // Points along the arc that land on a line pixel.
        let n = 400;
        let drawn = (1..n)
            .filter(|k| {
                let t = f64::from(*k) / f64::from(n);
                let (q, _) = view.to_screen(e.curve.point(pa, pb, t)).expect("in view");
                let p = img.at(q.x as u32, q.y as u32);
                p[..3].iter().map(|c| u32::from(*c)).sum::<u32>() < 3 * 120
            })
            .count();
        let share = drawn as f64 / f64::from(n - 1);
        println!("{id:?}: {share}");
        assert!(share > 0.98, "{id:?}: only {share} of it drawn");
    }
}
