//! Fillet and chamfer of a solid's edge (Exact Curves 5a).

use std::f64::consts::PI;

use prism_geom::Surface;
use prism_math::DVec3;
use prism_ops::round::{NOT_AN_EDGE, TOO_WIDE, default_size, refusal, round};
use prism_ops::{add_rect, check, push_pull};
use prism_topo::{EdgeId, Model};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A 2 m cube on the ground.
fn cube() -> Model {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    let f = m.faces().next().map(|(id, _)| id).expect("a face");
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(&mut m, f, 2.0 * up, false);
    m
}

/// The edge from `p` to `q`.
fn edge(m: &Model, p: DVec3, q: DVec3) -> EdgeId {
    let a = prism_ops::vertex_at(m, p).expect("p");
    let b = prism_ops::vertex_at(m, q).expect("q");
    m.edge_between(a, b).expect("an edge")
}

fn solid(m: &Model) {
    assert_eq!(check::problems(m), Vec::<String>::new());
    let h = prism_ops::health::check(m);
    assert_eq!((h.watertight, h.leaky), (1, 0), "{h:?}");
    assert!(h.reversed.is_empty(), "{h:?}");
    assert!(h.off_circle.is_empty(), "{h:?}");
}

/// The volume of a closed solid of flat faces.
fn flat_volume(m: &Model) -> f64 {
    m.faces()
        .map(|(id, f)| {
            let p = f.plane().expect("flat");
            m.face_area(id) * p.normal.dot(p.origin) / 3.0
        })
        .sum()
}

fn area_on(m: &Model, normal: DVec3, origin_dot: f64) -> f64 {
    m.faces()
        .filter_map(|(id, f)| {
            let p = f.plane()?;
            (p.normal.distance(normal) < 1e-9 && (p.normal.dot(p.origin) - origin_dot).abs() < 1e-9)
                .then(|| m.face_area(id))
        })
        .sum()
}

