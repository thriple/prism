//! Cut, Copy and Paste; Lock; Intersect Faces With Context (Everyday
//! Commands part 1, brief 33).

use std::collections::BTreeSet;

use prism_doc::{Assembly, Change, Command, Document, FaceSide, InstanceId, Material, ROOT};
use prism_math::{DAffine3, DVec3};
use prism_ops::{Selection, check};
use prism_topo::Model;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A box at `at`, `w` wide and `h` tall, at the root.
fn boxed(d: &mut Document, at: DVec3, w: f64, h: f64) {
    d.run(Command::Rect {
        x0: at.x,
        y0: at.y,
        x1: at.x + w,
        y1: at.y + w,
        z: at.z,
    });
    let f = prism_ops::face_at(d.model(), at + v(w / 2.0, w / 2.0, 0.0)).expect("base");
    let up = d.model().face(f).expect("face").plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face: f,
        distance: if up { h } else { -h },
        new_loop: false,
    });
}

fn everything(m: &Model) -> Selection {
    Selection {
        edges: m.edges().map(|(id, _)| id).collect(),
        faces: m.faces().map(|(id, _)| id).collect(),
        ..Default::default()
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

/// Copy: the whole root (its geometry and its groups).
fn copy_all(d: &Document) -> Assembly {
    let asm = d.assembly();
    let mut m = d.model().clone();
    let geo = prism_ops::select::take(&mut m, &everything(d.model()));
    let groups: BTreeSet<InstanceId> = asm.root().instances.keys().copied().collect();
    asm.clip(ROOT, &DAffine3::IDENTITY, geo, &groups)
}

fn paste(d: &mut Document, clip: &Assembly, by: DVec3) -> bool {
    d.run(Command::Paste {
        clip: Box::new(clip.clone()),
        at: DAffine3::from_translation(by),
    })
}

#[test]
fn loose_geometry_pastes_into_another_document_where_it_is_put() {
    let mut a = Document::new();
    boxed(&mut a, v(0.0, 0.0, 0.0), 2.0, 1.0);
    let clip = copy_all(&a);
    let mut b = Document::new();
    assert!(paste(&mut b, &clip, v(5.0, 0.0, 0.0)));
    assert_eq!(b.model().face_count(), 6);
    let (lo, hi) = b.model().bbox().expect("box");
    assert!(lo.distance(v(5.0, 0.0, 0.0)) < 1e-12);
    assert!(hi.distance(v(7.0, 2.0, 1.0)) < 1e-12);
    assert!(check::problems(b.model()).is_empty());
    // One step to undo.
    assert!(b.undo());
    assert_eq!(b.model().face_count(), 0);
}

#[test]
fn paste_in_place_on_itself_joins_by_the_overlap_rules() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 1.0);
    let clip = copy_all(&d);
    assert!(paste(&mut d, &clip, DVec3::ZERO));
    assert_eq!(d.model().face_count(), 6, "the copy welds onto the box");
    assert!(check::problems(d.model()).is_empty());
    // Beside it, sharing a wall: the two walls meet and go, as when a
    // group is exploded against another box (one solid of 10 faces).
    assert!(paste(&mut d, &clip, v(2.0, 0.0, 0.0)));
    assert_eq!(d.model().face_count(), 10);
    assert!(check::problems(d.model()).is_empty());
}

