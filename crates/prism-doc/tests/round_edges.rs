//! Many edges rounded in one command (Fillet Joins step B, brief 32): one
//! undo takes it all back, redo makes it again, and random sets of a box's
//! edges either round cleanly or are refused leaving the model as it was.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_doc::{Command, Document};
use prism_topo::EdgeId;
use proptest::prelude::*;

/// A document holding a 2 m cube.
fn cube() -> Document {
    let mut d = Document::new();
    assert!(d.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 2.0,
        y1: 2.0,
        z: 0.0,
    }));
    let face = d.model().faces().next().unwrap().0;
    let up = d.model().face(face).unwrap().plane().unwrap().normal.z;
    assert!(d.run(Command::PushPull {
        face,
        distance: 2.0 * up,
        new_loop: false,
    }));
    d
}

fn edges(d: &Document) -> Vec<EdgeId> {
    d.model().edges().map(|(id, _)| id).collect()
}

#[test]
fn rounding_many_edges_is_one_undo() {
    let mut d = cube();
    let before = d.model().geometry_json();
    let all: BTreeSet<EdgeId> = edges(&d).into_iter().collect();
    assert_eq!(all.len(), 12);
    let c = Command::RoundEdges {
        edges: all,
        size: 0.25,
        fillet: true,
    };
    assert_eq!(c.name(), "Fillet");
    assert!(d.run(c));
    let after = d.model().to_json();
    assert!(d.model().face_count() > 6);
    d.undo();
    assert_eq!(d.model().geometry_json(), before);
    d.redo();
    assert_eq!(d.model().to_json(), after);
}

#[test]
fn a_refused_round_is_not_an_edit() {
    let mut d = cube();
    let before = d.model().to_json();
    let e = edges(&d)[0];
    let c = Command::RoundEdges {
        edges: BTreeSet::from([e]),
        size: 5.0,
        fillet: true,
    };
    assert!(c.refusal(d.model()).is_some());
    assert!(!d.run(c));
    assert_eq!(d.model().to_json(), before);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn random_edge_sets_on_a_box_round_or_refuse_cleanly(
        pick in proptest::collection::btree_set(0usize..12, 1..=12),
        size in prop_oneof![Just(0.1), Just(0.25), Just(0.5)],
        fillet in any::<bool>(),
    ) {
        let mut d = cube();
        let all = edges(&d);
        let set: BTreeSet<EdgeId> = pick.iter().map(|i| all[*i]).collect();
        let before = d.model().geometry_json();
        let c = Command::RoundEdges { edges: set, size, fillet };
        if d.run(c) {
            let p = prism_ops::check::problems(d.model());
            prop_assert!(p.is_empty(), "{:?}", p);
            let h = prism_ops::health::check(d.model());
            prop_assert_eq!((h.watertight, h.leaky), (1, 0));
            d.undo();
        }
        prop_assert_eq!(d.model().geometry_json(), before);
    }
}
