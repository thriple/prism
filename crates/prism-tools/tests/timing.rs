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

/// How long one Push/Pull on the Large Scene takes (the preview runs it on
/// every mouse move).
#[test]
#[ignore = "prints timings"]
fn push_pull_on_the_large_scene() {
    let doc = prism_tools::scenes::large(50);
    let m = doc.model().clone();
    let (top, _) = m
        .faces()
        .max_by(|a, b| a.1.plane().origin.z.total_cmp(&b.1.plane().origin.z))
        .expect("a face");
    let n = 10;
    let t = std::time::Instant::now();
    for _ in 0..n {
        let mut c = m.clone();
        prism_ops::push_pull(&mut c, top, 0.5, false);
    }
    println!(
        "{} faces: push/pull {:.1} ms",
        m.face_count(),
        t.elapsed().as_secs_f64() * 1e3 / f64::from(n)
    );
}

/// How long the scene takes with many instances of one component (each
/// instance's scene is built from its model today).
#[test]
#[ignore = "prints timings"]
fn many_instances_of_one_component() {
    use prism_doc::{Change, Command};
    let mut doc = prism_tools::scenes::large(5);
    let m = doc.model();
    let sel = prism_ops::Selection {
        vertices: m.vertices().map(|(id, _)| id).collect(),
        edges: m.edges().map(|(id, _)| id).collect(),
        faces: m.faces().map(|(id, _)| id).collect(),
        guides: Default::default(),
    };
    let faces = m.face_count();
    let a = doc.assembly().next_instance();
    doc.run(Command::MakeComponent {
        selection: sel,
        instances: Default::default(),
        name: "Block".into(),
    });
    for n in [1u32, 10, 100] {
        let mut d = doc.clone();
        if n > 1 {
            d.run(Command::CopyInstances {
                instances: [a].into(),
                step: Change::Translate {
                    by: DVec3::new(0.0, 0.0, 50.0),
                },
                count: n - 1,
            });
        }
        let t = std::time::Instant::now();
        let _ = prism_tools::Session::new(d);
        println!(
            "{n} instances of {faces} faces: scene {:.1} ms",
            t.elapsed().as_secs_f64() * 1e3
        );
    }
}

/// Hidden must be free (PLAN 1.14): with 90% of the groups hidden, the
/// scene builds in about the time of a model holding only the visible 10%.
#[test]
#[ignore = "prints timings"]
fn ninety_percent_hidden() {
    use prism_doc::{Change, Command};
    let build = |hide: bool, count: u32| {
        let mut doc = prism_tools::scenes::large(5);
        let m = doc.model();
        let sel = prism_ops::Selection {
            vertices: m.vertices().map(|(id, _)| id).collect(),
            edges: m.edges().map(|(id, _)| id).collect(),
            faces: m.faces().map(|(id, _)| id).collect(),
            guides: Default::default(),
        };
        let a = doc.assembly().next_instance();
        doc.run(Command::MakeGroup {
            selection: sel,
            instances: Default::default(),
        });
        doc.run(Command::CopyInstances {
            instances: [a].into(),
            step: Change::Translate {
                by: DVec3::new(0.0, 0.0, 50.0),
            },
            count: count - 1,
        });
        if hide {
            let all: Vec<_> = doc.assembly().root().instances.keys().copied().collect();
            let hidden = all[..all.len() * 9 / 10].iter().copied().collect();
            doc.run(Command::SetHidden {
                instances: hidden,
                hidden: true,
            });
        }
        let t = std::time::Instant::now();
        let _ = prism_tools::Session::new(doc);
        t.elapsed().as_secs_f64() * 1e3
    };
    let hidden = build(true, 100);
    let small = build(false, 10);
    println!("100 groups, 90 hidden: scene {hidden:.1} ms; 10 groups: {small:.1} ms");
}

/// How long every Model Health check takes on the Large Scene.
#[test]
#[ignore = "prints timings"]
fn health_on_the_large_scene() {
    let doc = prism_tools::scenes::large(50);
    let t = std::time::Instant::now();
    let r = prism_ops::health::check(doc.model());
    println!(
        "{} faces: health {:.0} ms ({} interior, {} crossing)",
        doc.model().face_count(),
        t.elapsed().as_secs_f64() * 1e3,
        r.interior.count(),
        r.crossing.count()
    );
}