#[test]
fn a_group_pastes_as_its_own_group_and_a_component_stays_shared() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 1.0, 1.0);
    let g = group_all(&mut d);
    let clip = copy_all(&d);
    assert!(paste(&mut d, &clip, v(3.0, 0.0, 0.0)));
    let root = d.assembly().root();
    assert_eq!(root.instances.len(), 2);
    let defs: BTreeSet<_> = root.instances.values().map(|i| i.def).collect();
    assert_eq!(defs.len(), 2, "a pasted group has its own definition");
    let new = *root.instances.keys().find(|k| **k != g).expect("pasted");
    assert_eq!(
        d.assembly().world_of(&[new]).expect("placed").translation,
        v(3.0, 0.0, 0.0)
    );

    // A component: pasted in the same document, its copy shares it.
    let mut c = Document::new();
    boxed(&mut c, v(0.0, 0.0, 0.0), 1.0, 1.0);
    let id = c.assembly().next_instance();
    c.run(Command::MakeComponent {
        selection: everything(c.model()),
        instances: BTreeSet::new(),
        name: "Crate".into(),
    });
    let clip = copy_all(&c);
    assert!(paste(&mut c, &clip, v(3.0, 0.0, 0.0)));
    let defs: BTreeSet<_> = c
        .assembly()
        .root()
        .instances
        .values()
        .map(|i| i.def)
        .collect();
    assert_eq!(defs.len(), 1, "the same component is reused");
    assert_eq!(c.assembly().components().len(), 1);

    // Into a document with a different "Crate": the pasted one is renamed.
    let mut other = Document::new();
    boxed(&mut other, v(0.0, 0.0, 0.0), 2.0, 2.0);
    other.run(Command::MakeComponent {
        selection: everything(other.model()),
        instances: BTreeSet::new(),
        name: "Crate".into(),
    });
    assert!(paste(&mut other, &clip, v(5.0, 0.0, 0.0)));
    let mut names: Vec<String> = other
        .assembly()
        .components()
        .into_iter()
        .map(|(_, n)| n.to_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["Crate", "Crate#2"]);
    let _ = id;
}

#[test]
fn materials_and_tags_come_along_and_are_reused_by_name_and_value() {
    let mut a = Document::new();
    boxed(&mut a, v(0.0, 0.0, 0.0), 1.0, 1.0);
    a.run(Command::AddMaterial {
        material: Material::colour("Red", [200, 20, 20]),
    });
    let red = *a.assembly().materials().0.keys().next().expect("red");
    let sides = a
        .model()
        .faces()
        .map(|(id, _)| (id, FaceSide::Front))
        .collect();
    a.run(Command::Paint {
        sides,
        material: Some(red),
    });
    let g = group_all(&mut a);
    a.run(Command::AddTag {
        name: "Parts".into(),
    });
    a.run(Command::SetTag {
        instances: [g].into(),
        tag: Some(prism_doc::TagId(0)),
    });
    let clip = copy_all(&a);

    let mut b = Document::new();
    b.run(Command::AddMaterial {
        material: Material::colour("Blue", [20, 20, 200]),
    });
    b.run(Command::AddTag {
        name: "Walls".into(),
    });
    assert!(paste(&mut b, &clip, DVec3::ZERO));
    assert!(paste(&mut b, &clip, v(3.0, 0.0, 0.0)));
    let asm = b.assembly();
    let names: Vec<&str> = asm
        .materials()
        .0
        .values()
        .map(|m| m.name.as_str())
        .collect();
    assert_eq!(names, ["Blue", "Red"], "Red is added once and reused");
    let red_here = asm
        .materials()
        .0
        .iter()
        .find(|(_, m)| m.name == "Red")
        .map(|(k, _)| *k)
        .expect("red");
    for (i, inst) in &asm.root().instances {
        let m = &asm.def(inst.def).expect("def").model;
        assert!(m.faces().all(|(_, f)| f.paint.front == Some(red_here)));
        let t = asm.tag_of(*i).expect("tagged");
        assert_eq!(asm.tag(t).expect("tag").name, "Parts");
    }
    assert_eq!(asm.tags().0.len(), 2, "Parts is added once");
}

#[test]
fn paste_in_place_inside_an_open_group_lands_where_it_was_copied() {
    let mut d = Document::new();
    boxed(&mut d, v(4.0, 0.0, 0.0), 1.0, 1.0);
    let g = group_all(&mut d);
    // A box at the root, copied, pasted into the group (which sits at 4,0,0).
    boxed(&mut d, v(0.0, 0.0, 0.0), 1.0, 1.0);
    let mut m = d.model().clone();
    let geo = prism_ops::select::take(&mut m, &everything(d.model()));
    let clip = d
        .assembly()
        .clip(ROOT, &DAffine3::IDENTITY, geo, &BTreeSet::new());
    // The session puts world commands into the group's coordinates.
    let to_local = d.assembly().world_of(&[g]).unwrap().inverse();
    let def = d.assembly().context_def(&[g]).unwrap();
    let local = Command::Paste {
        clip: Box::new(clip),
        at: DAffine3::IDENTITY,
    }
    .to_local(&to_local, &d.assembly().def(def).unwrap().model);
    assert!(d.run_in(&[g], local));
    let inside = &d.assembly().def(def).unwrap().model;
    assert_eq!(inside.face_count(), 12);
    let (lo, _) = inside.bbox().unwrap();
    assert!(
        lo.distance(v(-4.0, 0.0, 0.0)) < 1e-12,
        "the world place kept"
    );
}

