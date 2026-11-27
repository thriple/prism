//! Snapping finds the crossings Intersect Faces makes on curved faces
//! (Exact Curves 6b): they are edges, so On Arc and Endpoint work there.

use prism_infer::{Context, Kind, Lock, Memory, Settings, infer};
use prism_math::DVec3;
use prism_ops::pushpull::push_pull;
use prism_topo::Model;
use prism_view::{Camera, Scene, View};

#[test]
fn a_crossing_on_a_cylinder_is_snapped_to() {
    let mut m = Model::new();
    prism_ops::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let disc = m.faces().next().map(|(id, _)| id).expect("a disc");
    push_pull(&mut m, disc, 2.0, false);
    prism_ops::add_rect(&mut m, -2.0, -2.0, 2.0, 2.0, 1.0);
    prism_ops::intersect::intersect_faces(&mut m, None);
    let cam = Camera {
        eye: DVec3::new(6.0, -5.0, 4.0),
        target: DVec3::new(0.0, 0.0, 1.0),
    };
    let scene = Scene::build(&m);
    let view = View::new(&cam, 900.0, 600.0);
    // On the circle at z = 1, on the side facing the camera.
    let t = (-30.0f64).to_radians();
    let on = DVec3::new(t.cos(), t.sin(), 1.0);
    let s = view.to_screen(on).expect("in front").0;
    let i = infer(
        &scene,
        &view,
        s + prism_math::DVec2::new(1.0, 1.0),
        &Context::default(),
        Lock::None,
        &Memory::default(),
        &Settings::default(),
    );
    assert!(
        matches!(i.kind, Kind::OnArc | Kind::Midpoint | Kind::Endpoint),
        "{:?}",
        i.kind
    );
    assert!(
        (i.point.truncate().length() - 1.0).abs() < 1e-9,
        "{:?}",
        i.point
    );
    assert!((i.point.z - 1.0).abs() < 1e-9, "{:?}", i.point);
}
