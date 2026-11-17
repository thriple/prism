//! DXF and SVG export (Solids & CAD part 6a, brief 45): flat parts and
//! views, with arcs as arcs and splines as splines.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_doc::{Command, Document};
use prism_file::drawing::{CURVED, Format, NOTHING, Scope, View, export};
use prism_geom::Nurbs;
use prism_math::{DVec3, LengthUnit};

fn everything() -> Scope<'static> {
    static NONE_F: BTreeSet<prism_topo::FaceId> = BTreeSet::new();
    static NONE_E: BTreeSet<prism_topo::EdgeId> = BTreeSet::new();
    static NONE_G: BTreeSet<prism_doc::InstanceId> = BTreeSet::new();
    Scope {
        path: &[],
        faces: &NONE_F,
        edges: &NONE_E,
        groups: &NONE_G,
    }
}

/// Pull the face at `p` by `by` along +z (its own way round, whichever
/// way it faces).
fn pull(d: &mut Document, p: DVec3, by: f64) {
    let face = prism_ops::face_at(d.model(), p).unwrap();
    let up = d.model().face(face).unwrap().plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face,
        distance: if up { by } else { -by },
        new_loop: false,
    });
}

/// A plate 100 by 60 mm and 6 thick, its back edge a spline, with a round
/// hole (10 mm across) and a slot (from x 60 to 80 mm, 10 wide).
fn plate() -> Document {
    let mut d = Document::new();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    d.run(Command::Segment {
        p: p(0.0, 0.0),
        q: p(0.1, 0.0),
    });
    d.run(Command::Segment {
        p: p(0.1, 0.0),
        q: p(0.1, 0.06),
    });
    d.run(Command::Spline {
        curve: Nurbs::open(
            &[p(0.1, 0.06), p(0.07, 0.075), p(0.03, 0.075), p(0.0, 0.06)],
            3,
        )
        .unwrap(),
    });
    d.run(Command::Segment {
        p: p(0.0, 0.06),
        q: p(0.0, 0.0),
    });
    pull(&mut d, p(0.05, 0.03), 0.006);
    let t = 0.006;
    d.run(Command::Circle {
        centre: DVec3::new(0.03, 0.03, t),
        normal: DVec3::Z,
        radius: 0.005,
    });
    let q = |x: f64, y: f64| DVec3::new(x, y, t);
    d.run(Command::Segment {
        p: q(0.06, 0.025),
        q: q(0.08, 0.025),
    });
    d.run(Command::Arc {
        centre: q(0.08, 0.03),
        normal: DVec3::Z,
        radius: 0.005,
        start: q(0.08, 0.025),
        sweep: PI,
        pie: false,
    });
    d.run(Command::Segment {
        p: q(0.08, 0.035),
        q: q(0.06, 0.035),
    });
    d.run(Command::Arc {
        centre: q(0.06, 0.03),
        normal: DVec3::Z,
        radius: 0.005,
        start: q(0.06, 0.035),
        sweep: PI,
        pie: false,
    });
    pull(&mut d, q(0.03, 0.03), -t);
    pull(&mut d, q(0.07, 0.03), -t);
    assert_eq!(prism_ops::check::problems(d.model()), Vec::<String>::new());
    d
}

fn count(text: &str, entity: &str) -> usize {
    text.matches(&format!("\n  0\n{entity}\n")).count()
}

#[test]
fn a_plate_laid_flat_writes_true_arcs_a_circle_and_a_spline() {
    let d = plate();
    let e = export(
        d.assembly(),
        everything(),
        View::Flat,
        LengthUnit::Millimetres,
        Format::Dxf,
    )
    .unwrap();
    assert_eq!(e.summary(), "3 paths in 1 part");
    let t = &e.text;
    assert!(t.contains("$INSUNITS\n 70\n4\n"), "in mm");
    assert_eq!(count(t, "CIRCLE"), 1, "the hole");
    assert_eq!(count(t, "LWPOLYLINE"), 1, "the slot");
    assert_eq!(count(t, "SPLINE"), 1, "the back edge");
    assert_eq!(count(t, "LINE"), 3, "the other edges");
    // The slot's two ends are half circles: bulges of 1 (or -1).
    assert_eq!(
        t.matches("\n 42\n1\n").count() + t.matches("\n 42\n-1\n").count(),
        2
    );
    assert!(t.contains("\n 40\n5\n"), "the hole's radius, 5 mm");
}

