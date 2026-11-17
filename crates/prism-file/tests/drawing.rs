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

// ---------- Import (part 6b) ----------

use prism_file::drawing::{Read, import, read};

/// The group's model in an imported assembly.
fn group_model(asm: &prism_doc::Assembly) -> &prism_topo::Model {
    let placed = asm.placed();
    assert_eq!(placed.len(), 1, "one group");
    &asm.def(placed[0].def).unwrap().model
}

fn areas(m: &prism_topo::Model) -> Vec<f64> {
    let mut a: Vec<f64> = m.faces().map(|(f, _)| m.face_area(f)).collect();
    a.sort_by(f64::total_cmp);
    a
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1e-12)
}

#[test]
fn the_golden_plate_comes_back_as_the_same_faces() {
    let d = plate();
    let top = prism_ops::face_at(d.model(), DVec3::new(0.05, 0.01, 0.006)).unwrap();
    let r = 0.005;
    let mut want = vec![
        d.model().face_area(top),
        PI * r * r,
        0.02 * 0.01 + PI * r * r,
    ];
    want.sort_by(f64::total_cmp);
    for f in [Format::Dxf, Format::Svg] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../docs/acceptance/files/plate.{}",
            f.extension()
        ));
        let r = read(&std::fs::read(path).unwrap(), f).unwrap();
        let stated = r.stated.expect("the file's unit");
        assert!(near(stated, 1e-3), "mm: {stated}");
        let i = import(&r, stated, "plate").unwrap();
        let m = group_model(&i.assembly);
        assert_eq!(prism_ops::check::problems(m), Vec::<String>::new());
        let got = areas(m);
        assert_eq!(
            got.len(),
            3,
            "{f:?}: the plate, the hole's disc and the slot"
        );
        for (g, w) in got.iter().zip(&want) {
            assert!((g - w).abs() < 1e-9, "{f:?}: {got:?} vs {want:?}");
        }
        assert!(
            m.edges().any(|(_, e)| e.curve.spline().is_some()),
            "the spline edge"
        );
        assert_eq!(
            i.assembly.name_of(i.assembly.placed()[0].path[0]),
            Some("plate")
        );
    }
}

/// A DXF by hand, in inches: a square block placed twice (once turned and
/// doubled), a polyline with a half-circle bulge, a spline, and a text.
const HAND: &str = "  0\nSECTION\n  2\nHEADER\n  9\n$INSUNITS\n 70\n1\n  0\nENDSEC\n  0\nSECTION\n  2\nBLOCKS\n  0\nBLOCK\n  2\nSQ\n 10\n0\n 20\n0\n  0\nLWPOLYLINE\n 90\n4\n 70\n1\n 10\n0\n 20\n0\n 10\n1\n 20\n0\n 10\n1\n 20\n1\n 10\n0\n 20\n1\n  0\nENDBLK\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nINSERT\n  2\nSQ\n 10\n10\n 20\n0\n  0\nINSERT\n  2\nSQ\n 10\n20\n 20\n0\n 41\n2\n 42\n2\n 50\n90\n  0\nLWPOLYLINE\n 90\n3\n 70\n1\n 10\n0\n 20\n10\n 42\n0\n 10\n4\n 20\n10\n 42\n1\n 10\n4\n 20\n14\n  0\nSPLINE\n 71\n2\n 72\n6\n 73\n3\n 40\n0\n 40\n0\n 40\n0\n 40\n1\n 40\n1\n 40\n1\n 10\n0\n 20\n20\n 30\n0\n 10\n2\n 20\n24\n 30\n0\n 10\n4\n 20\n20\n 30\n0\n  0\nTEXT\n  1\nHello\n 10\n0\n 20\n0\n  0\nENDSEC\n  0\nEOF\n";

#[test]
fn a_dxf_by_hand_in_inches_imports_its_blocks_bulges_and_spline() {
    let r = read(HAND.as_bytes(), Format::Dxf).unwrap();
    assert_eq!(r.stated, Some(0.0254));
    assert_eq!(r.skipped, vec!["text".to_owned()]);
    let i = import(&r, 0.0254, "hand").unwrap();
    assert!(
        i.summary()
            .ends_with("Left out 1: text (1) (not imported yet)"),
        "{}",
        i.summary()
    );
    let m = group_model(&i.assembly);
    let a = areas(m);
    let inch2 = 0.0254 * 0.0254;
    // The bulged polyline: a triangle (0,10) (4,10) (4,14) with a half
    // circle on its (4,10)-(4,14) side, out to the right.
    let tri = 8.0 + PI * 2.0 * 2.0 / 2.0;
    let want = [1.0, 4.0, tri].map(|x| x * inch2);
    assert_eq!(a.len(), 3, "{a:?}");
    for (g, w) in a.iter().zip(&want) {
        assert!((g - w).abs() < 1e-9, "{a:?} vs {want:?}");
    }
    // The second square turned a quarter: from x 18 to 20 inches.
    let xs: Vec<f64> = m.vertices().map(|(_, p)| p.x / 0.0254).collect();
    assert!(xs.iter().any(|x| (x - 18.0).abs() < 1e-9), "{xs:?}");
    // The spline stays a spline.
    assert!(m.edges().any(|(_, e)| e.curve.spline().is_some()));
}

