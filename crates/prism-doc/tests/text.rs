//! Text (Annotation & Sections part 3): AddText makes a named group of
//! geometry set in a bundled font, placed by its axes, in one undo step.

use prism_doc::{Command, Document};
use prism_math::{DAffine3, DVec3};
use prism_ops::text::TextMode;
use prism_text::Font;

fn add(d: &mut Document, text: &str, mode: TextMode, at: DAffine3) -> bool {
    d.run(Command::AddText {
        text: text.into(),
        font: Font::Sans,
        height: 1.0,
        mode,
        at,
    })
}

#[test]
fn text_becomes_a_named_group_placed_by_its_axes() {
    let mut d = Document::new();
    // Standing up, facing -y, starting at x = 5.
    let at = DAffine3::from_cols(DVec3::X, DVec3::Z, -DVec3::Y, DVec3::new(5.0, 0.0, 0.0));
    assert!(add(&mut d, "Hi there", TextMode::Solid { depth: 0.5 }, at));
    let root = d.assembly().root();
    assert_eq!(root.instances.len(), 1);
    let (id, inst) = root.instances.iter().next().unwrap();
    assert_eq!(d.assembly().name_of(*id), Some("Text: Hi there"));
    assert_eq!(inst.transform, at);
    assert_eq!(d.model().face_count(), 0, "nothing loose in the model");
    let (lo, hi) = d.assembly().flattened().bbox().expect("geometry");
    assert!(
        lo.x >= 5.0 - 1e-9 && hi.x > 6.5,
        "runs along x from 5: {lo} {hi}"
    );
    assert!(
        (hi.z - 1.0).abs() < 0.3,
        "capitals one tall, upwards: {}",
        hi.z
    );
    assert!(
        hi.y.abs() < 1e-9 && (lo.y + 0.5).abs() < 1e-9,
        "pulled out towards -y: {lo} {hi}"
    );
    // One undo step.
    assert!(d.undo());
    assert!(d.assembly().root().instances.is_empty());
    assert!(d.redo());
    assert_eq!(d.assembly().root().instances.len(), 1);
}

#[test]
fn long_names_are_cut_and_nothing_to_draw_adds_nothing() {
    let mut d = Document::new();
    assert!(!add(&mut d, "", TextMode::Face, DAffine3::IDENTITY));
    assert!(
        !add(&mut d, "   \n  ", TextMode::Face, DAffine3::IDENTITY),
        "spaces draw nothing"
    );
    assert!(d.assembly().root().instances.is_empty());
    assert!(add(
        &mut d,
        "A very long label that goes on and on\nsecond line",
        TextMode::Face,
        DAffine3::IDENTITY
    ));
    let id = *d.assembly().root().instances.keys().next().unwrap();
    assert_eq!(
        d.assembly().name_of(id),
        Some("Text: A very long label that g")
    );
}

#[test]
fn text_inside_an_open_group_is_made_there() {
    let mut d = Document::new();
    d.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 2.0,
        y1: 2.0,
        z: 0.0,
    });
    let all = d.model().faces().map(|(i, _)| i).collect();
    let g = d.assembly().next_instance();
    d.run(Command::MakeGroup {
        selection: prism_ops::Selection {
            faces: all,
            ..Default::default()
        },
        instances: Default::default(),
    });
    let c = Command::AddText {
        text: "In".into(),
        font: Font::Mono,
        height: 0.5,
        mode: TextMode::Face,
        at: DAffine3::IDENTITY,
    };
    assert!(d.run_in(&[g], c));
    assert!(
        d.assembly().root().instances.len() == 1,
        "the new group is inside the open one"
    );
    let (_, inner) = d.assembly().find_instance(g).expect("the group");
    assert_eq!(d.assembly().def(inner.def).unwrap().instances.len(), 1);
}

#[test]
fn an_even_scale_grows_the_text() {
    use prism_doc::Command as C;
    let m = prism_topo::Model::new();
    let c = C::AddText {
        text: "A".into(),
        font: Font::Serif,
        height: 1.0,
        mode: TextMode::Solid { depth: 0.2 },
        at: DAffine3::IDENTITY,
    };
    let to_local = DAffine3::from_scale(DVec3::splat(0.5));
    let C::AddText { height, mode, .. } = c.to_local(&to_local, &m) else {
        panic!("still text");
    };
    assert!((height - 0.5).abs() < 1e-12);
    assert!(matches!(mode, TextMode::Solid { depth } if (depth - 0.1).abs() < 1e-12));
}