#[test]
fn a_chamfered_cube_edge_is_a_closed_solid_with_the_exact_volume() {
    let mut m = cube();
    let e = edge(&m, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    assert_eq!(refusal(&m, e, 0.5, false), None);
    assert!(round(&mut m, e, 0.5, false));
    solid(&m);
    assert_eq!(m.face_count(), 7);
    // A prism of 0.5 × 0.5 / 2 × 2 is cut off.
    assert!(
        (flat_volume(&m) - (8.0 - 0.25)).abs() < 1e-9,
        "{}",
        flat_volume(&m)
    );
    let slope = v(1.0, 0.0, 1.0).normalize();
    assert!((area_on(&m, slope, slope.dot(v(2.0, 0.0, 1.5))) - 0.5f64.sqrt() * 2.0).abs() < 1e-9);
}

#[test]
fn a_filleted_cube_edge_is_round_and_closed() {
    let r = 0.5;
    let mut m = cube();
    let e = edge(&m, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    assert!(round(&mut m, e, r, true));
    solid(&m);
    assert_eq!(m.face_count(), 7);
    let round_faces: Vec<_> = m.faces().filter(|(_, f)| f.is_curved()).collect();
    let [(id, f)] = round_faces.as_slice() else {
        panic!("one round face");
    };
    let Surface::Cylinder(c) = f.surface else {
        unreachable!()
    };
    assert!((c.radius - r).abs() < 1e-12 && c.outward);
    assert!(c.axis.cross(DVec3::Y).length() < 1e-12, "along the edge");
    assert!(c.distance(v(1.5, 1.0, 1.5)) + r < 1e-12, "the axis");
    assert!((m.face_area(*id) - PI / 2.0 * r * 2.0).abs() < 1e-9);
    // The ends lose a corner square less a quarter disc.
    let end = 4.0 - (r * r - PI * r * r / 4.0);
    assert!((area_on(&m, v(0.0, -1.0, 0.0), 0.0) - end).abs() < 1e-9);
    // The exact volume: a corner square less a quarter disc, along 2 m.
    let v: f64 = m
        .faces()
        .map(|(id, _)| prism_ops::health::face_volume(&m, id))
        .sum();
    assert!(
        (v - (8.0 - (r * r - PI * r * r / 4.0) * 2.0)).abs() < 1e-9,
        "{v}"
    );
    // Where it meets the faces, the lines are soft and smooth.
    assert_eq!(m.edges().filter(|(_, e)| e.soft && e.smooth).count(), 2);
}

#[test]
fn a_concave_edge_is_filled() {
    // An L: a 4 × 2 slab with a 2 × 2 block on one half.
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 4.0, 2.0, 0.0);
    let f = m.faces().next().map(|(id, _)| id).unwrap();
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(&mut m, f, up, false);
    prism_ops::add_segment(&mut m, v(2.0, 0.0, 1.0), v(2.0, 2.0, 1.0));
    let top = prism_ops::face_at(&m, v(1.0, 1.0, 1.0)).expect("half the top");
    push_pull(&mut m, top, 1.0, false);
    let e = edge(&m, v(2.0, 0.0, 1.0), v(2.0, 2.0, 1.0));
    for fillet in [false, true] {
        let mut m = m.clone();
        assert_eq!(refusal(&m, e, 0.25, fillet), None);
        assert!(round(&mut m, e, 0.25, fillet));
        solid(&m);
        if !fillet {
            assert!((flat_volume(&m) - (12.0 + 0.25 * 0.25 / 2.0 * 2.0)).abs() < 1e-9);
        } else {
            let c = m.faces().find_map(|(_, f)| match f.surface.clone() {
                Surface::Cylinder(c) => Some(c),
                _ => None,
            });
            assert!(
                !c.expect("round").outward,
                "a concave fillet faces its axis"
            );
        }
    }
}

#[test]
fn refusals_say_why() {
    let m = cube();
    let e = edge(&m, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    assert_eq!(refusal(&m, e, 2.5, false), Some(TOO_WIDE));
    assert_eq!(refusal(&m, e, 0.0, true), Some(NOT_AN_EDGE));
    // A flat face's edge has one face.
    let mut flat = Model::new();
    add_rect(&mut flat, 0.0, 0.0, 2.0, 2.0, 0.0);
    let e = edge(&flat, v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0));
    assert_eq!(refusal(&flat, e, 0.1, true), Some(NOT_AN_EDGE));
    // A wedge's slanted end: a chamfer, and since Exact Curves 6a a fillet
    // too, its end an approximate ellipse.
    let mut w = Model::new();
    prism_ops::add_polygon(
        &mut w,
        &[v(0.0, 0.0, 0.0), v(3.0, 0.0, 0.0), v(0.0, 0.0, 2.0)],
    );
    let f = w.faces().next().map(|(id, _)| id).unwrap();
    let n = w.face(f).unwrap().plane().unwrap().normal;
    push_pull(&mut w, f, 2.0 * n.y.signum(), false);
    let base_y = if n.y > 0.0 { 2.0 } else { -2.0 };
    let along = edge(&w, v(0.0, 0.0, 0.0), v(0.0, base_y, 0.0));
    assert_eq!(refusal(&w, along, 0.2, true), None, "square ends");
    let slanted = edge(&w, v(0.0, 0.0, 0.0), v(3.0, 0.0, 0.0));
    assert_eq!(refusal(&w, slanted, 0.2, true), None);
    assert_eq!(refusal(&w, slanted, 0.2, false), None);
    let mut w2 = w.clone();
    assert!(round(&mut w2, slanted, 0.2, false));
    solid(&w2);
    let mut w3 = w.clone();
    assert!(round(&mut w3, slanted, 0.2, true));
    solid(&w3);
    let approx: Vec<_> = w3.edges().filter(|(_, e)| e.approx).collect();
    assert!(approx.len() > 2, "an ellipse of fine segments");
    // Every point of the ellipse on the fillet's cylinder and the end face.
    let cyl = w3
        .faces()
        .find_map(|(_, f)| match f.surface.clone() {
            prism_geom::Surface::Cylinder(c) => Some(c),
            _ => None,
        })
        .expect("the fillet");
    for (_, e) in approx {
        for p in [w3.vertex(e.a), w3.vertex(e.b)] {
            assert!(cyl.distance(p).abs() < 1e-9, "{p}");
        }
    }
}

#[test]
fn the_default_size_is_a_tenth_of_the_narrower_face() {
    let m = cube();
    let e = edge(&m, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    assert!((default_size(&m, e).unwrap() - 0.2).abs() < 1e-12);
}

#[test]
fn a_refused_round_changes_nothing() {
    let mut m = cube();
    let before = m.to_json();
    let e = edge(&m, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    assert!(!round(&mut m, e, 3.0, true));
    assert_eq!(m.to_json(), before);
}

// Property tests run natively only (proptest is a native dev-dependency).
#[cfg(not(target_arch = "wasm32"))]
mod random {
    use super::*;

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig { cases: 64, ..Default::default() })]

        /// Any edge of any box, rounded or cut by any size, is refused
        /// (changing nothing) or leaves one clean closed solid.
        #[test]
        fn any_round_of_a_box_edge_is_clean_or_refused(
            w in 0.5..4.0f64, l in 0.5..4.0f64, h in 0.5..4.0f64,
            pick in 0usize..12, size in 0.01..3.0f64, fillet: bool,
        ) {
            let mut m = Model::new();
            add_rect(&mut m, 0.0, 0.0, w, l, 0.0);
            let f = m.faces().next().map(|(id, _)| id).unwrap();
            let up = m.face(f).unwrap().plane().unwrap().normal.z;
            push_pull(&mut m, f, h * up, false);
            let edges: Vec<EdgeId> = m.edges().map(|(id, _)| id).collect();
            let e = edges[pick % edges.len()];
            let before = m.to_json();
            let refused = refusal(&m, e, size, fillet).is_some();
            proptest::prop_assert_eq!(round(&mut m, e, size, fillet), !refused);
            if refused {
                proptest::prop_assert_eq!(m.to_json(), before);
            } else {
                proptest::prop_assert_eq!(check::problems(&m), Vec::<String>::new());
                let hc = prism_ops::health::check(&m);
                proptest::prop_assert_eq!((hc.watertight, hc.leaky), (1, 0));
                proptest::prop_assert!(hc.reversed.is_empty());
            }
        }
    }
}
