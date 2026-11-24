//! The Hinge and Slide tools (Fit & Mechanisms part 1): clicking two
//! parts then an axis makes a joint, the way a person would.

use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_6;

use prism_doc::{Command, Document, InstanceId, JointKind};
use prism_math::{DVec2, DVec3};
use prism_ops::Selection;
use prism_tools::session::{HINGE_TOOL, MOVE, ROTATE, SELECT, SLIDE_TOOL};
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

/// Two boxes side by side, each its own group.
fn two_boxes_doc() -> (Document, InstanceId, InstanceId) {
    let mut d = Document::new();
    let a = boxed_group(&mut d, DVec3::new(0.0, 0.0, 0.0));
    let b = boxed_group(&mut d, DVec3::new(2.0, 0.0, 0.0));
    (d, a, b)
}

/// A session looking at `d` from an angle that sees both boxes' tops and
/// sides.
fn session_on(d: Document) -> Session {
    let mut s = Session::new(Document::new());
    s.open_document(
        d,
        Some(Camera {
            eye: DVec3::new(1.5, -8.0, 6.0),
            target: DVec3::new(1.5, 0.5, 0.5),
        }),
    );
    s
}

/// Two boxes side by side, each its own group, and a session looking at
/// both from an angle that sees their tops and sides.
fn two_boxes() -> (Session, InstanceId, InstanceId) {
    let (d, a, b) = two_boxes_doc();
    (session_on(d), a, b)
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

    fn hover(&mut self, p: DVec3) {
        let s = self.screen(p);
        self.send(Input::Move { x: s.x, y: s.y });
    }

    fn typed(&mut self, text: &str) {
        self.send(Input::Text { text: text.into() });
        self.send(Input::Key {
            key: prism_tools::Key::Enter,
        });
    }

    /// Alt + click, the way picking up a joint's handle or selecting its
    /// gizmo works.
    fn alt_click(&mut self, p: DVec3) {
        let s = self.screen(p);
        self.send(Input::Move { x: s.x, y: s.y });
        self.send(Input::Press {
            button: Button::Left,
            x: s.x,
            y: s.y,
            shift: false,
            ctrl: false,
            alt: true,
        });
        self.send(Input::Release {
            button: Button::Left,
        });
        self.t += 0.5;
    }

    fn action(&mut self, id: &str) {
        self.send(Input::Action { id: id.into() });
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

#[test]
fn rotating_a_hinged_part_turns_about_the_joints_axis_and_clamps_the_typed_angle() {
    let (mut doc, a, b) = two_boxes_doc();
    doc.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b,
        origin: DVec3::new(0.0, 0.0, 0.0),
        dir: DVec3::new(0.0, 0.0, 1.0),
    });
    let id = *doc.assembly().joints().0.keys().next().expect("one");
    doc.run(Command::SetJointBound {
        id,
        max: true,
        value: Some(45f64.to_radians()),
    });

    let mut d = Driver::new(session_on(doc));
    d.tool(SELECT);
    d.click(DVec3::new(0.5, 0.5, 0.5));
    d.tool(ROTATE);
    // The first two clicks would normally set the centre and the rotation
    // plane from the hovered face; the joint overrides both to its own
    // axis, so where exactly these land does not matter beyond landing on
    // the selected part and setting a start arm.
    d.click(DVec3::new(0.5, 0.5, 0.5));
    d.click(DVec3::new(1.0, 0.0, 0.0));
    d.hover(DVec3::new(0.0, 1.0, 0.0)); // 90 degrees round from the start arm
    d.typed("90"); // past the 45 degree limit

    let (_, inst) =
        d.s.document()
            .assembly()
            .find_instance(a)
            .expect("still there");
    let moved = inst.transform.transform_point3(DVec3::new(1.0, 0.0, 0.0));
    let (s45, c45) = (45f64.to_radians().sin(), 45f64.to_radians().cos());
    assert!(
        (moved - DVec3::new(c45, s45, 0.0)).length() < 1e-6,
        "clamped to 45 degrees: {moved:?}"
    );
}

#[test]
fn rotating_the_other_part_of_the_same_hinge_turns_about_the_same_axis() {
    let (mut doc, a, b) = two_boxes_doc();
    doc.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b,
        origin: DVec3::new(2.0, 0.0, 0.0),
        dir: DVec3::new(0.0, 0.0, 1.0),
    });

    let mut d = Driver::new(session_on(doc));
    d.tool(SELECT);
    d.click(DVec3::new(2.5, 0.5, 0.5)); // part b this time
    d.tool(ROTATE);
    d.click(DVec3::new(2.5, 0.5, 0.5));
    d.click(DVec3::new(3.0, 0.0, 0.0));
    d.hover(DVec3::new(2.0, 1.0, 0.0));
    d.typed("90");

    let (_, inst) = d.s.document().assembly().find_instance(b).expect("there");
    // b's own local corner at its own low corner's +x neighbour, (1,0,0)
    // in its own coordinates (the group's corner is its own origin).
    let moved = inst.transform.transform_point3(DVec3::new(1.0, 0.0, 0.0));
    assert!(
        (moved - DVec3::new(2.0, 1.0, 0.0)).length() < 1e-6,
        "turned 90 degrees about the joint's axis through (2,0,0): {moved:?}"
    );
}

