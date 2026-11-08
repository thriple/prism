//! Groups (Groups & Components part 1): Make Group, isolation, moving,
//! copying, Explode, and exact undo with shared definitions.

use std::collections::BTreeSet;

use prism_doc::{Change, Command, Document, InstanceId, Motion};
use prism_math::DVec3;
use prism_ops::Selection;
use prism_ops::check;
use prism_topo::Model;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A box at `at`, `w` wide and `h` tall, as commands at the root.
fn boxed(d: &mut Document, at: DVec3, w: f64, h: f64) {
    d.run(Command::Rect {
        x0: at.x,
        y0: at.y,
        x1: at.x + w,
        y1: at.y + w,
        z: at.z,
    });
    let f = prism_ops::face_at(d.model(), at + v(w / 2.0, w / 2.0, 0.0)).expect("base");
    let up = d.model().face(f).expect("face").plane().normal.z > 0.0;
    d.run(Command::PushPull {
        face: f,
        distance: if up { h } else { -h },
        new_loop: false,
    });
}

fn everything(m: &Model) -> Selection {
    Selection {
        vertices: m.vertices().map(|(id, _)| id).collect(),
        edges: m.edges().map(|(id, _)| id).collect(),
        faces: m.faces().map(|(id, _)| id).collect(),
        guides: BTreeSet::new(),
    }
}

fn group_all(d: &mut Document) -> InstanceId {
    let id = d.assembly().next_instance();
    let sel = everything(d.model());
    assert!(d.run(Command::MakeGroup {
        selection: sel,
        instances: BTreeSet::new(),
    }));
    id
}

#[test]
fn make_group_moves_the_geometry_into_a_group_at_its_corner() {
    let mut d = Document::new();
    boxed(&mut d, v(2.0, 3.0, 0.0), 2.0, 1.0);
    let g = group_all(&mut d);
    assert_eq!(d.model().face_count(), 0, "nothing left outside");
    let def = d.assembly().context_def(&[g]).expect("group");
    let inside = &d.assembly().def(def).expect("def").model;
    assert_eq!(inside.face_count(), 6);
    let (lo, _) = inside.bbox().expect("box");
    assert!(lo.length() < 1e-12, "local coordinates start at the corner");
    let world = d.assembly().world_of(&[g]).expect("world");
    assert_eq!(world.translation, v(2.0, 3.0, 0.0));
    assert!(check::problems(inside).is_empty());
}

#[test]
fn drawing_across_a_group_does_not_touch_it() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 2.0);
    let g = group_all(&mut d);
    let def = d.assembly().context_def(&[g]).unwrap();
    let before = d.assembly().def(def).unwrap().model.clone();
    // A rectangle on the ground through the group's base, and a line
    // across its top.
    d.run(Command::Rect {
        x0: -1.0,
        y0: 1.0,
        x1: 3.0,
        y1: 3.0,
        z: 0.0,
    });
    d.run(Command::Segment {
        p: v(-1.0, 1.0, 2.0),
        q: v(3.0, 1.0, 2.0),
    });
    assert_eq!(
        d.assembly().def(def).unwrap().model,
        before,
        "the group is untouched"
    );
    assert_eq!(d.model().face_count(), 1, "the rectangle is whole");
}

#[test]
fn drawing_inside_a_group_stays_in_it() {
    let mut d = Document::new();
    boxed(&mut d, v(5.0, 0.0, 0.0), 2.0, 2.0);
    let g = group_all(&mut d);
    // A line across the group's top, in its own coordinates.
    assert!(d.run_in(
        &[g],
        Command::Segment {
            p: v(0.0, 1.0, 2.0),
            q: v(2.0, 1.0, 2.0),
        }
    ));
    let def = d.assembly().context_def(&[g]).unwrap();
    assert_eq!(
        d.assembly().def(def).unwrap().model.face_count(),
        7,
        "the top is split"
    );
    assert_eq!(d.model().face_count(), 0);
    assert_eq!(d.undo_context(), vec![g], "undo knows where it was");
}

#[test]
fn moving_a_group_changes_only_its_placement_and_undo_shares_the_geometry() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 2.0);
    let g = group_all(&mut d);
    let def = d.assembly().context_def(&[g]).unwrap();
    let before = d.assembly().clone();
    d.run(Command::Place {
        instances: [g].into(),
        change: Change::Rotate {
            centre: v(1.0, 1.0, 0.0),
            axis: v(0.0, 0.0, 1.0),
            angle: 0.5,
        },
    });
    assert!(
        d.assembly().shares(&before, def),
        "the group's geometry is shared, not copied"
    );
    d.undo();
    assert_eq!(d.assembly(), &before);
    d.redo();
    assert!(
        (d.assembly().world_of(&[g]).unwrap().translation
            - before.world_of(&[g]).unwrap().translation)
            .length()
            > 0.1
    );
}

