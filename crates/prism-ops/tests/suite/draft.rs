//! Draft Angle (Shapes & Placement part 6).

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::draft;
use prism_ops::primitives::Primitive;
use prism_ops::{check, validate};
use prism_topo::{FaceId, Model};

fn walls(m: &Model) -> BTreeSet<FaceId> {
    m.faces()
        .filter(|(_, f)| f.plane().is_some_and(|p| p.normal.z.abs() < 1e-9))
        .map(|(id, _)| id)
        .collect()
}

fn frustum_volume(h: f64, a: f64, b: f64) -> f64 {
    h / 3.0 * (a * a + b * b + a * b)
}

#[test]
fn leaning_the_walls_of_a_box_makes_a_frustum() {
    let mut m = Primitive::Box {
        w: 2.0,
        d: 2.0,
        h: 2.0,
    }
    .build();
    let faces = walls(&m);
    assert_eq!(faces.len(), 4);
    let a = 5.0_f64.to_radians();
    assert_eq!(
        draft::draft(&mut m, &faces, DVec3::ZERO, DVec3::Z, a),
        Ok(4)
    );
    let problems = check::problems(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let c = &validate::validate(&m)[0];
    assert!(c.valid, "{:?}", c.reasons);
    // The top is narrower by tan(5 degrees) a metre each side per metre up.
    let top = 2.0 - 4.0 * a.tan();
    let want = frustum_volume(2.0, 2.0, top);
    assert!(
        (c.volume.expect("closed") - want).abs() < 1e-6,
        "{:?} against {want}",
        c.volume
    );
    // The bottom stays where it was.
    assert!(prism_ops::vertex_at(&m, DVec3::new(1.0, 1.0, 0.0)).is_some());
    assert!(
        prism_ops::vertex_at(
            &m,
            DVec3::new(1.0 - 2.0 * a.tan(), 1.0 - 2.0 * a.tan(), 2.0)
        )
        .is_some()
    );
}

#[test]
fn a_negative_angle_widens_the_part_upward() {
    let mut m = Primitive::Box {
        w: 2.0,
        d: 2.0,
        h: 1.0,
    }
    .build();
    let faces = walls(&m);
    let a = (-10.0_f64).to_radians();
    draft::draft(&mut m, &faces, DVec3::ZERO, DVec3::Z, a).expect("drafts");
    let c = &validate::validate(&m)[0];
    assert!(c.valid, "{:?}", c.reasons);
    assert!(c.volume.expect("closed") > 4.0, "wider at the top");
}

#[test]
fn only_the_chosen_faces_lean_and_it_says_what_it_cannot_do() {
    let m = Primitive::Box {
        w: 2.0,
        d: 2.0,
        h: 2.0,
    }
    .build();
    let all = walls(&m);
    let one: BTreeSet<FaceId> = all.iter().take(1).copied().collect();
    // One wall alone leans: its two corner pairs move, the box stays solid.
    let mut w = m.clone();
    assert_eq!(
        draft::draft(&mut w, &one, DVec3::ZERO, DVec3::Z, 0.1),
        Ok(1)
    );
    assert!(validate::validate(&w)[0].valid);
    // Nothing chosen, no angle, too much angle, a top face (parallel).
    let mut w = m.clone();
    assert_eq!(
        draft::draft(&mut w, &BTreeSet::new(), DVec3::ZERO, DVec3::Z, 0.1).err(),
        Some(draft::NOTHING)
    );
    assert_eq!(
        draft::draft(&mut w, &all, DVec3::ZERO, DVec3::Z, 0.0).err(),
        Some(draft::ANGLE_RANGE)
    );
    assert_eq!(
        draft::draft(&mut w, &all, DVec3::ZERO, DVec3::Z, 1.0).err(),
        Some(draft::ANGLE_RANGE)
    );
    let top: BTreeSet<FaceId> = m
        .faces()
        .filter(|(_, f)| f.plane().is_some_and(|p| p.normal.z > 0.5))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(
        draft::draft(&mut w, &top, DVec3::ZERO, DVec3::Z, 0.1).err(),
        Some(draft::PARALLEL)
    );
    // A curved face is refused.
    let cyl = Primitive::Cylinder { r: 1.0, h: 2.0 }.build();
    let side: BTreeSet<FaceId> = cyl
        .faces()
        .filter(|(_, f)| f.is_curved())
        .map(|(id, _)| id)
        .collect();
    let mut c = cyl.clone();
    assert_eq!(
        draft::draft(&mut c, &side, DVec3::ZERO, DVec3::Z, 0.1).err(),
        Some(draft::NOT_FLAT)
    );
    assert_eq!(c, cyl, "nothing changed when refused");
}