#[test]
fn sliding_a_slide_joints_part_stays_on_its_axis_and_clamps_the_typed_distance() {
    let (mut doc, a, b) = two_boxes_doc();
    doc.run(Command::AddJoint {
        kind: JointKind::Slide,
        a,
        b,
        origin: DVec3::new(0.0, 0.0, 0.0),
        dir: DVec3::new(1.0, 0.0, 0.0),
    });
    let id = *doc.assembly().joints().0.keys().next().expect("one");
    doc.run(Command::SetJointBound {
        id,
        max: true,
        value: Some(0.5),
    });

    let mut d = Driver::new(session_on(doc));
    d.tool(SELECT);
    d.click(DVec3::new(0.5, 0.5, 0.5));
    d.tool(MOVE);
    d.click(DVec3::new(0.5, 0.5, 0.5));
    d.hover(DVec3::new(1.5, 0.5, 0.5)); // a diagonal-ish drag, off the axis
    d.typed("2"); // past the 0.5 limit

    let (_, inst) = d.s.document().assembly().find_instance(a).expect("there");
    let moved = inst.transform.transform_point3(DVec3::ZERO);
    assert!(
        (moved - DVec3::new(0.5, 0.0, 0.0)).length() < 1e-6,
        "kept on the axis and clamped to 0.5: {moved:?}"
    );
}

/// A doc with one Slide joint between the two boxes, along x, with no
/// bounds set yet.
fn slide_doc() -> (Document, InstanceId, InstanceId, prism_doc::JointId) {
    let (mut doc, a, b) = two_boxes_doc();
    doc.run(Command::AddJoint {
        kind: JointKind::Slide,
        a,
        b,
        origin: DVec3::new(0.0, 0.0, 0.0),
        dir: DVec3::new(1.0, 0.0, 0.0),
    });
    let id = *doc.assembly().joints().0.keys().next().expect("one");
    (doc, a, b, id)
}

/// The gizmo's own reach for the two-box scene: a share of the model's
/// diagonal (both boxes span (0,0,0) to (3,1,1)), the same formula
/// `joint_reach` uses.
fn model_reach() -> f64 {
    (DVec3::new(3.0, 1.0, 1.0).length() * 0.2_f64).max(0.3)
}

/// Where a Hinge's handle sits at angle `v` round an axis along z, origin
/// at the world origin: `prism_infer::plane_frame((0,0,1))` gives
/// `u = (0,1,0)`, `w = (-1,0,0)`.
fn hinge_handle_point(v: f64, reach: f64) -> DVec3 {
    let u = DVec3::new(0.0, 1.0, 0.0);
    let w = DVec3::new(-1.0, 0.0, 0.0);
    (u * v.cos() + w * v.sin()) * reach
}

#[test]
fn alt_clicking_a_slide_handle_and_dragging_sets_its_limit() {
    let (doc, ..) = slide_doc();
    let mut d = Driver::new(session_on(doc));
    d.tool(SELECT);
    // The default (unset) max handle sits reach*0.4 out along the axis,
    // from the origin at (0,0,0): the handle starts life there so there is
    // something to grab.
    d.alt_click(DVec3::new(model_reach() * 0.4, 0.0, 0.0));
    d.hover(DVec3::new(0.6, 0.0, 0.0));
    d.click(DVec3::new(0.6, 0.0, 0.0)); // sets it down

    let (joints, _) = d.s.document().assembly().joints();
    let j = joints.values().next().expect("one");
    assert!(j.max.is_some_and(|v| (v - 0.6).abs() < 1e-3), "{:?}", j.max);
    assert!(j.min.is_none());
}

#[test]
fn typing_a_distance_while_a_slide_handle_is_picked_up_sets_it_exactly() {
    let (doc, ..) = slide_doc();
    let mut d = Driver::new(session_on(doc));
    d.tool(SELECT);
    d.alt_click(DVec3::new(model_reach() * 0.4, 0.0, 0.0));
    d.typed("0.75");

    let (joints, _) = d.s.document().assembly().joints();
    let j = joints.values().next().unwrap();
    assert!(
        j.max.is_some_and(|v| (v - 0.75).abs() < 1e-9),
        "{:?}",
        j.max
    );
}

#[test]
fn alt_clicking_a_hinge_handle_and_typing_an_angle_sets_its_limit() {
    let (mut doc, a, b) = two_boxes_doc();
    doc.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b,
        origin: DVec3::new(0.0, 0.0, 0.0),
        dir: DVec3::new(0.0, 0.0, 1.0),
    });
    let mut d = Driver::new(session_on(doc));
    d.tool(SELECT);
    // The unset min handle defaults to -30 degrees round the axis.
    let handle = hinge_handle_point(-FRAC_PI_6, model_reach());
    d.alt_click(handle);
    d.typed("-60");

    let (joints, _) = d.s.document().assembly().joints();
    let j = joints.values().next().unwrap();
    assert!(
        j.min
            .is_some_and(|v| (v - (-60f64).to_radians()).abs() < 1e-9),
        "{:?}",
        j.min
    );
}