#[test]
fn copies_of_a_group_are_independent() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 2.0);
    let g = group_all(&mut d);
    let copy = d.assembly().next_instance();
    d.run(Command::CopyInstances {
        instances: [g].into(),
        step: Motion::Translate {
            by: v(4.0, 0.0, 0.0),
        }
        .into(),
        count: 1,
    });
    let a = d.assembly().context_def(&[g]).unwrap();
    let b = d.assembly().context_def(&[copy]).unwrap();
    assert_ne!(a, b, "a copy has its own definition (DECISIONS #22)");
    d.run_in(
        &[copy],
        Command::Segment {
            p: v(0.0, 1.0, 2.0),
            q: v(2.0, 1.0, 2.0),
        },
    );
    assert_eq!(
        d.assembly().def(a).unwrap().model.face_count(),
        6,
        "the original is untouched"
    );
    assert_eq!(d.assembly().def(b).unwrap().model.face_count(), 7);
}

#[test]
fn erase_removes_the_group_and_its_definition() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 2.0);
    let g = group_all(&mut d);
    d.run(Command::EraseInstances {
        instances: [g].into(),
    });
    assert!(!d.assembly().has_instances());
    assert_eq!(d.assembly().defs().count(), 1, "only the root is left");
}

#[test]
fn nested_groups_explode_one_level_at_a_time() {
    let mut d = Document::new();
    boxed(&mut d, v(1.0, 1.0, 0.0), 1.0, 1.0);
    let inner = group_all(&mut d);
    boxed(&mut d, v(4.0, 0.0, 0.0), 1.0, 1.0);
    let outer = d.assembly().next_instance();
    let sel = everything(d.model());
    d.run(Command::MakeGroup {
        selection: sel,
        instances: [inner].into(),
    });
    assert_eq!(d.assembly().root().instances.len(), 1);
    let world_inner = d.assembly().world_of(&[outer, inner]).unwrap();
    assert_eq!(
        world_inner.translation,
        v(1.0, 1.0, 0.0),
        "the inner group did not move"
    );
    d.run(Command::Explode { instance: outer });
    assert_eq!(
        d.model().face_count(),
        6,
        "the outer group's box joined the root"
    );
    assert_eq!(
        d.assembly().world_of(&[inner]).unwrap().translation,
        v(1.0, 1.0, 0.0),
        "the inner group is now the root's, still in place"
    );
}

/// PLAN 1.9's worked example, with boxes: a group is copied and moved
/// aside, the copy is cut in half, the original is moved to line up with
/// the copy's half, and both are exploded. Coincident faces, edges and
/// points collapse to one valid model.
fn overlap_case(k: f64, turn: f64) {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0 * k, 2.0 * k);
    let a = group_all(&mut d);
    // A turn of both groups, so the case also runs off the axes.
    d.run(Command::Place {
        instances: [a].into(),
        change: Change::Rotate {
            centre: DVec3::ZERO,
            axis: v(0.3, 0.2, 1.0),
            angle: turn,
        },
    });
    let b = d.assembly().next_instance();
    let aside = d
        .assembly()
        .world_of(&[a])
        .unwrap()
        .transform_vector3(v(5.0 * k, 0.0, 0.0));
    d.run(Command::CopyInstances {
        instances: [a].into(),
        step: Change::Translate { by: aside },
        count: 1,
    });
    // Cut the copy in half: push its +x side in by half.
    let def_b = d.assembly().context_def(&[b]).unwrap();
    let side = prism_ops::face_at(
        &d.assembly().def(def_b).unwrap().model,
        v(2.0, 1.0, 1.0) * k,
    )
    .expect("side");
    let out = d
        .assembly()
        .def(def_b)
        .unwrap()
        .model
        .face(side)
        .unwrap()
        .plane()
        .normal
        .x
        > 0.0;
    d.run_in(
        &[b],
        Command::PushPull {
            face: side,
            distance: if out { -k } else { k },
            new_loop: false,
        },
    );
    // Line the original up with the copy's half.
    d.run(Command::Place {
        instances: [a].into(),
        change: Change::Translate { by: aside },
    });
    d.run(Command::Explode { instance: a });
    d.run(Command::Explode { instance: b });
    let m = d.model();
    let p = check::problems(m);
    assert!(p.is_empty(), "scale {k}, turn {turn}: {p:?}");
    // One box's worth of outside: 6 sides (some split where the half lay).
    let tol = m.tol();
    let mut area = 0.0;
    for (id, _) in m.faces() {
        area += m.face_area(id);
    }
    // Outside 24 k^2, plus the half's inner face (4 k^2) left inside.
    assert!(
        (area - 28.0 * k * k).abs() < 1e-6 * k * k + tol.area(),
        "scale {k}: area {area}"
    );
}

#[test]
fn exploding_overlapping_groups_gives_one_valid_model() {
    for k in [1e-4, 1.0, 1e3] {
        overlap_case(k, 0.0);
        overlap_case(k, 0.7);
    }
}