#[test]
fn cut_is_one_step() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 1.0, 1.0);
    let g = group_all(&mut d);
    boxed(&mut d, v(3.0, 0.0, 0.0), 1.0, 1.0);
    let before = d.assembly().clone();
    assert!(d.run(Command::Cut {
        selection: everything(d.model()),
        instances: [g].into(),
    }));
    assert_eq!(d.model().face_count(), 0);
    assert!(d.assembly().root().instances.is_empty());
    assert!(d.undo());
    assert_eq!(*d.assembly(), before);
}

#[test]
fn a_locked_group_refuses_edits_but_can_be_copied_hidden_and_unlocked() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 1.0, 1.0);
    let g = group_all(&mut d);
    assert!(d.run(Command::SetLocked {
        instances: [g].into(),
        locked: true,
    }));
    let one: BTreeSet<InstanceId> = [g].into();
    let refused = [
        Command::Place {
            instances: one.clone(),
            change: Change::Translate {
                by: v(1.0, 0.0, 0.0),
            },
        },
        Command::EraseInstances {
            instances: one.clone(),
        },
        Command::Explode { instance: g },
        Command::MakeUnique {
            instances: one.clone(),
        },
        Command::In {
            context: vec![g],
            command: Box::new(Command::Segment {
                p: v(0.0, 0.0, 1.0),
                q: v(1.0, 1.0, 1.0),
            }),
        },
        Command::Batch {
            commands: vec![Command::EraseInstances {
                instances: one.clone(),
            }],
        },
    ];
    for c in refused {
        assert!(c.locked_in(d.assembly()), "{c:?}");
        assert!(!d.run(c), "refused");
    }
    // Copies, hiding and naming are fine; a copy is not locked.
    let next = d.assembly().next_instance();
    assert!(d.run(Command::CopyInstances {
        instances: one.clone(),
        step: Change::Translate {
            by: v(2.0, 0.0, 0.0)
        },
        count: 1,
    }));
    assert!(d.run(Command::SetHidden {
        instances: one.clone(),
        hidden: true,
    }));
    assert!(d.assembly().is_locked(g));
    assert!(!d.assembly().is_locked(next) || next == g);
    // Undo the hide, the copy and the lock: it moves again.
    for _ in 0..3 {
        assert!(d.undo());
    }
    assert!(!d.assembly().is_locked(g));
    assert!(d.run(Command::Place {
        instances: one,
        change: Change::Translate {
            by: v(1.0, 0.0, 0.0)
        },
    }));
}

#[test]
fn with_context_cuts_only_the_open_group() {
    let mut d = Document::new();
    boxed(&mut d, v(0.0, 0.0, 0.0), 2.0, 2.0);
    let g = group_all(&mut d);
    // A wall standing through the group, at the root.
    d.run(Command::Loop {
        points: vec![
            v(1.0, -1.0, -1.0),
            v(1.0, 3.0, -1.0),
            v(1.0, 3.0, 3.0),
            v(1.0, -1.0, 3.0),
        ],
    });
    let outside = d.model().clone();
    let def = d.assembly().context_def(&[g]).unwrap();
    let faces_before = d.assembly().def(def).unwrap().model.face_count();
    // The wall, in the group's coordinates (the group sits at the origin).
    assert!(d.run_in(
        &[g],
        Command::IntersectWith {
            others: outside.clone()
        }
    ));
    let inside = &d.assembly().def(def).unwrap().model;
    // The wall crosses four of the box's faces; each splits in two, and
    // the crossings close a loop, which fills (as Intersect Faces does).
    assert_eq!(inside.face_count(), faces_before + 5);
    assert!(check::problems(inside).is_empty());
    assert_eq!(d.model().to_json(), outside.to_json(), "outside unchanged");
}