#[test]
fn an_svg_with_transforms_and_every_command_imports_the_right_way_up() {
    let svg = r#"<?xml version="1.0"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm" viewBox="0 0 200 100">
  <g transform="translate(10,10)">
    <g transform="scale(2)">
      <path d="M0 0 H10 V10 h-10 z"/>
    </g>
    <path d="m40 0 l10 0 c5 0 5 10 0 10 s-5 -10 -10 -10 q0 0 0 0 t0 0 Z"/>
    <path d="M70 5 a5 5 0 1 0 10 0 A5 5 0 1 0 70 5"/>
  </g>
  <circle cx="150" cy="50" r="10"/>
  <text x="0" y="0">Hi</text>
  <rect x="0" y="80" width="20" height="10" rx="2"/>
</svg>"#;
    let r = read(svg.as_bytes(), Format::Svg).unwrap();
    // 100 mm across 200 units.
    assert!(near(r.stated.unwrap(), 0.0005));
    assert_eq!(r.skipped, vec!["text".to_owned()]);
    let i = import(&r, r.stated.unwrap(), "page").unwrap();
    let m = group_model(&i.assembly);
    let world = i.assembly.placed()[0].world;
    let k2 = 0.0005 * 0.0005;
    let a = areas(m);
    // The scaled square (20 by 20 units); the circle of radius 5 drawn by
    // two arcs; the circle of radius 10; the rounded rectangle.
    let has = |x: f64| {
        a.iter()
            .any(|g| (g - x * k2).abs() < 1e-9 * x * k2.max(1.0))
    };
    assert!(has(400.0), "{a:?}");
    assert!(has(PI * 25.0), "{a:?}");
    assert!(has(PI * 100.0), "{a:?}");
    assert!(has(200.0 - (4.0 - PI) * 4.0), "{a:?}");
    // y up: the circle (at y 50 down the page) is above the rectangle (at
    // y 80 to 90 down the page).
    let circle_y = m
        .faces()
        .find(|(f, _)| (m.face_area(*f) - PI * 100.0 * k2).abs() < 1e-12)
        .map(|(_, f)| world.transform_point3(m.vertex(f.outer[0])).y)
        .unwrap();
    // Its vertices are its top and bottom, at y 40 and 60 down the page.
    assert!(
        near(circle_y, -40.0 * 0.0005) || near(circle_y, -60.0 * 0.0005),
        "{circle_y}"
    );
    let lowest = m
        .vertices()
        .map(|(_, p)| world.transform_point3(p).y)
        .fold(f64::MAX, f64::min);
    assert!(
        near(lowest, -90.0 * 0.0005),
        "the rectangle's bottom: {lowest}"
    );
}

#[test]
fn text_that_is_neither_is_refused() {
    assert!(read(b"hello", Format::Dxf).is_err());
    assert!(read(b"<html></html>", Format::Svg).is_err());
    let empty = Read {
        paths: Vec::new(),
        points: Vec::new(),
        stated: None,
        skipped: Vec::new(),
    };
    assert!(import(&empty, 1.0, "x").is_err());
}

#[test]
fn an_ellipse_goes_out_and_comes_back_with_its_area() {
    let mut d = Document::new();
    let n = DVec3::new(0.3, 0.5, 0.8).normalize();
    d.run(Command::Circle {
        centre: DVec3::new(0.02, 0.01, 0.0),
        normal: n,
        radius: 0.01,
    });
    // Seen from above: an ellipse of half-axes r and r n.z.
    let want = PI * 0.01 * 0.01 * n.z;
    for f in [Format::Dxf, Format::Svg] {
        let e = export(
            d.assembly(),
            everything(),
            View::Top,
            LengthUnit::Millimetres,
            f,
        )
        .unwrap();
        let r = read(e.text.as_bytes(), f).unwrap();
        let i = import(&r, r.stated.unwrap(), "e").unwrap();
        let a = areas(group_model(&i.assembly));
        assert_eq!(a.len(), 1, "{f:?}");
        assert!(
            (a[0] - want).abs() < 1e-6 * want,
            "{f:?}: {} vs {want}",
            a[0]
        );
    }
}
