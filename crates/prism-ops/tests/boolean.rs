//! Booleans of two solids (Solids & CAD part 2, brief 39): Union,
//! Intersect and Subtract give valid solids with exact volumes and no
//! seams on shared planes.

use std::f64::consts::PI;

use prism_math::DVec3;
use prism_ops::boolean::{NOT_SOLID, Op, boolean};
use prism_ops::validate::validate;
use prism_ops::{add_circle, add_rect, check, face_at, push_pull};
use prism_topo::Model;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A box from (x0, y0, z0) to (x1, y1, z1).
fn block(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> Model {
    let mut m = Model::new();
    add_rect(&mut m, x0, y0, x1, y1, z0);
    let f = face_at(&m, v((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0)).unwrap();
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(&mut m, f, (z1 - z0) * up, false);
    m
}

/// A cylinder of radius `r` about (x, y), from z0 to z1.
fn rod(x: f64, y: f64, r: f64, z0: f64, z1: f64) -> Model {
    let mut m = Model::new();
    add_circle(&mut m, v(x, y, z0), DVec3::Z, r);
    let f = face_at(&m, v(x, y, z0)).unwrap();
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(&mut m, f, (z1 - z0) * up, false);
    m
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

/// The result: clean, every piece a valid solid; the total volume.
fn solid_volume(m: &Model) -> f64 {
    assert_eq!(check::problems(m), Vec::<String>::new());
    let all = validate(m);
    assert!(all.iter().all(|c| c.valid), "{all:?}");
    all.iter().map(|c| c.volume.unwrap()).sum()
}

/// No two faces meet on one plane facing the same way.
fn no_seams(m: &Model) {
    let tol = m.tol();
    for (e, fs) in m.edge_faces() {
        if let [f, g] = fs.as_slice()
            && let (Some(p), Some(q)) = (m.face(*f).unwrap().plane(), m.face(*g).unwrap().plane())
        {
            assert!(
                !(p.same_surface(q, &tol, m.face_extent(m.face(*g).unwrap()))
                    && p.normal.dot(q.normal) > 0.0),
                "a seam at {e:?}"
            );
        }
    }
}

#[test]
fn two_overlapping_boxes() {
    // 2 x 2 x 2 at the origin, and the same moved by (1, 1, 0): they share
    // a 1 x 1 x 2 block, and their tops and bottoms lie on one plane.
    let a = block(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
    let b = block(1.0, 1.0, 0.0, 3.0, 3.0, 2.0);
    let u = boolean(&a, &b, Op::Union).unwrap();
    close(solid_volume(&u), 14.0);
    no_seams(&u);
    assert_eq!(validate(&u).len(), 1);
    let i = boolean(&a, &b, Op::Intersect).unwrap();
    close(solid_volume(&i), 2.0);
    no_seams(&i);
    assert_eq!(i.faces().count(), 6, "a plain box");
    let s = boolean(&a, &b, Op::Subtract).unwrap();
    close(solid_volume(&s), 6.0);
    no_seams(&s);
    let s = boolean(&b, &a, Op::Subtract).unwrap();
    close(solid_volume(&s), 6.0);
}

#[test]
fn boxes_touching_face_to_face() {
    let a = block(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
    let b = block(2.0, 0.0, 0.0, 4.0, 2.0, 2.0);
    let u = boolean(&a, &b, Op::Union).unwrap();
    close(solid_volume(&u), 16.0);
    no_seams(&u);
    assert_eq!(u.faces().count(), 6, "one long box");
    let i = boolean(&a, &b, Op::Intersect).unwrap();
    assert_eq!(i.faces().count(), 0, "nothing in both");
    let s = boolean(&a, &b, Op::Subtract).unwrap();
    close(solid_volume(&s), 8.0);
}

#[test]
fn a_box_inside_another_leaves_a_hollow() {
    let a = block(0.0, 0.0, 0.0, 4.0, 4.0, 4.0);
    let b = block(1.0, 1.0, 1.0, 3.0, 3.0, 3.0);
    let s = boolean(&a, &b, Op::Subtract).unwrap();
    close(solid_volume(&s), 64.0 - 8.0);
    assert_eq!(
        validate(&s).len(),
        1,
        "the outside and its hollow, one solid"
    );
    assert_eq!(s.faces().count(), 12);
    let u = boolean(&a, &b, Op::Union).unwrap();
    close(solid_volume(&u), 64.0);
    assert_eq!(u.faces().count(), 6);
}

#[test]
fn a_box_less_a_rod_has_a_round_hole() {
    let a = block(0.0, 0.0, 0.0, 4.0, 4.0, 2.0);
    let b = rod(2.0, 2.0, 1.0, -1.0, 3.0);
    let s = boolean(&a, &b, Op::Subtract).unwrap();
    close(solid_volume(&s), 32.0 - PI * 2.0);
    let i = boolean(&a, &b, Op::Intersect).unwrap();
    close(solid_volume(&i), PI * 2.0);
}

#[test]
fn an_open_box_is_refused() {
    let mut a = block(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
    let top = face_at(&a, v(1.0, 1.0, 2.0)).unwrap();
    a.remove_face(top);
    let b = block(1.0, 1.0, 0.0, 3.0, 3.0, 2.0);
    assert_eq!(boolean(&a, &b, Op::Union), Err(NOT_SOLID));
}

#[test]
fn boxes_overlapping_along_one_axis_union_in_either_order() {
    // Four faces of each lie on the other's planes, and overlap it.
    let a = block(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
    let b = block(1.0, 0.0, 0.0, 3.0, 2.0, 2.0);
    for (x, y) in [(&a, &b), (&b, &a)] {
        let u = boolean(x, y, Op::Union).unwrap();
        close(solid_volume(&u), 12.0);
        no_seams(&u);
        assert_eq!(u.faces().count(), 6, "one long box");
        let i = boolean(x, y, Op::Intersect).unwrap();
        close(solid_volume(&i), 4.0);
        let s = boolean(x, y, Op::Subtract).unwrap();
        close(solid_volume(&s), 4.0);
    }
}
