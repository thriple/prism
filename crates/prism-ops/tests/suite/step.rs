//! STEP through the kernel (Solids & CAD part 5a, brief 44): Prism's
//! solids go to a STEP file and come back with the same surfaces and
//! volumes, in any unit, and assemblies keep their names, placements and
//! colours.

use crate::common;

use std::collections::BTreeSet;
use std::f64::consts::PI;

use common::{node, same_through_step, shapes, volumes};
use prism_kernel::step;
use prism_math::{DAffine3, DQuat, DVec3, LengthUnit};
use prism_ops::{add_arc, add_circle, add_rect, add_segment, face_at, follow, push_pull};
use prism_topo::Model;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} vs {b}");
}

/// Pull the face at `p` by `d` along +z.
fn pull(m: &mut Model, p: DVec3, d: f64) {
    let f = face_at(m, p).expect("a face there");
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(m, f, d * up, false);
}

fn boxed() -> Model {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 2.0, 3.0, 0.0);
    pull(&mut m, v(1.0, 1.0, 0.0), 4.0);
    m
}

/// A profile drawn by `draw` turned round a circle about z by Follow Me.
fn turned(draw: impl Fn(&mut Model)) -> Model {
    let mut m = Model::new();
    draw(&mut m);
    let prof = m.faces().next().map(|(id, _)| id).unwrap();
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    add_circle(&mut m, v(0.0, 0.0, 10.0), DVec3::Z, 0.5);
    let path: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert!(follow::follow(&mut m, &path, prof));
    // Only the turned solid: the path's disc is left out.
    let keep = common::shapes(&m);
    let mut out = Model::new();
    for s in keep {
        prism_ops::bridge::put_back(&mut out, &BTreeSet::new(), &s.region).unwrap();
    }
    out
}

#[test]
fn a_box_goes_through_step_in_millimetres_and_inches() {
    let m = boxed();
    for unit in [
        LengthUnit::Millimetres,
        LengthUnit::Inches,
        LengthUnit::Metres,
    ] {
        let back = same_through_step(&m, unit);
        close(volumes(&back)[0], 24.0);
    }
    let (_, text) = common::through_step(&m, LengthUnit::Millimetres);
    assert!(text.contains("SI_UNIT(.MILLI.,.METRE.)"), "{text}");
    // 2 m is written as 2000 mm.
    assert!(text.contains("2000."), "{text}");
}

#[test]
fn the_same_model_writes_the_same_file() {
    let m = boxed();
    let (_, a) = common::through_step(&m, LengthUnit::Millimetres);
    let (_, b) = common::through_step(&m, LengthUnit::Millimetres);
    assert_eq!(a, b);
}

#[test]
fn a_cylinder_a_ball_a_torus_and_a_cone_go_through_step() {
    let mut cyl = Model::new();
    add_circle(&mut cyl, DVec3::ZERO, DVec3::Z, 1.0);
    pull(&mut cyl, DVec3::ZERO, 2.0);
    close(
        volumes(&same_through_step(&cyl, LengthUnit::Millimetres))[0],
        2.0 * PI,
    );

    let ball = turned(|m| {
        add_segment(m, v(0.0, 0.0, -1.0), v(0.0, 0.0, 1.0));
        add_arc(m, DVec3::ZERO, -DVec3::Y, 1.0, v(0.0, 0.0, -1.0), PI);
    });
    close(
        volumes(&same_through_step(&ball, LengthUnit::Millimetres))[0],
        4.0 / 3.0 * PI,
    );

    let torus = turned(|m| {
        add_circle(m, v(2.0, 0.0, 0.0), DVec3::Y, 0.5);
    });
    close(
        volumes(&same_through_step(&torus, LengthUnit::Inches))[0],
        PI * PI,
    );

    let cone = turned(|m| {
        prism_ops::add_polygon(m, &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 0.0, 2.0)]);
    });
    close(
        volumes(&same_through_step(&cone, LengthUnit::Millimetres))[0],
        PI * 2.0 / 3.0,
    );
}

