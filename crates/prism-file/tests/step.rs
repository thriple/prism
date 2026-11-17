//! STEP export (Solids & CAD part 5b, brief 44): the document's solids as
//! one STEP file, groups and components kept as products, hidden groups
//! and bodies the kernel cannot take left out and counted.

use prism_doc::{Assembly, Change, Command, Document, InstanceId};
use prism_file::step::{INVALID, export};
use prism_math::{DVec3, LengthUnit};
use prism_ops::Selection;

/// What is new in `m` since `from`.
fn all(m: &prism_topo::Model, from: &Selection) -> Selection {
    Selection {
        vertices: m
            .vertices()
            .map(|(id, _)| id)
            .filter(|v| !from.vertices.contains(v))
            .collect(),
        edges: m
            .edges()
            .map(|(id, _)| id)
            .filter(|e| !from.edges.contains(e))
            .collect(),
        faces: m
            .faces()
            .map(|(id, _)| id)
            .filter(|f| !from.faces.contains(f))
            .collect(),
        guides: Default::default(),
    }
}

/// A cube of side `w` at (x, y), and what it added.
fn box_at(d: &mut Document, x: f64, y: f64, w: f64) -> Selection {
    let before = all(d.model(), &Selection::default());
    d.run(Command::Rect {
        x0: x,
        y0: y,
        x1: x + w,
        y1: y + w,
        z: 0.0,
    });
    let face = prism_ops::face_at(d.model(), DVec3::new(x + w / 2.0, y + w / 2.0, 0.0)).unwrap();
    let up = d.model().face(face).unwrap().plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face,
        distance: if up { w } else { -w },
        new_loop: false,
    });
    all(d.model(), &before)
}

/// A "Wall" group of two "Chunk" groups, a component "Post" used three
/// times (the last mirrored), and a loose box. Returns the wall.
fn document() -> (Document, InstanceId) {
    let mut d = Document::new();
    let chunk = d.assembly().next_instance();
    let sel = box_at(&mut d, 0.0, 0.0, 1.0);
    d.run(Command::MakeGroup {
        selection: sel,
        instances: Default::default(),
    });
    let chunk2 = d.assembly().next_instance();
    let sel = box_at(&mut d, 1.0, 0.0, 1.0);
    d.run(Command::MakeGroup {
        selection: sel,
        instances: Default::default(),
    });
    for c in [chunk, chunk2] {
        d.run(Command::NameInstance {
            instance: c,
            name: "Chunk".into(),
        });
    }
    let wall = d.assembly().next_instance();
    d.run(Command::MakeGroup {
        selection: Default::default(),
        instances: [chunk, chunk2].into(),
    });
    d.run(Command::NameInstance {
        instance: wall,
        name: "Wall".into(),
    });
    let post = d.assembly().next_instance();
    let sel = box_at(&mut d, 5.0, 0.0, 0.5);
    d.run(Command::MakeComponent {
        selection: sel,
        instances: Default::default(),
        name: "Post".into(),
    });
    let copies = d.assembly().next_instance();
    d.run(Command::CopyInstances {
        instances: [post].into(),
        step: Change::Translate {
            by: DVec3::new(0.0, 2.0, 0.0),
        },
        count: 2,
    });
    let last = InstanceId(copies.0 + 1);
    d.run(Command::Place {
        instances: [last].into(),
        change: Change::Scale {
            centre: DVec3::new(5.25, 4.25, 0.0),
            factor: DVec3::new(-1.0, 1.0, 1.0),
        },
    });
    box_at(&mut d, -3.0, 0.0, 1.0);
    (d, wall)
}

/// The volume of everything read back from `text`.
fn volume_read(text: &str) -> f64 {
    let back = prism_kernel::step::read(text, 1e-9).unwrap();
    back.shapes
        .iter()
        .map(|s| {
            let r = &s.as_ref().unwrap().region;
            prism_kernel::solid::Solid::build(r, 1e-9)
                .unwrap()
                .volume()
                .unwrap()
        })
        .sum()
}

#[test]
fn groups_and_a_shared_component_go_out_as_products() {
    let (d, _) = document();
    let e = export(d.assembly(), "Workshop", LengthUnit::Millimetres, None).unwrap();
    // The loose box, two chunks, the post once, and the mirrored post
    // written where it stands.
    assert_eq!((e.solids, e.sheets), (5, 0));
    assert_eq!(e.left_out, Vec::<&str>::new());
    assert_eq!(e.summary(), "5 solids");
    assert!(e.text.contains("PRODUCT('Workshop'"));
    assert!(e.text.contains("PRODUCT('Wall'"));
    // The two posts placed as they are share one product; the mirrored
    // one is its own.
    assert_eq!(e.text.matches("PRODUCT('Post'").count(), 2);
    // Wall, its two chunks, and three posts.
    assert_eq!(e.text.matches("NEXT_ASSEMBLY_USAGE_OCCURRENCE(").count(), 6);
    let want = 3.0 + 3.0 * 0.125;
    let got = volume_read(&e.text);
    // Written in millimetres, read back in metres.
    assert!((got - want).abs() < 1e-9, "{got}");
}