#[test]
fn undo_and_redo_of_every_group_step_is_exact() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 2.0);
    let g = group_all(&mut d);
    d.run(Command::CopyInstances {
        instances: [g].into(),
        step: Change::Translate {
            by: v(3.0, 0.0, 0.0),
        },
        count: 2,
    });
    d.run(Command::Explode { instance: g });
    let steps: Vec<_> = d.log().cloned().collect();
    let end = d.assembly().clone();
    for _ in 0..steps.len() {
        d.undo();
    }
    assert!(!d.assembly().has_instances());
    for _ in 0..steps.len() {
        d.redo();
    }
    assert_eq!(d.assembly(), &end);
    let again = Document::replay(steps.iter());
    assert_eq!(again.assembly(), &end, "the log replays to the same groups");
}

#[test]
fn the_flattened_model_has_every_group_placed() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 1.0, 1.0);
    let g = group_all(&mut d);
    d.run(Command::CopyInstances {
        instances: [g].into(),
        step: Change::Translate {
            by: v(3.0, 0.0, 0.0),
        },
        count: 2,
    });
    boxed(&mut d, v(0.0, 4.0, 0.0), 1.0, 1.0);
    let m = d.assembly().flattened();
    assert_eq!(m.face_count(), 4 * 6, "three groups and a loose box");
    let (_, hi) = m.bbox().expect("box");
    assert!((hi.x - 7.0).abs() < 1e-9, "the last copy is placed: {hi:?}");
}

// ---------- components (part 2) ----------

fn component_all(d: &mut Document, name: &str) -> InstanceId {
    let id = d.assembly().next_instance();
    let sel = everything(d.model());
    assert!(d.run(Command::MakeComponent {
        selection: sel,
        instances: BTreeSet::new(),
        name: name.into(),
    }));
    id
}

#[test]
fn copies_of_a_component_share_it_and_change_together() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 2.0);
    let a = component_all(&mut d, "Box");
    let b = d.assembly().next_instance();
    d.run(Command::CopyInstances {
        instances: [a].into(),
        step: Change::Translate {
            by: v(4.0, 0.0, 0.0),
        },
        count: 2,
    });
    let da = d.assembly().context_def(&[a]).unwrap();
    assert_eq!(
        d.assembly().context_def(&[b]),
        Some(da),
        "the copy shares it"
    );
    assert_eq!(d.assembly().uses(da), 3);
    d.run_in(
        &[b],
        Command::Segment {
            p: v(0.0, 1.0, 2.0),
            q: v(2.0, 1.0, 2.0),
        },
    );
    assert_eq!(
        d.assembly().def(da).unwrap().model.face_count(),
        7,
        "every instance changed"
    );
    assert_eq!(d.assembly().components(), vec![(da, "Box")]);
}

#[test]
fn make_unique_splits_one_off_with_a_new_name() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 2.0);
    let a = component_all(&mut d, "Box");
    let b = d.assembly().next_instance();
    d.run(Command::CopyInstances {
        instances: [a].into(),
        step: Change::Translate {
            by: v(4.0, 0.0, 0.0),
        },
        count: 1,
    });
    d.run(Command::MakeUnique {
        instances: [b].into(),
    });
    let (da, db) = (
        d.assembly().context_def(&[a]).unwrap(),
        d.assembly().context_def(&[b]).unwrap(),
    );
    assert_ne!(da, db);
    assert_eq!(d.assembly().def(db).unwrap().name.as_deref(), Some("Box#2"));
    d.run_in(
        &[b],
        Command::Segment {
            p: v(0.0, 1.0, 2.0),
            q: v(2.0, 1.0, 2.0),
        },
    );
    assert_eq!(
        d.assembly().def(da).unwrap().model.face_count(),
        6,
        "the other is untouched"
    );
    // Renaming: a taken name is refused.
    assert!(!d.run(Command::Rename {
        def: db,
        name: "Box".into()
    }));
    assert!(d.run(Command::Rename {
        def: db,
        name: "Crate".into()
    }));
}

#[test]
fn exploding_one_instance_keeps_the_component_for_the_others() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 2.0);
    let a = component_all(&mut d, "Box");
    let b = d.assembly().next_instance();
    d.run(Command::CopyInstances {
        instances: [a].into(),
        step: Change::Translate {
            by: v(4.0, 0.0, 0.0),
        },
        count: 1,
    });
    d.run(Command::Explode { instance: a });
    assert_eq!(d.model().face_count(), 6);
    assert!(
        d.assembly().context_def(&[b]).is_some(),
        "the other instance still has it"
    );
    assert_eq!(d.assembly().components().len(), 1);
}

#[test]
fn a_mirrored_instance_explodes_into_a_valid_model() {
    let mut d = Document::new();
    // An L-shaped block, so mirroring shows.
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 1.0);
    let a = component_all(&mut d, "Block");
    d.run(Command::Place {
        instances: [a].into(),
        change: Change::Scale {
            centre: v(1.0, 1.0, 0.5),
            factor: v(-1.0, 1.0, 1.0),
        },
    });
    d.run(Command::Explode { instance: a });
    let m = d.model();
    assert!(check::problems(m).is_empty(), "{:?}", check::problems(m));
    // Faces still face out: the top's normal is up.
    let top = prism_ops::face_at(m, v(1.0, 1.0, 1.0)).expect("top");
    assert!(m.face(top).unwrap().plane().normal.z > 0.9);
}
