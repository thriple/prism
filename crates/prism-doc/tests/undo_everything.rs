//! Undo and redo of every operation restore the model exactly (rule 9).

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_doc::{Command, Document};
use prism_math::DVec3;
use prism_ops::select::Selection;
use prism_topo::Guide;
use proptest::prelude::*;

fn setup() -> Document {
    let mut d = Document::new();
    d.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 4.0,
        y1: 4.0,
        z: 0.0,
    });
    d.run(Command::Rect {
        x0: 1.0,
        y0: 1.0,
        x1: 3.0,
        y1: 3.0,
        z: 0.0,
    });
    d.run(Command::AddGuide {
        guide: Guide::Line {
            origin: DVec3::new(0.0, -1.0, 0.0),
            dir: DVec3::X,
        },
    });
    // An upright rectangle through the inner square, for Intersect Faces.
    d.run(Command::Loop {
        points: vec![
            DVec3::new(1.5, 2.5, -0.5),
            DVec3::new(2.5, 2.5, -0.5),
            DVec3::new(2.5, 2.5, 0.5),
            DVec3::new(1.5, 2.5, 0.5),
        ],
    });
    d
}

/// One command of each kind that applies to the current model.
fn every_kind(d: &Document) -> Vec<Command> {
    let m = d.model();
    let mut out = vec![
        Command::Segment {
            p: DVec3::new(0.0, 2.0, 0.0),
            q: DVec3::new(4.0, 2.0, 0.0),
        },
        Command::Rect {
            x0: 3.0,
            y0: 3.0,
            x1: 6.0,
            y1: 6.0,
            z: 0.0,
        },
        // Across both squares (Exact Curves 2b).
        Command::Circle {
            centre: DVec3::new(3.0, 2.0, 0.0),
            normal: DVec3::Z,
            radius: 1.5,
        },
        // A pie across both squares (Exact Curves 3a).
        Command::Arc {
            centre: DVec3::new(2.0, 1.0, 0.0),
            normal: DVec3::Z,
            radius: 2.0,
            start: DVec3::new(4.0, 1.0, 0.0),
            sweep: 2.0,
            pie: true,
        },
        // A rounded corner (Exact Curves 3d).
        Command::Fillet {
            corner: DVec3::new(0.0, 0.0, 0.0),
            radius: 0.5,
        },
    ];
    // A standing rectangle (on the plane x = 5).
    out.push(Command::Loop {
        points: vec![
            DVec3::new(5.0, 0.0, 0.0),
            DVec3::new(5.0, 2.0, 0.0),
            DVec3::new(5.0, 2.0, 1.0),
            DVec3::new(5.0, 0.0, 1.0),
        ],
    });
    if let Some((edge, _)) = m.edges().next() {
        out.push(Command::Erase {
            selection: Selection::edge(edge),
        });
    }
    if let Some(face) = prism_ops::face_at(m, DVec3::new(2.0, 2.0, 0.0)) {
        out.push(Command::PushPull {
            face,
            distance: 1.0,
            new_loop: false,
        });
    }
    if let Some(ring) = prism_ops::face_at(m, DVec3::new(0.5, 0.5, 0.0)) {
        out.push(Command::PushPull {
            face: ring,
            distance: 2.0,
            new_loop: true,
        });
    }
    // A face with arcs, pulled into cylinders (Exact Curves 4b).
    if let Some((face, _)) = m
        .faces()
        .find(|(id, f)| !f.is_curved() && m.face_has_arcs(*id))
    {
        out.push(Command::PushPull {
            face,
            distance: 0.75,
            new_loop: false,
        });
    }
    let all: BTreeSet<_> = m.vertices().map(|(id, _)| id).collect();
    if let Some(corner) = prism_ops::vertex_at(m, DVec3::ZERO) {
        out.push(Command::Translate {
            vertices: [corner].into(),
            by: DVec3::new(0.0, 0.0, 1.0),
        });
    }
    if let Some(corner) = prism_ops::vertex_at(m, DVec3::ZERO) {
        out.push(Command::Copy {
            selection: prism_ops::connected(m, corner),
            step: prism_doc::Motion::Translate {
                by: DVec3::new(6.0, 0.0, 0.0),
            },
            count: 3,
        });
        out.push(Command::Copy {
            selection: prism_ops::connected(m, corner),
            step: prism_doc::Motion::Rotate {
                centre: DVec3::new(-2.0, 0.0, 0.0),
                axis: DVec3::Z,
                angle: 0.5,
            },
            count: 1,
        });
    }
    if let Some(ring) = prism_ops::face_at(m, DVec3::new(0.5, 0.5, 0.0)) {
        out.push(Command::Offset {
            face: ring,
            distance: 0.25,
        });
        out.push(Command::Offset {
            face: ring,
            distance: -0.5,
        });
    }
    out.push(Command::AddGuide {
        guide: Guide::Point {
            at: DVec3::new(1.0, 2.0, 3.0),
        },
    });
    if let Some((g, _)) = m.guides().next() {
        out.push(Command::Erase {
            selection: Selection {
                guides: [g].into(),
                ..Default::default()
            },
        });
    }
    out.push(Command::Import {
        model: {
            let mut b = prism_topo::Model::new();
            prism_ops::add_rect(&mut b, 10.0, 0.0, 11.0, 2.0, 0.5);
            b
        },
    });
    out.push(Command::IntersectFaces { faces: None });
    out.push(Command::Rotate {
        vertices: all.clone(),
        centre: DVec3::ZERO,
        axis: DVec3::new(1.0, 1.0, 1.0),
        angle: 0.9,
    });
    out.push(Command::Scale {
        vertices: all,
        centre: DVec3::ZERO,
        factor: DVec3::new(2.0, 1.0, 0.5),
    });
    // Exact Curves 4c (last, so the regressions below keep their picks).
    if let Some((edge, e)) = m.edges().next() {
        out.push(Command::Soften {
            edges: [edge].into(),
            soft: !e.soft,
        });
    }
    // A cylinder face, given a new radius (Exact Curves 4d).
    if let Some((face, _)) = m.faces().find(|(_, f)| f.is_curved()) {
        out.push(Command::PushPull {
            face,
            distance: 0.25,
            new_loop: false,
        });
    }
    out
}