#[test]
fn alt_clicking_a_joints_body_selects_it_and_stop_unstop_and_remove_act_on_it() {
    let (doc, .., id) = slide_doc();
    let mut d = Driver::new(session_on(doc));
    d.tool(SELECT);
    // Click the rail itself, away from either handle's default position.
    d.alt_click(DVec3::new(-0.5, 0.0, 0.0));
    assert_eq!(d.s.picked_annotation(), Some("joint"));

    d.action("joint.stop");
    assert!(d.s.document().assembly().joint(id).unwrap().stopped);

    d.action("joint.unstop");
    assert!(!d.s.document().assembly().joint(id).unwrap().stopped);

    d.action("joint.delete");
    assert!(d.s.document().assembly().joints().0.is_empty());
}

#[test]
fn delete_key_removes_the_selected_joint() {
    let (doc, ..) = slide_doc();
    let mut d = Driver::new(session_on(doc));
    d.tool(SELECT);
    d.alt_click(DVec3::new(-0.5, 0.0, 0.0));
    assert_eq!(d.s.picked_annotation(), Some("joint"));

    d.action("edit.delete");
    assert!(d.s.document().assembly().joints().0.is_empty());
}

#[test]
fn the_island_shows_while_the_tool_is_active_or_a_joint_is_selected_and_closes_otherwise() {
    let (s, ..) = two_boxes();
    let mut d = Driver::new(s);
    d.tool(SELECT);
    assert!(
        d.s.joint_island().is_none(),
        "nothing to show with no joints at all"
    );

    d.tool(HINGE_TOOL);
    assert!(
        d.s.joint_island().is_none(),
        "still nothing: the tool is active but no joint exists yet"
    );
    d.click(DVec3::new(0.5, 0.5, 0.5));
    d.click(DVec3::new(2.5, 0.5, 0.5));
    d.click(top_of(DVec3::new(0.0, 0.0, 0.0)));
    let j = d.s.joint_island().expect("a joint now exists");
    assert_eq!(j.kind, JointKind::Hinge);
    assert!(j.min.is_none() && j.max.is_none(), "{j:?}");

    d.tool(SELECT);
    assert!(
        d.s.joint_island().is_none(),
        "closes once the tool is no longer Hinge/Slide and nothing is selected"
    );

    // Alt + click the gizmo's own circle, away from either default
    // handle (at +/- 30 degrees round from the axis' own `u`), selects
    // it. The axis comes from wherever the third click's inference
    // landed, not necessarily the world origin, so read it back rather
    // than assuming.
    let (joints, _) = d.s.document().assembly().joints();
    let j = *joints.values().next().expect("one");
    let (u, _) = prism_infer::plane_frame(j.axis_dir);
    d.alt_click(j.axis_origin - u * model_reach());
    assert!(
        d.s.joint_island().is_some(),
        "shows again once the joint is selected"
    );
}

#[test]
fn the_islands_fields_show_only_the_bounds_that_are_set() {
    let (doc, ..) = slide_doc();
    let mut d = Driver::new(session_on(doc));
    d.tool(SELECT);
    d.alt_click(DVec3::new(model_reach() * 0.4, 0.0, 0.0));
    d.typed("0.6"); // sets the max bound exactly

    let j =
        d.s.joint_island()
            .expect("selected after setting its bound");
    assert_eq!(j.max.as_deref(), Some("0.600 m"));
    assert!(j.min.is_none(), "the min bound is still unset: {j:?}");
}

#[test]
fn typing_in_the_islands_field_sets_the_bound_exactly() {
    let (mut doc, a, b) = two_boxes_doc();
    doc.run(Command::AddJoint {
        kind: JointKind::Hinge,
        a,
        b,
        origin: DVec3::new(0.0, 0.0, 0.0),
        dir: DVec3::new(0.0, 0.0, 1.0),
    });
    let mut s = session_on(doc);
    // No selection needed: with nothing selected, the island (and
    // joint_target, which it shares with Edit > Joint) acts on the
    // newest joint, the only one here.
    s.joint_island_input(prism_tools::session::JointIslandSet::Max("60".into()));

    let (joints, _) = s.document().assembly().joints();
    let j = joints.values().next().unwrap();
    assert!(
        j.max.is_some_and(|v| (v - 60f64.to_radians()).abs() < 1e-9),
        "{:?}",
        j.max
    );
}

#[test]
fn removing_from_the_island_deletes_the_joint_in_context() {
    let (doc, ..) = slide_doc();
    let mut s = session_on(doc);
    s.joint_remove_selected();
    assert!(s.document().assembly().joints().0.is_empty());
}
