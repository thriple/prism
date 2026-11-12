//! How big arris is for the web: a box minus a cylinder, filleted.
use arris::Control;
use arris::math::{Axis, Point3};
use arris::topo::Model;

#[unsafe(no_mangle)]
pub extern "C" fn volume() -> f64 {
    let c = &Control::NONE;
    let mut m = Model::default();
    let a = arris::ops::primitive_box(
        &mut m,
        Point3::new(-1.0, -1.0, -1.0),
        Point3::new(1.0, 1.0, 1.0),
        c,
    )
    .unwrap()
    .0;
    let t = arris::ops::primitive_cylinder(
        &mut m,
        Axis::z_at(Point3::new(0.0, 0.0, -2.0)),
        0.5,
        4.0,
        c,
    )
    .unwrap()
    .0;
    let b = arris::ops::cut(&mut m, a, t, c).unwrap().0;
    let edges = m.edges(b).unwrap();
    let b = arris::ops::fillet(&mut m, b, &edges[..1], 0.1, c)
        .map(|x| x.0)
        .unwrap_or(b);
    let l = arris::ops::loft(&mut m, &[], arris::ops::LoftKind::Smooth, false, c).is_ok();
    arris::ops::measure::mass_properties(&m, b, c)
        .unwrap()
        .volume
        + f64::from(u8::from(l))
}
