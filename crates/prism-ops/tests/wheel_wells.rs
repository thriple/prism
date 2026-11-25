//! A box with half-round wheel wells, as in the owner's session 10 (a
//! car's interior shell, 2026-11-25).
//!
//! - A new-loop (Alt/Option) push goes past the faces in its way and cuts
//!   them where it crosses, as every other edit does; a plain push stops
//!   (owner: "they should stop if it's not the new-loop mode"). The floor
//!   pushed down past the wells had left them crossing, uncut.
//! - Shell hollows it (it refused: "more than three faces meet at a
//!   corner").

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_math::DVec3;
use prism_ops::pushpull::{Pulled, push_pull};
use prism_ops::{add_arc, add_segment, face_at};
use prism_topo::{FaceId, Model};

/// A box 4 x 6 x 3 standing on the ground, with a half-round wheel well
/// of radius 1 pushed 2 into its front (y = 0) at the bottom.
fn box_with_well() -> Model {
    let mut m = Model::new();
    let c = [
        DVec3::ZERO,
        DVec3::new(4.0, 0.0, 0.0),
        DVec3::new(4.0, 6.0, 0.0),
        DVec3::new(0.0, 6.0, 0.0),
    ];
    for i in 0..4 {
        add_segment(&mut m, c[i], c[(i + 1) % 4]);
    }
    let base = face_at(&m, DVec3::new(2.0, 3.0, 0.0)).expect("base");
    let up = m.face(base).and_then(|f| f.plane()).expect("flat").normal.z;
    assert!(!matches!(
        push_pull(&mut m, base, 3.0 * up.signum(), false),
        Pulled::Nothing
    ));
    add_arc(
        &mut m,
        DVec3::new(2.0, 0.0, 0.0),
        DVec3::Y,
        1.0,
        DVec3::new(1.0, 0.0, 0.0),
        PI,
    );
    let well = face_at(&m, DVec3::new(2.0, 0.0, 0.5)).expect("half disc");
    let into = flat_dir(&m, well).dot(DVec3::Y).signum();
    assert!(!matches!(
        push_pull(&mut m, well, 2.0 * into, false),
        Pulled::Nothing
    ));
    assert!(prism_ops::check::problems(&m).is_empty());
    m
}

fn flat_dir(m: &Model, f: FaceId) -> DVec3 {
    m.face(f).and_then(|x| x.plane()).expect("flat").normal
}

#[test]
fn a_plain_push_down_stops_at_the_wheel_well() {
    let m = box_with_well();
    let top = face_at(&m, DVec3::new(3.5, 5.0, 3.0)).expect("top");
    let s = flat_dir(&m, top).dot(DVec3::Z).signum();
    let d = prism_ops::reach::limit(&m, top, -2.5 * s) * s;
    assert!((d + 2.0).abs() < 1e-7, "stops at the well's top: {d}");
}

#[test]
fn a_new_loop_push_down_past_the_wheel_well_cuts_it() {
    let mut m = box_with_well();
    let top = face_at(&m, DVec3::new(3.5, 5.0, 3.0)).expect("top");
    let s = flat_dir(&m, top).dot(DVec3::Z).signum();
    assert!(!matches!(
        push_pull(&mut m, top, -2.5 * s, true),
        Pulled::Nothing
    ));
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
    let r = prism_ops::health::check(&m);
    assert!(
        r.crossing.faces.is_empty(),
        "crossing: {:?}",
        r.crossing.faces
    );
    // The new floor at z = 0.5 is cut where the well rises through it:
    // edges run along the well's surface at that height.
    let on_floor_across_well = m.edges().any(|(_, e)| {
        let (a, b) = (m.vertex(e.a), m.vertex(e.b));
        (a.z - 0.5).abs() < 1e-7
            && (b.z - 0.5).abs() < 1e-7
            && (a.x - b.x).abs() < 1e-7
            && (a.x - 2.0).abs() > 0.5
            && (a.x - 2.0).abs() < 1.0
    });
    assert!(on_floor_across_well, "no cut along the well at the floor");
}

/// The area of a circle of radius `r` above a chord `h` from its centre.
fn above_chord(r: f64, h: f64) -> f64 {
    r * r * (h / r).acos() - h * (r * r - h * h).sqrt()
}

fn volume(m: &Model) -> f64 {
    m.faces()
        .map(|(id, _)| prism_ops::health::face_volume(m, id))
        .sum::<f64>()
        .abs()
}

/// Shell on the same box, its top open (owner, 2026-11-25: "I tried the
/// shell tool and was surprised it didn't work"). It refused: where the
/// well's round face meets the floor and the front, the corner from the
/// faces' normals alone is off the well's moved surface, so the moved
/// arc missed its own ends. Corners and edges round a curved face are now
/// polished onto the moved surfaces.
#[test]
fn shell_hollows_a_box_with_a_half_round_wheel_well() {
    let solid = 72.0 - PI;
    for t in [-0.2, 0.2] {
        let mut m = box_with_well();
        let top = face_at(&m, DVec3::new(3.5, 5.0, 3.0)).expect("top");
        let all: BTreeSet<FaceId> = m.faces().map(|(id, _)| id).collect();
        let open: BTreeSet<FaceId> = [top].into();
        assert_eq!(prism_ops::shelling::shell(&mut m, &all, &open, t), Ok(()));
        assert!(prism_ops::boolean::is_solid(&m), "closed");
        assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
        let want = if t < 0.0 {
            // The box less the hollow: 0.2 in from each side and the
            // floor, round a well 0.2 bigger.
            solid - (3.6 * 5.6 * 2.8 - above_chord(1.2, 0.2) * 2.0)
        } else {
            // The box grown 0.2 out (not at the open top), round a well
            // 0.2 smaller, less the box.
            4.4 * 6.4 * 3.2 - above_chord(0.8, -0.2) * 2.0 - solid
        };
        assert!(
            (volume(&m) - want).abs() < 1e-6,
            "{t}: {} is not {want}",
            volume(&m)
        );
    }
}

/// The owner's half shell, simplified: wells at both ends, the top and the
/// centre (x = 0, where the mirrored half joins) left open, walls 0.32.
#[test]
fn shell_hollows_a_half_body_with_two_wheel_wells() {
    let mut m = box_with_well();
    add_arc(
        &mut m,
        DVec3::new(2.0, 6.0, 0.0),
        DVec3::Y,
        1.0,
        DVec3::new(1.0, 6.0, 0.0),
        PI,
    );
    let well = face_at(&m, DVec3::new(2.0, 6.0, 0.5)).expect("half disc");
    let into = -flat_dir(&m, well).dot(DVec3::Y).signum();
    assert!(!matches!(
        push_pull(&mut m, well, 2.0 * into, false),
        Pulled::Nothing
    ));
    let top = face_at(&m, DVec3::new(3.5, 5.0, 3.0)).expect("top");
    let centre = face_at(&m, DVec3::new(0.0, 3.0, 1.5)).expect("centre");
    let all: BTreeSet<FaceId> = m.faces().map(|(id, _)| id).collect();
    let open: BTreeSet<FaceId> = [top, centre].into();
    assert_eq!(
        prism_ops::shelling::shell(&mut m, &all, &open, -0.32),
        Ok(())
    );
    assert!(prism_ops::boolean::is_solid(&m), "closed");
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
    let hollow = 3.68 * 5.36 * 2.68 - 2.0 * 2.0 * above_chord(1.32, 0.32);
    let want = 72.0 - 2.0 * PI - hollow;
    assert!(
        (volume(&m) - want).abs() < 1e-6,
        "{} is not {want}",
        volume(&m)
    );
}