#[test]
fn an_assembly_keeps_its_names_placements_and_colours() {
    let m = boxed();
    let mut s = shapes(&m);
    s[0].colours = vec![(0, [1.0, 0.0, 0.0])];
    // One part placed twice (one product), turned and moved.
    let turn =
        DAffine3::from_rotation_translation(DQuat::from_rotation_z(PI / 2.0), v(10.0, 0.0, 0.0));
    let mut a = node("block", DAffine3::IDENTITY, vec![0]);
    a.product = Some(1);
    a.colour = Some([0.0, 0.5, 1.0]);
    let mut b = a.clone();
    b.place = turn;
    let mut root = node("assembly", DAffine3::IDENTITY, Vec::new());
    root.children = vec![a, b];
    let w = step::write(&s, &[root], LengthUnit::Millimetres, m.tol().dist).unwrap();
    assert_eq!(w.text.matches("NEXT_ASSEMBLY_USAGE_OCCURRENCE(").count(), 2);
    let back = step::read(&w.text, m.tol().dist).unwrap();
    // Two placed solids, each at its place.
    assert_eq!(back.shapes.len(), 2);
    let [root] = &back.roots[..] else {
        panic!("{:?}", back.roots)
    };
    assert_eq!(root.name, "assembly");
    let names: Vec<&str> = root.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["block", "block"]);
    assert_eq!(root.children[0].product, root.children[1].product);
    assert_eq!(root.children[0].colour, Some([0.0, 0.5, 1.0]));
    let moved = &root.children[1];
    assert!(moved.place.translation.distance(v(10.0, 0.0, 0.0)) < 1e-9);
    assert!(moved.place.matrix3.x_axis.distance(DVec3::Y) < 1e-9);
    // The second solid stands turned and moved.
    let placed = back.shapes[moved.shapes[0]].as_ref().unwrap();
    let lo = placed
        .region
        .vertices
        .iter()
        .fold(DVec3::MAX, |a, p| a.min(p.at));
    assert!(lo.distance(v(7.0, 0.0, 0.0)) < 1e-9, "{lo}");
    // The face colour came back on a face of each.
    assert!(
        back.shapes
            .iter()
            .all(|s| s.as_ref().unwrap().colours.len() == 1)
    );
}

#[test]
fn a_shape_the_kernel_refuses_is_left_out_and_named() {
    let m = boxed();
    let mut s = shapes(&m);
    let mut open = s[0].clone();
    open.region.faces.pop();
    s.push(open);
    let root = node("part", DAffine3::IDENTITY, vec![0, 1]);
    let w = step::write(&s, &[root], LengthUnit::Millimetres, m.tol().dist).unwrap();
    assert_eq!(w.left_out, vec![(1, prism_kernel::solid::NOT_CLOSED)]);
    let back = step::read(&w.text, m.tol().dist).unwrap();
    assert_eq!(back.shapes.len(), 1);
    // Nothing left at all: refused.
    let root = node("part", DAffine3::IDENTITY, vec![1]);
    assert_eq!(
        step::write(&s, &[root], LengthUnit::Millimetres, m.tol().dist).err(),
        Some(step::NOTHING)
    );
}

#[test]
fn an_open_sheet_goes_through_step_as_a_surface() {
    let m = boxed();
    let mut s = shapes(&m);
    s[0].region.faces.pop();
    s[0].open = true;
    let root = node("sheet", DAffine3::IDENTITY, vec![0]);
    let w = step::write(&s, &[root], LengthUnit::Millimetres, m.tol().dist).unwrap();
    assert_eq!(w.left_out, Vec::new());
    assert!(w.text.contains("SHELL_BASED_SURFACE_MODEL"));
    let back = step::read(&w.text, m.tol().dist).unwrap();
    let sheet = back.shapes[0].as_ref().unwrap();
    assert!(sheet.open);
    assert_eq!(sheet.region.faces.len(), 5);
}

#[test]
fn text_that_is_not_step_is_refused() {
    assert!(step::read("hello", 1e-9).is_err());
}
