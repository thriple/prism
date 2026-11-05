//! Undo and redo of every operation restore the model exactly (rule 9).

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_doc::{Command, Document};
use prism_math::DVec3;
use prism_ops::select::Selection;
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
    let all: BTreeSet<_> = m.vertices().map(|(id, _)| id).collect();
    if let Some(corner) = prism_ops::vertex_at(m, DVec3::ZERO) {
        out.push(Command::Translate {
            vertices: [corner].into(),
            by: DVec3::new(0.0, 0.0, 1.0),
        });
        out.push(Command::Duplicate {
            selection: prism_ops::connected(m, corner),
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
    out
}

#[test]
fn undo_and_redo_of_every_operation_are_exact() {
    let base = setup();
    let kinds = every_kind(&base);
    assert_eq!(kinds.len(), 12, "one of each kind");
    for c in kinds {
        let mut d = base.clone();
        let before = d.model().geometry_json();
        assert!(d.run(c.clone()), "{} changed something", c.name());
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
    fn undo_all_then_redo_all(picks in prop::collection::vec(0usize..12, 1..6)) {
        let mut d = setup();
        let start = d.model().geometry_json();
        let mut ran = 0;
        for p in picks {
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
        prop_assert_eq!(d.model().geometry_json(), start);
        for _ in 0..ran {
            d.redo();
        }
        prop_assert_eq!(d.model().to_json(), end);
    }
}
