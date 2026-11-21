//! Push/Pull at a hundredth of a centimetre (owner, 2026-11-21: "I use that
//! tool to make extremely small adjustments for 3D printing"). Lengths are
//! metres, so 0.01 cm is 0.0001.

use prism_doc::{Command, Document};
use prism_math::DVec3;
use prism_ops::{boolean::is_solid, check, validate::validate};

const TINY: f64 = 1e-4;

fn volume(d: &Document) -> f64 {
    validate(d.model())
        .iter()
        .filter_map(|c| c.volume)
        .sum::<f64>()
        .abs()
}

fn box_of(size: f64) -> Document {
    let mut d = Document::new();
    d.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: size,
        y1: size,
        z: 0.0,
    });
    let f = prism_ops::face_at(d.model(), DVec3::new(size / 2.0, size / 2.0, 0.0)).unwrap();
    let up = d.model().face(f).unwrap().plane().unwrap().normal.z > 0.0;
    assert!(d.run(Command::PushPull {
        face: f,
        distance: if up { size } else { -size },
        new_loop: false,
    }));
    d
}

fn top_of(d: &Document, size: f64) -> prism_topo::FaceId {
    prism_ops::face_at(d.model(), DVec3::new(size / 2.0, size / 2.0, size)).unwrap()
}

#[test]
fn a_hundredth_of_a_centimetre_pull_on_a_box_is_exact_at_every_size() {
    // Up to a 15 m model (the hull's size), where the tolerance is about
    // 0.0015 cm: a 0.01 cm step is still several times it.
    for size in [0.02, 0.1, 1.0, 5.0, 15.0] {
        for sign in [1.0, -1.0] {
            let mut d = box_of(size);
            let whole = volume(&d);
            let f = top_of(&d, size);
            let step = TINY * sign;
            assert!(
                d.run(Command::PushPull {
                    face: f,
                    distance: step,
                    new_loop: false,
                }),
                "{size} {step}"
            );
            let expected = whole + size * size * step;
            assert!(
                (volume(&d) - expected).abs() < 1e-9 * whole.max(1.0),
                "{size} {step}: {} vs {expected}",
                volume(&d)
            );
            assert!(is_solid(d.model()), "{size} {step}");
            assert!(check::problems(d.model()).is_empty(), "{size} {step}");
            assert_eq!(d.model().face_count(), 6, "{size} {step}");
            // The top really moved by it.
            let top = d
                .model()
                .vertices()
                .map(|(_, p)| p.z)
                .fold(f64::MIN, f64::max);
            assert!((top - (size + step)).abs() < 1e-12, "{size} {step}: {top}");
        }
    }
}

#[test]
fn a_hundredth_of_a_centimetre_pull_on_a_cylinder_and_a_step_back_again() {
    let mut d = Document::new();
    d.run(Command::Circle {
        centre: DVec3::ZERO,
        normal: DVec3::Z,
        radius: 0.05,
    });
    let base = d
        .model()
        .faces()
        .map(|(id, _)| id)
        .next()
        .expect("the disc");
    let up = d.model().face(base).unwrap().plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face: base,
        distance: if up { 0.1 } else { -0.1 },
        new_loop: false,
    });
    let before = volume(&d);
    let lid = prism_ops::face_at(d.model(), DVec3::new(0.0, 0.0, 0.1)).unwrap();
    assert!(d.run(Command::PushPull {
        face: lid,
        distance: TINY,
        new_loop: false,
    }));
    let area = std::f64::consts::PI * 0.05 * 0.05;
    assert!((volume(&d) - (before + area * TINY)).abs() < 1e-9);
    assert!(is_solid(d.model()));
    assert!(check::problems(d.model()).is_empty());
    // Pull the same face back by it: the same solid again.
    let lid = prism_ops::face_at(d.model(), DVec3::new(0.0, 0.0, 0.1 + TINY)).unwrap();
    assert!(d.run(Command::PushPull {
        face: lid,
        distance: -TINY,
        new_loop: false,
    }));
    assert!((volume(&d) - before).abs() < 1e-9);
}
