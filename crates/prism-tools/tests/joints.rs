//! The Hinge and Slide tools (Fit & Mechanisms part 1): clicking two
//! parts then an axis makes a joint, the way a person would.

use std::collections::BTreeSet;

use prism_doc::{Command, Document, InstanceId, JointKind};
use prism_math::{DVec2, DVec3};
use prism_ops::Selection;
use prism_tools::session::{HINGE_TOOL, SLIDE_TOOL};
use prism_tools::{Button, Event, Input, Session};
use prism_view::Camera;

fn everything(m: &prism_topo::Model) -> Selection {
    Selection {
        edges: m.edges().map(|(id, _)| id).collect(),
        faces: m.faces().map(|(id, _)| id).collect(),
        ..Default::default()
    }
}

/// A box at `at`, 1x1x1, made into its own group. Returns its instance id.
fn boxed_group(d: &mut Document, at: DVec3) -> InstanceId {
    d.run(Command::Rect {
        x0: at.x,
        y0: at.y,
        x1: at.x + 1.0,
        y1: at.y + 1.0,
        z: at.z,
    });
    let f = prism_ops::face_at(d.model(), at + DVec3::new(0.5, 0.5, 0.0)).expect("base");
    let up = d.model().face(f).expect("face").plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face: f,
        distance: if up { 1.0 } else { -1.0 },
        new_loop: false,
    });
    let id = d.assembly().next_instance();
    assert!(d.run(Command::MakeGroup {
        selection: everything(d.model()),
        instances: BTreeSet::new(),
    }));
    id
}

/// Two boxes side by side, each its own group, and a session looking at
/// both from an angle that sees their tops and sides.
fn two_boxes() -> (Session, InstanceId, InstanceId) {
    let mut d = Document::new();
    let a = boxed_group(&mut d, DVec3::new(0.0, 0.0, 0.0));
    let b = boxed_group(&mut d, DVec3::new(2.0, 0.0, 0.0));
    let mut s = Session::new(Document::new());
    s.open_document(
        d,
        Some(Camera {
            eye: DVec3::new(1.5, -8.0, 6.0),
            target: DVec3::new(1.5, 0.5, 0.5),
        }),
    );
    (s, a, b)
}

struct Driver {
    s: Session,
    t: f64,
}

impl Driver {
    fn new(s: Session) -> Self {
        let mut d = Driver { s, t: 0.0 };
        d.send(Input::Resize {
            w: 1000.0,
            h: 700.0,
        });
        d
    }

    fn send(&mut self, input: Input) {
        self.t += 0.05;
        self.s.handle(&Event { t: self.t, input });
    }

    fn screen(&self, p: DVec3) -> DVec2 {
        self.s.view().to_screen(p).expect("in front").0
    }

    fn click(&mut self, p: DVec3) {
        let s = self.screen(p);
        self.send(Input::Move { x: s.x, y: s.y });
        self.send(Input::Press {
            button: Button::Left,
            x: s.x,
            y: s.y,
            shift: false,
            ctrl: false,
            alt: false,
        });
        self.send(Input::Release {
            button: Button::Left,
        });
        self.t += 0.5;
    }

    fn tool(&mut self, id: &str) {
        self.send(Input::Tool { id: id.into() });
    }
}

/// The centre of the top face of the box made at `at` (1x1x1).
fn top_of(at: DVec3) -> DVec3 {
    at + DVec3::new(0.5, 0.5, 1.0)
}

#[test]
fn clicking_two_parts_then_a_face_makes_a_hinge() {
    let (s, a, b) = two_boxes();
    let mut d = Driver::new(s);
    d.tool(HINGE_TOOL);
    d.click(DVec3::new(0.5, 0.5, 0.5)); // a face of a
    d.click(DVec3::new(2.5, 0.5, 0.5)); // a face of b
    d.click(top_of(DVec3::new(0.0, 0.0, 0.0))); // a's top face normal

    let (joints, _) = d.s.document().assembly().joints();
    assert_eq!(joints.len(), 1, "{joints:?}");
    let j = joints.values().next().expect("one");
    assert_eq!(j.kind, JointKind::Hinge);
    assert_eq!(BTreeSet::from([j.a, j.b]), BTreeSet::from([a, b]));
    assert!((j.axis_dir.z.abs() - 1.0).abs() < 1e-6, "{:?}", j.axis_dir);
    assert!(j.min.is_none() && j.max.is_none() && !j.stopped);
}

#[test]
fn clicking_two_parts_then_a_face_makes_a_slide() {
    let (s, ..) = two_boxes();
    let mut d = Driver::new(s);
    d.tool(SLIDE_TOOL);
    d.click(DVec3::new(0.5, 0.5, 0.5));
    d.click(DVec3::new(2.5, 0.5, 0.5));
    d.click(top_of(DVec3::new(0.0, 0.0, 0.0)));

    let (joints, _) = d.s.document().assembly().joints();
    assert_eq!(joints.len(), 1);
    assert_eq!(joints.values().next().unwrap().kind, JointKind::Slide);
}

#[test]
fn escape_starts_the_pick_over() {
    let (s, a, b) = two_boxes();
    let mut d = Driver::new(s);
    d.tool(HINGE_TOOL);
    d.click(DVec3::new(0.5, 0.5, 0.5));
    d.send(Input::Key {
        key: prism_tools::Key::Escape,
    });
    // Starting over: click both parts and the axis again works cleanly.
    d.click(DVec3::new(0.5, 0.5, 0.5));
    d.click(DVec3::new(2.5, 0.5, 0.5));
    d.click(top_of(DVec3::new(0.0, 0.0, 0.0)));
    let (joints, _) = d.s.document().assembly().joints();
    assert_eq!(joints.len(), 1);
    let j = joints.values().next().unwrap();
    assert_eq!(BTreeSet::from([j.a, j.b]), BTreeSet::from([a, b]));
}

#[test]
fn clicking_the_same_part_twice_does_not_make_a_joint() {
    let (s, ..) = two_boxes();
    let mut d = Driver::new(s);
    d.tool(HINGE_TOOL);
    d.click(DVec3::new(0.5, 0.5, 0.5));
    d.click(DVec3::new(0.5, 0.5, 0.5));
    d.click(top_of(DVec3::new(0.0, 0.0, 0.0)));
    assert!(d.s.document().assembly().joints().0.is_empty());
}