#[test]
fn undo_and_redo_of_every_operation_are_exact() {
    let base = setup();
    let kinds = every_kind(&base);
    assert_eq!(kinds.len(), 21, "one of each kind (no face has arcs yet)");
    for c in kinds {
        let mut d = base.clone();
        let before = d.model().geometry_json();
        assert!(d.run(c.clone()), "{} changed something", c.name());
        let p = d.model().bookkeeping_problems();
        assert!(p.is_empty(), "{}: {p:?}", c.name());
        let after = d.model().to_json();
        d.undo();
        assert_eq!(d.model().geometry_json(), before, "undo {}", c.name());
        d.redo();
        assert_eq!(d.model().to_json(), after, "redo {}", c.name());
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// Any sequence of commands, undone all the way and redone, ends exactly
    /// where it was.
    #[test]
    fn undo_all_then_redo_all(picks in prop::collection::vec(0usize..23, 1..6)) {
        let mut d = setup();
        let start = d.model().geometry_json();
        let mut ran = 0;
        for p in picks {
            let options = every_kind(&d);
            if let Some(c) = options.get(p % options.len())
                && d.run(c.clone())
            {
                ran += 1;
                // Loops still name the right edges (Exact Curves part 1).
                let p = d.model().bookkeeping_problems();
                prop_assert!(p.is_empty(), "{}: {:?}", c.name(), p);
            }
        }
        let end = d.model().to_json();
        for _ in 0..ran {
            d.undo();
        }
        prop_assert_eq!(d.model().geometry_json(), start);
        for _ in 0..ran {
            d.redo();
        }
        prop_assert_eq!(d.model().to_json(), end);
    }
}

/// Found by CI on Windows: a command that changes nothing must leave the
/// model exactly as it was (a 0.0 turned to -0.0 compares equal).
#[test]
fn undo_all_then_redo_all_regression_signed_zero() {
    let mut d = setup();
    let mut ran = 0;
    for p in [11usize, 0, 5] {
        let options = every_kind(&d);
        if let Some(c) = options.get(p % options.len())
            && d.run(c.clone())
        {
            ran += 1;
        }
    }
    let end = d.model().to_json();
    for _ in 0..ran {
        d.undo();
    }
    for _ in 0..ran {
        d.redo();
    }
    assert_eq!(d.model().to_json(), end);
}

/// Found by CI on Windows, once the check ran after every step: two
/// overlapping rotated copies leave a face whose loop runs along one edge
/// twice (a bridge), and splitting that edge only fixed the first run, so
/// the Push/Pull after them left a face using an edge that was gone.
#[test]
fn rotated_copies_then_a_pull_keep_the_loops_whole() {
    let mut d = setup();
    for p in [8usize, 8, 5] {
        let options = every_kind(&d);
        if let Some(c) = options.get(p % options.len()) {
            let name = c.name();
            d.run(c.clone());
            let problems = d.model().bookkeeping_problems();
            assert!(problems.is_empty(), "{name}: {problems:?}");
        }
    }
}

/// Faces with arcs pulled into cylinders undo and redo exactly
/// (Exact Curves 4b).
#[test]
fn pulling_faces_with_arcs_undoes_exactly() {
    let mut d = setup();
    d.run(Command::Circle {
        centre: DVec3::new(3.0, 2.0, 0.0),
        normal: DVec3::Z,
        radius: 1.5,
    });
    let base = d.clone();
    let pulls: Vec<Command> = every_kind(&d)
        .into_iter()
        .filter(|c| matches!(c, Command::PushPull { face, .. } if d.model().face_has_arcs(*face)))
        .collect();
    assert!(!pulls.is_empty());
    for c in pulls {
        let mut d = base.clone();
        let before = d.model().geometry_json();
        assert!(d.run(c.clone()), "pulled");
        assert!(d.model().faces().any(|(_, f)| f.is_curved()), "a cylinder");
        let p = prism_ops::check::problems(d.model());
        assert!(p.is_empty(), "{p:?}");
        let after = d.model().to_json();
        d.undo();
        assert_eq!(d.model().geometry_json(), before);
        d.redo();
        assert_eq!(d.model().to_json(), after);
    }
}

/// A cylinder's new radius undoes and redoes exactly (Exact Curves 4d).
#[test]
fn a_new_radius_undoes_exactly() {
    let mut d = setup();
    d.run(Command::Circle {
        centre: DVec3::new(3.0, 2.0, 5.0),
        normal: DVec3::Z,
        radius: 1.5,
    });
    let m = d.model();
    let disc = m
        .faces()
        .find(|(id, f)| m.face_has_arcs(*id) && f.plane().is_some_and(|p| p.origin.z > 4.0))
        .map(|(id, _)| id)
        .expect("the disc");
    assert!(d.run(Command::PushPull {
        face: disc,
        distance: 1.0,
        new_loop: false,
    }));
    let before = d.model().geometry_json();
    let c = every_kind(&d)
        .into_iter()
        .find(|c| matches!(c, Command::PushPull { face, .. } if d.model().face(*face).is_some_and(|f| f.is_curved())))
        .expect("a side to pull");
    assert!(d.run(c), "a new radius");
    let p = prism_ops::check::problems(d.model());
    assert!(p.is_empty(), "{p:?}");
    let after = d.model().to_json();
    d.undo();
    assert_eq!(d.model().geometry_json(), before);
    d.redo();
    assert_eq!(d.model().to_json(), after);
}
