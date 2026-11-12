//! How big monstertruck is for the web: a box minus a cylinder, filleted.
use monstertruck::meshing::prelude::*;
use monstertruck::modeling::*;

#[unsafe(no_mangle)]
pub extern "C" fn volume() -> f64 {
    let v = builder::vertex(Point3::new(-1.0, -1.0, -1.0));
    let e = builder::extrude(&v, Vector3::unit_x() * 2.0);
    let f = builder::extrude(&e, Vector3::unit_y() * 2.0);
    let cube: Solid = builder::extrude(&f, Vector3::unit_z() * 2.0);
    let v = builder::vertex(Point3::new(0.5, 0.0, -2.0));
    let w: Wire = builder::revolve(
        &v,
        Point3::new(0.0, 0.0, -2.0),
        Vector3::unit_z(),
        builder::SweepAngle::Closed,
        4,
    );
    let f: Face = builder::try_attach_plane(&[w]).unwrap();
    let cyl: Solid = builder::extrude(&f, Vector3::unit_z() * 4.0);
    let s = monstertruck::solid::difference(&cube, &cyl, 0.01).unwrap_or(cube);
    let mut shell = s.boundaries()[0].clone();
    let edges: Vec<Edge> = shell.edge_iter().take(1).collect();
    let _ = fillet_edges(&mut shell, &edges, Some(&FilletOptions::constant(0.1)));
    s.triangulation(0.01).to_polygon().volume()
}