#[test]
fn a_plate_as_svg_has_paths_with_arcs_at_its_true_size() {
    let d = plate();
    let mm = export(
        d.assembly(),
        everything(),
        View::Flat,
        LengthUnit::Millimetres,
        Format::Svg,
    )
    .unwrap();
    assert!(mm.text.contains(r#"width="100mm""#), "{}", mm.text);
    assert_eq!(mm.text.matches("<path ").count(), 3);
    assert!(mm.text.contains("A5 5 0 0 "), "arcs of 5 mm");
    assert!(mm.text.contains('C'), "the spline as a cubic curve");
    assert!(mm.close.is_empty(), "a cubic spline is exact");
    let inches = export(
        d.assembly(),
        everything(),
        View::Flat,
        LengthUnit::Inches,
        Format::Svg,
    )
    .unwrap();
    assert!(
        inches.text.contains(r#"width="3.937008in""#),
        "{}",
        inches.text
    );
}

#[test]
fn a_spline_seen_from_above_keeps_its_control_points() {
    let mut d = Document::new();
    let pts = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(0.01, 0.02, 0.005),
        DVec3::new(0.03, 0.02, 0.01),
        DVec3::new(0.04, 0.0, 0.0),
    ];
    d.run(Command::Spline {
        curve: Nurbs::open(&pts, 3).unwrap(),
    });
    let e = export(
        d.assembly(),
        everything(),
        View::Top,
        LengthUnit::Millimetres,
        Format::Dxf,
    )
    .unwrap();
    assert_eq!(count(&e.text, "SPLINE"), 1);
    for p in pts {
        let at = format!("\n 10\n{}\n 20\n{}\n 30\n0\n", p.x * 1000.0, p.y * 1000.0);
        assert!(e.text.contains(&at), "{at} in {}", e.text);
    }
}

#[test]
fn a_circle_seen_at_a_slant_is_an_ellipse() {
    let mut d = Document::new();
    d.run(Command::Circle {
        centre: DVec3::ZERO,
        normal: DVec3::new(0.0, 1.0, 1.0).normalize(),
        radius: 0.01,
    });
    let e = export(
        d.assembly(),
        everything(),
        View::Top,
        LengthUnit::Millimetres,
        Format::Dxf,
    )
    .unwrap();
    assert_eq!(count(&e.text, "ELLIPSE"), 2, "two halves: {}", e.text);
    assert!(
        e.text.contains("\n 40\n0.707107\n"),
        "its minor axis over its major"
    );
    // Seen straight on from the front, it is a circle no longer either;
    // from above, seen square on, a circle drawn flat stays one.
    let mut flat = Document::new();
    flat.run(Command::Circle {
        centre: DVec3::ZERO,
        normal: DVec3::Z,
        radius: 0.01,
    });
    let e = export(
        flat.assembly(),
        everything(),
        View::Top,
        LengthUnit::Millimetres,
        Format::Dxf,
    )
    .unwrap();
    assert_eq!(count(&e.text, "CIRCLE"), 1);
}

#[test]
fn the_same_model_writes_the_same_bytes() {
    let d = plate();
    for f in [Format::Dxf, Format::Svg] {
        let a = export(
            d.assembly(),
            everything(),
            View::Flat,
            LengthUnit::Millimetres,
            f,
        );
        let b = export(
            d.assembly(),
            everything(),
            View::Flat,
            LengthUnit::Millimetres,
            f,
        );
        assert_eq!(a.unwrap().text, b.unwrap().text);
    }
}

#[test]
fn parts_are_laid_in_a_row_and_hidden_ones_are_left_out() {
    let mut d = Document::new();
    let mut groups = Vec::new();
    for x in [0.0, 1.0] {
        let before: BTreeSet<_> = d.model().faces().map(|(f, _)| f).collect();
        d.run(Command::Rect {
            x0: x,
            y0: 0.0,
            x1: x + 0.2,
            y1: 0.1,
            z: 0.0,
        });
        pull(&mut d, DVec3::new(x + 0.1, 0.05, 0.0), 0.01);
        let m = d.model();
        let sel = prism_ops::Selection {
            faces: m
                .faces()
                .map(|(f, _)| f)
                .filter(|f| !before.contains(f))
                .collect(),
            edges: m.edges().map(|(e, _)| e).collect(),
            vertices: m.vertices().map(|(v, _)| v).collect(),
            guides: Default::default(),
        };
        groups.push(d.assembly().next_instance());
        d.run(Command::MakeGroup {
            selection: sel,
            instances: Default::default(),
        });
    }
    let svg = |d: &Document| {
        export(
            d.assembly(),
            everything(),
            View::Flat,
            LengthUnit::Millimetres,
            Format::Svg,
        )
        .unwrap()
    };
    let both = svg(&d);
    assert_eq!(
        both.paths, 2,
        "one part each, in one layer (both are \"Group\")"
    );
    // 200 mm, a 10 mm gap, 200 mm.
    assert!(both.text.contains(r#"width="410mm""#), "{}", both.text);
    d.run(Command::SetHidden {
        instances: [groups[1]].into(),
        hidden: true,
    });
    let one = svg(&d);
    assert_eq!(one.paths, 1);
    assert!(one.text.contains(r#"width="200mm""#));
}

#[test]
fn a_chosen_curved_face_is_counted_and_nothing_is_refused() {
    let mut d = Document::new();
    d.run(Command::Circle {
        centre: DVec3::ZERO,
        normal: DVec3::Z,
        radius: 0.01,
    });
    pull(&mut d, DVec3::ZERO, 0.02);
    let curved: BTreeSet<_> = d
        .model()
        .faces()
        .filter(|(_, f)| f.plane().is_none())
        .map(|(id, _)| id)
        .collect();
    let none_e = BTreeSet::new();
    let none_g = BTreeSet::new();
    let scope = Scope {
        path: &[],
        faces: &curved,
        edges: &none_e,
        groups: &none_g,
    };
    let r = export(
        d.assembly(),
        scope,
        View::Flat,
        LengthUnit::Millimetres,
        Format::Dxf,
    );
    assert_eq!(r.unwrap_err(), NOTHING);
    let mut both = curved.clone();
    let top = prism_ops::face_at(d.model(), DVec3::new(0.0, 0.0, 0.02)).unwrap();
    both.insert(top);
    let scope = Scope {
        faces: &both,
        ..scope
    };
    let e = export(
        d.assembly(),
        scope,
        View::Flat,
        LengthUnit::Millimetres,
        Format::Dxf,
    )
    .unwrap();
    assert_eq!(e.left_out, vec![CURVED; curved.len()]);
    assert_eq!(count(&e.text, "CIRCLE"), 1);
}

/// The golden DXF and SVG files of the plate; PRISM_WRITE_GOLDEN=1 writes
/// them afresh.
#[test]
fn the_golden_plate_writes_the_same_bytes() {
    let d = plate();
    for f in [Format::Dxf, Format::Svg] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../docs/acceptance/files/plate.{}",
            f.extension()
        ));
        let e = export(
            d.assembly(),
            everything(),
            View::Flat,
            LengthUnit::Millimetres,
            f,
        )
        .unwrap();
        if std::env::var_os("PRISM_WRITE_GOLDEN").is_some() {
            std::fs::write(&path, &e.text).unwrap();
        }
        let golden = std::fs::read_to_string(&path).expect("the golden file");
        assert_eq!(e.text, golden, "the {f:?} writer's output changed");
    }
}
