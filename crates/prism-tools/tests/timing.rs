//! Inference timing on the Large Scene, for NOTES (run by hand:
//! `cargo test --release -p prism-tools --test timing -- --ignored --nocapture`).

use prism_infer::{Context, Lock, Memory, Settings, infer};
use prism_math::{DVec2, DVec3};
use prism_view::{Camera, Scene, View};

#[test]
#[ignore = "prints timings"]
fn inference_on_the_large_scene() {
    let doc = prism_tools::scenes::large(50);
    let scene = Scene::build(doc.model());
    let cam = Camera {
        eye: DVec3::new(37.0, -30.0, 40.0),
        target: DVec3::new(37.0, 37.0, 0.0),
    };
    let view = View::new(&cam, 1200.0, 800.0);
    let mut mem = Memory::default();
    let start = DVec3::new(10.0, 10.0, 0.0);
    let ctx = Context {
        start: Some(start),
        ..Default::default()
    };
    let n = 40;
    let t = std::time::Instant::now();
    for k in 0..n {
        let s = DVec2::new(200.0 + f64::from(k) * 20.0, 300.0 + f64::from(k % 7) * 30.0);
        let i = infer(
            &scene,
            &view,
            s,
            &ctx,
            Lock::None,
            &mem,
            &Settings::default(),
        );
        mem.observe(&i, f64::from(k), &Settings::default());
    }
    let each = t.elapsed() / n;
    eprintln!(
        "{} faces, {} edges: inference {each:?} per mouse move",
        doc.model().face_count(),
        doc.model().edge_count()
    );
}

/// How long a Move preview of the whole Large Scene takes: rebuilding the
/// model and scene (the old way) against moving the drawn scene.
#[test]
#[ignore = "prints timings"]
fn move_preview_on_the_large_scene() {
    use prism_math::DVec3;
    let doc = prism_tools::scenes::large(50);
    let m = doc.model().clone();
    let all: std::collections::BTreeSet<_> = m.vertices().map(|(id, _)| id).collect();
    let scene = prism_view::Scene::build(&m);
    let t = std::time::Instant::now();
    let mut moved = m.clone();
    prism_doc::Command::Translate {
        vertices: all.clone(),
        by: DVec3::new(0.1, 0.0, 0.0),
    }
    .apply(&mut moved);
    let _ = prism_view::Scene::build(&moved);
    let slow = t.elapsed();
    let t = std::time::Instant::now();
    let d = DVec3::new(0.1, 0.0, 0.0);
    let _ = scene.rigidly_moved(&all, |p| p + d, |n| n).expect("rigid");
    let fast = t.elapsed();
    println!(
        "{} faces: rebuild {:.1} ms, rigid move {:.1} ms per mouse move",
        m.face_count(),
        slow.as_secs_f64() * 1e3,
        fast.as_secs_f64() * 1e3
    );
}