#[test]
fn the_same_document_writes_the_same_bytes() {
    let (d, _) = document();
    let a = export(d.assembly(), "W", LengthUnit::Inches, None).unwrap();
    let b = export(d.assembly(), "W", LengthUnit::Inches, None).unwrap();
    assert_eq!(a.text, b.text);
    assert!(a.text.contains("'INCH'"), "inches declared");
}

#[test]
fn a_hidden_group_is_left_out() {
    let (mut d, wall) = document();
    d.run(Command::SetHidden {
        instances: [wall].into(),
        hidden: true,
    });
    let e = export(d.assembly(), "W", LengthUnit::Millimetres, None).unwrap();
    assert_eq!(e.solids, 3);
    assert!(!e.text.contains("'Wall'"));
}

#[test]
fn only_the_selected_faces_go_out() {
    let mut d = Document::new();
    let a = box_at(&mut d, 0.0, 0.0, 1.0);
    box_at(&mut d, 3.0, 0.0, 1.0);
    let e = export(d.assembly(), "W", LengthUnit::Millimetres, Some(&a.faces)).unwrap();
    assert_eq!(e.solids, 1);
}

#[test]
fn approximate_and_invalid_bodies_are_left_out_and_counted() {
    let mut d = Document::new();
    box_at(&mut d, 0.0, 0.0, 1.0);
    let b = box_at(&mut d, 3.0, 0.0, 1.0);
    box_at(&mut d, 6.0, 0.0, 1.0);
    let mut m = d.model().clone();
    let e = *b.edges.iter().next().unwrap();
    m.set_approx(e, true);
    // The third box's top turned inward: closed but not a valid solid.
    let top = prism_ops::face_at(&m, DVec3::new(6.5, 0.5, 1.0)).unwrap();
    prism_ops::health::reverse(&mut m, &[top].into());
    let e = export(&Assembly::with_model(m), "W", LengthUnit::Millimetres, None).unwrap();
    assert_eq!(e.solids, 1);
    assert_eq!(e.left_out.len(), 2);
    assert!(e.left_out.contains(&INVALID));
    assert!(
        e.summary().starts_with("1 solid. Left out 2: "),
        "{}",
        e.summary()
    );
}

#[test]
fn an_open_box_goes_out_as_a_sheet() {
    let mut d = Document::new();
    box_at(&mut d, 0.0, 0.0, 1.0);
    let mut m = d.model().clone();
    let top = prism_ops::face_at(&m, DVec3::new(0.5, 0.5, 1.0)).unwrap();
    m.remove_face(top);
    let e = export(&Assembly::with_model(m), "W", LengthUnit::Millimetres, None).unwrap();
    assert_eq!((e.solids, e.sheets), (0, 1));
    assert_eq!(e.summary(), "1 sheet");
}

#[test]
fn nothing_to_export_is_refused() {
    let d = Document::new();
    assert_eq!(
        export(d.assembly(), "W", LengthUnit::Millimetres, None).err(),
        Some(prism_kernel::step::NOTHING)
    );
}

/// The golden STEP file of a small part (a block with a pocket and a round
/// hole); PRISM_WRITE_GOLDEN=1 writes it afresh.
#[test]
fn the_golden_part_writes_the_same_bytes_and_reads_back() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/acceptance/files/part.step");
    let mut d = prism_tools::scenes::pocket();
    // A hole through the pocket's floor (z 0.5) to the bottom.
    d.run(Command::Circle {
        centre: DVec3::new(2.0, 2.0, 0.5),
        normal: DVec3::Z,
        radius: 0.4,
    });
    let disc = prism_ops::face_at(d.model(), DVec3::new(2.0, 2.0, 0.5)).unwrap();
    let up = d.model().face(disc).unwrap().plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face: disc,
        distance: if up { -0.5 } else { 0.5 },
        new_loop: false,
    });
    let want = 32.0 - 2.4 * 2.4 * 1.5 - std::f64::consts::PI * 0.16 * 0.5;
    let e = export(d.assembly(), "Part", LengthUnit::Millimetres, None).unwrap();
    if std::env::var_os("PRISM_WRITE_GOLDEN").is_some() {
        std::fs::write(&path, &e.text).unwrap();
    }
    let golden = std::fs::read_to_string(&path).expect("the golden STEP file");
    assert_eq!(e.text, golden, "the writer's output changed");
    assert_eq!(e.summary(), "1 solid");
    assert!(golden.contains("CYLINDRICAL_SURFACE"), "the hole is exact");
    let got = volume_read(&golden);
    assert!((got - want).abs() < 1e-9, "{got} vs {want}");
}
