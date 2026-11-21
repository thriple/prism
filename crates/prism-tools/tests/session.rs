//! Scenario tests: a session driven by mouse and key events, as a person
//! would draw (brief 05, "Tests").

use prism_doc::Document;
use prism_infer::Kind;
use prism_math::{DVec2, DVec3};
use prism_tools::session::{LINE, RECT, SELECT};
use prism_tools::{Button, Event, Input, Key, Recording, Session};
use prism_view::Camera;

struct Driver {
    s: Session,
    t: f64,
}

impl Driver {
    fn new(doc: Document) -> Self {
        let mut s = Session::new(doc);
        s.ease = false;
        let mut d = Driver { s, t: 0.0 };
        d.send(Input::Resize {
            w: 1000.0,
            h: 700.0,
        });
        d.s.handle(&Event {
            t: 0.0,
            input: Input::Action { id: "noop".into() },
        });
        d
    }

    fn empty() -> Self {
        let mut d = Driver::new(Document::new());
        d.look(Camera {
            eye: DVec3::new(3.0, -8.0, 9.0),
            target: DVec3::new(3.0, 2.0, 0.0),
        });
        d
    }

    fn look(&mut self, c: Camera) {
        // A replay starts from a camera; tests set it the same way.
        let r = Recording {
            about: String::new(),
            model: self.s.model().clone(),
            camera: c,
            tool: self.s.tool().to_owned(),
            size: (1000.0, 700.0),
            settings: self.s.settings,
            events: vec![],
            result: String::new(),
            alt_loop: false,
            ctrl_copy: false,
            brush: Brush::default(),
            brush_copy: None,
            materials: Vec::new(),
            round_size: None,
            round_chamfer: false,
            shell_size: None,
            shell_outside: false,
            assembly: None,
            open: Vec::new(),
            selected: Vec::new(),
            result_assembly: None,
            start: None,
        };
        let old = std::mem::replace(&mut self.s, Session::replay(&r));
        self.s.ease = false;
        drop(old);
    }

    fn send(&mut self, input: Input) {
        self.t += 0.05;
        self.s.handle(&Event { t: self.t, input });
    }

    fn wait(&mut self, secs: f64) {
        self.t += secs;
        self.s.handle(&Event {
            t: self.t,
            input: Input::Time,
        });
    }

    fn screen(&self, p: DVec3) -> DVec2 {
        self.s.view().to_screen(p).expect("in front").0
    }

    fn hover(&mut self, p: DVec3) {
        let s = self.screen(p);
        self.send(Input::Move { x: s.x, y: s.y });
    }

    fn click(&mut self, p: DVec3) {
        let s = self.screen(p);
        self.click_px(s, false, false);
    }

    fn click_px(&mut self, s: DVec2, shift: bool, ctrl: bool) {
        self.send(Input::Move { x: s.x, y: s.y });
        self.send(Input::Press {
            button: Button::Left,
            x: s.x,
            y: s.y,
            shift,
            ctrl,
            alt: false,
        });
        self.send(Input::Release {
            button: Button::Left,
        });
        // Leave time so the next click is a new click, not a double-click.
        self.t += 0.5;
    }

    fn tool(&mut self, id: &str) {
        self.send(Input::Tool { id: id.into() });
    }

    fn typed(&mut self, text: &str) {
        self.send(Input::Text { text: text.into() });
        self.send(Input::Key { key: Key::Enter });
    }

    fn faces(&self) -> usize {
        self.s.model().face_count()
    }
}

#[test]
fn four_lines_close_a_square_and_the_line_stops() {
    let mut d = Driver::empty();
    d.tool(LINE);
    let c = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(4.0, 0.0, 0.0),
        DVec3::new(4.0, 3.0, 0.0),
        DVec3::new(0.0, 3.0, 0.0),
    ];
    for p in c {
        d.click(p);
    }
    assert!(d.s.in_operation(), "still drawing");
    d.click(c[0]);
    assert_eq!(d.faces(), 1, "closing the loop made a face");
    assert!(!d.s.in_operation(), "the line stops after closing a face");
    assert_eq!(d.s.model().edge_count(), 4);
    // Every vertex snapped exactly onto the corners.
    for p in c {
        assert!(prism_ops::vertex_at(d.s.model(), p).is_some(), "{p:?}");
    }
}

#[test]
fn a_typed_length_along_a_locked_axis_is_exact() {
    let mut d = Driver::empty();
    d.tool(LINE);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(3.0, 1.3, 0.0));
    d.send(Input::Key { key: Key::Right }); // lock red
    d.typed("2.5m");
    let m = d.s.model();
    assert!(
        prism_ops::vertex_at(m, DVec3::new(3.5, 1.0, 0.0)).is_some(),
        "ends exactly 2.5 m along red"
    );
    assert_eq!(m.edge_count(), 1);
    // The line chains on; lock red again and type in another unit.
    d.hover(DVec3::new(5.0, 1.3, 0.0));
    d.send(Input::Key { key: Key::Right });
    d.typed("400mm");
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(3.9, 1.0, 0.0)).is_some());
}

#[test]
fn a_rectangle_from_two_clicks_and_from_a_typed_size() {
    let mut d = Driver::empty();
    d.tool(RECT);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(2.0, 1.0, 0.0));
    assert_eq!(d.faces(), 1);
    // Typed: the size keeps the side the cursor is on.
    d.click(DVec3::new(5.0, 0.0, 0.0));
    d.hover(DVec3::new(6.0, 1.0, 0.0));
    d.typed("2,1.5");
    assert_eq!(d.faces(), 2);
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(7.0, 1.5, 0.0)).is_some());
    // One undo removes the whole rectangle.
    d.send(Input::Action {
        id: "edit.undo".into(),
    });
    assert_eq!(d.faces(), 1);
    assert_eq!(d.s.model().edge_count(), 4);
}

#[test]
fn retyping_changes_the_last_operation() {
    let mut d = Driver::empty();
    d.tool(RECT);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(2.0, 1.0, 0.0));
    d.typed("3,2");
    assert_eq!(d.faces(), 1);
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(3.0, 2.0, 0.0)).is_some());
    assert!(
        prism_ops::vertex_at(d.s.model(), DVec3::new(2.0, 1.0, 0.0)).is_none(),
        "the old corner is gone"
    );
    // The rectangle tool remembers it, and Enter after a first click reuses it.
    d.click(DVec3::new(6.0, 0.0, 0.0));
    d.hover(DVec3::new(7.0, 1.0, 0.0));
    d.send(Input::Key { key: Key::Enter });
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(9.0, 2.0, 0.0)).is_some());
    // A line's length too, once the line has stopped.
    d.tool(LINE);
    d.click(DVec3::new(0.0, 5.0, 0.0));
    d.click(DVec3::new(2.0, 5.0, 0.0));
    d.send(Input::Key { key: Key::Escape });
    d.typed("3");
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(3.0, 5.0, 0.0)).is_some());
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(2.0, 5.0, 0.0)).is_none());
}

#[test]
fn a_line_across_a_face_splits_it() {
    let mut d = Driver::empty();
    d.tool(RECT);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(4.0, 3.0, 0.0));
    d.tool(LINE);
    d.click(DVec3::new(2.0, 0.0, 0.0)); // the midpoint of the front edge
    d.click(DVec3::new(2.0, 3.0, 0.0)); // the midpoint of the back edge
    assert_eq!(d.faces(), 2);
}

#[test]
fn wake_a_corner_then_draw_to_where_the_guides_cross() {
    let mut d = Driver::empty();
    d.tool(RECT);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(4.0, 3.0, 0.0));
    d.tool(LINE);
    // Hover the far right corner long enough to wake it.
    d.hover(DVec3::new(4.0, 3.0, 0.0));
    assert_eq!(d.s.inference().map(|i| i.kind), Some(Kind::Endpoint));
    d.wait(0.3);
    d.hover(DVec3::new(4.0, 3.0, 0.0));
    // Start out on the ground to the right, then head up green.
    d.click(DVec3::new(7.0, -1.0, 0.0));
    let target = DVec3::new(7.0, 3.0, 0.0);
    let s = d.screen(target) + DVec2::new(3.0, 2.0);
    d.send(Input::Move { x: s.x, y: s.y });
    let i = d.s.inference().expect("an inference").clone();
    assert_eq!(i.kind, Kind::GuideCross, "{i:?}");
    assert!(i.point.distance(target) < 1e-6);
    d.send(Input::Press {
        button: Button::Left,
        x: s.x,
        y: s.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    assert!(prism_ops::vertex_at(d.s.model(), target).is_some());
}

#[test]
fn shift_locks_the_axis_while_held() {
    let mut d = Driver::empty();
    d.tool(LINE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    let s = d.screen(DVec3::new(3.0, 0.0, 0.0)) + DVec2::new(0.0, 3.0);
    d.send(Input::Move { x: s.x, y: s.y });
    assert_eq!(d.s.inference().map(|i| i.axis), Some(Some(0)));
    d.send(Input::ShiftDown);
    assert!(d.s.lock_text().is_some());
    // Far off the axis, still on red.
    d.hover(DVec3::new(2.0, 2.0, 0.0));
    let i = d.s.inference().expect("inference");
    assert!(i.locked && i.point.y.abs() < 1e-9);
    d.send(Input::ShiftUp);
    assert!(d.s.lock_text().is_none());
}

#[test]
fn box_select_window_and_crossing_with_shift_and_ctrl() {
    let mut d = Driver::new(prism_tools::scenes::cube());
    d.tool(SELECT);
    let all = |d: &Driver| d.s.view();
    let v = all(&d);
    // The cube's whole outline on screen.
    let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
    for (_, p) in d.s.model().vertices() {
        let s = v.to_screen(p).expect("front").0;
        lo = lo.min(s);
        hi = hi.max(s);
    }
    let drag = |d: &mut Driver, a: DVec2, b: DVec2, shift: bool, ctrl: bool| {
        d.send(Input::Move { x: a.x, y: a.y });
        d.send(Input::Press {
            button: Button::Left,
            x: a.x,
            y: a.y,
            shift,
            ctrl,
            alt: false,
        });
        d.send(Input::Move { x: b.x, y: b.y });
        d.send(Input::Release {
            button: Button::Left,
        });
        d.t += 0.5;
    };
    // Window around everything: every edge and face.
    drag(
        &mut d,
        lo - DVec2::splat(20.0),
        hi + DVec2::splat(20.0),
        false,
        false,
    );
    assert_eq!(
        (d.s.selection().edges.len(), d.s.selection().faces.len()),
        (12, 6)
    );
    // A small window in the middle selects nothing (nothing wholly inside)...
    let mid = (lo + hi) * 0.5;
    drag(
        &mut d,
        mid - DVec2::splat(10.0),
        mid + DVec2::splat(10.0),
        false,
        false,
    );
    assert!(d.s.selection().is_empty());
    // ...but as a crossing box (right to left) it catches the faces it touches.
    drag(
        &mut d,
        mid + DVec2::splat(10.0),
        mid - DVec2::splat(10.0),
        false,
        false,
    );
    assert!(!d.s.selection().faces.is_empty());
    let n = d.s.selection().faces.len();
    // Ctrl adds; Shift toggles back out.
    drag(
        &mut d,
        lo - DVec2::splat(20.0),
        hi + DVec2::splat(20.0),
        false,
        true,
    );
    assert_eq!(d.s.selection().faces.len(), 6);
    drag(
        &mut d,
        mid + DVec2::splat(10.0),
        mid - DVec2::splat(10.0),
        true,
        false,
    );
    assert_eq!(d.s.selection().faces.len(), 6 - n);
}

#[test]
fn shift_click_toggles_and_ctrl_click_adds() {
    let mut d = Driver::new(prism_tools::scenes::cube());
    d.tool(SELECT);
    let top = d.screen(DVec3::new(0.2, 0.1, 3.0));
    let front = d.screen(DVec3::new(0.2, -1.0, 2.0));
    d.click_px(top, false, false);
    assert_eq!(d.s.selection().faces.len(), 1);
    d.click_px(front, false, true);
    assert_eq!(d.s.selection().faces.len(), 2, "Ctrl adds");
    d.click_px(top, true, false);
    assert_eq!(d.s.selection().faces.len(), 1, "Shift toggles out");
    d.click_px(DVec2::new(5.0, 5.0), false, false);
    assert!(d.s.selection().is_empty(), "a click on nothing clears");
}

#[test]
fn a_recorded_session_replays_to_the_same_model() {
    let mut d = Driver::empty();
    d.s.start_recording();
    d.tool(RECT);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(2.0, 1.0, 0.0));
    d.tool(LINE);
    d.click(DVec3::new(1.0, 0.0, 0.0));
    d.click(DVec3::new(1.0, 1.0, 0.0));
    let json = d.s.stop_recording().expect("recording");
    // Run with PRISM_WRITE_SESSIONS=1 to save it as an acceptance session.
    if std::env::var_os("PRISM_WRITE_SESSIONS").is_some() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/acceptance/sessions");
        std::fs::write(dir.join("rectangle-split-by-a-line.json"), &json).expect("write");
    }
    let r: Recording = serde_json::from_str(&json).expect("valid");
    assert_eq!(Session::replay(&r).model().geometry_json(), r.result);
    assert_eq!(r.result, d.s.model().geometry_json());
    assert_eq!(d.faces(), 2);
}

/// Every recorded session in `docs/acceptance/sessions/` ends with the model
/// it recorded.
#[test]
fn saved_sessions_replay() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/acceptance/sessions");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).expect("sessions folder").flatten() {
        if e.path().extension().is_some_and(|x| x == "json") {
            let text = std::fs::read_to_string(e.path()).expect("read");
            let r: Recording = serde_json::from_str(&text).expect("a recording");
            // Recordings saved before loops named their edges still match.
            assert!(
                r.ends_with(Session::replay(&r).model()),
                "{}",
                e.path().display()
            );
            n += 1;
        }
    }
    assert!(n >= 1, "no recorded sessions");
}

#[test]
fn shift_double_and_triple_click_add_and_remove_without_touching_the_rest() {
    let mut d = Driver::new(prism_tools::scenes::pocket());
    d.tool(SELECT);
    // Something else selected first: one edge of the box's base.
    let base_edge = d.screen(DVec3::new(2.0, 0.0, 0.0));
    d.click_px(base_edge, false, false);
    assert_eq!(d.s.selection().edges.len(), 1);
    let other = d.s.selection().clone();
    // Shift + double-click the top ring face: it and its outer edges are added.
    let ring = d.screen(DVec3::new(0.4, 0.4, 2.0));
    let double = |d: &mut Driver, s: DVec2, shift: bool| {
        for _ in 0..2 {
            d.send(Input::Move { x: s.x, y: s.y });
            d.send(Input::Press {
                button: Button::Left,
                x: s.x,
                y: s.y,
                shift,
                ctrl: false,
                alt: false,
            });
            d.send(Input::Release {
                button: Button::Left,
            });
        }
        d.t += 0.5;
    };
    double(&mut d, ring, true);
    let sel = d.s.selection().clone();
    assert!(
        sel.edges.is_superset(&other.edges),
        "the earlier edge stays"
    );
    assert_eq!(sel.faces.len(), 1);
    assert!(sel.edges.len() > 4, "the ring's outer edges were added");
    // Shift + double-click it again: the ring and its edges go, the edge stays.
    double(&mut d, ring, true);
    let back = d.s.selection();
    assert!(back.faces.is_empty());
    assert_eq!(back.edges, other.edges);
    // Shift + triple-click adds everything connected, keeping the rest.
    for _ in 0..3 {
        d.send(Input::Move {
            x: ring.x,
            y: ring.y,
        });
        d.send(Input::Press {
            button: Button::Left,
            x: ring.x,
            y: ring.y,
            shift: true,
            ctrl: false,
            alt: false,
        });
        d.send(Input::Release {
            button: Button::Left,
        });
    }
    assert_eq!(d.s.selection().faces.len(), d.s.model().face_count());
}

// ---------- Edit Tools (brief 06) ----------

use prism_tools::session::{ERASE, MOVE, PUSH, ROTATE, SCALE};

fn rect_on_ground(d: &mut Driver, x0: f64, y0: f64, x1: f64, y1: f64) {
    d.tool(RECT);
    d.click(DVec3::new(x0, y0, 0.0));
    d.click(DVec3::new(x1, y1, 0.0));
}

fn valid(d: &Driver) {
    let p = prism_ops::check::problems(d.s.model());
    assert!(p.is_empty(), "{p:?}");
}

#[test]
fn push_pull_a_rectangle_into_a_box_by_typing() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.5));
    d.typed("1.5");
    assert_eq!(d.faces(), 6);
    let (lo, hi) = d.s.model().bbox().expect("box");
    assert!(
        (hi.z - lo.z - 1.5).abs() < 1e-9,
        "1.5 m tall: {lo:?} {hi:?}"
    );
    valid(&d);
    // Retype right after: the box becomes 2 m tall, still one step.
    d.typed("2");
    let (lo, hi) = d.s.model().bbox().expect("box");
    assert!((hi.z - lo.z - 2.0).abs() < 1e-9);
    d.send(Input::Action {
        id: "edit.undo".into(),
    });
    assert_eq!(d.faces(), 1, "one undo removes the pull");
}

#[test]
fn push_pull_takes_a_hundredth_of_a_centimetre_typed_in_any_unit() {
    // 3D printing: tiny adjustments, typed (owner, 2026-11-21).
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.5));
    d.typed("50cm");
    let (lo, hi) = d.s.model().bbox().expect("box");
    assert!((hi.z - lo.z - 0.5).abs() < 1e-12);
    for (text, step) in [
        ("0.01cm", 1e-4),
        ("0.1mm", 1e-4),
        ("0.0001", 1e-4),
        ("-0.01cm", -1e-4),
    ] {
        // Pull the top by it, a click at a time, as a person would.
        d.tool(SELECT);
        d.tool(PUSH);
        let (_, hi) = d.s.model().bbox().expect("box");
        d.click(DVec3::new(1.0, 1.0, hi.z));
        d.hover(DVec3::new(1.0, 1.0, hi.z + 0.1));
        d.typed(text);
        let (_, hi2) = d.s.model().bbox().expect("box");
        assert!(
            (hi2.z - hi.z - step).abs() < 1e-12,
            "{text}: {} moved",
            hi2.z - hi.z
        );
        valid(&d);
    }
    assert_eq!(d.faces(), 6);
}

#[test]
fn push_pull_of_a_freeform_face_moves_a_hundredth_of_a_centimetre() {
    // The sheet of two patches (rim_sheet), one face pulled out by 0.0001:
    // the slab's volume is its area times that.
    let mut doc = Document::with_model(rim_sheet());
    let face = doc.model().faces().map(|(id, _)| id).next().unwrap();
    assert!(doc.run(prism_doc::Command::PushPull {
        face,
        distance: 1e-4,
        new_loop: false,
    }));
    let v = model_volume(doc.model()).abs();
    assert!((v - 1.0 * 1e-4).abs() < 1e-9, "{v}");
    assert!(prism_ops::check::problems(doc.model()).is_empty());
}

#[test]
fn double_click_repeats_the_last_pull() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 1.0, 1.0);
    rect_on_ground(&mut d, 3.0, 0.0, 4.0, 1.0);
    d.tool(PUSH);
    d.click(DVec3::new(0.5, 0.5, 0.0));
    d.typed("0.75");
    // Double-click the other square.
    let s = d.screen(DVec3::new(3.5, 0.5, 0.0));
    for _ in 0..2 {
        d.send(Input::Move { x: s.x, y: s.y });
        d.send(Input::Press {
            button: Button::Left,
            x: s.x,
            y: s.y,
            shift: false,
            ctrl: false,
            alt: false,
        });
        d.send(Input::Release {
            button: Button::Left,
        });
    }
    assert_eq!(d.faces(), 12);
    let (_, hi) = d.s.model().bbox().expect("boxes");
    assert!((hi.z - 0.75).abs() < 1e-9);
    valid(&d);
}

#[test]
fn alt_pulls_a_new_loop() {
    // Pulling a box's top stretches it (6 faces); with Alt the top stays
    // and a new loop is pulled from it (four more walls).
    let mut d = Driver::new(prism_tools::scenes::cube());
    d.tool(PUSH);
    d.click(DVec3::new(0.2, 0.1, 3.0));
    d.typed("1");
    assert_eq!(d.faces(), 6, "stretched");
    d.send(Input::Action {
        id: "edit.undo".into(),
    });
    d.click(DVec3::new(0.2, 0.1, 3.0));
    d.send(Input::AltTap);
    d.typed("1");
    assert!(d.faces() > 6, "a new loop: {}", d.faces());
    valid(&d);
}

#[test]
fn pulling_matches_an_endpoints_height() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 1.0, 1.0);
    d.tool(PUSH);
    d.click(DVec3::new(0.5, 0.5, 0.0));
    d.typed("1.25");
    rect_on_ground(&mut d, 3.0, 0.0, 4.0, 1.0);
    d.tool(PUSH);
    d.click(DVec3::new(3.5, 0.5, 0.0));
    // Hover the first box's top corner: the pull matches its height.
    d.hover(DVec3::new(1.0, 0.0, 1.25));
    d.click(DVec3::new(1.0, 0.0, 1.25));
    let tops: Vec<f64> =
        d.s.model()
            .vertices()
            .map(|(_, p)| p.z)
            .filter(|z| *z > 0.1)
            .collect();
    assert!(tops.iter().all(|z| (z - 1.25).abs() < 1e-9), "{tops:?}");
}

/// Owner, 2026-11-09: an edge's middle is a snap too.
#[test]
fn pulling_matches_an_edges_midpoint() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 1.0, 1.0);
    d.tool(PUSH);
    d.click(DVec3::new(0.5, 0.5, 0.0));
    d.typed("1.25");
    rect_on_ground(&mut d, 3.0, 0.0, 4.0, 1.0);
    d.tool(PUSH);
    d.click(DVec3::new(3.5, 0.5, 0.0));
    // The middle of the first box's upright corner edge.
    let mid = DVec3::new(1.0, 0.0, 0.625);
    d.hover(mid);
    d.click(mid);
    let tops: Vec<f64> =
        d.s.model()
            .vertices()
            .filter(|(_, p)| p.x > 2.0 && p.z > 0.1)
            .map(|(_, p)| p.z)
            .collect();
    assert_eq!(tops.len(), 4);
    assert!(tops.iter().all(|z| (z - 0.625).abs() < 1e-9), "{tops:?}");
}

#[test]
fn move_by_a_typed_distance_and_copy_into_arrays() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 1.0, 1.0);
    d.tool(SELECT);
    d.click(DVec3::new(0.5, 0.5, 0.0));
    d.tool(MOVE);
    d.click(DVec3::new(0.0, 0.0, 0.0)); // grab the selected square by its corner
    d.hover(DVec3::new(2.0, 0.3, 0.0));
    d.send(Input::Key { key: Key::Right });
    d.typed("3");
    assert!(
        prism_ops::vertex_at(d.s.model(), DVec3::new(3.0, 0.0, 0.0)).is_some(),
        "moved 3 m along red"
    );
    // Copy: Ctrl during the move, then x3 makes three copies.
    d.tool(SELECT);
    d.click(DVec3::new(3.5, 0.5, 0.0));
    d.tool(MOVE);
    d.click(DVec3::new(3.0, 0.0, 0.0));
    d.send(Input::CtrlTap);
    d.hover(DVec3::new(4.0, 0.2, 0.0));
    d.send(Input::Key { key: Key::Right });
    d.typed("2");
    assert_eq!(d.faces(), 2, "a copy");
    d.typed("x3");
    assert_eq!(d.faces(), 4, "an array of three copies");
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(9.0, 0.0, 0.0)).is_some());
    d.typed("/4");
    // Four copies between, 1 m squares 0.5 m apart: they overlap, and the
    // coplanar rule splits them into six strips (PLAN 1.9).
    assert_eq!(
        d.faces(),
        6,
        "four copies between, split where they overlap"
    );
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(3.5, 0.0, 0.0)).is_some());
    d.send(Input::Action {
        id: "edit.undo".into(),
    });
    assert_eq!(d.faces(), 1, "the array was one step");
    valid(&d);
}

#[test]
fn rotate_by_typed_degrees_and_with_the_15_degree_snap() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 1.0);
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 0.5, 0.0));
    d.tool(ROTATE);
    d.click(DVec3::new(0.0, 0.0, 0.0)); // the centre
    d.click(DVec3::new(2.0, 0.0, 0.0)); // the start arm, along red
    d.hover(DVec3::new(0.0, 2.0, 0.0));
    d.typed("90");
    assert!(
        prism_ops::vertex_at(d.s.model(), DVec3::new(0.0, 2.0, 0.0)).is_some(),
        "turned onto green"
    );
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(-1.0, 2.0, 0.0)).is_some());
    valid(&d);
    // Retype: 45 degrees instead.
    d.typed("45");
    let s2 = 2f64.sqrt();
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(s2, s2, 0.0)).is_some());
}

#[test]
fn drag_from_the_centre_sets_the_start_arm() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 1.0);
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 0.5, 0.0));
    d.tool(ROTATE);
    let c = d.screen(DVec3::ZERO);
    let a = d.screen(DVec3::new(2.0, 0.0, 0.0));
    d.send(Input::Move { x: c.x, y: c.y });
    d.send(Input::Press {
        button: Button::Left,
        x: c.x,
        y: c.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Move { x: a.x, y: a.y });
    d.send(Input::Release {
        button: Button::Left,
    });
    d.hover(DVec3::new(0.0, 2.0, 0.0));
    d.typed("90");
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(0.0, 2.0, 0.0)).is_some());
}

#[test]
fn up_locks_the_protractor_to_a_face_then_it_can_move() {
    let mut d = Driver::new(prism_tools::scenes::cube());
    // The whole cube turns (with nothing selected, only the hovered face
    // would, DECISIONS #26).
    d.send(Input::Action {
        id: "edit.selall".into(),
    });
    d.tool(ROTATE);
    // Over the front face (normal -y): Up locks the protractor to it.
    d.hover(DVec3::new(0.2, -1.0, 2.0));
    d.send(Input::Key { key: Key::Up });
    // Away from the face, on the ground: still locked to the front's plane.
    d.hover(DVec3::new(3.0, -3.0, 0.0));
    let o = d.s.overlay();
    assert!(o.labels.iter().any(|l| l.text == "Plane locked"));
    // Rotate the cube about a y axis through (0,0,1) by a typed 90°.
    d.click(DVec3::new(0.0, -1.0, 1.0));
    d.click(DVec3::new(1.0, -1.0, 1.0));
    d.typed("90");
    let (lo, hi) = d.s.model().bbox().expect("cube");
    // A 2 m cube turned about y stays 2 m in every direction.
    assert!(
        ((hi - lo) - DVec3::splat(2.0)).abs().max_element() < 1e-6,
        "{lo:?} {hi:?}"
    );
    valid(&d);
    // Down releases the lock.
    d.send(Input::Key { key: Key::Down });
    assert!(
        !d.s.overlay()
            .labels
            .iter()
            .any(|l| l.text == "Plane locked")
    );
}

#[test]
fn rotate_copy_is_one_step() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 1.0, 0.0, 2.0, 1.0);
    d.tool(SELECT);
    d.click(DVec3::new(1.5, 0.5, 0.0));
    d.tool(ROTATE);
    d.click(DVec3::ZERO);
    d.click(DVec3::new(2.0, 0.0, 0.0));
    d.send(Input::CtrlTap);
    d.hover(DVec3::new(0.0, 2.0, 0.0));
    d.typed("90");
    assert_eq!(d.faces(), 2);
    d.send(Input::Action {
        id: "edit.undo".into(),
    });
    assert_eq!(d.faces(), 1);
}

fn has_point(d: &Driver, x: f64, y: f64) -> bool {
    prism_ops::vertex_at(d.s.model(), DVec3::new(x, y, 0.0)).is_some()
}

/// A 2 m square on the ground, picked by clicking it with Scale (the box
/// shows round it: flat, standing off 5%, so its grips are at -0.1 and
/// 2.1).
fn square_to_scale() -> Driver {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(SCALE);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    assert!(!d.s.selection().faces.is_empty(), "a click picks it");
    d
}

#[test]
fn scale_by_a_corner_grip_and_a_typed_factor() {
    let mut d = square_to_scale();
    d.click(DVec3::new(2.1, 2.1, 0.0));
    // The inference engine runs while scaling (owner, 2026-11-12); a typed
    // factor still decides.
    d.typed("1.5");
    // About the opposite corner.
    assert!(has_point(&d, 3.0, 3.0) && has_point(&d, 0.0, 0.0));
    valid(&d);
}

#[test]
fn alt_scales_about_the_centre() {
    let mut d = square_to_scale();
    d.send(Input::AltTap);
    d.click(DVec3::new(2.1, 2.1, 0.0));
    d.typed("1.5");
    assert!(has_point(&d, 2.5, 2.5) && has_point(&d, -0.5, -0.5));
}

#[test]
fn a_face_grip_scales_one_way() {
    let mut d = square_to_scale();
    d.click(DVec3::new(2.1, 1.0, 0.0));
    d.typed("2");
    assert!(has_point(&d, 4.0, 2.0) && has_point(&d, 0.0, 2.0), "x only");
    assert!(!has_point(&d, 4.0, 4.0));
}

#[test]
fn dragging_a_corner_grip_follows_the_line_from_the_anchor() {
    let mut d = square_to_scale();
    d.click(DVec3::new(2.1, 2.1, 0.0));
    // Twice as far from the opposite grip (at -0.1, -0.1).
    d.hover(DVec3::new(4.3, 4.3, 0.0));
    let (label, value, _) = d.s.measurement();
    assert_eq!(label, "Scale");
    assert!(value.starts_with("2.000"), "{value}");
    // The box scales with the preview: a grip stands off the 4 m square.
    let grip_at =
        |d: &Driver, p: DVec3| d.s.overlay().marks.iter().any(|m| m.at.distance(p) < 1e-6);
    assert!(grip_at(&d, DVec3::new(4.2, 4.2, 0.0)), "the box follows");
    assert!(!grip_at(&d, DVec3::new(2.1, 2.1, 0.0)));
    d.click(DVec3::new(4.3, 4.3, 0.0));
    assert!(has_point(&d, 4.0, 4.0) && has_point(&d, 0.0, 0.0));
    undo(&mut d);
    assert!(has_point(&d, 2.0, 2.0), "one undo");
}

#[test]
fn the_eraser_drags_over_an_edge_and_merges_the_faces() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 2.0);
    d.tool(LINE);
    d.click(DVec3::new(2.0, 0.0, 0.0));
    d.click(DVec3::new(2.0, 2.0, 0.0));
    assert_eq!(d.faces(), 2);
    d.tool(ERASE);
    let a = d.screen(DVec3::new(1.8, 1.0, 0.0));
    let b = d.screen(DVec3::new(2.2, 1.0, 0.0));
    let mid = d.screen(DVec3::new(2.0, 1.0, 0.0));
    d.send(Input::Move { x: a.x, y: a.y });
    d.send(Input::Press {
        button: Button::Left,
        x: a.x,
        y: a.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Move { x: mid.x, y: mid.y });
    d.send(Input::Move { x: b.x, y: b.y });
    d.send(Input::Release {
        button: Button::Left,
    });
    assert_eq!(
        d.faces(),
        1,
        "the two flat faces beside the erased edge merge (DECISIONS #15)"
    );
    assert_eq!(d.s.model().edge_count(), 6);
    let (id, _) = d.s.model().faces().next().unwrap();
    assert!((d.s.model().face_area(id) - 8.0).abs() < 1e-9);
    valid(&d);
}

#[test]
fn esc_cancels_an_edit_and_leaves_the_model() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 1.0, 1.0);
    let before = d.s.model().geometry_json();
    d.tool(PUSH);
    d.click(DVec3::new(0.5, 0.5, 0.0));
    d.hover(DVec3::new(0.5, 0.5, 1.0));
    d.send(Input::Key { key: Key::Escape });
    assert_eq!(d.s.model().geometry_json(), before);
    assert!(!d.s.in_operation());
    assert_eq!(d.s.tool(), PUSH, "the tool stays");
}

/// A small building, start to finish: a box, a roof ridge moved up, and a
/// window pushed into the wall, then copied. Saved as an acceptance session
/// with PRISM_WRITE_SESSIONS=1.
#[test]
fn model_a_small_building() {
    let mut d = Driver::empty();
    d.s.start_recording();
    rect_on_ground(&mut d, 0.0, 0.0, 6.0, 4.0);
    d.tool(PUSH);
    d.click(DVec3::new(3.0, 2.0, 0.0));
    d.typed("3");
    assert_eq!(d.faces(), 6);
    // A ridge line across the top, through the midpoints of the short sides.
    d.tool(LINE);
    d.click(DVec3::new(0.0, 2.0, 3.0));
    d.click(DVec3::new(6.0, 2.0, 3.0));
    d.send(Input::Key { key: Key::Escape });
    assert_eq!(d.faces(), 7, "the top is split in two");
    // Select the ridge and move it up 1.5 m.
    d.tool(SELECT);
    d.click(DVec3::new(3.0, 2.0, 3.0));
    assert_eq!(d.s.selection().edges.len(), 1);
    d.tool(MOVE);
    d.click(DVec3::new(3.0, 2.0, 3.0));
    d.send(Input::Key { key: Key::Up });
    d.hover(DVec3::new(3.0, 2.0, 4.0));
    d.typed("1.5");
    assert!(
        prism_ops::vertex_at(d.s.model(), DVec3::new(0.0, 2.0, 4.5)).is_some(),
        "the ridge is up"
    );
    // A window on the front wall (y = 0), pushed in 0.2 m.
    d.tool(RECT);
    d.click(DVec3::new(1.0, 0.0, 1.0));
    d.click(DVec3::new(2.0, 0.0, 2.0));
    d.tool(PUSH);
    d.click(DVec3::new(1.5, 0.0, 1.5));
    d.typed("-0.2");
    let inner =
        d.s.model()
            .vertices()
            .filter(|(_, p)| (p.y - 0.2).abs() < 1e-9)
            .count();
    assert_eq!(inner, 4, "the window is 0.2 m into the wall");
    valid(&d);
    let json = d.s.stop_recording().expect("recording");
    let r: Recording = serde_json::from_str(&json).expect("valid");
    assert_eq!(Session::replay(&r).model().geometry_json(), r.result);
    if std::env::var_os("PRISM_WRITE_SESSIONS").is_some() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/acceptance/sessions");
        std::fs::write(dir.join("small-building.json"), &json).expect("write");
    }
}

// ---------- Edit Tools part 2 (brief 07) ----------

use prism_tools::session::{OFFSET, PROTRACTOR, TAPE};
use prism_topo::Guide;

fn areas(d: &Driver) -> Vec<f64> {
    let m = d.s.model();
    let mut a: Vec<f64> = m.faces().map(|(id, _)| m.face_area(id)).collect();
    a.sort_by(f64::total_cmp);
    a
}

fn same(a: &[f64], b: &[f64]) {
    assert_eq!(a.len(), b.len(), "{a:?} vs {b:?}");
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() < 1e-9, "{a:?} vs {b:?}");
    }
}

fn undo(d: &mut Driver) {
    d.send(Input::Action {
        id: "edit.undo".into(),
    });
}

fn guides(d: &Driver) -> Vec<Guide> {
    d.s.model().guides().map(|(_, g)| *g).collect()
}

fn double_click(d: &mut Driver, p: DVec3) {
    let s = d.screen(p);
    for _ in 0..2 {
        d.send(Input::Move { x: s.x, y: s.y });
        d.send(Input::Press {
            button: Button::Left,
            x: s.x,
            y: s.y,
            shift: false,
            ctrl: false,
            alt: false,
        });
        d.send(Input::Release {
            button: Button::Left,
        });
    }
    d.t += 0.5;
}

#[test]
fn offset_by_a_typed_distance_then_retyped() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 4.0);
    d.tool(OFFSET);
    d.click(DVec3::new(0.5, 2.0, 0.0));
    d.hover(DVec3::new(1.0, 2.0, 0.0));
    assert!(d.s.display_scene().faces.len() == 2, "a live preview");
    d.typed("1");
    same(&areas(&d), &[4.0, 12.0]);
    valid(&d);
    d.typed("0.5");
    same(&areas(&d), &[7.0, 9.0]);
    undo(&mut d);
    assert_eq!(d.faces(), 1, "one undo removes the offset");
}

#[test]
fn offset_by_the_mouse_inward_and_outward() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 4.0);
    d.tool(OFFSET);
    d.click(DVec3::new(1.0, 2.0, 0.0));
    d.click(DVec3::new(2.0, 2.0, 0.0));
    same(&areas(&d), &[4.0, 12.0]);
    rect_on_ground(&mut d, 10.0, 10.0, 12.0, 12.0);
    d.tool(OFFSET);
    d.click(DVec3::new(10.5, 11.0, 0.0));
    d.click(DVec3::new(9.5, 11.0, 0.0));
    // The 2 x 2 square grows a 1 m ring: 16 - 4.
    same(&areas(&d), &[4.0, 4.0, 12.0, 12.0]);
    valid(&d);
}

#[test]
fn double_click_repeats_the_last_offset() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    rect_on_ground(&mut d, 4.0, 0.0, 6.0, 2.0);
    d.tool(OFFSET);
    d.click(DVec3::new(0.2, 1.0, 0.0));
    d.typed("0.25");
    double_click(&mut d, DVec3::new(5.0, 1.0, 0.0));
    same(&areas(&d), &[1.75, 1.75, 2.25, 2.25]);
    valid(&d);
}

#[test]
fn tape_makes_a_parallel_guide_that_snaps() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 2.0);
    d.tool(TAPE);
    d.click(DVec3::new(1.0, 0.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.0));
    d.typed("0.5");
    let on = |g: &Guide, y: f64| match *g {
        Guide::Line { origin, dir } => {
            (origin.y - y).abs() < 1e-9 && origin.z.abs() < 1e-9 && dir.x.abs() > 1.0 - 1e-12
        }
        Guide::Point { .. } => false,
    };
    assert!(on(&guides(&d)[0], 0.5), "{:?}", guides(&d));
    // Retyping moves it, still one guide.
    d.typed("0.75");
    assert_eq!(guides(&d).len(), 1);
    assert!(on(&guides(&d)[0], 0.75));
    // Line snaps to it, and to where it crosses an edge.
    d.tool(LINE);
    d.hover(DVec3::new(2.6, 0.75, 0.0));
    assert_eq!(d.s.inference().expect("inference").label, "On Guide");
    d.hover(DVec3::new(0.0, 0.75, 0.0));
    let i = d.s.inference().expect("inference");
    assert_eq!(i.kind, Kind::Intersection);
    assert!((i.point - DVec3::new(0.0, 0.75, 0.0)).length() < 1e-9);
    // Double-click the top edge: a guide along it at the last offset,
    // towards its face.
    d.tool(TAPE);
    double_click(&mut d, DVec3::new(3.0, 2.0, 0.0));
    assert_eq!(guides(&d).len(), 2);
    assert!(on(&guides(&d)[1], 1.25), "{:?}", guides(&d));
    valid(&d);
}

#[test]
fn tape_from_a_corner_makes_a_guide_point() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 2.0);
    d.tool(TAPE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.send(Input::Key { key: Key::Right });
    d.hover(DVec3::new(1.0, 0.3, 0.0));
    d.typed("1.5");
    match guides(&d)[..] {
        [Guide::Point { at }] => assert!((at - DVec3::new(1.5, 0.0, 0.0)).length() < 1e-9),
        ref g => panic!("{g:?}"),
    }
    d.tool(LINE);
    d.hover(DVec3::new(1.5, 0.0, 0.0));
    assert_eq!(d.s.inference().expect("inference").kind, Kind::GuidePoint);
}

#[test]
fn ctrl_makes_the_tape_measure_only() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 2.0);
    let steps = d.s.document().log().count();
    d.tool(TAPE);
    d.send(Input::CtrlTap);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.hover(DVec3::new(4.0, 2.0, 0.0));
    let (label, value, _) = d.s.measurement();
    assert_eq!(label, "Length");
    assert!(value.contains("4.47"), "{value}");
    d.click(DVec3::new(4.0, 2.0, 0.0));
    assert!(guides(&d).is_empty());
    assert_eq!(d.s.document().log().count(), steps, "nothing to undo");
}

#[test]
fn protractor_makes_a_guide_at_a_typed_angle() {
    let mut d = Driver::empty();
    d.tool(PROTRACTOR);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.hover(DVec3::new(2.0, 0.0, 0.0));
    d.click(DVec3::new(2.0, 0.0, 0.0));
    d.typed("30");
    match guides(&d)[..] {
        [Guide::Line { origin, dir }] => {
            assert!(origin.length() < 1e-9);
            assert!(dir.z.abs() < 1e-12, "on the ground plane");
            assert!(
                (dir.x.abs() - 30f64.to_radians().cos()).abs() < 1e-9,
                "{dir:?}"
            );
        }
        ref g => panic!("{g:?}"),
    }
    // Retyping turns it, still one guide.
    d.typed("45");
    match guides(&d)[..] {
        [Guide::Line { dir, .. }] => {
            assert!((dir.x.abs() - 45f64.to_radians().cos()).abs() < 1e-9)
        }
        ref g => panic!("{g:?}"),
    }
}

#[test]
fn the_eraser_takes_a_guide_and_an_edge_in_one_step() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 2.0);
    d.tool(TAPE);
    d.click(DVec3::new(1.0, 0.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.0));
    d.typed("0.5");
    assert_eq!(guides(&d).len(), 1);
    let before = d.s.model().geometry_json();
    d.tool(ERASE);
    let a = d.screen(DVec3::new(2.5, 0.5, 0.0));
    let b = d.screen(DVec3::new(4.0, 1.0, 0.0));
    d.send(Input::Move { x: a.x, y: a.y });
    d.send(Input::Press {
        button: Button::Left,
        x: a.x,
        y: a.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Move { x: b.x, y: b.y });
    d.send(Input::Release {
        button: Button::Left,
    });
    assert!(guides(&d).is_empty());
    assert_eq!(d.faces(), 0, "the edge took its face");
    valid(&d);
    undo(&mut d);
    assert_eq!(
        d.s.model().geometry_json(),
        before,
        "one undo brings both back"
    );
}

#[test]
fn hidden_guides_are_not_snapped_to() {
    let mut d = Driver::empty();
    d.tool(TAPE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.send(Input::Key { key: Key::Right });
    d.typed("2");
    d.tool(LINE);
    d.hover(DVec3::new(2.0, 0.0, 0.0));
    assert_eq!(d.s.inference().expect("i").kind, Kind::GuidePoint);
    d.send(Input::Action {
        id: "view.guides".into(),
    });
    assert!(
        d.s.scene().guides.is_empty(),
        "hidden guides are not in the scene"
    );
    d.hover(DVec3::new(2.0, 0.0, 0.0));
    assert_ne!(d.s.inference().expect("i").kind, Kind::GuidePoint);
    d.send(Input::Action {
        id: "view.guides".into(),
    });
    d.hover(DVec3::new(2.0, 0.0, 0.0));
    assert_eq!(d.s.inference().expect("i").kind, Kind::GuidePoint);
}

/// A window placed from tape guides, and the roof offset in: saved as
/// an acceptance session with PRISM_WRITE_SESSIONS=1.
#[test]
fn model_a_window_from_guides() {
    let mut d = Driver::empty();
    d.s.start_recording();
    rect_on_ground(&mut d, 0.0, 0.0, 6.0, 4.0);
    d.tool(PUSH);
    d.click(DVec3::new(3.0, 2.0, 0.0));
    d.typed("3");
    // A sill guide 1 m up the front wall, and a jamb guide 2 m in.
    d.tool(TAPE);
    d.click(DVec3::new(2.0, 0.0, 0.0));
    d.hover(DVec3::new(2.0, 0.0, 0.6));
    d.typed("1");
    // (Not at z = 1, where the sill guide crosses the edge.)
    d.click(DVec3::new(0.0, 0.0, 1.3));
    d.hover(DVec3::new(0.6, 0.0, 1.3));
    d.typed("2");
    assert_eq!(guides(&d).len(), 2);
    // The window from where the guides cross.
    d.tool(RECT);
    d.hover(DVec3::new(2.0, 0.0, 1.0));
    assert_eq!(d.s.inference().expect("i").kind, Kind::GuideCross);
    d.click(DVec3::new(2.0, 0.0, 1.0));
    d.hover(DVec3::new(3.0, 0.0, 2.0));
    d.typed("1.5,1.2");
    d.tool(PUSH);
    d.click(DVec3::new(2.7, 0.0, 1.6));
    d.typed("-0.2");
    let inner =
        d.s.model()
            .vertices()
            .filter(|(_, p)| (p.y - 0.2).abs() < 1e-9)
            .count();
    assert_eq!(inner, 4, "the window is 0.2 m into the wall");
    assert!(
        prism_ops::vertex_at(d.s.model(), DVec3::new(3.5, 0.2, 2.2)).is_some(),
        "1.5 by 1.2 from the guide cross"
    );
    // A parapet: the roof offset 0.3 m in.
    d.tool(OFFSET);
    d.click(DVec3::new(3.0, 0.2, 3.0));
    d.typed("0.3");
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(0.3, 0.3, 3.0)).is_some());
    valid(&d);
    let json = d.s.stop_recording().expect("recording");
    let r: Recording = serde_json::from_str(&json).expect("valid");
    assert_eq!(Session::replay(&r).model().geometry_json(), r.result);
    if std::env::var_os("PRISM_WRITE_SESSIONS").is_some() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/acceptance/sessions");
        std::fs::write(dir.join("window-from-guides.json"), &json).expect("write");
    }
}

/// DECISIONS #26: with nothing selected, Rotate turns the hovered face
/// alone, and the faces attached to it fold.
#[test]
fn rotate_twists_a_hovered_face_and_folds_its_neighbours() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("2");
    assert_eq!(d.faces(), 6);
    let bottom =
        d.s.model()
            .vertices()
            .filter(|(_, p)| p.z.abs() < 1e-9)
            .count();
    d.tool(ROTATE);
    assert!(d.s.selection().is_empty());
    d.click(DVec3::new(1.0, 1.0, 2.0));
    d.hover(DVec3::new(2.0, 1.0, 2.0));
    d.click(DVec3::new(2.0, 1.0, 2.0));
    d.typed("30");
    // The base stays where it was; the walls fold.
    let still =
        d.s.model()
            .vertices()
            .filter(|(_, p)| p.z.abs() < 1e-9)
            .count();
    assert_eq!(still, bottom, "the bottom did not turn");
    assert!(d.faces() > 6, "the side walls folded: {}", d.faces());
    valid(&d);
}

/// Edit > Intersect Faces (DECISIONS #29), as one undo step.
#[test]
fn intersect_faces_with_the_model_is_one_step() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 4.0);
    // An upright rectangle standing through the square.
    d.send(Input::Action { id: "noop".into() });
    let mut doc = Document::with_model(d.s.model().clone());
    doc.run(prism_doc::Command::Loop {
        points: vec![
            DVec3::new(1.0, 2.0, -1.0),
            DVec3::new(3.0, 2.0, -1.0),
            DVec3::new(3.0, 2.0, 1.0),
            DVec3::new(1.0, 2.0, 1.0),
        ],
    });
    let mut d = Driver::new(Document::with_model(doc.model().clone()));
    let before = d.s.model().geometry_json();
    let faces = d.faces();
    d.send(Input::Action {
        id: "isect.model".into(),
    });
    assert!(d.faces() > faces, "the crossing split the faces");
    valid(&d);
    undo(&mut d);
    assert_eq!(d.s.model().geometry_json(), before, "one undo puts it back");
}

/// The owner's stairs: press on the lower step's top, drag over the higher
/// step's top and release. The lower step rises level with it, committed
/// on release. Erasing the line between the two tops then leaves one face.
#[test]
fn dragging_a_pull_over_a_parallel_face_lifts_it_level() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    rect_on_ground(&mut d, 2.0, 0.0, 4.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("0.5");
    d.click(DVec3::new(3.0, 1.0, 0.0));
    d.typed("1");
    let low = d.screen(DVec3::new(1.0, 1.0, 0.5));
    let high = d.screen(DVec3::new(3.0, 1.0, 1.0));
    d.send(Input::Move { x: low.x, y: low.y });
    let before = d.s.model().clone();
    d.send(Input::Press {
        button: Button::Left,
        x: low.x,
        y: low.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Move {
        x: high.x,
        y: high.y,
    });
    assert_eq!(&before, d.s.model(), "nothing committed while held");
    d.send(Input::Release {
        button: Button::Left,
    });
    let top = prism_ops::face_at(d.s.model(), DVec3::new(1.0, 1.0, 1.0));
    assert!(top.is_some(), "the lower step rose to 1 m");
    valid(&d);
    // The line between the two tops: erase it, one top face is left.
    let a = prism_ops::vertex_at(d.s.model(), DVec3::new(2.0, 0.0, 1.0)).unwrap();
    let b = prism_ops::vertex_at(d.s.model(), DVec3::new(2.0, 2.0, 1.0)).unwrap();
    let e =
        d.s.model()
            .edge_between(a, b)
            .expect("the line between the tops");
    let mut m = d.s.model().clone();
    prism_ops::remove(&mut m, &prism_ops::select::Selection::edge(e));
    let one = prism_ops::face_at(&m, DVec3::new(2.0, 1.0, 1.0)).expect("one top");
    assert!((m.face_area(one) - 8.0).abs() < 1e-9);
    assert!(prism_ops::check::problems(&m).is_empty());
}

/// Offset snaps like the other tools: a corner anywhere in the model, put
/// onto the face's plane, sets the distance (owner feedback, 2026-11-08).
#[test]
fn offset_snaps_to_a_corner_off_its_plane() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 4.0);
    // A box beside it, so its top corner is above the ground.
    rect_on_ground(&mut d, 6.0, 2.0, 7.0, 3.0);
    d.tool(PUSH);
    d.click(DVec3::new(6.5, 2.5, 0.0));
    d.typed("1");
    d.tool(OFFSET);
    d.click(DVec3::new(4.0, 1.0, 0.0)); // on the square's right edge
    d.hover(DVec3::new(6.0, 2.0, 1.0));
    let i = d.s.inference().expect("an inference while offsetting");
    assert_eq!(i.kind, prism_infer::Kind::Endpoint);
    let grown =
        d.s.display_scene()
            .vertices
            .iter()
            .any(|(_, p)| p.distance(DVec3::new(6.0, 6.0, 0.0)) < 1e-6);
    assert!(grown, "offset 2 m out, to the corner's x");
}

/// A snapped point is where the offset loop goes, wherever the first click
/// was, and past a corner the loop's mitre reaches it (owner feedback,
/// 2026-11-08: lining an offset up with another edge over- or undershot).
#[test]
fn offset_through_a_snapped_point_past_a_corner() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 4.0);
    rect_on_ground(&mut d, 6.0, 5.0, 7.0, 6.0);
    d.tool(PUSH);
    d.click(DVec3::new(6.5, 5.5, 0.0));
    d.typed("1");
    d.tool(OFFSET);
    d.click(DVec3::new(3.0, 2.0, 0.0)); // inside, 1 m from the right edge
    d.hover(DVec3::new(6.0, 5.0, 1.0));
    let i = d.s.inference().expect("an inference while offsetting");
    assert_eq!(i.kind, prism_infer::Kind::Endpoint);
    let has = |x: f64, y: f64| {
        d.s.display_scene()
            .vertices
            .iter()
            .any(|(_, p)| p.distance(DVec3::new(x, y, 0.0)) < 1e-6)
    };
    assert!(has(6.0, 6.0) && has(-2.0, -2.0), "offset 2 m out");
}

// ---------- groups (Groups & Components part 1) ----------

fn act(d: &mut Driver, id: &str) {
    d.send(Input::Action { id: id.into() });
}

fn key(d: &mut Driver, k: Key) {
    d.send(Input::Key { key: k });
}

/// A 2 m box at the origin, made into a group. Returns the group.
fn grouped_box(d: &mut Driver) -> prism_doc::InstanceId {
    rect_on_ground(d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("2");
    d.tool(SELECT);
    act(d, "edit.selall");
    act(d, "edit.group");
    assert_eq!(d.faces(), 0, "the box went into the group");
    *d.s.selected_groups()
        .iter()
        .next()
        .expect("the new group is selected")
}

/// A box, selected, made into a component (the Make Component action).
fn component_box(d: &mut Driver) {
    rect_on_ground(d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("2");
    d.tool(SELECT);
    act(d, "edit.selall");
    act(d, "edit.comp");
}

#[test]
fn a_new_component_asks_for_its_name_and_the_name_is_part_of_making_it() {
    // Brief 50 part 1: the window opens the Inspector at the name field;
    // the name given is in the same undo step as the making.
    let mut d = Driver::empty();
    let log = d.s.document().log().count();
    component_box(&mut d);
    assert!(d.s.take_name_request(), "the window is asked once");
    assert!(!d.s.take_name_request());
    assert_eq!(d.s.document().assembly().components()[0].1, "Component");
    d.s.inspector_set("name", "Wall").expect("names it");
    let comps = d.s.document().assembly().components();
    assert_eq!(comps.len(), 1);
    assert_eq!(comps[0].1, "Wall");
    assert_eq!(d.s.selected_groups().len(), 1, "still selected");
    assert!(
        d.s.inspector().summary.contains("Wall"),
        "{}",
        d.s.inspector().summary
    );
    // One undo step took away both the making and the name.
    let steps = d.s.document().log().count();
    act(&mut d, "edit.undo");
    assert!(d.s.document().assembly().components().is_empty());
    assert_eq!(d.s.document().log().count(), steps - 1);
    let _ = log;
    // Redo brings back the named component.
    act(&mut d, "edit.redo");
    assert_eq!(d.s.document().assembly().components()[0].1, "Wall");
}

#[test]
fn esc_keeps_the_default_name_and_a_later_rename_is_its_own_step() {
    let mut d = Driver::empty();
    component_box(&mut d);
    d.s.take_name_request();
    d.s.name_cancelled();
    let id = *d.s.selected_groups().iter().next().expect("one selected");
    d.s.inspector_set("name", "Wall").expect("renames");
    // After Esc the name is the instance's own: the component keeps its.
    assert_eq!(d.s.document().assembly().name_of(id), Some("Wall"));
    assert_eq!(d.s.document().assembly().components()[0].1, "Component");
    // The making is still there under the rename: two steps.
    act(&mut d, "edit.undo");
    assert_eq!(d.s.document().assembly().components().len(), 1);
    act(&mut d, "edit.undo");
    assert!(d.s.document().assembly().components().is_empty());
}

#[test]
fn a_component_name_another_component_has_is_refused() {
    let mut d = Driver::empty();
    component_box(&mut d);
    d.s.inspector_set("name", "Wall").expect("names it");
    // A second box, made into a component: "Wall" is taken (case aside).
    d.tool(SELECT);
    rect_on_ground(&mut d, 5.0, 5.0, 7.0, 7.0);
    d.tool(SELECT);
    act(&mut d, "edit.selall");
    // Select only the loose rectangle's face by clicking it.
    d.click(DVec3::new(6.0, 6.0, 0.0));
    act(&mut d, "edit.comp");
    assert!(d.s.take_name_request());
    let why = d.s.inspector_set("name", "wall").unwrap_err();
    assert!(why.contains("already called"), "{why}");
    let names: Vec<_> =
        d.s.document()
            .assembly()
            .components()
            .into_iter()
            .map(|c| c.1.to_owned())
            .collect();
    assert!(names.contains(&"Component".to_owned()), "{names:?}");
    d.s.inspector_set("name", "Plinth").expect("a new name");
    // A group's name is not held to it.
    assert_eq!(d.s.document().assembly().components().len(), 2);
}

#[test]
fn tools_that_cannot_change_a_closed_group_say_to_open_it() {
    // Tool pass part 2 (from the Fillet window's leftovers).
    use prism_tools::session::{CLOSED_GROUP, FILLET};
    let mut d = Driver::empty();
    grouped_box(&mut d);
    for t in [PUSH, FILLET] {
        d.tool(t);
        d.hover(DVec3::new(1.0, 1.0, 2.0));
        assert_eq!(d.s.hint(), CLOSED_GROUP, "{t}");
        d.hover(DVec3::new(6.0, 6.0, 0.0));
        assert_ne!(d.s.hint(), CLOSED_GROUP, "{t}: off the group");
    }
}

#[test]
fn a_group_is_clicked_moved_opened_and_closed() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    // A click on its face selects the whole group.
    act(&mut d, "edit.desel");
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 0.0, 1.0));
    assert_eq!(
        d.s.selected_groups().iter().copied().collect::<Vec<_>>(),
        [g]
    );
    assert!(d.s.selection().is_empty());

    // Move moves the group's placement only.
    d.tool(MOVE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.hover(DVec3::new(1.0, 0.0, 0.0));
    key(&mut d, Key::Right);
    d.typed("3");
    let w = d.s.document().assembly().world_of(&[g]).expect("group");
    assert!(
        (w.translation - DVec3::new(3.0, 0.0, 0.0)).length() < 1e-9,
        "{w:?}"
    );
    assert_eq!(d.faces(), 0);

    // A double-click opens it; a line across its top lands inside.
    d.tool(SELECT);
    act(&mut d, "edit.desel");
    double_click(&mut d, DVec3::new(4.0, 1.0, 2.0));
    assert_eq!(d.s.open_path(), [g]);
    d.tool(LINE);
    d.click(DVec3::new(3.0, 1.0, 2.0));
    d.click(DVec3::new(5.0, 1.0, 2.0));
    key(&mut d, Key::Escape);
    let def = d.s.document().assembly().context_def(&[g]).unwrap();
    let inside = &d.s.document().assembly().def(def).unwrap().model;
    assert_eq!(inside.face_count(), 7, "the top is split inside the group");
    assert!(prism_ops::check::problems(inside).is_empty());
    assert_eq!(d.faces(), 0, "nothing at the root");

    // Esc closes it (after the line tool's own Esc), and selects it.
    d.tool(SELECT);
    key(&mut d, Key::Escape);
    assert!(d.s.open_path().is_empty());
    assert_eq!(d.s.selected_groups().len(), 1);

    // Undo of the line opens the group again.
    undo(&mut d);
    assert_eq!(d.s.open_path(), [g]);
}

#[test]
fn the_blue_box_moves_with_the_group_during_move() {
    let mut d = Driver::empty();
    grouped_box(&mut d);
    d.tool(MOVE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.hover(DVec3::new(1.0, 0.0, 0.0));
    key(&mut d, Key::Right);
    d.hover(DVec3::new(1.0, 0.0, 0.0));
    let blue = d.s.style.selected;
    let xs: Vec<f64> =
        d.s.overlay()
            .tested
            .iter()
            .filter(|l| l.color == blue)
            .flat_map(|l| [l.a.x, l.b.x])
            .collect();
    assert_eq!(xs.len(), 24, "one box of 12 edges");
    let (lo, hi) = xs
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
    assert!(
        (lo - 1.0).abs() < 1e-6 && (hi - 3.0).abs() < 1e-6,
        "{lo} {hi}"
    );
}

/// Inside an open group, a double-click takes a face and its edges, and a
/// triple-click everything connected (owner feedback, 2026-11-08).
#[test]
fn double_and_triple_click_work_inside_an_open_group() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    act(&mut d, "edit.desel");
    d.tool(SELECT);
    double_click(&mut d, DVec3::new(1.0, 1.0, 2.0));
    assert_eq!(d.s.open_path(), [g]);
    d.wait(1.0);
    double_click(&mut d, DVec3::new(1.0, 1.0, 2.0));
    let sel = d.s.selection();
    assert_eq!(
        (sel.faces.len(), sel.edges.len()),
        (1, 4),
        "the top and its edges"
    );
    d.wait(1.0);
    let s = d.screen(DVec3::new(1.0, 1.0, 2.0));
    for _ in 0..3 {
        d.send(Input::Press {
            button: Button::Left,
            x: s.x,
            y: s.y,
            shift: false,
            ctrl: false,
            alt: false,
        });
        d.send(Input::Release {
            button: Button::Left,
        });
    }
    let sel = d.s.selection();
    assert_eq!((sel.faces.len(), sel.edges.len()), (6, 12), "the whole box");
}

/// Push/Pull and Offset do nothing on a closed group's faces (owner
/// feedback, 2026-11-08: the drag still ran).
#[test]
fn push_pull_and_offset_leave_a_closed_group_alone() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    act(&mut d, "edit.desel");
    let inside = |d: &Driver| {
        let a = d.s.document().assembly();
        let def = a.context_def(&[g]).expect("group");
        a.def(def).expect("def").model.geometry_json()
    };
    let before = inside(&d);
    for tool in [PUSH, OFFSET] {
        d.tool(tool);
        d.click(DVec3::new(1.0, 1.0, 2.0));
        assert!(!d.s.in_operation(), "{tool}: no step started");
        d.hover(DVec3::new(1.0, 1.0, 3.0));
        assert!(d.s.inference().is_none(), "{tool}: no snapping");
        d.click(DVec3::new(1.0, 1.0, 3.0));
    }
    assert_eq!(inside(&d), before);
    assert_eq!(d.faces(), 0, "nothing at the root");
}

/// The open group's dashed box stands off its contents, so it shows round
/// a flat rectangle too (owner feedback, 2026-11-08).
#[test]
fn the_open_box_stands_off_its_contents() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    d.s.open_path_to(&[g]);
    let c = d.s.open_box().expect("a box while open");
    let (lo, hi) = c
        .iter()
        .fold((c[0], c[0]), |(l, h), p| (l.min(*p), h.max(*p)));
    assert!((lo - DVec3::splat(-0.1)).length() < 1e-9, "{lo}");
    assert!((hi - DVec3::splat(2.1)).length() < 1e-9, "{hi}");
}

#[test]
fn drawing_inside_a_group_snaps_to_geometry_outside_it() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    // A square at the root beside the group.
    rect_on_ground(&mut d, 4.0, 0.0, 5.0, 1.0);
    d.tool(SELECT);
    act(&mut d, "edit.desel");
    double_click(&mut d, DVec3::new(1.0, 0.0, 1.0));
    assert_eq!(d.s.open_path(), [g]);
    d.tool(LINE);
    d.hover(DVec3::new(4.0, 1.0, 0.0));
    let i = d.s.inference().expect("inference");
    assert_eq!(
        i.kind,
        Kind::Endpoint,
        "snaps to the square's corner outside"
    );
    assert!(i.point.distance(DVec3::new(4.0, 1.0, 0.0)) < 1e-9);
}

#[test]
fn a_click_on_geometry_outside_closes_the_group() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    rect_on_ground(&mut d, 4.0, 0.0, 6.0, 2.0);
    d.tool(SELECT);
    act(&mut d, "edit.desel");
    double_click(&mut d, DVec3::new(1.0, 0.0, 1.0));
    assert_eq!(d.s.open_path(), [g]);
    d.click(DVec3::new(5.0, 1.0, 0.0));
    assert!(d.s.open_path().is_empty());
}

#[test]
fn explode_puts_the_box_back_and_undo_restores_the_group() {
    let mut d = Driver::empty();
    let _g = grouped_box(&mut d);
    act(&mut d, "edit.explode");
    assert_eq!(d.faces(), 6);
    assert!(!d.s.document().assembly().has_instances());
    valid(&d);
    undo(&mut d);
    assert_eq!(d.faces(), 0);
    assert!(d.s.document().assembly().has_instances());
}

#[test]
fn box_select_takes_groups_inside_the_window() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    act(&mut d, "edit.desel");
    d.tool(SELECT);
    // A window from far left to far right around everything.
    let a = d.screen(DVec3::new(-3.0, -3.0, 0.0));
    let b = d.screen(DVec3::new(5.0, 5.0, 3.0));
    let (lo, hi) = (a.min(b) - DVec2::splat(40.0), a.max(b) + DVec2::splat(40.0));
    d.send(Input::Move { x: lo.x, y: lo.y });
    d.send(Input::Press {
        button: Button::Left,
        x: lo.x,
        y: lo.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Move { x: hi.x, y: hi.y });
    d.send(Input::Release {
        button: Button::Left,
    });
    assert_eq!(
        d.s.selected_groups().iter().copied().collect::<Vec<_>>(),
        [g]
    );
}

#[test]
fn editing_one_component_instance_changes_the_other() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("2");
    d.tool(SELECT);
    act(&mut d, "edit.selall");
    act(&mut d, "edit.comp");
    let a = *d.s.selected_groups().iter().next().expect("the component");
    // A linked copy with Move and Ctrl.
    d.tool(MOVE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.send(Input::CtrlTap);
    d.hover(DVec3::new(1.0, 0.0, 0.0));
    key(&mut d, Key::Right);
    d.typed("4");
    let asm = d.s.document().assembly();
    let def = asm.context_def(&[a]).unwrap();
    assert_eq!(asm.uses(def), 2, "a linked copy");
    let b = *asm
        .root()
        .instances
        .keys()
        .find(|i| **i != a)
        .expect("the copy");
    // Edit the copy: the original changes too.
    d.tool(SELECT);
    act(&mut d, "edit.desel");
    double_click(&mut d, DVec3::new(5.0, 0.0, 1.0));
    assert_eq!(d.s.open_path(), [b]);
    d.tool(LINE);
    d.click(DVec3::new(4.0, 1.0, 2.0));
    d.click(DVec3::new(6.0, 1.0, 2.0));
    key(&mut d, Key::Escape);
    let asm = d.s.document().assembly();
    assert_eq!(asm.def(def).unwrap().model.face_count(), 7);
    assert_eq!(asm.context_def(&[a]), Some(def), "the same definition");
    // The other instance is drawn normally while the copy is open.
    assert!(
        d.s.scene()
            .faces
            .iter()
            .any(|f| !f.faded && d.s.scene().face_owner.contains_key(&f.id)),
        "the original shows the edit, not faded"
    );
    d.tool(SELECT);
    key(&mut d, Key::Escape);
    // Make Unique, then the copy no longer follows the original.
    act(&mut d, "edit.unique");
    let asm = d.s.document().assembly();
    assert_ne!(asm.context_def(&[b]), asm.context_def(&[a]));
    // Flip the copy along red: still valid when exploded.
    act(&mut d, "flip.x");
    act(&mut d, "edit.explode");
    valid(&d);
    // The exploded copy's own component went with it.
    assert_eq!(d.s.component_rows().len(), 1);
}

#[test]
fn a_hidden_group_is_not_picked_snapped_to_or_selected() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    act(&mut d, "edit.hide");
    assert!(d.s.document().assembly().is_hidden(g));
    assert!(d.s.scene().faces.is_empty(), "nothing is built for it");
    // Not picked.
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 0.0, 1.0));
    assert!(d.s.selected_groups().is_empty());
    // Not snapped to.
    d.tool(LINE);
    d.hover(DVec3::new(2.0, 0.0, 2.0));
    assert_ne!(d.s.inference().map(|i| i.kind), Some(Kind::Endpoint));
    key(&mut d, Key::Escape);
    // Not taken by Select All.
    d.tool(SELECT);
    act(&mut d, "edit.selall");
    assert!(d.s.selected_groups().is_empty());
    // Unhide Last brings it back.
    act(&mut d, "edit.unhidelast");
    assert!(!d.s.document().assembly().is_hidden(g));
    assert!(!d.s.scene().faces.is_empty());
}

#[test]
fn isolate_hides_the_others_and_the_outliner_selects_inside_groups() {
    let mut d = Driver::empty();
    let a = grouped_box(&mut d);
    // A copy, and a group made of the copy (nested).
    d.tool(MOVE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.send(Input::CtrlTap);
    d.hover(DVec3::new(1.0, 0.0, 0.0));
    key(&mut d, Key::Right);
    d.typed("4");
    d.tool(SELECT);
    act(&mut d, "edit.desel");
    d.click(DVec3::new(5.0, 0.0, 1.0));
    let b = *d.s.selected_groups().iter().next().expect("the copy");
    act(&mut d, "edit.group");
    let outer =
        *d.s.selected_groups()
            .iter()
            .next()
            .expect("the outer group");
    // Isolate the outer group: the first box is hidden.
    act(&mut d, "edit.isolate");
    assert!(d.s.document().assembly().is_hidden(a));
    act(&mut d, "edit.unhideall");
    assert!(!d.s.document().assembly().is_hidden(a));
    // The Outliner lists all three, nested, and selects inside a group.
    let rows = d.s.outline();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().any(|r| r.path == [outer, b] && r.depth == 1));
    d.s.select_path(&[outer, b]);
    assert_eq!(d.s.open_path(), [outer], "its parent is opened");
    assert_eq!(
        d.s.selected_groups().iter().copied().collect::<Vec<_>>(),
        [b]
    );
    // Entity Info names it.
    d.s.name_instance(b, "Shed");
    match d.s.entity_info() {
        prism_tools::EntityInfo::Instance { name, .. } => assert_eq!(name, "Shed"),
        other => panic!("{other:?}"),
    }
}

// ---------- Model Health ----------

#[test]
fn model_health_finds_a_flipped_face_and_orient_fixes_it_with_undo() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("2");
    let count = |d: &Driver, key: &str| {
        d.s.health_rows()
            .into_iter()
            .find(|r| r.key == key)
            .map(|r| r.count)
            .expect("row")
    };
    assert_eq!(count(&d, "reversed"), 0);
    // Flip the top with Reverse Faces.
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 1.0, 2.0));
    act(&mut d, "edit.revface");
    assert_eq!(count(&d, "reversed"), 1);
    // The row selects it.
    act(&mut d, "edit.desel");
    d.s.select_health("reversed");
    assert_eq!(d.s.selection().faces.len(), 1);
    // Orient Faces fixes it, as one step that undo takes back.
    d.s.health_fix("reversed");
    assert_eq!(count(&d, "reversed"), 0);
    undo(&mut d);
    assert_eq!(count(&d, "reversed"), 1);
    let rows = d.s.health_rows();
    assert!(rows.iter().any(|r| r.key == "nesting"));
    assert!(
        rows.iter()
            .any(|r| r.key == "open" && r.detail.contains("1 watertight"))
    );
}

#[test]
fn model_health_draws_open_edges_and_reversed_faces_in_the_model() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("2");
    d.s.set_health_view(true);
    let style = prism_view::Style::default();
    let red = |d: &Driver| {
        d.s.overlay()
            .lines
            .iter()
            .filter(|l| l.color == style.health_open)
            .count()
    };
    assert_eq!(red(&d), 0, "a closed box has none");
    assert!(d.s.overlay().tris.is_empty());
    // Flip the top: it is tinted.
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 1.0, 2.0));
    act(&mut d, "edit.revface");
    assert!(
        d.s.overlay()
            .tris
            .iter()
            .any(|t| t.color == style.health_reversed),
        "the reversed face is tinted"
    );
    // Erase the top: four red edges, following the edit.
    act(&mut d, "edit.delete");
    assert_eq!(red(&d), 4);
    d.s.set_health_view(false);
    assert_eq!(red(&d), 0, "off when the panel closes");
}

// ---------- View Settings (part 1) ----------

fn pulled_box(d: &mut Driver) {
    rect_on_ground(d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("2");
    d.tool(SELECT);
    act(d, "edit.desel");
}

/// In Wireframe faces are not drawn and cannot be picked.
#[test]
fn wireframe_faces_cannot_be_picked() {
    let mut d = Driver::empty();
    pulled_box(&mut d);
    d.click(DVec3::new(1.0, 1.0, 2.0));
    assert_eq!(d.s.selection().faces.len(), 1, "a shaded face picks");
    act(&mut d, "edit.desel");
    d.s.set_view_style(prism_view::ViewStyle::preset("Wireframe").unwrap());
    d.click(DVec3::new(1.0, 1.0, 2.0));
    assert!(d.s.selection().is_empty(), "{:?}", d.s.selection());
}

/// In X-ray a corner behind the box can be snapped to.
#[test]
fn xray_snaps_to_a_corner_behind_faces() {
    let mut d = Driver::empty();
    pulled_box(&mut d);
    let eye = d.s.view().eye;
    let hidden = [0.0, 2.0]
        .iter()
        .flat_map(|x| [0.0, 2.0].map(|y| DVec3::new(*x, y, 0.0)))
        .find(|p| d.s.scene().occluded(eye, *p))
        .expect("a bottom corner hidden behind the box");
    d.tool(LINE);
    d.hover(hidden);
    let snapped = |d: &Driver| {
        d.s.inference().is_some_and(|i| {
            i.kind == prism_infer::Kind::Endpoint && i.point.distance(hidden) < 1e-9
        })
    };
    assert!(!snapped(&d), "hidden when shaded");
    act(&mut d, "view.xray");
    assert!(d.s.view_style().xray);
    d.hover(hidden + DVec3::new(0.01, 0.0, 0.0));
    d.hover(hidden);
    assert!(snapped(&d), "{:?}", d.s.inference());
}

/// View > Hidden Geometry shows a hidden group as a ghost that can be
/// clicked and unhidden; with it off the group cannot be picked.
#[test]
fn hidden_groups_show_as_ghosts_to_unhide() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    act(&mut d, "edit.hide");
    assert!(d.s.document().assembly().is_hidden(g));
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 0.0, 1.0));
    assert!(d.s.selected_groups().is_empty(), "hidden: nothing to pick");
    act(&mut d, "view.hidden");
    assert!(d.s.view_style().hidden);
    assert!(
        d.s.scene().edges.iter().any(|e| e.ghost),
        "drawn as a ghost"
    );
    // Edges only: no faces to draw, shade or cast shadows.
    assert!(d.s.scene().faces.is_empty() && d.s.scene().mesh.indices.is_empty());
    d.wait(1.0);
    // The face's middle is empty now; a click on an edge picks the ghost.
    d.click(DVec3::new(1.0, 0.0, 1.0));
    assert!(d.s.selected_groups().is_empty(), "nothing at the face");
    d.click(DVec3::new(1.0, 0.0, 2.0));
    assert_eq!(
        d.s.selected_groups().iter().copied().collect::<Vec<_>>(),
        [g]
    );
    act(&mut d, "edit.unhide");
    assert!(!d.s.document().assembly().is_hidden(g));
    assert!(d.s.scene().edges.iter().all(|e| !e.ghost));
}

/// The sun and Shadows are menu toggles on the view style.
#[test]
fn lighting_menu_items_switch_the_sun() {
    let mut d = Driver::empty();
    assert!(!d.s.view_style().sun);
    act(&mut d, "light.sun");
    assert!(d.s.view_style().sun);
    assert!(d.s.style.face_shadows, "the sun casts shadows at 1:30 pm");
    act(&mut d, "view.shadow");
    assert!(!d.s.style.face_shadows && !d.s.style.ground_shadow);
    act(&mut d, "light.default");
    assert!(!d.s.view_style().sun);
}

/// A 6 by 4 rectangle with a circle of radius 1 at (3, 2), and a line
/// across the rectangle through the circle's seams.
fn circle_in_a_rectangle() -> Driver {
    use prism_doc::Command;
    let mut doc = Document::new();
    doc.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 6.0,
        y1: 4.0,
        z: 0.0,
    });
    doc.run(Command::Circle {
        centre: DVec3::new(3.0, 2.0, 0.0),
        normal: DVec3::Z,
        radius: 1.0,
    });
    doc.run(Command::Segment {
        p: DVec3::new(3.0, 0.0, 0.0),
        q: DVec3::new(3.0, 4.0, 0.0),
    });
    let mut d = Driver::new(doc);
    d.look(Camera {
        eye: DVec3::new(3.0, -4.0, 9.0),
        target: DVec3::new(3.0, 2.0, 0.0),
    });
    d
}

#[test]
fn a_click_on_an_arc_cut_by_a_line_selects_that_arc_alone() {
    // The line across the circle parts it into two arcs, each picked on
    // its own, as in SketchUp (owner, Session 10).
    let mut d = circle_in_a_rectangle();
    assert_eq!(d.faces(), 4);
    d.tool(SELECT);
    d.click(DVec3::new(4.0, 2.0, 0.0));
    let sel = d.s.selection().clone();
    assert_eq!(sel.edges.len(), 1, "the half clicked");
    for e in &sel.edges {
        assert!(d.s.model().edge(*e).unwrap().curve.is_arc());
    }
    // A circle with nothing across it is still taken whole.
    let mut doc = Document::new();
    doc.run(prism_doc::Command::Circle {
        centre: DVec3::new(3.0, 2.0, 0.0),
        normal: DVec3::Z,
        radius: 1.0,
    });
    let mut d = Driver::new(doc);
    d.look(Camera {
        eye: DVec3::new(3.0, -4.0, 9.0),
        target: DVec3::new(3.0, 2.0, 0.0),
    });
    d.tool(SELECT);
    d.click(DVec3::new(4.0, 2.0, 0.0));
    assert_eq!(
        d.s.selection().edges.len(),
        2,
        "both halves of a whole circle"
    );
}

#[test]
fn the_eraser_takes_the_arc_clicked_and_the_faces_merge() {
    let mut d = circle_in_a_rectangle();
    d.tool(ERASE);
    d.click(DVec3::new(4.0, 2.0, 0.0));
    assert_eq!(d.faces(), 3, "the right half merges; the left half stays");
    assert_eq!(
        d.s.model()
            .edges()
            .filter(|(_, e)| e.curve.is_arc())
            .count(),
        1
    );
    let half = std::f64::consts::PI / 2.0;
    same(&areas(&d), &[half, 12.0 - half, 12.0]);
    valid(&d);
}

#[test]
fn model_health_repairs_an_arc_off_its_circle_with_undo() {
    let mut m = prism_topo::Model::new();
    prism_ops::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let seam = m
        .vertices()
        .find(|(_, p)| p.distance(DVec3::Y) < 1e-9)
        .map(|(id, _)| id)
        .expect("a seam at (0, 1)");
    m.set_vertex(seam, DVec3::new(0.0, 1.01, 0.0));
    let mut d = Driver::new(Document::with_model(m));
    let row = |d: &Driver| {
        d.s.health_rows()
            .into_iter()
            .find(|r| r.key == "arcs")
            .expect("row")
    };
    assert_eq!(row(&d).count, 2);
    assert_eq!(row(&d).fix, Some("Repair"));
    d.s.select_health("arcs");
    assert_eq!(d.s.selection().edges.len(), 2);
    d.s.health_fix("arcs");
    assert_eq!(row(&d).count, 0);
    assert!(d.s.model().vertex(seam).distance(DVec3::Y) < 1e-12);
    undo(&mut d);
    assert_eq!(row(&d).count, 2, "one step, undone");
}

#[test]
fn refused_edits_of_arcs_say_why() {
    use prism_ops::transform::{ARC_FACE_WOULD_FOLD, ARC_WOULD_BEND};
    let mut d = circle_in_a_rectangle();
    let before = d.s.model().clone();
    // A seam moved on its own would bend both halves of the circle.
    d.tool(SELECT);
    d.click(DVec3::new(3.0, 3.0, 0.0));
    assert_eq!(d.s.selection().vertices.len(), 1);
    d.tool(MOVE);
    d.click(DVec3::new(3.0, 3.0, 0.0));
    d.send(Input::Key { key: Key::Right });
    d.hover(DVec3::new(3.5, 3.0, 0.0));
    assert_eq!(d.s.hint(), ARC_WOULD_BEND);
    d.send(Input::Key { key: Key::Escape });
    assert_ne!(d.s.hint(), ARC_WOULD_BEND, "gone once the move ends");
    // Lifting a corner would fold the face around the circle.
    d.tool(SELECT);
    d.click(DVec3::new(6.0, 4.0, 0.0));
    d.tool(MOVE);
    d.click(DVec3::new(6.0, 4.0, 0.0));
    d.send(Input::Key { key: Key::Up });
    d.hover(DVec3::new(6.0, 4.0, 1.0));
    assert_eq!(d.s.hint(), ARC_FACE_WOULD_FOLD);
    d.send(Input::Key { key: Key::Escape });
    assert_eq!(d.s.model(), &before, "nothing changed");
    // The Eraser does not show the move's refusal while it erases
    // (Session 10).
    d.tool(ERASE);
    let p = d.screen(DVec3::new(3.0, 0.0, 0.0));
    d.send(Input::Move { x: p.x, y: p.y });
    d.send(Input::Press {
        button: Button::Left,
        x: p.x,
        y: p.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    assert_ne!(d.s.hint(), ARC_FACE_WOULD_FOLD);
    d.send(Input::Release {
        button: Button::Left,
    });
}

// ---------- Circle and Polygon (brief 23, step 2d) ----------

#[test]
fn the_circle_tool_draws_by_clicks_typing_and_a_double_click() {
    use prism_tools::session::CIRCLE;
    let mut d = Driver::empty();
    d.tool(CIRCLE);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.click(DVec3::new(2.0, 1.0, 0.0));
    assert_eq!(d.faces(), 1);
    same(&areas(&d), &[std::f64::consts::PI]);
    assert_eq!(
        d.s.model()
            .edges()
            .filter(|(_, e)| e.curve.is_arc())
            .count(),
        2
    );
    // A typed radius.
    d.click(DVec3::new(6.0, 1.0, 0.0));
    d.hover(DVec3::new(6.5, 1.0, 0.0));
    d.typed("1.5");
    same(
        &areas(&d),
        &[std::f64::consts::PI, std::f64::consts::PI * 2.25],
    );
    // A double-click places the last size again.
    d.click(DVec3::new(4.0, 5.0, 0.0));
    d.click(DVec3::new(4.0, 5.0, 0.0));
    assert_eq!(d.faces(), 3);
    valid(&d);
    undo(&mut d);
    assert_eq!(d.faces(), 2, "each circle is one step");
}

#[test]
fn the_polygon_tool_takes_a_side_count_and_its_sides_are_one_curve() {
    use prism_tools::session::POLYGON;
    let mut d = Driver::empty();
    d.tool(POLYGON);
    d.click(DVec3::new(2.0, 2.0, 0.0));
    d.click(DVec3::new(3.0, 2.0, 0.0));
    let m = d.s.model();
    assert_eq!(m.edge_count(), 6, "6 sides by default");
    let hex = 6.0 * 0.5 * (std::f64::consts::TAU / 6.0).sin();
    same(&areas(&d), &[hex]);
    // The first corner is where the radius was clicked (inscribed).
    assert!(prism_ops::vertex_at(m, DVec3::new(3.0, 2.0, 0.0)).is_some());
    // Eight sides, typed first (after choosing the tool again: typed
    // right after the hexagon, it would change the hexagon).
    d.tool(SELECT);
    d.tool(POLYGON);
    d.typed("8s");
    // Up picks the ground plane (the centre may land on a guide).
    d.send(Input::Key { key: Key::Up });
    d.click(DVec3::new(6.0, 2.0, 0.0));
    d.hover(DVec3::new(6.0, 3.0, 0.0));
    d.typed("1");
    assert_eq!(d.s.model().edge_count(), 14);
    let oct = 8.0 * 0.5 * (std::f64::consts::TAU / 8.0).sin();
    same(&areas(&d), &[hex, oct]);
    assert!(d.s.model().edges().all(|(_, e)| !e.curve.is_arc()));
    // A click on one side selects all of them.
    d.tool(SELECT);
    let pts =
        prism_tools::session::polygon_points(DVec3::new(6.0, 2.0, 0.0), DVec3::Z, DVec3::Y, 1.0, 8);
    d.click(pts[0].lerp(pts[1], 0.5));
    assert_eq!(d.s.selection().edges.len(), 8);
    valid(&d);
}

// ---------- The Arc tools (brief 24, step 3a) ----------

/// Each arc in the model: its centre, radius and sweep.
fn arc_list(d: &Driver) -> Vec<(DVec3, f64, f64)> {
    let m = d.s.model();
    m.edges()
        .filter_map(|(_, e)| match e.curve {
            prism_geom::Curve::Arc { centre, radius, .. } => {
                Some((centre, radius, e.curve.sweep(m.vertex(e.a), m.vertex(e.b))))
            }
            _ => None,
        })
        .collect()
}

fn has_arc(d: &Driver, centre: DVec3, radius: f64, sweep: f64) {
    let all = arc_list(d);
    assert!(
        all.iter().any(|(c, r, s)| c.distance(centre) < 1e-9
            && (r - radius).abs() < 1e-9
            && (s - sweep).abs() < 1e-9),
        "no arc about {centre} of radius {radius} and sweep {sweep}: {all:?}"
    );
}

/// Clicks with an arc tool on the ground (Up picks the ground plane).
fn arc_clicks(d: &mut Driver, tool: &str, pts: &[DVec3]) {
    d.tool(tool);
    d.send(Input::Key { key: Key::Up });
    for p in pts {
        d.click(*p);
    }
}

#[test]
fn the_two_point_arc_by_clicks_bulge_and_radius() {
    use prism_tools::session::ARC;
    use std::f64::consts::PI;
    let mut d = Driver::empty();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    // A bulge of half the chord: a half circle.
    arc_clicks(&mut d, ARC, &[p(0.0, 0.0), p(2.0, 0.0), p(1.0, 1.0)]);
    has_arc(&d, p(1.0, 0.0), 1.0, PI);
    // A typed bulge of 0.5 on a chord of 2: radius 1.25.
    arc_clicks(&mut d, ARC, &[p(4.0, 0.0), p(6.0, 0.0)]);
    d.hover(p(5.0, 0.3));
    d.typed("0.5");
    has_arc(&d, p(5.0, -0.75), 1.25, 2.0 * (0.8f64).asin());
    // A typed radius.
    arc_clicks(&mut d, ARC, &[p(8.0, 0.0), p(10.0, 0.0)]);
    d.hover(p(9.0, 0.2));
    d.typed("2r");
    let h = 2.0 - 3.0f64.sqrt();
    has_arc(&d, p(9.0, h - 2.0), 2.0, 2.0 * (0.5f64).asin());
    assert_eq!(arc_list(&d).len(), 3);
    valid(&d);
    undo(&mut d);
    assert_eq!(arc_list(&d).len(), 2, "each arc is one step");
}

#[test]
fn the_three_point_centre_and_pie_arcs() {
    use prism_tools::session::{ARC_CENTRE, ARC3, PIE};
    use std::f64::consts::PI;
    let mut d = Driver::empty();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    arc_clicks(&mut d, ARC3, &[p(0.0, 3.0), p(1.0, 4.0), p(2.0, 3.0)]);
    has_arc(&d, p(1.0, 3.0), 1.0, PI);
    // Centre, start, then a quarter turn round.
    arc_clicks(&mut d, ARC_CENTRE, &[p(5.0, 1.0), p(6.0, 1.0)]);
    d.hover(p(5.7, 1.7));
    d.click(p(5.0, 2.0));
    has_arc(&d, p(5.0, 1.0), 1.0, PI / 2.0);
    // Typed: the radius after the centre, then the angle.
    arc_clicks(&mut d, ARC_CENTRE, &[p(8.0, 1.0)]);
    d.hover(p(8.5, 1.0));
    d.typed("1.5");
    d.hover(p(8.5, 2.0));
    d.typed("60");
    has_arc(&d, p(8.0, 1.0), 1.5, PI / 3.0);
    assert_eq!(d.faces(), 0, "open arcs make no face");
    // A pie closes into a face.
    arc_clicks(&mut d, PIE, &[p(5.0, 5.0), p(6.0, 5.0)]);
    d.hover(p(5.7, 5.7));
    d.click(p(5.0, 6.0));
    assert_eq!(d.faces(), 1);
    same(&areas(&d), &[PI / 4.0]);
    valid(&d);
}

// ---------- Tangents (brief 24, step 3b) ----------

#[test]
fn a_line_snaps_tangent_off_an_arc_end() {
    use prism_tools::session::ARC_CENTRE;
    let mut d = Driver::empty();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    // An eighth of a turn about (3, 1), from (4, 1).
    arc_clicks(&mut d, ARC_CENTRE, &[p(3.0, 1.0), p(4.0, 1.0)]);
    d.hover(p(3.9, 1.4));
    d.typed("45");
    let r = 0.5f64.sqrt();
    let end = p(3.0 + r, 1.0 + r);
    let dir = DVec3::new(-1.0, 1.0, 0.0).normalize();
    d.tool(LINE);
    d.click(end);
    d.hover(end + dir * 1.0 + DVec3::new(0.02, 0.0, 0.0));
    assert_eq!(d.s.inference().map(|i| i.kind), Some(Kind::Tangent));
    d.click(end + dir * 1.0 + DVec3::new(0.02, 0.0, 0.0));
    let m = d.s.model();
    let line = m
        .edges()
        .find(|(_, e)| !e.curve.is_arc())
        .map(|(_, e)| m.vertex(e.b) - m.vertex(e.a))
        .expect("the line");
    assert!(line.normalize().cross(dir).length() < 1e-9, "{line:?}");
}

#[test]
fn a_two_point_arc_snaps_tangent_to_the_line_it_starts_from() {
    use prism_tools::session::ARC;
    use std::f64::consts::PI;
    let mut d = Driver::empty();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    d.tool(LINE);
    d.click(p(0.0, 1.0));
    d.click(p(2.0, 1.0));
    d.send(Input::Key { key: Key::Escape });
    // From the line's end to (3, 2): tangent is a quarter circle about (2, 2).
    arc_clicks(&mut d, ARC, &[p(2.0, 1.0), p(3.0, 2.0)]);
    let tangent_mid = p(2.0, 2.0) + DVec3::new(0.5f64.sqrt(), -(0.5f64.sqrt()), 0.0);
    d.hover(tangent_mid + DVec3::new(0.03, 0.03, 0.0));
    d.click(tangent_mid + DVec3::new(0.03, 0.03, 0.0));
    has_arc(&d, p(2.0, 2.0), 1.0, PI / 2.0);
    valid(&d);
}

#[test]
fn a_two_point_arc_snaps_tangent_to_the_line_it_ends_on() {
    use prism_tools::session::ARC;
    use std::f64::consts::PI;
    let mut d = Driver::empty();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    d.tool(LINE);
    d.click(p(0.0, 1.0));
    d.click(p(2.0, 1.0));
    d.send(Input::Key { key: Key::Escape });
    // The same quarter circle, drawn from (3, 2) to the line's end.
    arc_clicks(&mut d, ARC, &[p(3.0, 2.0), p(2.0, 1.0)]);
    let tangent_mid = p(2.0, 2.0) + DVec3::new(0.5f64.sqrt(), -(0.5f64.sqrt()), 0.0);
    d.hover(tangent_mid + DVec3::new(0.03, 0.03, 0.0));
    let labels: Vec<String> = d.s.overlay().labels.into_iter().map(|l| l.text).collect();
    assert!(labels.iter().any(|l| l == "Tangent"), "{labels:?}");
    d.click(tangent_mid + DVec3::new(0.03, 0.03, 0.0));
    has_arc(&d, p(2.0, 2.0), 1.0, PI / 2.0);
    valid(&d);
}

/// The owner's session 3 (2026-11-17): arcs from a rectangle's corner
/// into it, and arcs ending partway along an edge, got no tangent guide.
/// An arc end on an edge now offers the arc turning back along the edge
/// as well as the one carrying on past it.
#[test]
fn a_two_point_arc_snaps_tangent_back_along_the_line_it_starts_from() {
    use prism_tools::session::ARC;
    use std::f64::consts::PI;
    let mut d = Driver::empty();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    d.tool(LINE);
    d.click(p(2.0, 0.0));
    d.click(p(2.0, 2.0));
    d.send(Input::Key { key: Key::Escape });
    // From the line's top end down and round to (0, 0): a quarter circle
    // about (0, 2), leaving straight down along the line.
    arc_clicks(&mut d, ARC, &[p(2.0, 2.0), p(0.0, 0.0)]);
    let mid = p(0.0, 2.0) + DVec3::new(0.5f64.sqrt(), -(0.5f64.sqrt()), 0.0) * 2.0;
    d.hover(mid + DVec3::new(0.03, 0.03, 0.0));
    let labels: Vec<String> = d.s.overlay().labels.into_iter().map(|l| l.text).collect();
    assert!(labels.iter().any(|l| l == "Tangent"), "{labels:?}");
    d.click(mid + DVec3::new(0.03, 0.03, 0.0));
    has_arc(&d, p(0.0, 2.0), 2.0, PI / 2.0);
    valid(&d);
}

#[test]
fn a_two_point_arc_snaps_tangent_to_a_line_it_ends_partway_along() {
    use prism_tools::session::ARC;
    let mut d = Driver::empty();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    d.tool(LINE);
    d.click(p(0.0, 0.0));
    d.click(p(4.0, 0.0));
    d.send(Input::Key { key: Key::Escape });
    // From (1, 2) down to (2, 0) on the line, touching it there: the
    // circle about (2, 1.25) of radius 1.25.
    arc_clicks(&mut d, ARC, &[p(1.0, 2.0), p(2.0, 0.0)]);
    let c = p(2.0, 1.25);
    let start = (0.75f64).atan2(-1.0);
    let end = 1.5 * std::f64::consts::PI;
    let mid_angle = 0.5 * (start + end);
    let mid = c + DVec3::new(mid_angle.cos(), mid_angle.sin(), 0.0) * 1.25;
    d.hover(mid + DVec3::new(0.02, 0.02, 0.0));
    let labels: Vec<String> = d.s.overlay().labels.into_iter().map(|l| l.text).collect();
    assert!(labels.iter().any(|l| l == "Tangent"), "{labels:?}");
    d.click(mid + DVec3::new(0.02, 0.02, 0.0));
    has_arc(&d, c, 1.25, end - start);
    valid(&d);
}

// ---------- Offset with arcs (brief 24, step 3c) ----------

#[test]
fn offset_of_a_disc_by_a_click_and_by_typing() {
    use prism_tools::session::{CIRCLE, OFFSET};
    let mut d = Driver::empty();
    d.tool(CIRCLE);
    d.send(Input::Key { key: Key::Up });
    d.click(DVec3::new(3.0, 2.0, 0.0));
    d.click(DVec3::new(5.0, 2.0, 0.0));
    assert_eq!(d.faces(), 1);
    // Click on the disc, then 0.5 towards its middle: 0.5 in.
    d.tool(OFFSET);
    d.click(DVec3::new(3.0, 3.5, 0.0));
    d.click(DVec3::new(3.0, 3.0, 0.0));
    assert_eq!(d.faces(), 2);
    has_arc(&d, DVec3::new(3.0, 2.0, 0.0), 1.5, std::f64::consts::PI);
    undo(&mut d);
    // Typed: 0.25 in (the way the cursor went).
    d.click(DVec3::new(3.0, 3.2, 0.0));
    d.hover(DVec3::new(3.0, 3.0, 0.0));
    d.typed("0.25");
    assert_eq!(d.faces(), 2);
    has_arc(&d, DVec3::new(3.0, 2.0, 0.0), 1.75, std::f64::consts::PI);
    valid(&d);
}

// ---------- Corner fillets (brief 24, step 3d) ----------

#[test]
fn a_double_click_on_a_corner_rounds_it_and_a_radius_retypes_it() {
    use prism_tools::session::ARC;
    use std::f64::consts::PI;
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 2.0);
    d.tool(ARC);
    // No radius typed: a tenth of the shorter edge (0.2).
    double_click(&mut d, DVec3::new(4.0, 2.0, 0.0));
    has_arc(&d, DVec3::new(3.8, 1.8, 0.0), 0.2, PI / 2.0);
    let corner = |r: f64| 8.0 - (r * r - PI * r * r / 4.0);
    same(&areas(&d), &[corner(0.2)]);
    // Retyped right after: 0.5, still one step.
    d.typed("0.5r");
    has_arc(&d, DVec3::new(3.5, 1.5, 0.0), 0.5, PI / 2.0);
    same(&areas(&d), &[corner(0.5)]);
    assert_eq!(arc_list(&d).len(), 1);
    // The next corner takes the typed radius.
    double_click(&mut d, DVec3::new(0.0, 2.0, 0.0));
    has_arc(&d, DVec3::new(0.5, 1.5, 0.0), 0.5, PI / 2.0);
    valid(&d);
    undo(&mut d);
    undo(&mut d);
    same(&areas(&d), &[8.0]);
    // Too big a radius is refused, and says so.
    d.typed("3r");
    double_click(&mut d, DVec3::new(4.0, 0.0, 0.0));
    assert_eq!(d.s.hint(), prism_ops::fillet::TOO_SHORT);
    same(&areas(&d), &[8.0]);
}

// ---------- Cylinders from Push/Pull (brief 25, step 4b) ----------

/// The faces of the model on a cylinder.
fn curved_faces(d: &Driver) -> Vec<prism_topo::FaceId> {
    d.s.model()
        .faces()
        .filter(|(_, f)| f.is_curved())
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn push_pull_makes_a_cylinder_that_picks_and_snaps() {
    use prism_tools::session::CIRCLE;
    let mut d = Driver::empty();
    // Seen from the front (`look` starts a new session, so first).
    d.look(Camera {
        eye: DVec3::new(0.5, -6.0, 3.0),
        target: DVec3::new(0.0, 0.0, 1.0),
    });
    d.tool(CIRCLE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(1.0, 0.0, 0.0));
    d.tool(PUSH);
    d.click(DVec3::new(0.2, 0.2, 0.0));
    d.hover(DVec3::new(0.2, 0.2, 0.5));
    d.typed("2");
    assert_eq!(d.faces(), 4);
    assert_eq!(curved_faces(&d).len(), 2);
    valid(&d);
    let (lo, hi) = d.s.model().bbox().expect("a cylinder");
    assert!((hi.z - lo.z - 2.0).abs() < 1e-9, "{lo} {hi}");
    // Its side: snapped on, and picked.
    d.tool(LINE);
    d.hover(DVec3::new(0.6, -0.8, 1.2));
    let i = d.s.inference().expect("an inference").clone();
    assert_eq!(i.kind, Kind::OnFace);
    assert!(curved_faces(&d).contains(&i.face.expect("a face")));
    assert!(
        (i.point.truncate().length() - 1.0).abs() < 1e-9,
        "on it: {}",
        i.point
    );
    d.tool(SELECT);
    d.click(DVec3::new(0.6, -0.8, 1.2));
    // The whole smooth side (step 4c).
    let picked: Vec<_> = d.s.selection().faces.iter().copied().collect();
    assert_eq!(picked, curved_faces(&d));
    // The cap pulled again lengthens it.
    d.tool(PUSH);
    d.click(DVec3::new(0.2, 0.2, 2.0));
    d.hover(DVec3::new(0.2, 0.2, 2.5));
    d.typed("1");
    assert_eq!(d.faces(), 4, "the same faces, longer");
    let (lo, hi) = d.s.model().bbox().expect("a cylinder");
    assert!((hi.z - lo.z - 3.0).abs() < 1e-9, "{lo} {hi}");
    valid(&d);
    // One undo each.
    undo(&mut d);
    assert_eq!(d.faces(), 4);
    undo(&mut d);
    assert_eq!(d.faces(), 1);
}

#[test]
fn push_pull_on_a_cylinders_side_gives_it_a_new_radius() {
    use prism_tools::session::CIRCLE;
    let mut d = Driver::empty();
    d.look(Camera {
        eye: DVec3::new(0.5, -6.0, 3.0),
        target: DVec3::new(0.0, 0.0, 1.0),
    });
    d.tool(CIRCLE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(1.0, 0.0, 0.0));
    d.tool(PUSH);
    d.click(DVec3::new(0.2, 0.2, 0.0));
    d.hover(DVec3::new(0.2, 0.2, 0.5));
    d.typed("2");
    // The side, pulled out from the axis by clicks...
    d.click(DVec3::new(0.6, -0.8, 1.2));
    d.hover(DVec3::new(0.9, -1.2, 1.2));
    d.click(DVec3::new(0.9, -1.2, 1.2));
    let r = |d: &Driver| -> Vec<f64> {
        d.s.model()
            .faces()
            .filter_map(|(_, f)| match f.surface.clone() {
                prism_geom::Surface::Cylinder(c) => Some(c.radius),
                _ => None,
            })
            .collect()
    };
    assert_eq!(d.faces(), 4, "the same faces");
    assert!(r(&d).iter().all(|x| (x - 1.5).abs() < 1e-6), "{:?}", r(&d));
    valid(&d);
    // ...and back in by typing.
    d.click(DVec3::new(0.9, -1.2, 1.2));
    d.hover(DVec3::new(0.6, -0.8, 1.2));
    d.typed("1");
    assert!(r(&d).iter().all(|x| (x - 0.5).abs() < 1e-9), "{:?}", r(&d));
    valid(&d);
    // A radius of zero or less is refused.
    d.click(DVec3::new(0.3, -0.4, 1.2));
    d.hover(DVec3::new(0.15, -0.2, 1.2));
    d.typed("1");
    assert!(r(&d).iter().all(|x| (x - 0.5).abs() < 1e-9), "{:?}", r(&d));
    undo(&mut d);
    assert!(r(&d).iter().all(|x| (x - 1.5).abs() < 1e-6));
}

// ---------- Soften and smooth (brief 25, step 4c) ----------

fn soft_count(d: &Driver) -> usize {
    d.s.model().edges().filter(|(_, e)| e.soft).count()
}

fn action(d: &mut Driver, id: &str) {
    d.send(Input::Action { id: id.into() });
}

#[test]
fn a_cylinders_seams_are_hidden_and_its_side_picks_whole() {
    let mut doc = Document::new();
    doc.run(prism_doc::Command::Circle {
        centre: DVec3::ZERO,
        normal: DVec3::Z,
        radius: 1.0,
    });
    let f = doc.model().faces().next().map(|(id, _)| id).unwrap();
    doc.run(prism_doc::Command::PushPull {
        face: f,
        distance: 2.0,
        new_loop: false,
    });
    let mut d = Driver::new(doc);
    d.look(Camera {
        eye: DVec3::new(0.5, -6.0, 3.0),
        target: DVec3::new(0.0, 0.0, 1.0),
    });
    assert_eq!(soft_count(&d), 2);
    assert!(d.s.scene().edges.iter().all(|e| !e.soft), "not drawn");
    d.tool(SELECT);
    d.click(DVec3::new(0.6, -0.8, 1.2));
    assert_eq!(d.s.selection().faces.len(), 2, "the whole side");
    // View > Hidden Geometry shows them as ghosts.
    action(&mut d, "view.hidden");
    let ghosts: Vec<_> = d.s.scene().edges.iter().filter(|e| e.soft).collect();
    assert_eq!(ghosts.len(), 2);
    assert!(ghosts.iter().all(|e| e.ghost));
}

#[test]
fn the_eraser_with_ctrl_softens_and_with_shift_unsoftens() {
    use prism_tools::session::ERASE;
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.5));
    d.typed("1");
    let faces = d.faces();
    // An upright corner edge of the box.
    let corner = DVec3::new(2.0, 0.0, 0.5);
    d.tool(ERASE);
    let s = d.screen(corner);
    d.click_px(s, false, true);
    assert_eq!(soft_count(&d), 1, "softened");
    assert_eq!(d.faces(), faces, "nothing erased");
    valid(&d);
    assert!(
        d.s.hint().contains("Ctrl softens"),
        "the hint says so: {}",
        d.s.hint()
    );
    // Hidden now; with Hidden Geometry shown, Ctrl+Shift makes it plain.
    action(&mut d, "view.hidden");
    d.click_px(s, true, true);
    assert_eq!(soft_count(&d), 0, "unsoftened");
    assert_eq!(d.faces(), faces);
    undo(&mut d);
    assert_eq!(soft_count(&d), 1, "one undo each");
}

#[test]
fn edit_soften_toggles_the_selected_edges() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.5));
    d.typed("1");
    d.tool(SELECT);
    d.click(DVec3::new(2.0, 0.0, 0.5));
    assert_eq!(d.s.selection().edges.len(), 1);
    action(&mut d, "edit.soften");
    assert_eq!(soft_count(&d), 1);
    assert!(
        d.s.model()
            .edges()
            .filter(|(_, e)| e.soft)
            .all(|(_, e)| e.smooth),
        "smoothed too"
    );
    action(&mut d, "edit.soften");
    assert_eq!(soft_count(&d), 0, "all soft: made plain again");
}

// ---------- Fillet / Chamfer (brief 26, step 5a) ----------

fn radii(d: &Driver) -> Vec<f64> {
    d.s.model()
        .faces()
        .filter_map(|(_, f)| match f.surface.clone() {
            prism_geom::Surface::Cylinder(c) => Some(c.radius),
            _ => None,
        })
        .collect()
}

#[test]
fn the_fillet_tool_rounds_an_edge_and_ctrl_chamfers_one() {
    use prism_tools::session::FILLET;
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.5));
    d.typed("2");
    let faces = d.faces();
    d.tool(FILLET);
    assert!(d.s.hint().contains("Ctrl swaps"), "{}", d.s.hint());
    // An upright corner edge, at its middle: collected, then Enter rounds
    // it with a tenth of the face's width.
    d.click(DVec3::new(2.0, 0.0, 1.0));
    assert_eq!(d.faces(), faces, "a click only collects");
    d.send(Input::Key { key: Key::Enter });
    assert_eq!(d.faces(), faces + 1);
    assert_eq!(radii(&d).len(), 1);
    assert!((radii(&d)[0] - 0.2).abs() < 1e-9, "{:?}", radii(&d));
    valid(&d);
    // A typed size changes it.
    d.typed("0.5");
    assert!((radii(&d)[0] - 0.5).abs() < 1e-9, "{:?}", radii(&d));
    assert_eq!(d.faces(), faces + 1);
    valid(&d);
    // Ctrl held at Enter cuts another edge off flat, with the same size.
    let s = d.screen(DVec3::new(0.0, 0.0, 1.0));
    d.click_px(s, false, false);
    d.send(Input::Held {
        ctrl: true,
        alt: false,
    });
    d.send(Input::Key { key: Key::Enter });
    d.send(Input::Held {
        ctrl: false,
        alt: false,
    });
    assert_eq!(d.faces(), faces + 2);
    assert_eq!(radii(&d).len(), 1, "flat");
    valid(&d);
    // Too big for the faces: refused, and the hint says why.
    d.typed("5");
    assert!(d.s.hint().contains("too large"), "{}", d.s.hint());
    undo(&mut d);
    undo(&mut d);
    assert_eq!(d.faces(), faces, "one undo each");
}

/// The Fillet window (brief 29): hovering an edge previews the round at
/// the window's kind and size without changing the model; Ctrl and the
/// window's kind swap it; a refused edge says why and shows nothing; the
/// window's size and kind change the last round in one undo step; leaving
/// the tool takes the preview away.
#[test]
fn the_fillet_window_previews_and_changes_the_last_round() {
    use prism_tools::session::{FILLET, RoundOp};
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.5));
    d.typed("2");
    let faces = d.faces();
    d.tool(FILLET);
    let panel = d.s.round_panel().expect("shown with the tool");
    assert!(!panel.chamfer && panel.size.is_empty() && !panel.changes_last);
    let shown = |d: &Driver| d.s.display_scene().faces.len();
    let real = shown(&d);
    // Hovering an upright edge: the preview has the round, the model not.
    d.hover(DVec3::new(2.0, 0.0, 1.0));
    assert_eq!(d.faces(), faces, "the model is unchanged");
    assert_eq!(shown(&d), real + 1, "the preview has one more face");
    let curved = |d: &Driver| {
        d.s.display_scene()
            .faces
            .iter()
            .filter(|f| f.curved.is_some())
            .count()
    };
    assert_eq!(curved(&d), 1, "rounded");
    assert!(
        d.s.round_panel().unwrap().fits.is_some(),
        "the size that fits"
    );
    // Ctrl held: the preview is a chamfer.
    d.send(Input::Held {
        ctrl: true,
        alt: false,
    });
    assert_eq!(curved(&d), 0, "flat while Ctrl is held");
    assert_eq!(d.s.measurement().0, "Distance");
    d.send(Input::Held {
        ctrl: false,
        alt: false,
    });
    assert_eq!(curved(&d), 1);
    // The window's kind: chamfer.
    d.s.round_input(RoundOp::Fillet(false));
    assert_eq!(curved(&d), 0, "the window chose chamfer");
    d.s.round_input(RoundOp::Fillet(true));
    // Too big a size: nothing previewed, and the reason shows first.
    d.s.round_text("5");
    assert_eq!(shown(&d), real, "no preview");
    assert_eq!(d.s.hint(), prism_ops::round::TOO_WIDE);
    assert_eq!(
        d.s.round_panel().unwrap().refused,
        Some(prism_ops::round::TOO_WIDE)
    );
    // Back to fitting each edge, then click and Enter.
    d.s.round_text("");
    assert_eq!(shown(&d), real + 1);
    d.click(DVec3::new(2.0, 0.0, 1.0));
    d.send(Input::Key { key: Key::Enter });
    assert_eq!(d.faces(), faces + 1);
    assert!((radii(&d)[0] - 0.2).abs() < 1e-9, "{:?}", radii(&d));
    assert!(d.s.round_panel().unwrap().changes_last);
    // The window's size and kind change it in place.
    d.s.round_text("0.5");
    assert!((radii(&d)[0] - 0.5).abs() < 1e-9, "{:?}", radii(&d));
    assert_eq!(d.s.round_panel().unwrap().size, d.s.number.show(0.5));
    d.s.round_input(RoundOp::Fillet(false));
    assert!(radii(&d).is_empty(), "now a chamfer");
    assert_eq!(d.faces(), faces + 1);
    valid(&d);
    undo(&mut d);
    assert_eq!(d.faces(), faces, "one undo step");
    // Hover again, then leave the tool: no preview left behind.
    d.hover(DVec3::new(0.0, 0.0, 1.0));
    assert_eq!(shown(&d), real + 1);
    d.tool(PUSH);
    assert_eq!(shown(&d), real);
    assert!(d.s.round_panel().is_none(), "only with the tool");
}

#[test]
fn a_cube_has_a_box_with_grips_all_round() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.5));
    d.typed("2");
    // A click with Scale picks the whole cube (what is connected).
    d.tool(SCALE);
    d.click(DVec3::new(1.0, 1.0, 2.0));
    assert_eq!(d.s.selection().faces.len(), 6);
    // The top face's grip, 5% above the cube: scales up only.
    d.click(DVec3::new(1.0, 1.0, 2.1));
    d.typed("1.5");
    let (lo, hi) = d.s.model().bbox().expect("the cube");
    assert!((hi.z - 3.0).abs() < 1e-9 && lo.z.abs() < 1e-9, "{lo} {hi}");
    assert!((hi.x - 2.0).abs() < 1e-9);
    valid(&d);
}

/// Follow Me with Alt (brief 26, step 5d): the selected face's edge is the
/// path.
#[test]
fn follow_me_with_alt_sweeps_round_the_selected_face() {
    use prism_tools::session::FOLLOW;
    let mut m = prism_topo::Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    // A square standing at the ground's corner, facing across it.
    let u = DVec3::new(1.0, -1.0, 0.0).normalize() * 0.5;
    let c = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(a, b)| DVec3::new(0.0, 0.0, 1.5) + u * a + DVec3::Z * 0.5 * b);
    for i in 0..4 {
        prism_ops::add_segment(&mut m, c[i], c[(i + 1) % 4]);
    }
    let mut d = Driver::new(Document::with_model(m));
    d.look(Camera {
        eye: DVec3::new(6.0, -8.0, 9.0),
        target: DVec3::new(2.0, 1.0, 1.0),
    });
    d.tool(SELECT);
    d.click(DVec3::new(2.0, 2.0, 0.0));
    assert_eq!(d.s.selection().faces.len(), 1);
    assert!(d.s.selection().edges.is_empty());
    d.tool(FOLLOW);
    // Without Alt a selected face is not a path: it says to drag.
    assert!(d.s.hint().contains("drag along"), "{}", d.s.hint());
    d.send(Input::AltTap);
    assert!(d.s.hint().contains("face's edge"), "{}", d.s.hint());
    d.click(DVec3::new(0.0, 0.0, 1.5));
    valid(&d);
    // Four walls a side round the ground; the profile went.
    assert_eq!(d.faces(), 1 + 4 * 4);
    undo(&mut d);
    assert_eq!(d.faces(), 2, "one undo");
}

/// Follow Me (brief 26, step 5c): select a path, then click the profile.
#[test]
fn follow_me_sweeps_the_clicked_profile_along_the_selected_path() {
    use prism_tools::session::FOLLOW;
    let mut m = prism_topo::Model::new();
    let c = [(-0.5, 1.5), (0.5, 1.5), (0.5, 2.5), (-0.5, 2.5)].map(|(y, z)| DVec3::new(0.0, y, z));
    for i in 0..4 {
        prism_ops::add_segment(&mut m, c[i], c[(i + 1) % 4]);
    }
    prism_ops::add_segment(&mut m, DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0));
    prism_ops::add_segment(&mut m, DVec3::new(4.0, 0.0, 0.0), DVec3::new(4.0, 3.0, 0.0));
    let mut d = Driver::new(Document::with_model(m));
    d.look(Camera {
        eye: DVec3::new(6.0, -8.0, 9.0),
        target: DVec3::new(2.0, 1.0, 1.0),
    });
    // With nothing selected it says to drag along edges (or select a
    // path); a click on the face starts that, and makes nothing yet.
    d.tool(FOLLOW);
    assert!(d.s.hint().contains("drag along"), "{}", d.s.hint());
    d.click(DVec3::new(0.0, 0.0, 2.0));
    assert_eq!(d.faces(), 1, "nothing made");
    assert!(d.s.in_operation(), "the path is being dragged out");
    key(&mut d, Key::Escape);
    // Select the two lines, then click the profile.
    d.tool(SELECT);
    let a = d.screen(DVec3::new(2.0, 0.0, 0.0));
    d.click_px(a, false, false);
    let b = d.screen(DVec3::new(4.0, 1.5, 0.0));
    d.click_px(b, true, false);
    assert_eq!(d.s.selection().edges.len(), 2);
    d.tool(FOLLOW);
    assert!(d.s.hint().contains("profile"), "{}", d.s.hint());
    d.click(DVec3::new(0.0, 0.0, 2.0));
    assert_eq!(d.faces(), 2 + 4 * 2, "two caps, four walls a line");
    valid(&d);
    let v: f64 =
        d.s.model()
            .faces()
            .map(|(id, _)| prism_ops::health::face_volume(d.s.model(), id))
            .sum();
    assert!((v - 7.0).abs() < 1e-9, "{v}");
    undo(&mut d);
    assert_eq!(d.faces(), 1, "one undo");
}

/// Owner, 2026-11-11: hovering a cylinder's side lit only half of it, and
/// its rim only half a circle. The hover takes what a click would select.
#[test]
fn hovering_a_cylinder_takes_its_whole_side_and_whole_rim() {
    let mut m = prism_topo::Model::new();
    prism_ops::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let f = m.faces().next().map(|(id, _)| id).expect("disc");
    let up = m
        .face(f)
        .and_then(|x| x.plane())
        .map_or(1.0, |p| p.normal.z);
    prism_ops::push_pull(&mut m, f, 2.0 * up, false);
    let mut d = Driver::new(Document::with_model(m));
    d.look(Camera {
        eye: DVec3::new(0.0, -6.0, 4.0),
        target: DVec3::new(0.0, 0.0, 1.0),
    });
    d.tool("tool.select");
    d.hover(DVec3::new(0.3, -0.95, 1.0));
    assert!(matches!(d.s.hover(), Some(prism_view::Hit::Face(_))));
    assert_eq!(d.s.hover_more().faces.len(), 2, "both halves of the side");
    d.hover(DVec3::new(0.6, -0.8, 2.0));
    assert!(matches!(d.s.hover(), Some(prism_view::Hit::Edge(_))));
    assert_eq!(d.s.hover_more().edges.len(), 2, "both halves of the rim");
}

/// Owner, 2026-11-11: Push/Pull's new loop came on unasked and stayed on,
/// unseen, so pulls left their original faces behind. It is for one pull
/// now, the hint says when it is on, and choosing a tool turns it off.
#[test]
fn a_new_loop_is_shown_and_lasts_one_pull() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.send(Input::AltTap);
    assert!(d.s.hint().contains("NEW LOOP"), "{}", d.s.hint());
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.5));
    d.typed("1");
    assert!(!d.s.hint().contains("NEW LOOP"), "off after the pull");
    // A pull after it is a plain one: no face left behind.
    let faces = d.faces();
    d.click(DVec3::new(1.0, 1.0, 1.0));
    d.hover(DVec3::new(1.0, 1.0, 1.5));
    d.typed("0.5");
    assert_eq!(d.faces(), faces, "the top moved, nothing left behind");
    // Choosing a tool turns it off too.
    d.send(Input::AltTap);
    d.tool("tool.select");
    d.tool(PUSH);
    assert!(!d.s.hint().contains("NEW LOOP"));
}

// ---------- the Paint Bucket (Basic Materials 1b) ----------

use prism_tools::session::{Brush, PAINT, PUSH as PAINT_PUSH};

/// A 2 m box at the origin (not grouped).
fn paint_box(d: &mut Driver) {
    rect_on_ground(d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PAINT_PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("2");
}

fn face_at(d: &Driver, p: DVec3) -> prism_topo::FaceId {
    prism_ops::face_at(d.s.model(), p).expect("a face there")
}

fn paint_of(d: &Driver, p: DVec3) -> prism_topo::Paint {
    d.s.model().face(face_at(d, p)).expect("face").paint
}

fn press_with(d: &mut Driver, p: DVec3, shift: bool, ctrl: bool, alt: bool) {
    let s = d.screen(p);
    d.send(Input::Move { x: s.x, y: s.y });
    d.send(Input::Press {
        button: Button::Left,
        x: s.x,
        y: s.y,
        shift,
        ctrl,
        alt,
    });
    d.send(Input::Release {
        button: Button::Left,
    });
    d.t += 0.5;
}

#[test]
fn the_paint_bucket_paints_the_side_clicked_and_undo_takes_it_back() {
    let mut d = Driver::empty();
    paint_box(&mut d);
    d.tool(PAINT);
    let top = DVec3::new(1.0, 1.0, 2.0);
    // Hovering previews the paint on the face, in the brush's colour.
    d.hover(top);
    let (faces, colour) = d.s.paint_preview().expect("a preview");
    assert_eq!(faces, [face_at(&d, top)]);
    assert_eq!(colour, prism_view::style::Rgba::rgb(0xb0, 0x4a, 0x3a));
    d.click(top);
    let mats = d.s.document().assembly().materials().0.clone();
    assert_eq!(mats.len(), 1, "the palette colour went into the document");
    let (id, m) = mats.iter().next().expect("one");
    assert_eq!(m.name, "Brick Red");
    assert_eq!(paint_of(&d, top).front, Some(*id), "the outside, seen");
    assert_eq!(paint_of(&d, top).back, None);
    assert_eq!(d.s.brush(), Brush::Material(*id));
    assert_eq!(d.s.document().undo_name(), Some("Paint"), "one step");
    // The drawn scene has its colour.
    let f = face_at(&d, top);
    assert_eq!(d.s.scene().material_on(f, true), Some(*id));
    assert!(d.s.scene().palette.contains_key(id));
    // Undo takes the paint and the new material away; painting again
    // brings the material back.
    act(&mut d, "edit.undo");
    assert!(paint_of(&d, top).is_default());
    assert!(d.s.document().assembly().materials().0.is_empty());
    d.click(top);
    let mats = d.s.document().assembly().materials().0.clone();
    assert_eq!(mats.len(), 1);
    assert_eq!(paint_of(&d, top).front, mats.keys().next().copied());
    valid(&d);
}

/// A rectangle on the ground faces down: seen from above, its back is
/// painted.
#[test]
fn the_paint_bucket_paints_the_back_when_the_back_is_seen() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    let p = DVec3::new(1.0, 1.0, 0.0);
    let f = face_at(&d, p);
    let up =
        d.s.model()
            .face(f)
            .and_then(|x| x.plane().map(|pl| pl.normal.z > 0.0));
    d.tool(PAINT);
    d.click(p);
    let paint = paint_of(&d, p);
    match up {
        Some(true) => assert!(paint.front.is_some() && paint.back.is_none()),
        _ => assert!(paint.back.is_some() && paint.front.is_none()),
    }
}

#[test]
fn ctrl_fills_connected_and_shift_replaces_everywhere() {
    let mut d = Driver::empty();
    paint_box(&mut d);
    d.tool(PAINT);
    let top = DVec3::new(1.0, 1.0, 2.0);
    let side = DVec3::new(1.0, 0.0, 1.0);
    // Ctrl: every Default face joined to the top.
    press_with(&mut d, top, false, true, false);
    let red = paint_of(&d, top).front.expect("painted");
    assert!(
        d.s.model().faces().all(|(_, f)| f.paint.front == Some(red)),
        "all six outsides"
    );
    // A plain click on a side with another colour.
    d.s.set_brush(Brush::Palette(7));
    d.click(side);
    let sky = paint_of(&d, side).front.expect("painted");
    assert_ne!(sky, red);
    // Ctrl on the top: the red faces joined to it, not the sky side.
    d.s.set_brush(Brush::Palette(4));
    press_with(&mut d, top, false, true, false);
    let olive = paint_of(&d, top).front.expect("painted");
    let count = |d: &Driver, m| {
        d.s.model()
            .faces()
            .filter(|(_, f)| f.paint.front == Some(m))
            .count()
    };
    assert_eq!((count(&d, olive), count(&d, sky)), (5, 1));
    // Shift on the sky side: sky replaced everywhere (one face here).
    d.s.set_brush(Brush::Material(olive));
    press_with(&mut d, side, true, false, false);
    assert_eq!(count(&d, olive), 6);
    valid(&d);
}

#[test]
fn alt_samples_held_or_tapped() {
    let mut d = Driver::empty();
    paint_box(&mut d);
    d.tool(PAINT);
    let top = DVec3::new(1.0, 1.0, 2.0);
    let side = DVec3::new(1.0, 0.0, 1.0);
    d.click(top);
    let red = paint_of(&d, top).front.expect("painted");
    // Held: Alt+click picks up the side's Default.
    press_with(&mut d, side, false, false, true);
    assert_eq!(d.s.brush(), Brush::Default);
    assert!(paint_of(&d, side).is_default(), "sampling paints nothing");
    // Tapped: the next click samples, then the bucket paints again.
    d.send(Input::AltTap);
    assert!(d.s.paint_sampling());
    assert!(d.s.hint().contains("SAMPLE"));
    d.hover(top);
    assert!(d.s.paint_preview().is_none(), "no preview while sampling");
    d.click(top);
    assert_eq!(d.s.brush(), Brush::Material(red));
    assert!(!d.s.paint_sampling());
    d.click(side);
    assert_eq!(paint_of(&d, side).front, Some(red));
}

/// A closed group is painted as a whole: its Default faces show the
/// material, its own faces are untouched, and the nearest painted group
/// wins.
#[test]
fn a_group_is_painted_as_a_whole_and_the_nearest_wins() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    act(&mut d, "edit.desel");
    // Group the group: an outer group holding it.
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 0.0, 1.0));
    act(&mut d, "edit.group");
    let outer = *d.s.selected_groups().iter().next().expect("outer");
    assert_ne!(outer, g);
    act(&mut d, "edit.desel");
    d.tool(PAINT);
    let side = DVec3::new(1.0, 0.0, 1.0);
    d.click(side);
    let asm = d.s.document().assembly();
    let red = asm.painted_with(outer).expect("the outer group is painted");
    assert_eq!(asm.painted_with(g), None);
    let inner_def = asm.context_def(&[outer, g]).expect("inner");
    assert!(
        asm.def(inner_def)
            .expect("def")
            .model
            .faces()
            .all(|(_, f)| f.paint.is_default()),
        "nothing is copied onto faces"
    );
    let shown = |d: &Driver| {
        let sc = d.s.scene();
        sc.faces
            .iter()
            .map(|f| sc.material_on(f.id, true))
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(shown(&d), [Some(red)].into());
    // Inside the outer group, the inner one painted blue: blue wins there.
    d.tool(SELECT);
    double_click(&mut d, side);
    assert_eq!(d.s.open_path(), [outer]);
    d.tool(PAINT);
    d.s.set_brush(Brush::Palette(8));
    d.click(side);
    let blue =
        d.s.document()
            .assembly()
            .painted_with(g)
            .expect("the inner group is painted");
    assert_eq!(shown(&d), [Some(blue)].into());
    // Undo: red again.
    act(&mut d, "edit.undo");
    assert_eq!(shown(&d), [Some(red)].into());
}

/// A cylinder's side is painted as a whole, like a click selects it.
#[test]
fn a_cylinders_side_is_painted_whole() {
    let mut d = Driver::new(Document::with_model(prism_tools::scenes::cylinder(
        1.0, 2.0,
    )));
    d.look(Camera {
        eye: DVec3::new(6.0, -8.0, 5.0),
        target: DVec3::new(0.0, 0.0, 1.0),
    });
    d.tool(PAINT);
    let p = DVec3::new(0.6, -0.8, 1.0);
    d.hover(p);
    let (faces, _) = d.s.paint_preview().expect("a preview");
    d.click(p);
    let painted: Vec<_> =
        d.s.model()
            .faces()
            .filter(|(_, f)| f.paint.front.is_some())
            .map(|(id, _)| id)
            .collect();
    assert!(!painted.is_empty());
    assert_eq!(painted, faces, "the preview showed what was painted");
    let side = prism_ops::select::surface(d.s.model(), painted[0]);
    assert_eq!(painted.len(), side.len(), "the whole side");
}

/// Ctrl+Shift replaces a material in the open group only; Shift alone
/// everywhere.
#[test]
fn ctrl_shift_replaces_in_the_open_group_only() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    act(&mut d, "edit.desel");
    // A second box at the root, beside the group.
    rect_on_ground(&mut d, 4.0, 0.0, 6.0, 2.0);
    d.tool(PAINT_PUSH);
    d.click(DVec3::new(5.0, 1.0, 0.0));
    d.typed("2");
    d.tool(PAINT);
    let root_top = DVec3::new(5.0, 1.0, 2.0);
    press_with(&mut d, root_top, false, true, false);
    let red = paint_of(&d, root_top).front.expect("painted");
    // Into the group: fill it red too.
    d.tool(SELECT);
    let side = DVec3::new(1.0, 0.0, 1.0);
    double_click(&mut d, side);
    assert_eq!(d.s.open_path(), [g]);
    d.tool(PAINT);
    d.s.set_brush(Brush::Material(red));
    press_with(&mut d, side, false, true, false);
    let inside = |d: &Driver| {
        let a = d.s.document().assembly();
        let def = a.context_def(&[g]).expect("group");
        a.def(def)
            .expect("def")
            .model
            .faces()
            .map(|(_, f)| f.paint.front)
            .collect::<Vec<_>>()
    };
    assert!(inside(&d).iter().all(|m| *m == Some(red)));
    // Ctrl+Shift with another colour: the group's red only.
    d.s.set_brush(Brush::Palette(9));
    press_with(&mut d, side, true, true, false);
    let plum = inside(&d)[0].expect("painted");
    assert_ne!(plum, red);
    assert!(inside(&d).iter().all(|m| *m == Some(plum)));
    let root = |d: &Driver| {
        d.s.document()
            .model()
            .faces()
            .filter(|(_, f)| f.paint.front == Some(red))
            .count()
    };
    assert_eq!(root(&d), 6, "the root box keeps its red");
    // Shift alone, from inside: plum replaced everywhere, red stays.
    d.s.set_brush(Brush::Palette(10));
    press_with(&mut d, side, true, false, false);
    assert!(inside(&d).iter().all(|m| *m != Some(plum)));
    assert_eq!(root(&d), 6);
}

/// A recording saves the brush it started with, so its replay paints the
/// same colour (as new loop taught us, 2026-11-11).
#[test]
fn a_recording_replays_with_the_brush_it_started_with() {
    let mut d = Driver::empty();
    paint_box(&mut d);
    d.tool(PAINT);
    d.s.set_brush(Brush::Palette(6));
    let top = DVec3::new(1.0, 1.0, 2.0);
    d.click(top);
    let teal = paint_of(&d, top).front.expect("painted");
    d.s.start_recording();
    d.click(DVec3::new(1.0, 0.0, 1.0));
    let json = d.s.stop_recording().expect("recorded");
    let r: Recording = serde_json::from_str(&json).expect("reads back");
    assert_eq!(r.brush, Brush::Material(teal));
    let replayed = Session::replay(&r);
    assert!(r.ends_with(replayed.model()), "the replay paints the same");
    // The model's materials came along: the top replays in teal.
    let m = replayed.document().assembly().material(teal).expect("teal");
    assert_eq!(m.name, "Teal");
    assert_eq!(replayed.document().assembly().materials().0.len(), 1);
}

/// The preview follows the keys held: Ctrl shows the connected fill,
/// Shift every face it would replace, Alt (sampling) nothing.
#[test]
fn the_paint_preview_follows_the_keys_held() {
    let mut d = Driver::empty();
    paint_box(&mut d);
    rect_on_ground(&mut d, 4.0, 0.0, 5.0, 1.0);
    d.tool(PAINT);
    let top = DVec3::new(1.0, 1.0, 2.0);
    d.hover(top);
    assert_eq!(d.s.paint_preview().expect("plain").0.len(), 1);
    d.send(Input::Held {
        ctrl: true,
        alt: false,
    });
    assert_eq!(d.s.paint_preview().expect("ctrl").0.len(), 6, "the box");
    d.send(Input::Held {
        ctrl: false,
        alt: false,
    });
    d.send(Input::ShiftDown);
    assert_eq!(
        d.s.paint_preview().expect("shift").0.len(),
        7,
        "every Default face"
    );
    d.send(Input::ShiftUp);
    d.send(Input::Held {
        ctrl: false,
        alt: true,
    });
    assert!(d.s.paint_preview().is_none());
    assert!(d.s.hint().contains("SAMPLE"));
    // A plain click while Alt is held samples, and paints nothing.
    d.click(top);
    assert!(paint_of(&d, top).is_default());
}

/// Right after a circle or polygon, typing a radius (or a polygon's sides
/// as 8s) changes it, as for the Rectangle; after any other change it
/// does not (owner's known gaps session, 2026-11-12).
#[test]
fn typing_right_after_a_circle_or_polygon_changes_it() {
    use prism_tools::session::{CIRCLE, POLYGON};
    let mut d = Driver::empty();
    d.tool(CIRCLE);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.click(DVec3::new(2.0, 1.0, 0.0));
    same(&areas(&d), &[std::f64::consts::PI]);
    d.typed("2");
    same(&areas(&d), &[4.0 * std::f64::consts::PI]);
    assert_eq!(d.faces(), 1, "the same circle, larger");
    // One step: Undo goes back to nothing.
    act(&mut d, "edit.undo");
    assert_eq!(d.faces(), 0);
    // After Undo, typing no longer changes anything.
    d.typed("3");
    assert_eq!(d.faces(), 0);
    // A polygon: a new radius, then a new side count.
    d.tool(POLYGON);
    d.click(DVec3::new(5.0, 1.0, 0.0));
    d.click(DVec3::new(6.0, 1.0, 0.0));
    let sides = |d: &Driver| d.s.model().edge_count();
    assert_eq!(sides(&d), 6);
    d.typed("2");
    let (lo, hi) = d.s.model().bbox().expect("polygon");
    assert!((hi.x - lo.x - 4.0).abs() < 1e-9, "radius 2: {lo:?} {hi:?}");
    d.typed("8s");
    assert_eq!(sides(&d), 8);
    assert_eq!(d.faces(), 1);
}

// ---------- the Materials window (Basic Materials 1c) ----------

use prism_tools::session::{EntityInfo, MaterialOp};

#[test]
fn the_materials_window_makes_picks_edits_and_deletes_materials() {
    let mut d = Driver::empty();
    paint_box(&mut d);
    // New: a palette colour, picked, with the Paint Bucket.
    assert_eq!(d.s.material_op(MaterialOp::New), Some(PAINT));
    let rows = d.s.material_rows();
    assert_eq!(rows.len(), 1);
    let a = rows[0].id;
    assert_eq!(d.s.brush(), Brush::Material(a));
    assert!(!rows[0].used, "not painted yet");
    // Paint the top with it: used now, listed first.
    d.tool(PAINT);
    d.click(DVec3::new(1.0, 1.0, 2.0));
    assert_eq!(d.s.material_op(MaterialOp::Duplicate(a)), Some(PAINT));
    let rows = d.s.material_rows();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].used && rows[0].id == a, "used ones first");
    assert!(rows[1].name.ends_with("#2"), "{}", rows[1].name);
    // Editing a colour many times in a row (a picker dragged) is one step.
    let steps = d.s.document().log().count();
    for k in 0..5u8 {
        let mut m = rows[0].material.clone();
        m.surface_mut().base[0] = f32::from(k) / 10.0;
        d.s.material_op(MaterialOp::Edit(a, m));
    }
    assert_eq!(d.s.document().log().count(), steps + 1, "one undo step");
    let base =
        d.s.document()
            .assembly()
            .material(a)
            .expect("a")
            .surface()
            .base[0];
    assert!((base - 0.4).abs() < 1e-6);
    // Entity Info names the selected face's materials.
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 1.0, 2.0));
    match d.s.entity_info() {
        EntityInfo::Geometry {
            materials: Some((front, back)),
            ..
        } => {
            assert_eq!(front, rows[0].name);
            assert_eq!(back, "Default");
        }
        other => panic!("{other:?}"),
    }
    // Model Health: one unused material, and Purge removes it.
    let row = |d: &Driver| {
        d.s.health_rows()
            .into_iter()
            .find(|r| r.key == "materials")
            .expect("row")
    };
    assert_eq!((row(&d).count, row(&d).fix), (1, Some("Purge")));
    d.s.health_fix("materials");
    assert_eq!(d.s.material_rows().len(), 1);
    assert_eq!(row(&d).count, 0);
    // Delete the one the bucket holds: the top goes back to Default.
    assert_eq!(
        d.s.material_op(MaterialOp::Pick(Brush::Material(a))),
        Some(PAINT)
    );
    d.s.material_op(MaterialOp::Delete(a));
    assert!(d.s.material_rows().is_empty());
    assert!(paint_of(&d, DVec3::new(1.0, 1.0, 2.0)).is_default());
    assert_eq!(d.s.brush(), Brush::default(), "the brush lets it go");
}

#[test]
fn select_all_with_the_same_material() {
    let mut d = Driver::empty();
    paint_box(&mut d);
    d.tool(PAINT);
    // Two sides red, the top teal.
    d.click(DVec3::new(1.0, 0.0, 1.0));
    d.click(DVec3::new(2.0, 1.0, 1.0));
    d.s.set_brush(Brush::Palette(6));
    d.click(DVec3::new(1.0, 1.0, 2.0));
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 0.0, 1.0));
    act(&mut d, "edit.selmat");
    assert_eq!(d.s.selection().faces.len(), 2, "the two red sides");
    // From the teal top: just the top.
    d.click(DVec3::new(1.0, 1.0, 2.0));
    act(&mut d, "edit.selmat");
    assert_eq!(d.s.selection().faces.len(), 1, "the top");
}

/// Follow Me as in SketchUp (owner, 2026-11-12): press on a face, drag
/// along an edge joined to it, release: the face is swept along. Esc
/// before releasing leaves the model alone; click, move, click works too.
#[test]
fn follow_me_drags_a_face_along_the_edges_joined_to_it() {
    use prism_tools::session::FOLLOW;
    let build = |d: &mut Driver| {
        paint_box(d);
        // A line from the box's front right corner out along red.
        d.tool(LINE);
        d.click(DVec3::new(2.0, 0.0, 0.0));
        d.click(DVec3::new(5.0, 0.0, 0.0));
        key(d, Key::Escape);
    };
    let volume = |d: &Driver| -> f64 {
        let m = d.s.model();
        m.faces()
            .map(|(id, _)| prism_ops::health::face_volume(m, id))
            .sum()
    };
    // Press and drag.
    let mut d = Driver::empty();
    build(&mut d);
    let faces = d.faces();
    d.tool(FOLLOW);
    let side = d.screen(DVec3::new(2.0, 1.0, 1.0));
    d.send(Input::Move {
        x: side.x,
        y: side.y,
    });
    d.send(Input::Press {
        button: Button::Left,
        x: side.x,
        y: side.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    d.hover(DVec3::new(5.0, 0.0, 0.0));
    assert!(d.s.in_operation(), "dragging");
    assert!(
        d.s.display_scene().faces.len() > faces,
        "the sweep is previewed"
    );
    // Esc: nothing changes.
    key(&mut d, Key::Escape);
    d.send(Input::Release {
        button: Button::Left,
    });
    assert_eq!(d.faces(), faces);
    // Again, released this time.
    let side = d.screen(DVec3::new(2.0, 1.0, 1.0));
    d.t += 0.5;
    d.send(Input::Move {
        x: side.x,
        y: side.y,
    });
    d.send(Input::Press {
        button: Button::Left,
        x: side.x,
        y: side.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    d.hover(DVec3::new(5.0, 0.0, 0.0));
    d.send(Input::Release {
        button: Button::Left,
    });
    assert!(!d.s.in_operation());
    let v = volume(&d);
    assert!(
        (v - 8.0 - 12.0).abs() < 1e-6,
        "the box grew along the line: {v}"
    );
    valid(&d);
    // Click, move, click.
    let mut d = Driver::empty();
    build(&mut d);
    d.tool(FOLLOW);
    d.click(DVec3::new(2.0, 1.0, 1.0));
    d.hover(DVec3::new(5.0, 0.0, 0.0));
    d.click(DVec3::new(5.0, 0.0, 0.0));
    let v = volume(&d);
    assert!((v - 20.0).abs() < 1e-6, "{v}");
}

/// Push/Pull matches the height of a point on an edge (owner,
/// 2026-11-12: only points, midpoints and faces were matched).
#[test]
fn push_pull_matches_a_point_on_an_edge() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    // A post: a vertical line 4 m tall beside it.
    d.tool(LINE);
    d.click(DVec3::new(5.0, 0.0, 0.0));
    d.click(DVec3::new(5.0, 0.0, 4.0));
    key(&mut d, Key::Escape);
    d.tool(PAINT_PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    // Over the post three metres up (not an end or its middle).
    d.hover(DVec3::new(5.0, 0.0, 3.0));
    assert!(d.s.hint().contains("pull"), "{}", d.s.hint());
    d.click(DVec3::new(5.0, 0.0, 3.0));
    let (lo, hi) = d.s.model().bbox().expect("box");
    let top =
        d.s.model()
            .faces()
            .filter_map(|(_, f)| {
                f.plane()
                    .filter(|p| p.normal.z.abs() > 0.9)
                    .map(|p| p.origin.z)
            })
            .fold(f64::MIN, f64::max);
    assert!(
        (top - 3.0).abs() < 1e-6,
        "pulled level with the point: {top} ({lo:?} {hi:?})"
    );
}

/// Follow Me stops partway along an edge, at the snapped point, and at a
/// path's corner (owner, 2026-11-12: inference while following).
#[test]
fn follow_me_stops_where_the_inference_snaps_on_the_path() {
    use prism_tools::session::FOLLOW;
    let volume = |d: &Driver| -> f64 {
        let m = d.s.model();
        m.faces()
            .map(|(id, _)| prism_ops::health::face_volume(m, id))
            .sum()
    };
    let build = |d: &mut Driver| {
        paint_box(d);
        d.tool(LINE);
        d.click(DVec3::new(2.0, 0.0, 0.0));
        d.click(DVec3::new(5.0, 0.0, 0.0));
        d.click(DVec3::new(5.0, 3.0, 0.0));
        key(d, Key::Escape);
    };
    // Partway along the first line.
    let mut d = Driver::empty();
    build(&mut d);
    d.tool(FOLLOW);
    d.click(DVec3::new(2.0, 1.0, 1.0));
    d.hover(DVec3::new(3.5, 0.0, 0.0));
    d.click(DVec3::new(3.5, 0.0, 0.0));
    let v = volume(&d);
    assert!((v - 8.0 - 6.0).abs() < 1e-6, "swept 1.5 m: {v}");
    valid(&d);
    // The rest of the line stays.
    let m = d.s.model();
    assert!(m.edges().any(|(_, e)| {
        let (a, b) = (m.vertex(e.a), m.vertex(e.b));
        (a.x.max(b.x) - 5.0).abs() < 1e-9 && (a.x.min(b.x) - 3.5).abs() < 1e-9
    }));
    // At the corner: the whole first line, not round the corner.
    let mut d = Driver::empty();
    build(&mut d);
    d.tool(FOLLOW);
    d.click(DVec3::new(2.0, 1.0, 1.0));
    d.hover(DVec3::new(5.0, 0.0, 0.0));
    d.click(DVec3::new(5.0, 0.0, 0.0));
    let v = volume(&d);
    assert!((v - 20.0).abs() < 1e-6, "swept 3 m: {v}");
}

/// Follow Me stops where the path lines up with another point along an
/// axis (a woken point's guide crossing the path).
#[test]
fn follow_me_stops_level_with_another_point() {
    use prism_tools::session::FOLLOW;
    let mut d = Driver::empty();
    paint_box(&mut d);
    d.tool(LINE);
    d.click(DVec3::new(2.0, 0.0, 0.0));
    d.click(DVec3::new(6.0, 0.0, 0.0));
    key(&mut d, Key::Escape);
    // A short line elsewhere, whose end is the point to line up with.
    d.click(DVec3::new(3.2, 4.0, 0.0));
    d.click(DVec3::new(3.2, 5.0, 0.0));
    key(&mut d, Key::Escape);
    d.tool(FOLLOW);
    d.click(DVec3::new(2.0, 1.0, 1.0));
    // Wake the point, then come back near the path along green.
    d.hover(DVec3::new(3.2, 4.0, 0.0));
    d.wait(0.5);
    d.hover(DVec3::new(3.2, 4.0, 0.0));
    d.hover(DVec3::new(3.2, 0.25, 0.0));
    let i = d.s.inference().expect("inference").clone();
    d.click(DVec3::new(3.2, 0.25, 0.0));
    let m = d.s.model();
    let v: f64 = m
        .faces()
        .map(|(id, _)| prism_ops::health::face_volume(m, id))
        .sum();
    assert!(
        (v - 8.0 - 1.2 * 4.0).abs() < 1e-6,
        "swept to x = 3.2: {v} ({:?} {})",
        i.kind,
        i.label
    );
}

/// Scale's grip goes level with a point it snaps to (owner, 2026-11-12:
/// make sure no tool misses inference).
#[test]
fn scale_matches_a_point_it_snaps_to() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    // A line whose end, at x = 3, the right grip should reach.
    d.tool(LINE);
    d.click(DVec3::new(3.0, 4.0, 0.0));
    d.click(DVec3::new(3.0, 5.0, 0.0));
    key(&mut d, Key::Escape);
    d.tool(SCALE);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    // The right face grip (middle of the box's right side), dragged to the
    // line's end.
    d.click(DVec3::new(2.1, 1.0, 0.0));
    d.hover(DVec3::new(3.0, 4.0, 0.0));
    d.click(DVec3::new(3.0, 4.0, 0.0));
    let (lo, hi) = d.s.model().bbox().expect("model");
    let right =
        d.s.model()
            .vertices()
            .filter(|(_, p)| p.y < 2.5)
            .map(|(_, p)| p.x)
            .fold(f64::MIN, f64::max);
    // The square's right side (not the box drawn 5% off it) reaches x = 3.
    assert!((right - 3.0).abs() < 1e-6, "{right} ({lo:?} {hi:?})");
}

/// A textured material's image reaches the drawn scene, decoded once, with
/// its size and pixels; View > Textures reaches the scene (see-through
/// pixels hide nothing only while textures are drawn) (Basic Materials 2b).
#[test]
fn a_textured_material_reaches_the_scene() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/acceptance/files/textured.prism"
    );
    let bytes = std::fs::read(path).expect("the golden file");
    let contents = prism_file::read(&bytes).expect("opens");
    let mut s = Session::new(Document::with_assembly(contents.assembly));
    let (id, m) = {
        let mats = s.document().assembly().materials().0.clone();
        mats.into_iter().next().expect("a material")
    };
    let t = s.scene().textures.get(&id).expect("its texture");
    assert_eq!(t.key, m.texture.as_ref().unwrap().image.0);
    assert_eq!(t.size, m.texture.as_ref().unwrap().size);
    assert_eq!(t.rgba.len(), (t.width * t.height * 4) as usize);
    let again = s.scene().textures[&id].rgba.clone();
    assert!(
        std::sync::Arc::ptr_eq(&t.rgba.clone(), &again),
        "decoded once"
    );
    assert!(s.scene().materials_shown && s.scene().textures_shown);
    let mut v = s.view_style().clone();
    assert!(v.toggle("view.textures"));
    s.set_view_style(v);
    assert!(!s.scene().textures_shown);
}

fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba(rgba));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .expect("png");
    out.into_inner()
}

/// The Materials window's textures (Basic Materials 2c): an image becomes
/// a new material named after its file and picked; loading onto a material
/// keeps its texture's width; the size typed keeps the shape (or not);
/// Remove Texture and undo; Model Health counts the image left unused.
#[test]
fn the_materials_window_loads_sizes_and_removes_textures() {
    use prism_tools::session::{Brush, MaterialOp, PAINT};
    let mut d = Driver::empty();
    paint_box(&mut d);
    // Not an image: refused, nothing changes.
    assert_eq!(
        d.s.load_image(None, "notes.png", b"hello".to_vec()),
        Err(prism_doc::image::NOT_AN_IMAGE)
    );
    assert!(d.s.material_rows().is_empty());
    // A new material from an image, 1 m wide keeping its shape.
    let tool = d.s.load_image(
        None,
        "C:\\pics\\Oak Floor.jpg.png",
        png(40, 20, [150, 100, 50, 255]),
    );
    assert_eq!(tool, Ok(Some(PAINT)));
    let rows = d.s.material_rows();
    assert_eq!(rows.len(), 1);
    let id = rows[0].id;
    assert_eq!(rows[0].name, "Oak Floor.jpg");
    assert_eq!(d.s.brush(), Brush::Material(id), "picked to paint with");
    let t = rows[0].texture.clone().expect("textured");
    assert_eq!((t.pixels, t.size), ([40, 20], [1.0, 0.5]));
    assert_eq!(&rows[0].srgba[..3], &[150, 100, 50], "its average colour");
    assert_eq!(d.s.document().undo_name(), Some("New Material"), "one step");
    // A width typed keeps the shape; unlocked, a height leaves the width.
    d.s.material_op(MaterialOp::TextureSize {
        id,
        text: "2".into(),
        height: false,
        keep_shape: true,
    });
    assert_eq!(
        d.s.material_rows()[0].texture.as_ref().unwrap().size,
        [2.0, 1.0]
    );
    d.s.material_op(MaterialOp::TextureSize {
        id,
        text: "300mm".into(),
        height: true,
        keep_shape: false,
    });
    assert_eq!(
        d.s.material_rows()[0].texture.as_ref().unwrap().size,
        [2.0, 0.3]
    );
    d.s.material_op(MaterialOp::TextureSize {
        id,
        text: "-1".into(),
        height: false,
        keep_shape: true,
    });
    assert_eq!(
        d.s.take_material_refusal(),
        Some(prism_tools::session::NOT_A_SIZE)
    );
    assert_eq!(
        d.s.material_rows()[0].texture.as_ref().unwrap().size,
        [2.0, 0.3]
    );
    // Another image onto it: the width stays, the height follows the image.
    assert_eq!(
        d.s.load_image(Some(id), "b.png", png(10, 30, [0, 0, 0, 255])),
        Ok(None)
    );
    let t = d.s.material_rows()[0].texture.clone().unwrap();
    assert_eq!((t.pixels, t.size), ([10, 30], [2.0, 6.0]));
    assert_eq!(d.s.material_rows().len(), 1, "no new material");
    // The first image is now unused, and so is the material (painted on
    // nothing yet): Model Health counts both.
    let row = |s: &Session| {
        s.health_rows()
            .into_iter()
            .find(|r| r.key == "materials")
            .expect("the row")
    };
    assert_eq!(row(&d.s).count, 2);
    let detail = row(&d.s).detail;
    assert_eq!(
        detail,
        "1 material painted on nothing, 1 image no material uses"
    );
    // Remove Texture: back to its colour; undo brings it back.
    d.s.material_op(MaterialOp::RemoveTexture(id));
    assert!(d.s.material_rows()[0].texture.is_none());
    assert_eq!(row(&d.s).count, 3, "both images unused");
    act(&mut d, "edit.undo");
    assert!(d.s.material_rows()[0].texture.is_some());
    // Painted on nothing, it goes with Purge, and its images with it.
    d.s.health_fix("materials");
    assert!(d.s.material_rows().is_empty());
    assert!(d.s.document().assembly().images().is_empty());
}

fn press_at(d: &mut Driver, p: DVec3) {
    let s = d.screen(p);
    d.send(Input::Move { x: s.x, y: s.y });
    d.send(Input::Press {
        button: Button::Left,
        x: s.x,
        y: s.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
}

fn release(d: &mut Driver) {
    d.send(Input::Release {
        button: Button::Left,
    });
    d.t += 0.5;
}

/// Texture > Position (Basic Materials part 3): the red pin moves the
/// texture, the green pin turns and sizes it about the red one; Enter
/// keeps it as one step, Esc puts it back; Reset Position goes back to the
/// default; a face without a texture is refused.
#[test]
fn position_texture_moves_turns_and_sizes_with_pins() {
    use prism_tools::session::{NOT_TEXTURED, TEXTURE};
    let mut d = Driver::empty();
    paint_box(&mut d);
    let top = DVec3::new(1.0, 1.0, 2.0);
    // An untextured face is refused.
    d.tool(TEXTURE);
    d.click(top);
    assert_eq!(d.s.hint(), NOT_TEXTURED);
    // Paint the top with a 1 m × 0.5 m texture.
    d.s.load_image(None, "wood.png", png(40, 20, [150, 100, 50, 255]))
        .expect("loads");
    d.tool(prism_tools::session::PAINT);
    d.click(top);
    let face = face_at(&d, top);
    let map0 = d.s.model().face(face).unwrap().texture_map().unwrap();
    // Pick the tool, click the top: the pins sit one image-width apart.
    d.tool(TEXTURE);
    d.click(top);
    let (red, green) = d.s.texture_pins().expect("started");
    assert!((green - red).length() - 1.0 < 1e-9 && (green - red).length() > 1.0 - 1e-9);
    // Drag the red pin: the texture moves with it.
    press_at(&mut d, red);
    let to = red + DVec3::new(0.25, 0.1, 0.0);
    d.hover(to);
    release(&mut d);
    let m = d.s.texture_map_now().expect("still placing");
    let moved = |p: DVec3| m.at(p + (to - red));
    for p in [top, DVec3::new(0.3, 1.7, 2.0)] {
        assert!(moved(p).distance(map0.at(p)) < 1e-6, "moved with the pin");
    }
    // The model is untouched until Enter.
    assert_eq!(d.s.model().face(face).unwrap().paint.uv, None);
    // Drag the green pin a quarter turn round and to twice as far.
    let (red, green) = d.s.texture_pins().unwrap();
    let a = green - red;
    let b = DVec3::new(-a.y, a.x, 0.0) * 2.0;
    press_at(&mut d, green);
    d.hover(red + b);
    release(&mut d);
    let m = d.s.texture_map_now().unwrap();
    // Along the new direction the image is twice as long.
    let du = m.at(red + b) - m.at(red);
    assert!((du.length() - 1.0).abs() < 1e-6, "one image across: {du}");
    assert!(du.y.abs() < 1e-6, "along the image's width: {du}");
    // Enter keeps it, in one step.
    d.send(Input::Key { key: Key::Enter });
    let stored = d.s.model().face(face).unwrap().paint.uv.expect("stored");
    assert_eq!(stored, m);
    assert_eq!(d.s.document().undo_name(), Some("Position Texture"));
    // Esc puts a placement in progress back.
    d.click(top);
    let (red, _) = d.s.texture_pins().unwrap();
    press_at(&mut d, red);
    d.hover(red + DVec3::new(0.3, 0.0, 0.0));
    release(&mut d);
    d.send(Input::Key { key: Key::Escape });
    assert!(d.s.texture_pins().is_none());
    assert_eq!(d.s.model().face(face).unwrap().paint.uv, Some(stored));
    // Reset Position: the default again.
    d.tool(SELECT);
    d.click(top);
    act(&mut d, "tex.reset");
    assert_eq!(d.s.model().face(face).unwrap().paint.uv, None);
    assert_eq!(d.s.document().undo_name(), Some("Reset Texture"));
}

/// Imported materials join the document in the same step as the geometry,
/// after the ones already there (Basic Materials part 3).
#[test]
fn an_import_brings_its_materials_in_one_step() {
    let mut d = Driver::empty();
    d.s.load_image(None, "a.png", png(4, 4, [9, 9, 9, 255]))
        .unwrap();
    let mut m = prism_topo::Model::new();
    prism_ops::add_rect(&mut m, 5.0, 5.0, 6.0, 6.0, 0.0);
    let f = m.faces().next().unwrap().0;
    m.edit_face(f, |x| x.paint.front = Some(prism_topo::MaterialId(0)));
    let glass = prism_doc::Material::colour("Glass", [200, 220, 255]);
    assert!(d.s.import_painted(m, vec![(glass, None)]));
    let mats = d.s.document().assembly().materials().0.clone();
    assert_eq!(mats.len(), 2);
    let glass = mats
        .iter()
        .find(|(_, x)| x.name == "Glass")
        .expect("added")
        .0;
    let painted =
        d.s.model()
            .faces()
            .filter(|(_, x)| x.paint.front == Some(*glass))
            .count();
    assert_eq!(painted, 1);
    assert_eq!(d.s.document().undo_name(), Some("Import"));
    act(&mut d, "edit.undo");
    assert_eq!(d.s.document().assembly().materials().0.len(), 1);
}

// ---------- Freeform NURBS part 1: the Curve tool ----------

fn spline_edges(d: &Driver) -> usize {
    d.s.model()
        .edges()
        .filter(|(_, e)| e.curve.is_spline())
        .count()
}

#[test]
fn the_curve_tool_draws_a_curve_and_enter_ends_it() {
    use prism_tools::session::CURVE;
    let mut d = Driver::empty();
    d.tool(CURVE);
    for p in [(0.0, 0.0), (1.0, 2.0), (3.0, 2.0), (4.0, 0.0)] {
        d.click(DVec3::new(p.0, p.1, 0.0));
    }
    assert!(d.s.in_operation());
    assert_eq!(spline_edges(&d), 0);
    d.send(Input::Key { key: Key::Enter });
    assert_eq!(spline_edges(&d), 1);
    assert!(!d.s.in_operation());
    valid(&d);
    // The curve starts and ends at the first and last click, and is pulled
    // towards the others without reaching them.
    let m = d.s.model();
    let (_, e) = m.edges().next().unwrap();
    let c = e.curve.spline().unwrap();
    assert_eq!(c.curve.cage().len(), 4);
    assert!(c.curve.eval(0.5).y < 2.0);
    undo(&mut d);
    assert_eq!(d.s.model().edge_count(), 0);
}

#[test]
fn clicking_the_start_closes_the_curve_into_a_face() {
    use prism_tools::session::CURVE;
    let mut d = Driver::empty();
    d.tool(CURVE);
    for p in [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0), (0.0, 0.0)] {
        d.click(DVec3::new(p.0, p.1, 0.0));
    }
    assert_eq!(spline_edges(&d), 2, "a closed curve is two halves");
    assert_eq!(d.faces(), 1);
    valid(&d);
    // A click on it selects the whole curve.
    d.tool(SELECT);
    let on = d.s.model().edges().next().map(|(_, e)| {
        let m = d.s.model();
        e.curve.point(m.vertex(e.a), m.vertex(e.b), 0.3)
    });
    d.click(on.unwrap());
    assert_eq!(d.s.selection().edges.len(), 2);
}

#[test]
fn ctrl_makes_the_curve_pass_through_the_clicks_and_a_double_click_ends_it() {
    use prism_tools::session::CURVE;
    let mut d = Driver::empty();
    d.tool(CURVE);
    d.send(Input::Held {
        ctrl: true,
        alt: false,
    });
    d.send(Input::Held {
        ctrl: false,
        alt: false,
    });
    assert!(d.s.curve_through());
    let pts = [(0.0, 0.0), (1.0, 2.0), (3.0, 2.0)];
    for p in pts {
        d.click(DVec3::new(p.0, p.1, 0.0));
    }
    double_click(&mut d, DVec3::new(4.0, 0.0, 0.0));
    assert_eq!(spline_edges(&d), 1);
    let m = d.s.model();
    let (_, e) = m.edges().next().unwrap();
    let (pa, pb) = (m.vertex(e.a), m.vertex(e.b));
    for p in pts {
        let p = DVec3::new(p.0, p.1, 0.0);
        assert!(e.curve.closest_point(pa, pb, p).distance(p) < 1e-9);
    }
    valid(&d);
}

#[test]
fn a_line_snaps_onto_a_curve_and_splits_it() {
    use prism_tools::session::CURVE;
    let mut d = Driver::empty();
    d.tool(CURVE);
    for p in [(0.0, 0.0), (1.0, 2.0), (3.0, 2.0), (4.0, 0.0)] {
        d.click(DVec3::new(p.0, p.1, 0.0));
    }
    d.send(Input::Key { key: Key::Enter });
    let on = {
        let m = d.s.model();
        let (_, e) = m.edges().next().unwrap();
        e.curve.point(m.vertex(e.a), m.vertex(e.b), 0.35)
    };
    d.tool(LINE);
    d.click(DVec3::new(1.3, -1.0, 0.0));
    // Its middle (halfway along it) is a Midpoint.
    d.hover(DVec3::new(2.0, 1.5, 0.0));
    assert_eq!(d.s.inference().expect("a snap").label, "Midpoint");
    d.hover(on + DVec3::new(0.02, 0.02, 0.0));
    let i = d.s.inference().expect("a snap");
    assert_eq!(i.label, "On Curve");
    assert!(i.point.distance(on) < 0.05);
    d.click(on + DVec3::new(0.02, 0.02, 0.0));
    assert_eq!(
        spline_edges(&d),
        2,
        "the curve is split where the line lands"
    );
    valid(&d);
}

// ---------- Freeform NURBS part 2: freeform faces ----------

#[test]
fn push_pull_makes_freeform_walls_that_are_drawn_and_picked() {
    use prism_tools::session::CURVE;
    let mut d = Driver::empty();
    d.tool(CURVE);
    for p in [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0), (0.0, 0.0)] {
        d.click(DVec3::new(p.0, p.1, 0.0));
    }
    d.tool(PUSH);
    d.click(DVec3::new(2.0, 1.5, 0.0));
    d.hover(DVec3::new(2.0, 1.5, 0.5));
    d.typed("2");
    assert_eq!(d.faces(), 4);
    valid(&d);
    let walls: Vec<_> =
        d.s.model()
            .faces()
            .filter(|(_, f)| f.surface.patch().is_some())
            .map(|(id, _)| id)
            .collect();
    assert_eq!(walls.len(), 2);
    // Each wall is in the scene with triangles, on the surface, finer
    // than its four corners.
    let scene = d.s.scene();
    for id in &walls {
        let f = scene.face(*id).expect("in the scene");
        assert!(f.tris.len() > 3 * 16, "{:?}", f.tris);
    }
    // A click on the wall that faces the camera selects it; undo takes
    // the pull back.
    d.tool(SELECT);
    let mut picked = 0;
    for id in &walls {
        let on = {
            let m = d.s.model();
            let p = m.face(*id).unwrap().surface.patch().unwrap().clone();
            p.surface.eval((p.u.0 + p.u.1) / 2.0, (p.v.0 + p.v.1) / 2.0)
        };
        d.click(on);
        picked += usize::from(d.s.selection().faces.contains(id));
    }
    assert_eq!(picked, 1, "the near wall, not the far one behind it");
    undo(&mut d);
    assert_eq!(d.faces(), 1);
}

// ---------- Freeform NURBS part 3: cage editing ----------

fn drawn_curve() -> Driver {
    use prism_tools::session::CURVE;
    let mut d = Driver::empty();
    d.tool(CURVE);
    for p in [(0.0, 0.0), (1.0, 2.0), (3.0, 2.0), (4.0, 0.0)] {
        d.click(DVec3::new(p.0, p.1, 0.0));
    }
    d.send(Input::Key { key: Key::Enter });
    d
}

#[test]
fn edit_cage_moves_a_control_point_with_the_move_tool() {
    let mut d = drawn_curve();
    // Nothing selected: it says what to select.
    action(&mut d, "cage.edit");
    assert!(!d.s.cage_active());
    assert_eq!(d.s.hint(), prism_ops::cage::NO_CAGE);
    d.tool(SELECT);
    let on = {
        let m = d.s.model();
        let (_, e) = m.edges().next().unwrap();
        e.curve.point(m.vertex(e.a), m.vertex(e.b), 0.3)
    };
    d.click(on);
    action(&mut d, "cage.edit");
    assert!(d.s.cage_active());
    assert_eq!(d.s.cage_points().len(), 4);
    // Select picks a control point; Shift adds another.
    d.click(DVec3::new(1.0, 2.0, 0.0));
    assert_eq!(d.s.cage_picked(), vec![1]);
    let s = d.screen(DVec3::new(3.0, 2.0, 0.0));
    d.click_px(s, true, false);
    assert_eq!(d.s.cage_picked(), vec![1, 2]);
    // Move: both picked points go 1 m along green, by a typed distance.
    d.tool(MOVE);
    d.click(DVec3::new(0.0, -1.0, 0.0));
    d.hover(DVec3::new(0.0, 0.0, 0.0));
    d.typed("1");
    let pts = d.s.cage_points();
    assert!(
        pts[1].distance(DVec3::new(1.0, 3.0, 0.0)) < 1e-9,
        "{:?}",
        pts[1]
    );
    assert!(pts[2].distance(DVec3::new(3.0, 3.0, 0.0)) < 1e-9);
    valid(&d);
    // A click on a point that is not picked moves that one alone.
    d.click(DVec3::new(4.0, 0.0, 0.0));
    d.click(DVec3::new(5.0, 0.0, 0.0));
    assert!(d.s.cage_points()[3].distance(DVec3::new(5.0, 0.0, 0.0)) < 1e-9);
    assert!(
        d.s.model()
            .vertices()
            .any(|(_, p)| p.distance(DVec3::new(5.0, 0.0, 0.0)) < 1e-9)
    );
    valid(&d);
    // Undo takes the last step back; Esc leaves the cage.
    undo(&mut d);
    assert!(d.s.cage_points()[3].distance(DVec3::new(4.0, 0.0, 0.0)) < 1e-9);
    d.send(Input::Key { key: Key::Escape });
    assert!(!d.s.cage_active());
}

#[test]
fn rotate_scale_and_add_work_on_the_cage_and_refusals_say_why() {
    let mut d = drawn_curve();
    d.tool(SELECT);
    d.click(DVec3::new(0.0, 0.0, 0.0) + DVec3::new(0.02, 0.04, 0.0));
    if d.s.selection().edges.is_empty() {
        let on = {
            let m = d.s.model();
            let (_, e) = m.edges().next().unwrap();
            e.curve.point(m.vertex(e.a), m.vertex(e.b), 0.3)
        };
        d.click(on);
    }
    action(&mut d, "cage.edit");
    assert!(d.s.cage_active());
    // Rotate with nothing picked: it says to pick first.
    d.tool(ROTATE);
    d.click(DVec3::new(2.0, 0.0, 0.0));
    assert_eq!(d.s.hint(), prism_tools::session::PICK_FIRST);
    // The two middle points turned a quarter about their middle (2, 2).
    d.s.cage_pick(&[1, 2]);
    d.click(DVec3::new(3.0, 2.0, 0.0));
    d.typed("90");
    let pts = d.s.cage_points();
    assert!(
        pts[1].distance(DVec3::new(2.0, 1.0, 0.0)) < 1e-9,
        "{:?}",
        pts[1]
    );
    assert!(
        pts[2].distance(DVec3::new(2.0, 3.0, 0.0)) < 1e-9,
        "{:?}",
        pts[2]
    );
    // Scaled to twice the size about the same middle: Scale's box is round
    // the picked points (flat across, so its grips are only at the ends,
    // standing off 5%); Alt scales about the middle.
    d.tool(SCALE);
    assert!(
        d.s.overlay()
            .marks
            .iter()
            .any(|m| m.at.distance(DVec3::new(2.0, 3.1, 0.0)) < 1e-9),
        "a grip above the top point"
    );
    d.send(Input::AltTap);
    d.click(DVec3::new(2.0, 3.1, 0.0));
    d.typed("2");
    let pts = d.s.cage_points();
    assert!(
        pts[1].distance(DVec3::new(2.0, 0.0, 0.0)) < 1e-9,
        "{:?}",
        pts[1]
    );
    assert!(pts[2].distance(DVec3::new(2.0, 4.0, 0.0)) < 1e-9);
    valid(&d);
    // Add Control Points: one more, the shape kept.
    action(&mut d, "cage.add");
    assert_eq!(d.s.cage_points().len(), 5);
    valid(&d);
}

#[test]
fn control_points_are_boxed_and_rotate_shows_its_protractor() {
    let mut d = drawn_curve();
    d.tool(SELECT);
    let on = {
        let m = d.s.model();
        let (_, e) = m.edges().next().unwrap();
        e.curve.point(m.vertex(e.a), m.vertex(e.b), 0.3)
    };
    d.click(on);
    action(&mut d, "cage.edit");
    assert!(d.s.cage_active());
    // A box round the two top points (1, 2) and (3, 2) picks them.
    let (a, b) = (
        d.screen(DVec3::new(0.5, 2.6, 0.0)),
        d.screen(DVec3::new(3.5, 1.4, 0.0)),
    );
    drag_px(&mut d, a, b);
    assert_eq!(d.s.cage_picked(), vec![1, 2]);
    assert!(d.s.cage_active(), "the box does not leave the cage");
    // With Shift, a box round the right two toggles them.
    let (a, b) = (
        d.screen(DVec3::new(2.5, 2.6, 0.0)),
        d.screen(DVec3::new(4.5, -0.5, 0.0)),
    );
    d.send(Input::Move { x: a.x, y: a.y });
    d.send(Input::Press {
        button: Button::Left,
        x: a.x,
        y: a.y,
        shift: true,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Move { x: b.x, y: b.y });
    d.send(Input::Release {
        button: Button::Left,
    });
    assert_eq!(d.s.cage_picked(), vec![1, 3]);
    // Rotate: the protractor sits on the picked points' middle (2, 2),
    // and shows the angle once turning.
    d.s.cage_pick(&[1, 2]);
    d.tool(ROTATE);
    d.hover(DVec3::new(3.0, 2.0, 0.0));
    let ring = |d: &Driver| {
        d.s.overlay()
            .marks
            .iter()
            .any(|m| m.at.distance(DVec3::new(2.0, 2.0, 0.0)) < 1e-9)
    };
    assert!(ring(&d), "the pivot shows before the first click");
    d.click(DVec3::new(3.0, 2.0, 0.0));
    d.hover(DVec3::new(2.0, 3.0, 0.0));
    assert!(
        d.s.overlay()
            .labels
            .iter()
            .any(|l| l.text.starts_with("90.0")),
        "{:?}",
        d.s.overlay()
            .labels
            .iter()
            .map(|l| &l.text)
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_freeform_solid_is_pulled_into_shape_by_its_cage() {
    use prism_tools::session::CURVE;
    let mut d = Driver::empty();
    d.tool(CURVE);
    for p in [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0), (0.0, 0.0)] {
        d.click(DVec3::new(p.0, p.1, 0.0));
    }
    d.tool(PUSH);
    d.click(DVec3::new(2.0, 1.5, 0.0));
    d.hover(DVec3::new(2.0, 1.5, 0.5));
    d.typed("2");
    let walls: Vec<_> =
        d.s.model()
            .faces()
            .filter(|(_, f)| f.surface.patch().is_some())
            .map(|(id, f)| {
                let p = f.surface.patch().unwrap();
                (
                    id,
                    p.surface.eval((p.u.0 + p.u.1) / 2.0, (p.v.0 + p.v.1) / 2.0),
                )
            })
            .collect();
    d.tool(SELECT);
    for (id, on) in walls {
        d.click(on);
        if d.s.selection().faces.contains(&id) {
            break;
        }
    }
    assert!(
        !d.s.selection().faces.is_empty(),
        "a click takes the smooth wall whole"
    );
    action(&mut d, "cage.edit");
    assert_eq!(d.s.cage_points().len(), 8);
    // The top row lifted 1 m: the flat top goes with it.
    d.s.cage_pick(&[1, 3, 5, 7]);
    d.tool(MOVE);
    d.click(DVec3::new(2.0, -1.0, 0.0));
    d.send(Input::Key { key: Key::Up });
    d.hover(DVec3::new(2.0, -1.0, 0.5));
    d.typed("1");
    let (_, hi) = d.s.model().bbox().unwrap();
    assert!((hi.z - 3.0).abs() < 1e-9, "{hi}");
    valid(&d);
    // One top point alone would bend the flat top: refused, with why.
    d.s.cage_pick(&[1]);
    d.click(DVec3::new(2.0, -1.0, 0.0));
    d.send(Input::Key { key: Key::Up });
    d.hover(DVec3::new(2.0, -1.0, 0.5));
    d.typed("1");
    assert_eq!(d.s.hint(), prism_ops::cage::FLAT_BESIDE);
    let (_, hi) = d.s.model().bbox().unwrap();
    assert!((hi.z - 3.0).abs() < 1e-9);
}

// ---------- Freeform NURBS part 4: Loft and Revolve ----------

fn three_curves() -> Driver {
    let mut m = prism_topo::Model::new();
    for (z, bump) in [(0.0, 2.0), (2.0, 0.5), (5.0, 1.0)] {
        let c = prism_geom::Nurbs::open(
            &[
                DVec3::new(0.0, 0.0, z),
                DVec3::new(1.0, bump, z),
                DVec3::new(3.0, -bump, z),
                DVec3::new(4.0, 0.0, z),
            ],
            3,
        )
        .unwrap();
        prism_ops::add_spline(&mut m, &c);
    }
    Driver::new(Document::with_model(m))
}

#[test]
fn a_loft_you_can_pull_into_shape() {
    let mut d = three_curves();
    // With nothing selected, Loft says what to click.
    action(&mut d, "surf.loft");
    assert!(d.s.hint().starts_with("Loft: click"), "{}", d.s.hint());
    action(&mut d, "edit.selall");
    action(&mut d, "surf.loft");
    assert_eq!(d.faces(), 2);
    assert_eq!(d.s.selection().faces.len(), 2, "the new faces are selected");
    valid(&d);
    // Straight into its cage: pull a middle control point 1.5 m up green.
    action(&mut d, "cage.edit");
    assert_eq!(d.s.cage_points().len(), 12);
    let before = d.s.cage_points()[4];
    d.s.cage_pick(&[4]);
    d.tool(MOVE);
    d.click(DVec3::new(0.0, -2.0, 0.0));
    d.hover(DVec3::new(0.0, -1.0, 0.0));
    d.typed("1.5");
    let after = d.s.cage_points()[4];
    assert!(
        (after - before).distance(DVec3::new(0.0, 1.5, 0.0)) < 1e-9,
        "{after}"
    );
    valid(&d);
    // Two undos: the pull, then the loft.
    d.send(Input::Key { key: Key::Escape });
    undo(&mut d);
    undo(&mut d);
    assert_eq!(d.faces(), 0);
    assert_eq!(d.s.model().edge_count(), 3);
}

#[test]
fn surface_commands_wait_for_clicks_and_show_a_ghost() {
    // Tool pass part 2 (owner: "visual feedback to a tool like that is
    // nice"): Loft with too few curves waits; clicks add curves without
    // Shift; a ghost shows once they make it; Enter makes it.
    let mut d = three_curves();
    d.look(Camera {
        eye: DVec3::new(2.0, -9.0, 3.0),
        target: DVec3::new(2.0, 0.0, 2.5),
    });
    action(&mut d, "surf.loft");
    assert_eq!(d.s.tool(), SELECT);
    assert!(d.s.hint().starts_with("Loft: click"), "{}", d.s.hint());
    d.click(DVec3::new(2.0, 0.0, 0.0));
    assert_eq!(d.s.surface_waiting().map(|w| w.2), Some(false), "one curve");
    let lines = d.s.overlay().lines.len();
    d.click(DVec3::new(2.0, 0.0, 2.0));
    assert_eq!(
        d.s.surface_waiting().map(|w| w.2),
        Some(true),
        "two make it"
    );
    assert!(
        d.s.hint().starts_with("Enter makes the loft"),
        "{}",
        d.s.hint()
    );
    assert!(d.s.overlay().lines.len() > lines, "a ghost of the faces");
    assert_eq!(d.faces(), 0, "only a ghost so far");
    key(&mut d, Key::Enter);
    assert_eq!(d.faces(), 1);
    assert!(d.s.surface_waiting().is_none());
    valid(&d);
    // Esc leaves without making anything.
    undo(&mut d);
    action(&mut d, "edit.desel");
    action(&mut d, "surf.loft");
    assert!(d.s.surface_waiting().is_some());
    key(&mut d, Key::Escape);
    assert!(d.s.surface_waiting().is_none());
    assert_eq!(d.faces(), 0);
    // It waits in Select; the Loft tool's own action picks the tool again,
    // even if the app's dock still showed it as active.
    action(&mut d, "surf.loft");
    assert_eq!(d.s.tool(), SELECT);
    action(&mut d, prism_tools::session::LOFT);
    assert_eq!(d.s.tool(), prism_tools::session::LOFT);
    // With sides picked in the Loft tool, a Surface command only says what
    // it needs: the sides stay.
    d.click(DVec3::new(2.0, 0.0, 0.0));
    let sides = d.s.loft_sides();
    assert!(!sides.is_empty());
    action(&mut d, "surf.revolve");
    assert_eq!(d.s.tool(), prism_tools::session::LOFT);
    assert_eq!(d.s.loft_sides(), sides);
    assert_eq!(d.s.hint(), prism_ops::loft::NO_AXIS);
}

#[test]
fn revolve_turns_a_curve_about_the_selected_line() {
    let mut m = prism_topo::Model::new();
    prism_ops::add_segment(&mut m, DVec3::ZERO, DVec3::new(0.0, 0.0, 4.0));
    let side = prism_geom::Nurbs::open(
        &[
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(2.5, 0.0, 1.0),
            DVec3::new(0.5, 0.0, 2.5),
            DVec3::new(1.5, 0.0, 4.0),
        ],
        3,
    )
    .unwrap();
    prism_ops::add_spline(&mut m, &side);
    let mut d = Driver::new(Document::with_model(m));
    action(&mut d, "edit.selall");
    action(&mut d, "surf.revolve");
    assert_eq!(d.faces(), 2);
    valid(&d);
    let scene = d.s.scene();
    for (id, _) in d.s.model().faces() {
        assert!(scene.face(id).is_some_and(|f| f.tris.len() > 300));
    }
    undo(&mut d);
    assert_eq!(d.faces(), 0);
}

// ---------- Freeform NURBS part 5: Sweep and Patch ----------

#[test]
fn sweep_and_patch_run_from_the_selection() {
    let curve = |pts: &[[f64; 3]]| {
        let pts: Vec<DVec3> = pts.iter().map(|p| DVec3::from_array(*p)).collect();
        prism_geom::Nurbs::open(&pts, 3).unwrap()
    };
    // Sweep: a profile along a path.
    let mut m = prism_topo::Model::new();
    prism_ops::add_spline(
        &mut m,
        &curve(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 2.0],
            [3.0, 0.0, -1.0],
            [4.0, 0.0, 0.0],
        ]),
    );
    prism_ops::add_spline(
        &mut m,
        &curve(&[
            [0.0, 0.0, 0.0],
            [0.0, 2.0, 1.0],
            [1.0, 4.0, 0.0],
            [0.0, 6.0, 0.0],
        ]),
    );
    let mut d = Driver::new(Document::with_model(m));
    action(&mut d, "surf.sweep");
    assert!(d.s.hint().starts_with("Sweep: click"), "{}", d.s.hint());
    action(&mut d, "edit.selall");
    action(&mut d, "surf.sweep");
    assert_eq!(d.faces(), 1);
    valid(&d);
    undo(&mut d);
    assert_eq!(d.faces(), 0);
    // Patch: a ring of four.
    let mut m = prism_topo::Model::new();
    for c in [
        curve(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 1.5],
            [3.0, 0.0, 1.5],
            [4.0, 0.0, 0.0],
        ]),
        curve(&[[4.0, 0.0, 0.0], [4.5, 1.0, 1.0], [4.0, 3.0, 0.0]]),
        curve(&[[4.0, 3.0, 0.0], [2.0, 3.0, -1.0], [0.0, 3.0, 0.0]]),
        curve(&[[0.0, 3.0, 0.0], [-0.5, 1.5, 0.5], [0.0, 0.0, 0.0]]),
    ] {
        prism_ops::add_spline(&mut m, &c);
    }
    let mut d = Driver::new(Document::with_model(m));
    action(&mut d, "edit.selall");
    action(&mut d, "surf.patch");
    assert_eq!(d.faces(), 1);
    assert_eq!(d.s.model().edge_count(), 4, "the four curves are its sides");
    valid(&d);
}

// ---------- Freeform NURBS part 6: curvature display ----------

#[test]
fn zebra_and_combs_are_view_toggles_and_combs_follow_the_bend() {
    let mut d = drawn_curve();
    let lines = |d: &Driver| d.s.overlay().lines.len();
    d.tool(SELECT);
    let on = {
        let m = d.s.model();
        let (_, e) = m.edges().next().unwrap();
        e.curve.point(m.vertex(e.a), m.vertex(e.b), 0.3)
    };
    d.click(on);
    assert!(d.s.curvature_combs().is_empty(), "off by default");
    let before = lines(&d);
    action(&mut d, "view.combs");
    assert!(d.s.view_style().combs);
    let combs = d.s.curvature_combs();
    assert_eq!(combs.len(), 1);
    assert!(lines(&d) > before + 30);
    // Teeth point away from the centre of the bend, and the longest is
    // 15 % of the curve's length.
    let comb = &combs[0];
    let longest = comb.iter().map(|(p, t)| p.distance(*t)).fold(0.0, f64::max);
    let length: f64 = comb.windows(2).map(|w| w[0].0.distance(w[1].0)).sum();
    assert!((longest / length - 0.15).abs() < 0.01, "{longest} {length}");
    let (p, tip) = comb[comb.len() / 2];
    assert!(tip.y > p.y, "the curve arches up, so its teeth point up");
    // Nothing selected: none.
    d.click(DVec3::new(8.0, 8.0, 0.0));
    assert!(d.s.curvature_combs().is_empty());
    action(&mut d, "view.zebra");
    assert!(d.s.view_style().zebra);
    action(&mut d, "view.zebra");
    assert!(!d.s.view_style().zebra);
}

// ---------- Everyday Commands part 1 (brief 33): clipboard and lock ----------

#[test]
fn copy_then_paste_follows_the_cursor_and_a_click_places_it() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    act(&mut d, "edit.copy");
    act(&mut d, "edit.paste");
    assert!(d.s.in_operation(), "the copy follows the cursor");
    assert!(d.s.hint().contains("place the copy"));
    // Held at the low corner of its box: a click on the ground at (5, 0)
    // puts that corner there, snapped like Move.
    d.click(DVec3::new(5.0, 0.0, 0.0));
    assert!(!d.s.in_operation());
    let asm = d.s.document().assembly();
    let groups: Vec<_> = asm.root().instances.keys().copied().collect();
    assert_eq!(groups.len(), 2);
    let new = *groups.iter().find(|i| **i != g).expect("pasted");
    let w = asm.world_of(&[new]).expect("placed").translation;
    assert!((w - DVec3::new(5.0, 0.0, 0.0)).length() < 1e-6, "{w:?}");
    // The pasted group is selected; one undo removes it.
    assert_eq!(
        d.s.selected_groups().iter().copied().collect::<Vec<_>>(),
        [new]
    );
    undo(&mut d);
    assert_eq!(d.s.document().assembly().root().instances.len(), 1);
}

#[test]
fn esc_cancels_a_paste_and_paste_in_place_puts_it_back() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(SELECT);
    act(&mut d, "edit.selall");
    act(&mut d, "edit.cut");
    assert_eq!(d.faces(), 0, "cut deletes");
    act(&mut d, "edit.paste");
    d.hover(DVec3::new(4.0, 4.0, 0.0));
    key(&mut d, Key::Escape);
    assert!(!d.s.in_operation());
    assert_eq!(d.faces(), 0, "nothing pasted");
    act(&mut d, "edit.pasteip");
    assert_eq!(d.faces(), 1, "back where it was");
    let (lo, hi) = d.s.model().bbox().expect("box");
    assert!(lo.length() < 1e-9 && (hi - DVec3::new(2.0, 2.0, 0.0)).length() < 1e-9);
    assert_eq!(
        d.s.selection().faces.len(),
        1,
        "what was pasted is selected"
    );
}

#[test]
fn the_clipboard_goes_to_another_document() {
    let mut d = Driver::empty();
    grouped_box(&mut d);
    act(&mut d, "edit.copy");
    let clip = d.s.clipboard().expect("copied");
    let mut e = Driver::empty();
    e.s.set_clipboard(Some(clip));
    act(&mut e, "edit.pasteip");
    assert_eq!(e.s.document().assembly().root().instances.len(), 1);
}

#[test]
fn a_locked_group_cannot_be_moved_erased_or_opened() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    act(&mut d, "edit.lock");
    assert!(d.s.document().assembly().is_locked(g));
    // Move does not start, and says why.
    d.tool(MOVE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    assert!(!d.s.in_operation());
    assert_eq!(d.s.hint(), prism_tools::session::LOCKED);
    // Delete and Explode are refused; a double-click does not open it.
    d.tool(SELECT);
    act(&mut d, "edit.delete");
    act(&mut d, "edit.explode");
    assert_eq!(d.s.document().assembly().root().instances.len(), 1);
    act(&mut d, "edit.desel");
    double_click(&mut d, DVec3::new(1.0, 0.0, 1.0));
    assert!(d.s.open_path().is_empty());
    // Copy works, and the copy is not locked.
    act(&mut d, "edit.selall");
    act(&mut d, "edit.copy");
    act(&mut d, "edit.pasteip");
    let asm = d.s.document().assembly();
    let copy = asm.root().instances.keys().find(|i| **i != g).copied();
    assert!(copy.is_some_and(|c| !asm.is_locked(c)));
    // Unlock: it moves again.
    act(&mut d, "edit.desel");
    d.click(DVec3::new(1.0, 0.0, 1.0));
    act(&mut d, "edit.unlock");
    assert!(!d.s.document().assembly().is_locked(g));
}

#[test]
fn with_context_adds_edges_inside_the_open_group_only() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    // A wall standing through the box, at the root.
    d.s.handle(&Event {
        t: 0.0,
        input: Input::Action { id: "noop".into() },
    });
    let mut doc = d.s.document().clone();
    doc.run(prism_doc::Command::Loop {
        points: vec![
            DVec3::new(1.0, -1.0, -1.0),
            DVec3::new(1.0, 3.0, -1.0),
            DVec3::new(1.0, 3.0, 3.0),
            DVec3::new(1.0, -1.0, 3.0),
        ],
    });
    d.s.open_document(doc, Some(d.s.camera()));
    let outside = d.s.model().to_json();
    d.s.open_group(g);
    act(&mut d, "isect.ctx");
    let def = d.s.document().assembly().context_def(&[g]).unwrap();
    let inside = &d.s.document().assembly().def(def).unwrap().model;
    assert!(
        inside.face_count() > 6,
        "the box is cut where the wall crosses"
    );
    assert!(prism_ops::check::problems(inside).is_empty());
    assert_eq!(d.s.model().to_json(), outside, "outside unchanged");
}

// ---------- Everyday Commands part 2 (brief 33): the camera ----------

fn drag_px(d: &mut Driver, a: DVec2, b: DVec2) {
    d.send(Input::Move { x: a.x, y: a.y });
    d.send(Input::Press {
        button: Button::Left,
        x: a.x,
        y: a.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    for k in 1..=4 {
        let p = a.lerp(b, f64::from(k) / 4.0);
        d.send(Input::Move { x: p.x, y: p.y });
    }
    d.send(Input::Release {
        button: Button::Left,
    });
    d.t += 0.5;
}

#[test]
fn standard_views_and_parallel_projection_and_drawing_in_them() {
    let mut d = Driver::empty();
    grouped_box(&mut d);
    act(&mut d, "cam.v.front");
    let f = d.s.camera().forward();
    assert!((f - DVec3::Y).length() < 1e-9, "{f:?}");
    act(&mut d, "cam.parallel");
    assert!(d.s.lens().parallel);
    assert!(d.s.view().parallel.is_some());
    // Seen from the front in parallel, the box's front face is a square on
    // screen: its corners are level and plumb.
    let a = d.screen(DVec3::new(0.0, 0.0, 0.0));
    let b = d.screen(DVec3::new(2.0, 0.0, 2.0));
    let c = d.screen(DVec3::new(2.0, 2.0, 2.0));
    assert!(((b.x - a.x) - (a.y - b.y)).abs() < 1e-6, "square");
    assert!((b - c).length() < 1e-6, "depth does not shrink anything");
    // A line drawn over the box's front face lands on it (picking and
    // inference work in a parallel view).
    d.tool(LINE);
    act(&mut d, "edit.desel");
    d.click(DVec3::new(0.5, 0.0, 0.7));
    d.click(DVec3::new(1.5, 0.0, 1.3));
    key(&mut d, Key::Escape);
    let m = d.s.model();
    assert_eq!(m.edge_count(), 1);
    let (lo, hi) = m.bbox().expect("a line");
    assert!(lo.distance(DVec3::new(0.5, 0.0, 0.7)) < 1e-6, "{lo:?}");
    assert!(hi.distance(DVec3::new(1.5, 0.0, 1.3)) < 1e-6, "{hi:?}");
    act(&mut d, "cam.persp");
    assert!(!d.s.lens().parallel);
    act(&mut d, "cam.v.top");
    assert!(d.s.camera().forward().z < -0.999);
}

#[test]
fn field_of_view_is_typed_with_the_zoom_tool() {
    let mut d = Driver::empty();
    act(&mut d, "cam.fov");
    assert_eq!(d.s.tool(), prism_tools::session::ZOOM);
    assert_eq!(d.s.measurement().0, "Field of View");
    assert!(d.s.accepts_char('5'));
    d.typed("50");
    assert!((d.s.lens().fov - 50.0).abs() < 1e-12);
    d.typed("500deg");
    assert_eq!(
        d.s.lens().fov,
        prism_view::camera::FOV_MAX_DEG,
        "kept in range"
    );
}

#[test]
fn zoom_selection_and_zoom_window_fit_what_is_asked() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 0.5, 0.5);
    d.tool(SELECT);
    act(&mut d, "edit.selall");
    let far = d.s.camera().distance();
    act(&mut d, "cam.zoomsel");
    let cam = d.s.camera();
    assert!(cam.distance() < far, "closer to the small square");
    assert!(cam.target.distance(DVec3::new(0.25, 0.25, 0.0)) < 1e-9);
    // Zoom Window: a box a quarter of the view wide brings the view four
    // times closer to the box's middle, and Esc goes back to Select.
    act(&mut d, "edit.desel");
    d.tool(prism_tools::session::WINDOW);
    let before = d.s.camera().distance();
    drag_px(&mut d, DVec2::new(400.0, 300.0), DVec2::new(650.0, 400.0));
    let after = d.s.camera().distance();
    assert!((after / before - 0.25).abs() < 1e-6, "{}", after / before);
    key(&mut d, Key::Escape);
    assert_eq!(d.s.tool(), SELECT);
}

#[test]
fn look_around_and_walk_by_dragging() {
    let mut d = Driver::empty();
    let start = d.s.camera();
    d.tool(prism_tools::session::LOOK);
    drag_px(&mut d, DVec2::new(500.0, 350.0), DVec2::new(560.0, 350.0));
    let looked = d.s.camera();
    assert_eq!(looked.eye, start.eye, "it turns about the eye");
    assert!((looked.forward() - start.forward()).length() > 1e-3);
    d.tool(prism_tools::session::WALK);
    drag_px(&mut d, DVec2::new(500.0, 400.0), DVec2::new(500.0, 300.0));
    let walked = d.s.camera();
    assert!(
        (walked.eye.z - looked.eye.z).abs() < 1e-9,
        "the height is kept"
    );
    assert!(walked.eye.distance(looked.eye) > 1e-3, "it went forward");
}

#[test]
fn walk_steps_with_the_arrow_keys_and_takes_a_typed_eye_height() {
    let mut d = Driver::empty();
    d.tool(prism_tools::session::WALK);
    assert_eq!(d.s.measurement().0, "Eye Height");
    let start = d.s.camera();
    d.send(Input::Key { key: Key::Up });
    let ahead = d.s.camera();
    assert!(ahead.eye.distance(start.eye) > 1e-3, "a step forward");
    assert!((ahead.eye.z - start.eye.z).abs() < 1e-9, "on the level");
    assert!((ahead.forward() - start.forward()).length() < 1e-9);
    d.send(Input::Key { key: Key::Down });
    assert!(d.s.camera().eye.distance(start.eye) < 1e-9, "and back");
    d.send(Input::Key { key: Key::Left });
    let turned = d.s.camera();
    assert_eq!(turned.eye, start.eye, "it turns where it stands");
    let (f0, f1) = (start.forward(), turned.forward());
    let deg = f0.truncate().angle_to(f1.truncate()).to_degrees();
    assert!((deg - 5.0).abs() < 1e-6, "five degrees left: {deg}");
    // A typed eye height: the eye goes to it, still looking the same way.
    assert!(d.s.accepts_char('1'), "typing goes to the eye height");
    d.typed("1.6");
    let tall = d.s.camera();
    assert!((tall.eye.z - 1.6).abs() < 1e-9, "{}", tall.eye.z);
    assert!((tall.forward() - turned.forward()).length() < 1e-9);
    assert_eq!(d.s.tool(), prism_tools::session::WALK);
}

// ---------- Everyday Commands part 3: Rotated Rectangle and Freehand ----------

#[test]
fn a_rotated_rectangle_from_three_clicks_on_a_slant() {
    use prism_tools::session::RRECT;
    let mut d = Driver::empty();
    d.tool(RRECT);
    d.click(DVec3::ZERO);
    d.click(DVec3::new(2.0, 2.0, 0.0));
    assert!(d.s.in_operation());
    assert_eq!(d.s.measurement().0, "Width");
    // The width is measured square to the first side.
    d.click(DVec3::new(0.0, 2.0, 0.0));
    assert_eq!(d.faces(), 1);
    assert!(!d.s.in_operation());
    for p in [(2.0, 2.0), (1.0, 3.0), (-1.0, 1.0)] {
        assert!(has_point(&d, p.0, p.1), "{p:?}");
    }
    valid(&d);
    // One undo removes it.
    undo(&mut d);
    assert_eq!(d.s.model().edge_count(), 0);
}

#[test]
fn a_rotated_rectangle_typed_with_a_length_then_width_and_tilt() {
    use prism_tools::session::RRECT;
    let mut d = Driver::empty();
    d.tool(RRECT);
    d.click(DVec3::new(5.0, 0.0, 0.0));
    d.hover(DVec3::new(6.0, 0.3, 0.0));
    d.send(Input::Key { key: Key::Right });
    d.typed("3");
    assert_eq!(d.s.measurement().0, "Width");
    // Tilted 90 degrees up from the ground: a wall.
    d.hover(DVec3::new(6.0, 1.0, 0.0));
    d.typed("1,90");
    assert_eq!(d.faces(), 1);
    let m = d.s.model();
    for p in [
        DVec3::new(8.0, 0.0, 0.0),
        DVec3::new(8.0, 0.0, 1.0),
        DVec3::new(5.0, 0.0, 1.0),
    ] {
        assert!(prism_ops::vertex_at(m, p).is_some(), "{p}");
    }
    valid(&d);
}

#[test]
fn esc_cancels_a_rotated_rectangle() {
    use prism_tools::session::RRECT;
    let mut d = Driver::empty();
    d.tool(RRECT);
    d.click(DVec3::ZERO);
    d.click(DVec3::new(2.0, 0.0, 0.0));
    d.send(Input::Key { key: Key::Escape });
    assert!(!d.s.in_operation());
    d.click(DVec3::new(1.0, 1.0, 0.0));
    assert_eq!(d.faces(), 0);
}

/// A freehand stroke through world points (each one a mouse move).
fn stroke(d: &mut Driver, pts: &[DVec3]) {
    stroke_with(d, pts, false);
}

fn stroke_with(d: &mut Driver, pts: &[DVec3], shift: bool) {
    let a = d.screen(pts[0]);
    d.send(Input::Move { x: a.x, y: a.y });
    d.send(Input::Press {
        button: Button::Left,
        x: a.x,
        y: a.y,
        shift,
        ctrl: false,
        alt: false,
    });
    for p in &pts[1..] {
        let s = d.screen(*p);
        d.send(Input::Move { x: s.x, y: s.y });
    }
    d.send(Input::Release {
        button: Button::Left,
    });
    d.t += 0.5;
}

#[test]
fn a_freehand_stroke_becomes_one_smooth_curve_on_the_ground() {
    use prism_tools::session::FREEHAND;
    let mut d = Driver::empty();
    d.tool(FREEHAND);
    let arc: Vec<DVec3> = (0..=60)
        .map(|k| {
            let t = f64::from(k) / 60.0 * std::f64::consts::PI;
            DVec3::new(3.0 + 2.0 * t.cos(), 2.0 * t.sin(), 0.0)
        })
        .collect();
    stroke(&mut d, &arc);
    assert!(!d.s.in_operation());
    assert_eq!(d.s.model().edge_count(), 1);
    assert_eq!(spline_edges(&d), 1);
    let m = d.s.model();
    let (_, e) = m.edges().next().unwrap();
    let c = &e.curve.spline().unwrap().curve;
    // Flat on the ground, close to the stroke, with far fewer points.
    assert!(c.cage().len() < 30, "{}", c.cage().len());
    let (t0, t1) = c.domain();
    for k in 0..=20 {
        let p = c.eval(t0 + (t1 - t0) * f64::from(k) / 20.0);
        assert!(p.z.abs() < 1e-9, "{p}");
        let r = (p - DVec3::new(3.0, 0.0, 0.0)).length();
        assert!((r - 2.0).abs() < 0.05, "{p} {r}");
    }
    valid(&d);
}

#[test]
fn a_freehand_stroke_with_shift_runs_over_faces_in_3d() {
    use prism_tools::session::FREEHAND;
    // A 2 m box; the stroke starts on the ground beside it and runs onto
    // its top.
    let path: Vec<DVec3> = (0..=40)
        .map(|k| {
            let t = f64::from(k) / 40.0;
            let x = 5.0 - 4.0 * t;
            let y = 1.0 + 0.4 * (t * 6.0).sin();
            let z = if x < 2.0 { 2.0 } else { 0.0 };
            DVec3::new(x, y, z)
        })
        .collect();
    let heights = |shift: bool| {
        let mut d = Driver::empty();
        rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
        d.tool(PUSH);
        d.click(DVec3::new(1.0, 1.0, 0.0));
        d.hover(DVec3::new(1.0, 1.0, 0.5));
        d.typed("2");
        d.tool(FREEHAND);
        stroke_with(&mut d, &path, shift);
        valid(&d);
        let m = d.s.model();
        let c = m
            .edges()
            .find_map(|(_, e)| e.curve.spline().map(|s| s.curve.clone()))
            .expect("a curve");
        let (t0, t1) = c.domain();
        let zs: Vec<f64> = (0..=40)
            .map(|k| c.eval(t0 + (t1 - t0) * f64::from(k) / 40.0).z)
            .collect();
        (
            zs.iter().copied().fold(f64::MAX, f64::min),
            zs.iter().copied().fold(f64::MIN, f64::max),
        )
    };
    // Without Shift it stays on the ground plane it started on.
    let (lo, hi) = heights(false);
    assert!(lo.abs() < 1e-9 && hi.abs() < 1e-9, "{lo} {hi}");
    // With Shift it climbs onto the top.
    let (lo, hi) = heights(true);
    assert!(lo < 0.1 && hi > 1.9, "{lo} {hi}");
}

#[test]
fn a_closed_freehand_stroke_makes_a_face() {
    use prism_tools::session::FREEHAND;
    let mut d = Driver::empty();
    d.tool(FREEHAND);
    let ring: Vec<DVec3> = (0..=80)
        .map(|k| {
            let t = f64::from(k) / 80.0 * std::f64::consts::TAU;
            DVec3::new(3.0 + 1.5 * t.cos(), 2.0 + 1.5 * t.sin(), 0.0)
        })
        .collect();
    stroke(&mut d, &ring);
    assert_eq!(d.faces(), 1);
    valid(&d);
    undo(&mut d);
    assert_eq!(d.s.model().edge_count(), 0);
}

#[test]
fn freehand_on_a_wall_stays_flat_on_it() {
    use prism_tools::session::FREEHAND;
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 4.0);
    d.tool(PUSH);
    d.click(DVec3::new(2.0, 2.0, 0.0));
    d.hover(DVec3::new(2.0, 2.0, 0.5));
    d.typed("1");
    assert_eq!(d.faces(), 6);
    d.tool(FREEHAND);
    // A ring on the front wall (y = 0).
    let ring: Vec<DVec3> = (0..=80)
        .map(|k| {
            let t = f64::from(k) / 80.0 * std::f64::consts::TAU;
            DVec3::new(2.0 + 0.4 * t.cos(), 0.0, 0.5 + 0.4 * t.sin())
        })
        .collect();
    stroke(&mut d, &ring);
    assert_eq!(d.faces(), 7);
    let m = d.s.model();
    let (_, e) = m.edges().find(|(_, e)| e.curve.is_spline()).unwrap();
    assert!(
        e.curve
            .spline()
            .unwrap()
            .curve
            .cage()
            .iter()
            .all(|p| p.y.abs() < 1e-9)
    );
    valid(&d);
}

/// The owner's arc (2026-11-13): started on a box's top edge and ended at a
/// bottom corner, the arc lies on the side face that holds both, not on
/// the top face the first click was over.
#[test]
fn a_two_point_arc_turns_to_the_face_holding_both_ends() {
    use prism_tools::session::ARC;
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 4.0);
    d.tool(PUSH);
    d.click(DVec3::new(2.0, 2.0, 0.0));
    d.hover(DVec3::new(2.0, 2.0, 0.5));
    d.typed("4");
    for start in [DVec3::new(0.0, 0.0, 4.0), DVec3::new(2.0, 0.0, 4.0)] {
        d.tool(ARC);
        d.click(start);
        d.click(DVec3::new(4.0, 0.0, 0.0));
        // The bulge, on the front face (y = 0).
        d.click(DVec3::new(3.2, 0.0, 3.0));
        let m = d.s.model();
        let arc = m
            .edges()
            .find_map(|(_, e)| match e.curve {
                prism_geom::Curve::Arc { centre, axis, .. } => Some((centre, axis)),
                _ => None,
            })
            .expect("an arc");
        assert!(arc.1.y.abs() > 0.999, "{start} {arc:?}");
        assert!(arc.0.y.abs() < 1e-9, "{start} {arc:?}");
        assert!(prism_ops::vertex_at(m, start).is_some());
        assert!(prism_ops::vertex_at(m, DVec3::new(4.0, 0.0, 0.0)).is_some());
        valid(&d);
        undo(&mut d);
    }
}

// ---------- Everyday Commands part 4 (brief 33): Smart Guides ----------

/// Groups in a row along x, 2 m cubes on the ground at 0, 3 and `last`,
/// seen from far enough to show them all.
fn row_of_groups(last: f64) -> (Driver, prism_doc::InstanceId) {
    let mut d = Driver::empty();
    d.look(Camera {
        eye: DVec3::new(6.0, -12.0, 12.0),
        target: DVec3::new(6.0, 1.0, 0.0),
    });
    // The pushed box goes down from the ground: copy it, and paste the
    // copies on the ground.
    grouped_box(&mut d);
    act(&mut d, "edit.copy");
    act(&mut d, "edit.delete");
    for x in [0.0, 3.0, last] {
        act(&mut d, "edit.paste");
        d.click(DVec3::new(x, 0.0, 0.0));
    }
    let asm = d.s.document().assembly();
    let last_group = *asm
        .root()
        .instances
        .iter()
        .find(|(_, i)| (i.transform.translation.x - last).abs() < 1e-9)
        .expect("the last copy")
        .0;
    act(&mut d, "edit.desel");
    (d, last_group)
}

fn placed(d: &Driver, g: prism_doc::InstanceId) -> DVec3 {
    d.s.document()
        .assembly()
        .world_of(&[g])
        .expect("placed")
        .translation
}

fn is_one(gap: Option<f64>) -> bool {
    gap.is_some_and(|g| (g - 1.0).abs() < 1e-9)
}

fn spacing(d: &Driver) -> Option<f64> {
    d.s.smart_shown().iter().find_map(|g| match g {
        prism_tools::smart::Guide::Space { gap, .. } => Some(*gap),
        _ => None,
    })
}

#[test]
fn a_moved_group_snaps_to_the_gap_its_neighbours_leave() {
    let (mut d, c) = row_of_groups(10.0);
    d.tool(MOVE);
    // Grab the third box by its bottom edge's midpoint, and slide it to
    // just past a gap of 1 after the second box (4.02 rather than 4).
    d.click(DVec3::new(11.0, 0.0, 0.0));
    assert_eq!(
        d.s.selected_groups().iter().copied().collect::<Vec<_>>(),
        [c]
    );
    d.hover(DVec3::new(7.02, -1.0, 0.0));
    assert!(is_one(spacing(&d)), "{:?}", d.s.smart_shown());
    let o = d.s.overlay();
    assert!(
        o.labels.iter().any(|l| l.text.starts_with("d: ")),
        "the d: label"
    );
    assert!(
        o.lines
            .iter()
            .any(|l| l.color == d.s.style.feedback.smart_space),
        "the arrows"
    );
    d.click(DVec3::new(7.02, -1.0, 0.0));
    let w = placed(&d, c);
    assert!((w - DVec3::new(6.0, -1.0, 0.0)).length() < 1e-9, "{w:?}");
    assert!(d.s.smart_shown().is_empty(), "the guides go with the move");
}

/// Smart Guides in Scale (gaps session): a box scaled up by its top grip
/// takes a neighbour's height, and the side being dragged lines up with
/// the neighbour's top.
#[test]
fn a_scaled_group_takes_a_neighbours_height() {
    let (mut d, c) = row_of_groups(10.0);
    // The first box made 4 m tall.
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 0.0, 1.0));
    d.tool(SCALE);
    d.click(DVec3::new(1.0, 1.0, 2.1));
    d.typed("2");
    act(&mut d, "edit.desel");
    // The third box's top grip, dragged to 3.97 m tall.
    d.tool(SELECT);
    d.click(DVec3::new(11.0, 0.0, 1.0));
    d.tool(SCALE);
    d.click(DVec3::new(11.0, 1.0, 2.1));
    // The drawn box stands 0.1 off the cube: t = (z + 0.1) / 2.2.
    let z = 1.985 * 2.2 - 0.1;
    d.hover(DVec3::new(11.0, 1.0, z));
    let size = d.s.smart_shown().iter().find_map(|g| match g {
        prism_tools::smart::Guide::Size { axis: 2, size, .. } => Some(*size),
        _ => None,
    });
    assert!(
        size.is_some_and(|h| (h - 4.0).abs() < 1e-9),
        "{:?}",
        d.s.smart_shown()
    );
    let aligned = d.s.smart_shown().iter().any(|g| {
        matches!(g, prism_tools::smart::Guide::Align { axis: 2, at, .. } if (*at - 4.0).abs() < 1e-9)
    });
    assert!(
        aligned,
        "the top meets the first box's top: {:?}",
        d.s.smart_shown()
    );
    assert!(
        d.s.overlay()
            .labels
            .iter()
            .any(|l| l.text.starts_with("Height "))
    );
    d.click(DVec3::new(11.0, 1.0, z));
    let w = d.s.document().assembly().world_of(&[c]).expect("placed");
    assert!((w.matrix3.z_axis.z - 2.0).abs() < 1e-9, "{w:?}");
    assert!(d.s.smart_shown().is_empty());
}

/// Smart Guides in Rotate: a box turned near a neighbour's angle turns to
/// it exactly.
#[test]
fn a_turned_group_faces_the_way_its_neighbour_does() {
    let (mut d, c) = row_of_groups(10.0);
    let deg = |a: f64| a.to_radians();
    // The first box turned 37° about its corner on the ground.
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 0.0, 1.0));
    d.tool(ROTATE);
    d.click(DVec3::new(1.0, -1.0, 0.0));
    d.click(DVec3::new(2.0, -1.0, 0.0));
    d.typed("37");
    act(&mut d, "edit.desel");
    // The third box turned to 35.5°: pulled to 37°.
    d.tool(SELECT);
    d.click(DVec3::new(11.0, 0.0, 1.0));
    d.tool(ROTATE);
    d.click(DVec3::new(11.0, -1.0, 0.0));
    d.click(DVec3::new(12.0, -1.0, 0.0));
    let at = DVec3::new(
        11.0 + 2.0 * deg(35.5).cos(),
        -1.0 + 2.0 * deg(35.5).sin(),
        0.0,
    );
    d.hover(at);
    assert!(
        d.s.smart_shown()
            .iter()
            .any(|g| matches!(g, prism_tools::smart::Guide::Turn { square: false, .. })),
        "{:?}",
        d.s.smart_shown()
    );
    assert!(d.s.overlay().labels.iter().any(|l| l.text == "Parallel"));
    d.click(at);
    let x =
        d.s.document()
            .assembly()
            .world_of(&[c])
            .expect("placed")
            .matrix3
            .x_axis;
    assert!(
        (x.y.atan2(x.x) - deg(37.0)).abs() < 1e-9,
        "{}",
        x.y.atan2(x.x).to_degrees()
    );
}

/// Angle Guides (brief 38): a box turned near the angle of a neighbour's
/// edge turns to make them parallel, though the neighbour's heading (its
/// longest edge) points another way.
#[test]
fn a_turned_group_matches_the_angle_of_a_neighbours_edge() {
    let (mut d, c) = row_of_groups(10.0);
    let deg = |a: f64| a.to_radians();
    // A triangle on the ground, away from the boxes: its longest side
    // along x, another at 37°.
    let a = DVec3::new(5.0, 6.0, 0.0);
    let tip = a + DVec3::new(deg(37.0).cos(), deg(37.0).sin(), 0.0) * 2.0;
    d.tool(LINE);
    for p in [a, DVec3::new(9.0, 6.0, 0.0), tip, a] {
        d.click(p);
    }
    key(&mut d, Key::Escape);
    let m = d.s.work_model();
    assert!(
        m.vertices().any(|(_, p)| p.distance(tip) < 1e-9) && m.faces().count() == 1,
        "the triangle as drawn"
    );
    act(&mut d, "edit.desel");
    // The third box turned to 35.5°: pulled to 37°.
    d.tool(SELECT);
    d.click(DVec3::new(11.0, 0.0, 1.0));
    d.tool(ROTATE);
    d.click(DVec3::new(11.0, -1.0, 0.0));
    d.click(DVec3::new(12.0, -1.0, 0.0));
    let at = DVec3::new(
        11.0 + 2.0 * deg(35.5).cos(),
        -1.0 + 2.0 * deg(35.5).sin(),
        0.0,
    );
    d.hover(at);
    assert!(
        d.s.smart_shown()
            .iter()
            .any(|g| matches!(g, prism_tools::smart::Guide::Angle { .. })),
        "{:?}",
        d.s.smart_shown()
    );
    assert!(d.s.overlay().labels.iter().any(|l| l.text == "Parallel"));
    d.click(at);
    let x =
        d.s.document()
            .assembly()
            .world_of(&[c])
            .expect("placed")
            .matrix3
            .x_axis;
    assert!(
        (x.y.atan2(x.x) - deg(37.0)).abs() < 1e-9,
        "{}",
        x.y.atan2(x.x).to_degrees()
    );
}

#[test]
fn a_moved_group_lines_up_with_a_neighbour_and_ctrl_u_turns_it_off() {
    let (mut d, c) = row_of_groups(10.0);
    // Slide the third box towards y = -3 and x = 3.03: its low side meets
    // the second box's low side in x (y is far from anything).
    d.tool(MOVE);
    d.click(DVec3::new(11.0, 0.0, 0.0));
    d.hover(DVec3::new(4.03, -3.0, 0.0));
    let aligned = d.s.smart_shown().iter().any(|g| {
        matches!(g, prism_tools::smart::Guide::Align { axis: 0, at, .. } if (*at - 3.0).abs() < 1e-9)
    });
    assert!(aligned, "{:?}", d.s.smart_shown());
    assert!(d.s.overlay().labels.iter().any(|l| l.text == "Align X"));
    d.click(DVec3::new(4.03, -3.0, 0.0));
    assert!((placed(&d, c).x - 3.0).abs() < 1e-9);
    undo(&mut d);

    // Off (View > Smart Guides): the cursor's point is kept.
    d.s.set_smart_guides(false);
    d.tool(SELECT);
    act(&mut d, "edit.desel");
    d.tool(MOVE);
    d.click(DVec3::new(11.0, 0.0, 0.0));
    d.hover(DVec3::new(4.03, -3.0, 0.0));
    assert!(d.s.smart_shown().is_empty() && d.s.smart_level().is_none());
    d.click(DVec3::new(4.03, -3.0, 0.0));
    assert!((placed(&d, c).x - 3.03).abs() < 1e-6, "{}", placed(&d, c).x);
}

#[test]
fn a_paste_lines_up_and_spaces_like_a_move() {
    let (mut d, _) = row_of_groups(10.0);
    act(&mut d, "edit.copy");
    assert!(d.s.selected_groups().is_empty());
    // Copy the first box again, and paste it just short of a gap of 1
    // before the first box (low corner at -3.03, the gap ends at -1).
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 0.0, 1.0));
    act(&mut d, "edit.copy");
    act(&mut d, "edit.paste");
    d.hover(DVec3::new(-3.02, -1.0, 0.0));
    assert!(is_one(spacing(&d)), "{:?}", d.s.smart_shown());
    d.click(DVec3::new(-3.02, -1.0, 0.0));
    let asm = d.s.document().assembly();
    let xs: Vec<f64> = asm
        .root()
        .instances
        .values()
        .map(|i| i.transform.translation.x)
        .collect();
    assert!(xs.iter().any(|x| (x + 3.0).abs() < 1e-9), "{xs:?}");
}

#[test]
fn a_snapped_point_wins_over_the_guides() {
    let (mut d, c) = row_of_groups(10.0);
    d.tool(MOVE);
    d.click(DVec3::new(10.0, 0.0, 0.0));
    // The grabbed corner onto the second box's far bottom corner (5, 0, 0):
    // an endpoint, kept exactly though a gap guide is near.
    d.hover(DVec3::new(5.0, 0.0, 0.0));
    assert!(d.s.smart_shown().is_empty());
    d.click(DVec3::new(5.0, 0.0, 0.0));
    assert!((placed(&d, c) - DVec3::new(5.0, 0.0, 0.0)).length() < 1e-9);
}

#[test]
fn a_loose_body_gets_guides_but_part_of_one_does_not() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    rect_on_ground(&mut d, 3.0, 0.0, 5.0, 2.0);
    rect_on_ground(&mut d, 8.0, 0.0, 10.0, 2.0);
    // The whole third square (a double-click takes it and its edges).
    d.tool(SELECT);
    double_click(&mut d, DVec3::new(9.0, 1.0, 0.0));
    assert_eq!(d.s.selection().edges.len(), 4);
    d.tool(MOVE);
    d.click(DVec3::new(9.0, 2.0, 0.0));
    d.hover(DVec3::new(7.02, 1.0, 0.0));
    assert!(is_one(spacing(&d)), "{:?}", d.s.smart_shown());
    key(&mut d, Key::Escape);
    // One edge of it: no guides.
    d.tool(SELECT);
    act(&mut d, "edit.desel");
    d.click(DVec3::new(9.0, 0.0, 0.0));
    assert_eq!(d.s.selection().edges.len(), 1);
    d.tool(MOVE);
    d.click(DVec3::new(9.0, 0.0, 0.0));
    d.hover(DVec3::new(7.02, 0.5, 0.0));
    assert!(d.s.smart_level().is_none());
}

static FAKE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A clock that moves 10 ms every time it is read: everything is slow.
fn slow_clock() -> f64 {
    FAKE_MS.fetch_add(10, std::sync::atomic::Ordering::SeqCst) as f64 / 1e3
}

#[test]
fn over_the_time_budget_spacing_goes_then_everything_until_the_next_move() {
    use prism_tools::smart::Level;
    let (mut d, _) = row_of_groups(10.0);
    d.s.measure = Some(slow_clock);
    d.tool(MOVE);
    d.click(DVec3::new(11.0, 0.0, 0.0));
    let mut levels = Vec::new();
    for i in 0..12 {
        d.hover(DVec3::new(7.02 - f64::from(i) * 0.001, -1.0, 0.0));
        levels.push(d.s.smart_level().expect("working"));
    }
    // One step down for every three slow adjustments, in order.
    assert!(levels.windows(2).all(|w| w[0] <= w[1]), "{levels:?}");
    for l in [Level::AlignOnly, Level::Near, Level::Paused] {
        assert!(levels.contains(&l), "{levels:?}");
    }
    assert!(d.s.hint().contains("Smart Guides paused"), "{}", d.s.hint());
    assert!(d.s.smart_shown().is_empty());
    key(&mut d, Key::Escape);
    // The next move starts at the full level again (the clock taken away
    // so that the press itself is not slow).
    d.s.measure = None;
    d.click(DVec3::new(11.0, 0.0, 0.0));
    assert_eq!(d.s.smart_level(), Some(Level::Full));
}

// ---------- The Loft tool (brief 34) ----------

/// The owner's test (2026-11-13): a 4 m box with three quarter circles
/// drawn round its corner at (4, 0, 4), one on each face, seen from that
/// side.
fn arcs_round_a_corner() -> Driver {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 4.0);
    d.tool(PUSH);
    d.click(DVec3::new(2.0, 2.0, 0.0));
    d.hover(DVec3::new(2.0, 2.0, 0.5));
    d.typed("4");
    let mut m = d.s.model().clone();
    let half = std::f64::consts::FRAC_PI_2;
    for (centre, axis, start) in [
        // On the top: from (3, 0, 4) to (4, 1, 4).
        ([3.0, 1.0, 4.0], [0.0, 0.0, 1.0], [3.0, 0.0, 4.0]),
        // On the side x = 4: from (4, 1, 4) to (4, 0, 3).
        ([4.0, 1.0, 3.0], [1.0, 0.0, 0.0], [4.0, 1.0, 4.0]),
        // On the front: from (4, 0, 3) to (3, 0, 4).
        ([3.0, 0.0, 3.0], [0.0, -1.0, 0.0], [4.0, 0.0, 3.0]),
    ] {
        let (c, n, s) = (
            DVec3::from_array(centre),
            DVec3::from_array(axis),
            DVec3::from_array(start),
        );
        prism_ops::add_arc(&mut m, c, n, 1.0, s, half);
    }
    // The corner cut away inside the arcs: its three edges go, and the
    // faces between them.
    let corner = prism_ops::vertex_at(&m, DVec3::new(4.0, 0.0, 4.0)).expect("the corner");
    let sel = prism_ops::Selection {
        edges: m
            .edges()
            .filter(|(_, e)| e.has(corner))
            .map(|(id, _)| id)
            .collect(),
        ..Default::default()
    };
    prism_ops::remove(&mut m, &sel);
    let mut d = Driver::new(Document::with_model(m));
    d.look(Camera {
        eye: DVec3::new(9.0, -5.0, 8.0),
        target: DVec3::new(3.5, 0.5, 3.5),
    });
    d
}

/// The middle of the quarter circle about `centre` from `a` to `b`.
fn arc_middle(centre: [f64; 3], a: [f64; 3], b: [f64; 3]) -> DVec3 {
    let c = DVec3::from_array(centre);
    let m = (DVec3::from_array(a) + DVec3::from_array(b)) / 2.0;
    c + (m - c).normalize()
}

const TOP_ARC: ([f64; 3], [f64; 3], [f64; 3]) = ([3.0, 1.0, 4.0], [3.0, 0.0, 4.0], [4.0, 1.0, 4.0]);
const SIDE_ARC: ([f64; 3], [f64; 3], [f64; 3]) =
    ([4.0, 1.0, 3.0], [4.0, 1.0, 4.0], [4.0, 0.0, 3.0]);
const FRONT_ARC: ([f64; 3], [f64; 3], [f64; 3]) =
    ([3.0, 0.0, 3.0], [4.0, 0.0, 3.0], [3.0, 0.0, 4.0]);

fn pick(d: &mut Driver, arc: ([f64; 3], [f64; 3], [f64; 3])) {
    d.click(arc_middle(arc.0, arc.1, arc.2));
}

#[test]
fn three_arcs_round_a_corner_fill_by_clicks_with_three_faces() {
    use prism_tools::session::LOFT;
    let mut d = arcs_round_a_corner();
    let before = d.faces();
    d.tool(LOFT);
    pick(&mut d, TOP_ARC);
    assert_eq!(d.s.loft_sides(), vec![d.s.loft_sides()[0].clone()]);
    assert_eq!(
        d.s.loft_sides()[0].len(),
        1,
        "the arc alone, not the box's edges"
    );
    key(&mut d, Key::Enter);
    pick(&mut d, SIDE_ARC);
    key(&mut d, Key::Enter);
    assert!(
        !d.s.loft_previewing(),
        "two sides that touch make nothing yet"
    );
    pick(&mut d, FRONT_ARC);
    assert_eq!(
        d.s.loft_sides().len(),
        3,
        "the closing arc is taken as a side"
    );
    assert!(d.s.loft_previewing(), "the ring closes: a preview");
    assert!(!d.s.overlay().lines.is_empty());
    assert_eq!(d.faces(), before, "only a preview so far");
    key(&mut d, Key::Enter);
    assert_eq!(d.faces(), before + 3);
    valid(&d);
    // Three new faces meet at one point in the middle, near the sphere
    // round the corner.
    let m = d.s.model();
    let freeform: Vec<_> = m
        .faces()
        .filter(|(_, f)| f.surface.patch().is_some())
        .map(|(id, _)| id)
        .collect();
    assert_eq!(freeform.len(), 3);
    let centre = m
        .vertices()
        .find(|(v, _)| freeform.iter().all(|f| m.face(*f).unwrap().uses_vertex(*v)))
        .map(|(_, p)| p)
        .expect("a shared centre");
    assert!(
        centre.x > 3.3 && centre.z > 3.3 && centre.y < 0.7,
        "{centre}"
    );
    // One undo takes it all away.
    undo(&mut d);
    assert_eq!(d.faces(), before);
    valid(&d);
}

#[test]
fn a_square_ring_is_one_patch_and_two_curves_loft() {
    use prism_tools::session::LOFT;
    let mut m = prism_topo::Model::new();
    let sq = [
        DVec3::ZERO,
        DVec3::new(3.0, 0.0, 0.5),
        DVec3::new(3.0, 3.0, 0.0),
        DVec3::new(0.0, 3.0, 0.5),
    ];
    for i in 0..4 {
        prism_ops::add_segment(&mut m, sq[i], sq[(i + 1) % 4]);
    }
    let mut d = Driver::new(Document::with_model(m));
    d.look(Camera {
        eye: DVec3::new(1.5, -6.0, 7.0),
        target: DVec3::new(1.5, 1.5, 0.0),
    });
    d.tool(LOFT);
    for i in 0..3 {
        d.click(sq[i].lerp(sq[(i + 1) % 4], 0.5));
        key(&mut d, Key::Enter);
    }
    // The last side closes the ring: the click takes it as a side.
    d.click(sq[3].lerp(sq[0], 0.5));
    let p = d.s.loft_panel().unwrap();
    assert_eq!(p.sides, vec![1, 1, 1, 1]);
    assert_eq!(p.makes.as_deref(), Some("Fill a ring of 4 sides"));
    d.s.loft_input(prism_tools::session::LoftOp::Make);
    assert_eq!(d.faces(), 1);
    valid(&d);

    // Two curves apart: a loft through them, straight with the preset,
    // and the preset changes the last loft in place (one undo step).
    let curve = |pts: &[[f64; 3]]| {
        let pts: Vec<DVec3> = pts.iter().map(|p| DVec3::from_array(*p)).collect();
        prism_geom::Nurbs::open(&pts, 3).unwrap()
    };
    let mut m = prism_topo::Model::new();
    prism_ops::add_spline(
        &mut m,
        &curve(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [2.0, 0.0, -0.5],
            [3.0, 0.0, 0.0],
        ]),
    );
    prism_ops::add_spline(
        &mut m,
        &curve(&[
            [0.0, 3.0, 0.5],
            [1.0, 3.0, 1.5],
            [2.0, 3.0, 0.5],
            [3.0, 3.0, 0.5],
        ]),
    );
    let mut d = Driver::new(Document::with_model(m));
    d.look(Camera {
        eye: DVec3::new(1.5, -6.0, 7.0),
        target: DVec3::new(1.5, 1.5, 0.0),
    });
    d.tool(LOFT);
    let on = |d: &Driver, y: f64| {
        let m = d.s.model();
        let (_, e) = m
            .edges()
            .find(|(_, e)| (m.vertex(e.a).y - y).abs() < 1e-9)
            .unwrap();
        e.curve.point(m.vertex(e.a), m.vertex(e.b), 0.4)
    };
    let p0 = on(&d, 0.0);
    d.click(p0);
    key(&mut d, Key::Enter);
    let p1 = on(&d, 3.0);
    d.click(p1);
    assert_eq!(
        d.s.loft_panel().unwrap().makes.as_deref(),
        Some("Loft through 2 sections")
    );
    key(&mut d, Key::Enter);
    key(&mut d, Key::Enter);
    assert_eq!(d.faces(), 1);
    d.s.loft_input(prism_tools::session::LoftOp::Rows(3));
    assert_eq!(d.faces(), 3, "three rows, in place of the one");
    valid(&d);
    undo(&mut d);
    assert_eq!(d.faces(), 0, "one undo step");
}

#[test]
fn a_ring_picked_out_of_order_and_edge_by_edge_fills_with_one_enter() {
    // As in Session 10: sides picked out of order, the last one an edge
    // at a time (its corner is a junction, so no edges are offered to
    // close the ring), then one Enter.
    use prism_tools::session::{LOFT, LoftOp};
    let a = DVec3::ZERO;
    let b = DVec3::new(3.0, 0.0, 0.5);
    let c = DVec3::new(3.0, 3.0, 0.0);
    let dd = DVec3::new(0.0, 3.0, 0.5);
    let mid = dd.lerp(a, 0.5);
    let mut m = prism_topo::Model::new();
    for (p, q) in [(a, b), (b, c), (c, dd), (dd, mid), (mid, a)] {
        prism_ops::add_segment(&mut m, p, q);
    }
    // A stub at the middle point makes it a junction.
    prism_ops::add_segment(&mut m, mid, mid - DVec3::new(1.0, 0.0, 0.0));
    let mut d = Driver::new(Document::with_model(m));
    d.look(Camera {
        eye: DVec3::new(1.5, -6.0, 7.0),
        target: DVec3::new(1.5, 1.5, 0.0),
    });
    d.tool(LOFT);
    for (p, q) in [(a, b), (c, dd), (b, c)] {
        d.click(p.lerp(q, 0.5));
        key(&mut d, Key::Enter);
    }
    assert!(d.s.loft_closing_edges().is_empty());
    assert!(d.s.hint().contains("open"), "{}", d.s.hint());
    // A click on an edge that joins the side being picked adds to it.
    d.click(dd.lerp(mid, 0.5));
    d.click(mid.lerp(a, 0.5));
    let p = d.s.loft_panel().unwrap();
    assert_eq!(p.sides, vec![1, 1, 1]);
    assert_eq!(p.current, 2, "both edges in the last side");
    assert_eq!(p.makes.as_deref(), Some("Fill a ring of 4 sides"));
    assert!(
        !p.straight_applies,
        "smooth or straight does not shape a ring"
    );
    // Smooth or straight leaves the picked ring alone.
    let v = d.s.version();
    d.s.loft_input(LoftOp::Straight(true));
    assert_eq!(d.s.version(), v);
    // The preview shows the rows the faces will be cut into.
    let lines = d.s.overlay().lines.len();
    d.s.loft_input(LoftOp::Rows(3));
    assert!(
        d.s.overlay().lines.len() > lines,
        "rows show in the preview"
    );
    d.s.loft_input(LoftOp::Rows(1));
    assert_eq!(d.s.overlay().lines.len(), lines);
    key(&mut d, Key::Enter);
    // Two faces: the patch is cut across at the point in its last side.
    assert_eq!(d.faces(), 2, "one Enter makes the closed ring");
    valid(&d);
    // ... and the loft just made.
    let v = d.s.version();
    d.s.loft_input(LoftOp::Straight(false));
    assert_eq!(d.s.version(), v, "not remade");
    // Rows change it in place, and show.
    let lines = d.s.overlay().lines.len();
    d.s.loft_input(LoftOp::Rows(3));
    assert_eq!(d.faces(), 4, "cut at a third, the middle and two thirds");
    assert!(d.s.overlay().lines.len() > lines, "the new rows show");
    undo(&mut d);
    assert_eq!(d.faces(), 0);
}

#[test]
fn alt_click_takes_one_edge_whatever_the_threshold() {
    // Owner, Session 10: a straight run is one side by a click; Alt+click
    // takes one segment of it.
    use prism_tools::session::LOFT;
    let (a, mid, b) = (
        DVec3::ZERO,
        DVec3::new(1.5, 0.0, 0.0),
        DVec3::new(3.0, 0.0, 0.0),
    );
    let mut m = prism_topo::Model::new();
    prism_ops::add_segment(&mut m, a, mid);
    prism_ops::add_segment(&mut m, mid, b);
    let mut d = Driver::new(Document::with_model(m));
    d.look(Camera {
        eye: DVec3::new(1.5, -6.0, 7.0),
        target: DVec3::new(1.5, 1.5, 0.0),
    });
    d.tool(LOFT);
    d.click(a.lerp(mid, 0.5));
    assert_eq!(
        d.s.loft_panel().unwrap().current,
        2,
        "the whole straight run"
    );
    key(&mut d, Key::Escape);
    let p = d.screen(a.lerp(mid, 0.5));
    d.send(Input::Move { x: p.x, y: p.y });
    d.send(Input::Press {
        button: Button::Left,
        x: p.x,
        y: p.y,
        shift: false,
        ctrl: false,
        alt: true,
    });
    d.send(Input::Release {
        button: Button::Left,
    });
    assert_eq!(d.s.loft_panel().unwrap().current, 1, "Alt: one edge");
}

#[test]
fn an_open_ring_shows_where_and_a_click_closes_it_and_esc_steps_back() {
    use prism_tools::session::{LOFT, SELECT};
    let mut d = arcs_round_a_corner();
    let before = d.faces();
    d.tool(LOFT);
    pick(&mut d, TOP_ARC);
    key(&mut d, Key::Enter);
    pick(&mut d, SIDE_ARC);
    key(&mut d, Key::Enter);
    assert!(d.s.hint().contains("open"), "{}", d.s.hint());
    let closing = d.s.loft_closing_edges().to_vec();
    assert_eq!(closing.len(), 1, "the front arc closes it");
    // Clicking the highlighted arc takes it as the third side.
    pick(&mut d, FRONT_ARC);
    assert_eq!(d.s.loft_sides().len(), 3);
    assert!(d.s.loft_previewing());
    // Esc steps back one side at a time, then leaves the tool.
    key(&mut d, Key::Escape);
    assert_eq!(d.s.loft_sides().len(), 2);
    assert!(!d.s.loft_previewing());
    key(&mut d, Key::Escape);
    key(&mut d, Key::Escape);
    assert!(d.s.loft_sides().is_empty());
    assert_eq!(d.s.tool(), LOFT);
    key(&mut d, Key::Escape);
    assert_eq!(d.s.tool(), SELECT);
    assert_eq!(d.faces(), before);
}

// ---------- Fillet Joins step C (brief 32): picking many edges ----------

/// A 2 m box on the ground, and the Fillet tool with a size of 0.2.
fn box_for_rounding() -> Driver {
    use prism_tools::session::FILLET;
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.5));
    d.typed("2");
    d.tool(FILLET);
    d.typed("0.2");
    d
}

fn cylinders(d: &Driver) -> usize {
    d.s.model()
        .faces()
        .filter(|(_, f)| matches!(f.surface, prism_geom::Surface::Cylinder(_)))
        .count()
}

#[test]
fn a_double_click_on_a_face_rounds_all_its_edges() {
    let mut d = box_for_rounding();
    assert_eq!(d.faces(), 6);
    double_click(&mut d, DVec3::new(1.0, 1.0, 2.0));
    assert_eq!(d.s.selection().edges.len(), 4, "collected");
    d.send(Input::Key { key: Key::Enter });
    assert_eq!(cylinders(&d), 4, "the four top edges");
    assert_eq!(d.faces(), 10, "the top face stays, smaller");
    assert!(d.s.model().edges().all(|(_, e)| !e.approx), "exact mitres");
    valid(&d);
    // A new size changes the whole set in place.
    d.typed("0.3");
    assert_eq!((cylinders(&d), d.faces()), (4, 10));
    assert!((radii(&d)[0] - 0.3).abs() < 1e-9, "{:?}", radii(&d));
    valid(&d);
    undo(&mut d);
    assert_eq!(d.faces(), 6, "one undo");
}

#[test]
fn edges_selected_first_are_rounded_by_enter() {
    use prism_tools::session::FILLET;
    let mut d = box_for_rounding();
    d.tool(SELECT);
    let a = d.screen(DVec3::new(2.0, 0.0, 1.0));
    let b = d.screen(DVec3::new(0.0, 0.0, 1.0));
    d.click_px(a, false, false);
    d.click_px(b, true, false);
    assert_eq!(d.s.selection().edges.len(), 2);
    d.tool(FILLET);
    assert!(d.s.hint().contains("collected edges"), "{}", d.s.hint());
    d.send(Input::Key { key: Key::Enter });
    assert_eq!(cylinders(&d), 2);
    assert!(d.s.selection().is_empty(), "the rounded edges are gone");
    valid(&d);
}

#[test]
fn shift_click_gathers_edges_and_enter_rounds_them() {
    let mut d = box_for_rounding();
    let a = d.screen(DVec3::new(2.0, 0.0, 1.0));
    let b = d.screen(DVec3::new(1.0, 0.0, 2.0));
    d.click_px(a, true, false);
    d.click_px(b, true, false);
    assert_eq!(d.s.selection().edges.len(), 2);
    assert_eq!(d.faces(), 6, "Shift only gathers");
    // Shift on one again takes it away.
    d.click_px(b, true, false);
    assert_eq!(d.s.selection().edges.len(), 1);
    d.click_px(b, true, false);
    d.send(Input::Key { key: Key::Enter });
    assert_eq!(cylinders(&d), 2, "two edges meeting at a corner");
    valid(&d);
    undo(&mut d);
    assert_eq!(d.faces(), 6, "one undo");
}

#[test]
fn hovering_highlights_what_a_click_rounds() {
    let mut d = box_for_rounding();
    d.hover(DVec3::new(2.0, 0.0, 1.0));
    let hover = d.s.style.hover;
    assert!(
        d.s.overlay().lines.iter().any(|l| l.color == hover),
        "the edge is highlighted"
    );
    assert_eq!(d.faces(), 6, "hovering changes nothing");
}

#[test]
fn a_line_in_pieces_is_rounded_as_a_whole() {
    // The box's top front edge split at its middle by a line down the
    // front face.
    let mut d = box_for_rounding();
    d.tool(LINE);
    d.click(DVec3::new(1.0, 0.0, 2.0));
    d.click(DVec3::new(1.0, 0.0, 0.0));
    d.send(Input::Key { key: Key::Escape });
    let top_front = |d: &Driver| {
        d.s.model()
            .edges()
            .filter(|(_, e)| {
                let (a, b) = (d.s.model().vertex(e.a), d.s.model().vertex(e.b));
                a.y.abs() < 1e-9
                    && b.y.abs() < 1e-9
                    && (a.z - 2.0).abs() < 1e-9
                    && (b.z - 2.0).abs() < 1e-9
            })
            .map(|(id, _)| id)
            .collect::<Vec<_>>()
    };
    let pieces = top_front(&d);
    assert_eq!(pieces.len(), 2, "the edge is in two pieces");
    let run = prism_ops::rounds::straight_run(d.s.model(), pieces[0]);
    assert_eq!(run.len(), 2, "one straight run");
    // A click on one piece takes both.
    use prism_tools::session::FILLET;
    d.tool(FILLET);
    d.click(DVec3::new(0.5, 0.0, 2.0));
    d.send(Input::Key { key: Key::Enter });
    assert_eq!(cylinders(&d), 2, "a round on each side of the split front");
    assert!(top_front(&d).is_empty(), "both pieces went");
    valid(&d);
    undo(&mut d);
    // A piece picked by hand is taken alone as the pick, but the kernel
    // carries a round on along edges that continue smoothly, so the whole
    // line is still rounded (NOTES, step C).
    d.tool(SELECT);
    d.click(DVec3::new(0.5, 0.0, 2.0));
    assert_eq!(d.s.selection().edges.len(), 1);
    d.tool(FILLET);
    d.send(Input::Key { key: Key::Enter });
    assert_eq!(cylinders(&d), 2);
    valid(&d);
}

#[test]
fn a_face_is_rounded_inside_an_open_group() {
    use prism_tools::session::FILLET;
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    act(&mut d, "edit.desel");
    d.s.open_group(g);
    d.tool(FILLET);
    d.typed("0.2");
    double_click(&mut d, DVec3::new(1.0, 1.0, 2.0));
    d.send(Input::Key { key: Key::Enter });
    let rounds =
        d.s.work_model()
            .faces()
            .filter(|(_, f)| matches!(f.surface, prism_geom::Surface::Cylinder(_)))
            .count();
    assert_eq!(rounds, 4, "the group's own top edges");
    assert_eq!(
        prism_ops::check::problems(d.s.work_model()),
        Vec::<String>::new()
    );
    assert_eq!(d.s.model().face_count(), 0, "nothing outside the group");
    undo(&mut d);
    assert_eq!(d.s.work_model().face_count(), 6);
}

/// Brief 35, part 2: a file's objects come in as groups, one undo step.
#[test]
fn imported_objects_become_groups_in_one_step() {
    let mut src = Driver::empty();
    let g = grouped_box(&mut src);
    let mut asm = src.s.document().assembly().clone();
    asm.set_name(g, "Crate");
    let mut d = Driver::empty();
    assert!(d.s.import_assembly(asm));
    let root = d.s.document().assembly().root();
    assert_eq!(root.instances.len(), 1);
    let id = *root.instances.keys().next().unwrap();
    assert_eq!(d.s.document().assembly().name_of(id), Some("Crate"));
    undo(&mut d);
    assert!(d.s.document().assembly().root().instances.is_empty());
}

// ---------- Group Axes (brief 36) ----------

fn group_transform(d: &Driver, g: prism_doc::InstanceId) -> prism_math::DAffine3 {
    d.s.document()
        .assembly()
        .find_instance(g)
        .expect("group")
        .1
        .transform
}

fn world_points(d: &Driver) -> Vec<DVec3> {
    let m = d.s.document().assembly().flattened();
    m.vertices().map(|(_, p)| p).collect()
}

fn kept(a: &[DVec3], b: &[DVec3]) {
    assert_eq!(a.len(), b.len());
    for p in a {
        assert!(b.iter().any(|q| q.distance(*p) < 1e-9), "{p} moved");
    }
}

#[test]
fn the_axes_tool_sets_an_open_groups_axes_in_three_clicks() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    let before = world_points(&d);
    let t0 = group_transform(&d, g);
    // At the top level it sets the model's own axes (a first click starts
    // that; Esc leaves everything as it was).
    d.tool(prism_tools::session::AXES_TOOL);
    assert!(d.s.hint().contains("model's new origin"));
    d.click(DVec3::new(2.0, 0.0, 0.0));
    assert!(d.s.hint().contains("red axis"));
    key(&mut d, Key::Escape);
    assert_eq!(group_transform(&d, g), t0);
    // Change Axes opens it and starts the tool.
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 0.0, 1.0));
    act(&mut d, "axes.change");
    assert_eq!(d.s.open_path(), &[g]);
    assert_eq!(d.s.tool(), prism_tools::session::AXES_TOOL);
    d.click(DVec3::new(2.0, 0.0, 0.0));
    d.click(DVec3::new(2.0, 2.0, 0.0));
    d.click(DVec3::new(0.0, 2.0, 0.0));
    let t = group_transform(&d, g);
    assert!(
        t.translation.distance(DVec3::new(2.0, 0.0, 0.0)) < 1e-9,
        "{t:?}"
    );
    assert!(
        t.matrix3.x_axis.abs_diff_eq(DVec3::Y, 1e-9),
        "red runs along y"
    );
    assert!(t.matrix3.y_axis.abs_diff_eq(-DVec3::X, 1e-9));
    kept(&before, &world_points(&d));
    // One undo step.
    act(&mut d, "edit.undo");
    assert_eq!(group_transform(&d, g), t0);
}

#[test]
fn the_axes_tool_at_the_top_level_sets_the_models_axes() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    let before = world_points(&d);
    let cam = d.s.camera();
    d.tool(prism_tools::session::AXES_TOOL);
    d.click(DVec3::new(2.0, 0.0, 0.0));
    d.click(DVec3::new(2.0, 2.0, 0.0));
    d.click(DVec3::new(0.0, 2.0, 0.0));
    // The new axes are the world's: everything moved by their inverse.
    let axes = prism_tools::session::axes_from(
        DVec3::new(2.0, 0.0, 0.0),
        DVec3::new(2.0, 2.0, 0.0),
        DVec3::new(0.0, 2.0, 0.0),
        1e-9,
    )
    .expect("axes");
    let back = axes.inverse();
    let after = world_points(&d);
    let expected: Vec<DVec3> = before.iter().map(|p| back.transform_point3(*p)).collect();
    kept(&expected, &after);
    let c = d.s.camera();
    assert!(
        c.eye.distance(back.transform_point3(cam.eye)) < 1e-9,
        "the view goes with the model"
    );
    assert!(d.s.document().assembly().find_instance(g).is_some());
    // One undo step puts both back; redo does it again.
    act(&mut d, "edit.undo");
    kept(&before, &world_points(&d));
    assert!(d.s.camera().eye.distance(cam.eye) < 1e-9);
    act(&mut d, "edit.redo");
    kept(&expected, &world_points(&d));
}

#[test]
fn reset_axes_and_axes_to_centre_from_the_menu() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    let before = world_points(&d);
    act(&mut d, "axes.centre");
    let t = group_transform(&d, g);
    assert!(t.translation.distance(DVec3::new(1.0, 1.0, 1.0)) < 1e-9);
    kept(&before, &world_points(&d));
    act(&mut d, "axes.reset");
    let t = group_transform(&d, g);
    assert!(t.translation.length() < 1e-9, "back at the low corner");
    kept(&before, &world_points(&d));
}

// ---------- the Solid tools (Solids & CAD part 2, brief 39) ----------

/// The world volume of a group at the root.
fn group_volume(d: &Driver, g: prism_doc::InstanceId) -> f64 {
    let asm = d.s.document().assembly();
    let i = asm.root().instances[&g];
    let mut m = asm.def(i.def).unwrap().model.clone();
    prism_ops::transform::affine(&mut m, &i.transform);
    prism_ops::validate::validate(&m)
        .iter()
        .map(|c| c.volume.unwrap_or(0.0))
        .sum()
}

/// Three 2 m boxes along x at 0, 3 and 1: the first and the last overlap.
fn overlapping_groups() -> Driver {
    let (d, _) = row_of_groups(1.0);
    d
}

#[test]
fn subtract_by_two_clicks_cutter_first_and_esc_starts_over() {
    use prism_tools::session::SOLID_TOOLS;
    let mut d = overlapping_groups();
    let subtract = SOLID_TOOLS[1];
    d.tool(subtract);
    // The last box (1 to 3), then Esc: forgotten.
    d.click(DVec3::new(2.5, 0.0, 1.0));
    key(&mut d, Key::Escape);
    // The first box (0 to 2) is the cutter, the last is cut: 2 to 3 is left.
    d.click(DVec3::new(0.5, 0.0, 1.0));
    d.click(DVec3::new(2.5, 0.0, 1.0));
    assert_eq!(d.s.document().undo_name(), Some("Subtract"));
    let asm = d.s.document().assembly();
    assert_eq!(asm.root().instances.len(), 2, "the box at 3 and the result");
    let (g, i) = asm
        .root()
        .instances
        .iter()
        .find(|(_, i)| i.transform.translation.x < 2.0)
        .map(|(g, i)| (*g, *i))
        .expect("the result, at the cut box's axes");
    assert!((i.transform.translation.x - 1.0).abs() < 1e-9, "{i:?}");
    assert!((group_volume(&d, g) - 4.0).abs() < 1e-9);
    act(&mut d, "edit.undo");
    assert_eq!(d.s.document().assembly().root().instances.len(), 3);
}

#[test]
fn union_runs_at_once_on_two_selected_groups() {
    use prism_tools::session::SOLID_TOOLS;
    let mut d = overlapping_groups();
    d.tool(SELECT);
    d.click(DVec3::new(0.5, 0.0, 1.0));
    let s2 = d.screen(DVec3::new(2.5, 0.0, 1.0));
    d.click_px(s2, false, true);
    assert_eq!(d.s.selected_groups().len(), 2);
    d.tool(SOLID_TOOLS[0]);
    assert_eq!(d.s.document().undo_name(), Some("Union"));
    let asm = d.s.document().assembly();
    assert_eq!(asm.root().instances.len(), 2);
    let g = *asm
        .root()
        .instances
        .iter()
        .find(|(_, i)| i.transform.translation.x < 2.0)
        .unwrap()
        .0;
    assert!((group_volume(&d, g) - 12.0).abs() < 1e-9);
}

// ---------- Shell / Thicken (brief 42, part 3b) ----------

use prism_tools::session::SHELL;

/// A 2 m box at the origin, at the root.
fn box_2m(d: &mut Driver) {
    rect_on_ground(d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("2");
    assert_eq!(d.faces(), 6);
}

fn model_volume(m: &prism_topo::Model) -> f64 {
    m.faces()
        .map(|(id, _)| prism_ops::health::face_volume(m, id))
        .sum()
}

#[test]
fn shell_hollows_a_box_into_a_tray_with_a_ghost_first() {
    let mut d = Driver::empty();
    box_2m(&mut d);
    d.tool(SHELL);
    assert!(d.s.hint().starts_with("Click a face to shell"));
    // Shift+click the top to leave it open, then click a side.
    d.hover(DVec3::new(1.0, 1.0, 2.0));
    d.send(Input::Press {
        button: Button::Left,
        x: d.screen(DVec3::new(1.0, 1.0, 2.0)).x,
        y: d.screen(DVec3::new(1.0, 1.0, 2.0)).y,
        shift: true,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Release {
        button: Button::Left,
    });
    d.click(DVec3::new(1.0, 0.0, 1.0));
    assert!(d.s.shell_sizing().is_some_and(|t| t.abs() < 1e-9), "sizing");
    assert!(d.s.hint().starts_with("Move to set the thickness"));
    // Into the box: a ghost of the tray.
    d.hover(DVec3::new(1.0, 0.3, 1.2));
    assert!(d.s.shell_sizing().is_some_and(|t| t < 0.0));
    let ghost = d.s.overlay().lines.len();
    assert!(ghost >= 14 * 2, "the ghost shows: {ghost} lines");
    assert_eq!(d.faces(), 6, "nothing made yet");
    d.typed("0.1");
    assert_eq!(d.s.shell_sizing(), None);
    assert_eq!(d.faces(), 5 + 5 + 4, "a tray");
    let v = model_volume(d.s.model());
    assert!((v - (8.0 - 1.8 * 1.8 * 1.9)).abs() < 1e-6, "{v}");
    assert_eq!(d.s.document().undo_name(), Some("Shell / Thicken"));
    valid(&d);
}

#[test]
fn shell_says_why_and_esc_steps_back() {
    let mut d = Driver::empty();
    box_2m(&mut d);
    d.tool(SHELL);
    d.click(DVec3::new(1.0, 1.0, 2.0));
    d.hover(DVec3::new(1.0, 1.0, 1.0));
    d.typed("1.5");
    assert_eq!(d.s.hint(), prism_ops::shelling::TOO_THICK);
    assert_eq!(d.faces(), 6);
    assert!(d.s.shell_sizing().is_some(), "still sizing");
    key(&mut d, Key::Escape);
    assert_eq!(d.s.shell_sizing(), None);
    assert_eq!(d.faces(), 6);
    assert!(d.s.hint().starts_with("Click a face to shell"));
}

#[test]
fn shell_grows_a_box_outside_inside_a_group() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    act(&mut d, "edit.desel");
    double_click(&mut d, DVec3::new(1.0, 0.0, 1.0));
    assert_eq!(d.s.open_path(), [g]);
    d.tool(SHELL);
    d.click(DVec3::new(1.0, 1.0, 2.0));
    d.hover(DVec3::new(1.0, 1.0, 2.5));
    assert!(
        d.s.shell_sizing().is_some_and(|t| t > 0.0),
        "out of the top"
    );
    d.typed("0.1");
    let asm = d.s.document().assembly();
    let m = &asm.def(asm.context_def(&[g]).unwrap()).unwrap().model;
    assert_eq!(m.face_count(), 12, "two shells");
    let v = model_volume(m);
    assert!((v - (2.2f64.powi(3) - 8.0)).abs() < 1e-6, "{v}");
    assert_eq!(d.faces(), 0, "nothing at the root");
}

#[test]
fn shell_takes_a_curved_face() {
    let mut d = Driver::empty();
    // A cylinder of radius 1 and height 1.
    let mut doc = d.s.document().clone();
    doc.run(prism_doc::Command::Circle {
        centre: DVec3::ZERO,
        normal: DVec3::Z,
        radius: 1.0,
    });
    d.s.open_document(doc, Some(d.s.camera()));
    d.tool(PUSH);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.typed("1");
    let v0 = model_volume(d.s.model());
    assert!((v0 - std::f64::consts::PI).abs() < 1e-6, "{v0}");
    // Click the round side and move out from it.
    d.tool(SHELL);
    d.click(DVec3::new(0.0, -1.0, 0.5));
    d.hover(DVec3::new(0.0, -1.3, 0.5));
    assert!(
        d.s.shell_sizing().is_some_and(|t| t > 0.0),
        "out of the side"
    );
    d.typed("0.1");
    let v = model_volume(d.s.model());
    let want = std::f64::consts::PI * (1.21 * 1.2 - 1.0);
    assert!((v - want).abs() < 1e-6, "{v} is not {want}");
    valid(&d);
}

#[test]
fn a_wall_thinner_than_a_tenth_of_a_centimetre_is_refused_in_red() {
    // The limiter on Shell / Thicken (owner, 2026-11-21): 0.1 cm at least.
    // Drawing and the rest stay exact to 0.01 cm (below).
    let min = prism_ops::shelling::MIN_THICKNESS;
    assert!((min - 0.001).abs() < 1e-15, "0.1 cm is a millimetre");
    let mut d = Driver::empty();
    box_2m(&mut d);
    d.tool(SHELL);
    // Sizing by the pointer: too thin shows no ghost and a red notice.
    let top = DVec3::new(1.0, 1.0, 2.0);
    d.click(top);
    d.hover(DVec3::new(1.0, 1.0, 2.0 - 0.0005));
    assert!(d.s.hint_is_notice(), "red");
    assert!(d.s.hint().starts_with("Too thin"), "{}", d.s.hint());
    assert!(d.s.overlay().lines.len() < 12, "no ghost");
    // Typing a thin wall refuses it and makes nothing.
    d.typed("0.0005");
    assert!(d.s.hint_is_notice() && d.s.hint().starts_with("Too thin"));
    assert_eq!(d.faces(), 6, "nothing made");
    // Far enough, the notice goes and the ghost shows again.
    d.hover(DVec3::new(1.0, 1.0, 2.0 - 0.05));
    assert!(!d.s.hint_is_notice(), "{}", d.s.hint());
    assert!(d.s.overlay().lines.len() >= 12);
    // Exactly 0.1 cm is allowed.
    d.typed("0.001");
    assert_eq!(d.faces(), 12);
    // The window's thickness refuses it too, and keeps what it had.
    act(&mut d, "edit.undo");
    d.s.shell_text("0.0004");
    assert!(d.s.hint_is_notice() && d.s.hint().starts_with("Too thin"));
    assert!(d.s.shell_panel().unwrap().size.is_empty(), "not taken");
    d.s.shell_text("0.002");
    assert!(!d.s.shell_panel().unwrap().size.is_empty());
}

#[test]
fn the_inspector_refuses_a_shell_thinner_than_a_tenth_of_a_centimetre() {
    let mut d = Driver::empty();
    box_2m(&mut d);
    d.tool(SHELL);
    d.click(DVec3::new(1.0, 1.0, 2.0));
    d.typed("0.1");
    d.tool(SELECT);
    d.s.select_feature(0);
    let why = d.s.inspector_set("feature.0.size", "-0.0005").unwrap_err();
    assert!(why.starts_with("Too thin"), "{why}");
    let why = d.s.inspector_set("feature.0.size", "0.0009").unwrap_err();
    assert!(why.starts_with("Too thin"), "{why}");
    d.s.inspector_set("feature.0.size", "-0.001")
        .expect("0.1 cm");
    d.s.inspector_set("feature.0.size", "0.002")
        .expect("outside");
}

#[test]
fn shell_window_thickness_makes_a_click_shell_at_once() {
    use prism_tools::session::ShellSet;
    let mut d = Driver::empty();
    box_2m(&mut d);
    d.tool(SHELL);
    let panel = d.s.shell_panel().expect("the window");
    assert!(panel.size.is_empty() && !panel.outside && panel.open == 0);
    d.s.shell_text("0.1");
    assert!(
        d.s.hint()
            .starts_with("Click a face to shell or thicken it by the window")
    );
    // Hovering shows the ghost; nothing is made until the click.
    d.hover(DVec3::new(1.0, 1.0, 2.0));
    assert!(d.s.overlay().lines.len() >= 12 * 2, "the ghost shows");
    assert_eq!(d.faces(), 6);
    d.click(DVec3::new(1.0, 1.0, 2.0));
    assert_eq!(d.faces(), 12, "hollow at once");
    let v = model_volume(d.s.model());
    assert!((v - (8.0 - 1.8f64.powi(3))).abs() < 1e-6, "inside: {v}");
    // Outside, and an open face counted and closed again.
    act(&mut d, "edit.undo");
    d.s.shell_input(ShellSet::Outside(true));
    let shift_click = |d: &mut Driver, p: DVec3| {
        let s = d.screen(p);
        d.send(Input::Move { x: s.x, y: s.y });
        d.send(Input::Press {
            button: Button::Left,
            x: s.x,
            y: s.y,
            shift: true,
            ctrl: false,
            alt: false,
        });
        d.send(Input::Release {
            button: Button::Left,
        });
    };
    shift_click(&mut d, DVec3::new(1.0, 1.0, 2.0));
    assert_eq!(d.s.shell_panel().unwrap().open, 1);
    d.s.shell_input(ShellSet::CloseAll);
    assert_eq!(d.s.shell_panel().unwrap().open, 0);
    d.click(DVec3::new(1.0, 0.0, 1.0));
    let v = model_volume(d.s.model());
    assert!((v - (2.2f64.powi(3) - 8.0)).abs() < 1e-6, "outside: {v}");
    // Cleared, the pointer sizes it again.
    d.s.shell_text("");
    d.click(DVec3::new(1.0, 1.0, 2.2));
    assert!(d.s.shell_sizing().is_some());
}

// ---------- Rectangle off the ground (Session 8, owner 2026-11-17) ----------

#[test]
fn a_rectangle_to_a_point_above_stands_up() {
    let mut d = Driver::empty();
    // A post 2 m tall at the origin.
    d.tool(LINE);
    d.click(DVec3::ZERO);
    d.click(DVec3::new(0.0, 0.0, 2.0));
    key(&mut d, Key::Escape);
    // From the ground 3 m along green to the post's top: the rectangle
    // stands in the green-blue plane.
    d.tool(RECT);
    d.click(DVec3::new(0.0, 3.0, 0.0));
    d.click(DVec3::new(0.0, 0.0, 2.0));
    assert_eq!(d.s.document().undo_name(), Some("Rectangle"));
    let m = d.s.model();
    let f = m
        .faces()
        .find(|(_, f)| f.plane().is_some_and(|p| p.normal.x.abs() > 0.99))
        .map(|(id, _)| id)
        .expect("a face standing in the green-blue plane");
    assert!((m.face_area(f) - 6.0).abs() < 1e-9, "{}", m.face_area(f));
    valid(&d);
}

#[test]
fn a_corner_in_line_with_the_first_keeps_the_rectangle_going() {
    let mut d = Driver::empty();
    d.tool(RECT);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    // The same point again makes nothing: the first corner stays, as in
    // the other drawing tools.
    d.click(DVec3::new(1.0, 1.0, 0.0));
    assert_eq!(d.faces(), 0);
    assert!(d.s.hint().starts_with("Click the opposite corner"));
    d.click(DVec3::new(3.0, 2.0, 0.0));
    same(&areas(&d), &[2.0]);
}

#[test]
fn a_rectangle_to_a_point_off_the_axes_stands_up_toward_it() {
    let mut d = Driver::empty();
    // A post at (1, 2), 2 m tall.
    d.tool(LINE);
    d.click(DVec3::new(1.0, 2.0, 0.0));
    d.click(DVec3::new(1.0, 2.0, 2.0));
    key(&mut d, Key::Escape);
    // From the origin to the post's top: an upright rectangle along the
    // diagonal, sqrt(5) wide and 2 tall.
    d.tool(RECT);
    d.click(DVec3::ZERO);
    d.click(DVec3::new(1.0, 2.0, 2.0));
    assert_eq!(d.s.document().undo_name(), Some("Rectangle"));
    let m = d.s.model();
    let f = m
        .faces()
        .find(|(_, f)| f.plane().is_some_and(|p| p.normal.z.abs() < 1e-9))
        .map(|(id, _)| id)
        .expect("an upright face");
    assert!(
        (m.face_area(f) - 2.0 * 5f64.sqrt()).abs() < 1e-9,
        "{}",
        m.face_area(f)
    );
    valid(&d);
}

// ---------- Weld (brief 43, part 4a) ----------

#[test]
fn welded_edges_are_picked_and_erased_as_one() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 1.0);
    d.tool(SELECT);
    act(&mut d, "edit.selall");
    act(&mut d, "edit.weld");
    assert_eq!(d.s.document().undo_name(), Some("Weld"));
    act(&mut d, "edit.desel");
    d.click(DVec3::new(1.0, 0.0, 0.0));
    assert_eq!(d.s.selection().edges.len(), 4, "one click takes the curve");
    // Weld again: already one curve, said until the next click.
    act(&mut d, "edit.weld");
    assert_eq!(d.s.hint(), prism_ops::curves::ALREADY);
    d.click(DVec3::new(1.0, 0.0, 0.0));
    assert_ne!(d.s.hint(), prism_ops::curves::ALREADY);
    d.tool(ERASE);
    d.click(DVec3::new(2.0, 0.5, 0.0));
    assert_eq!(d.s.model().edge_count(), 0, "erased as one");
}

#[test]
fn convert_turns_a_selected_chain_into_one_curve_and_undo_restores_it() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 1.0);
    let before = d.s.model().geometry_json();
    d.tool(SELECT);
    act(&mut d, "edit.selall");
    act(&mut d, "conv.curve");
    assert_eq!(d.s.document().undo_name(), Some("Convert Curve"));
    let m = d.s.model();
    assert_eq!(m.edge_count(), 2, "one closed curve in two halves");
    assert!(m.edges().all(|(_, e)| e.curve.is_spline()));
    assert!((areas(&d)[0] - 2.0).abs() < 1e-9);
    act(&mut d, "edit.undo");
    assert_eq!(d.s.model().geometry_json(), before);
    // A face alone is not a chain: said until the next click.
    act(&mut d, "edit.desel");
    act(&mut d, "conv.curve");
    assert_eq!(d.s.hint(), prism_ops::convert::NOTHING);
    d.click(DVec3::new(1.0, 0.5, 0.0));
    assert_ne!(d.s.hint(), prism_ops::convert::NOTHING);
}

#[test]
fn convert_turns_a_selected_freeform_face_into_flat_faces() {
    let mut d = Driver::empty();
    // A lofted surface between two curves: one freeform face.
    let mut doc = d.s.document().clone();
    for y in [0.0, 2.0] {
        doc.run(prism_doc::Command::Spline {
            curve: prism_geom::Nurbs::open(
                &[
                    DVec3::new(0.0, y, 0.0),
                    DVec3::new(1.0, y, 0.6),
                    DVec3::new(2.0, y, -0.4),
                    DVec3::new(3.0, y, 0.0),
                ],
                3,
            )
            .unwrap(),
        });
    }
    let edges = doc.model().edges().map(|(id, _)| id).collect();
    doc.run(prism_doc::Command::Loft { edges });
    d.s.open_document(doc, Some(d.s.camera()));
    assert_eq!(d.faces(), 1);
    d.tool(SELECT);
    act(&mut d, "edit.selall");
    act(&mut d, "conv.patch");
    assert_eq!(d.s.document().undo_name(), Some("Convert Faces"));
    assert!(d.faces() > 4);
    assert!(d.s.model().faces().all(|(_, f)| f.plane().is_some()));
    valid(&d);
}

#[test]
fn new_loop_shows_at_the_cursor_before_the_pull() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    let label = |d: &Driver| d.s.overlay().labels.iter().any(|l| l.text == "New loop");
    d.hover(DVec3::new(1.0, 1.0, 0.0));
    assert!(!label(&d));
    d.send(Input::AltTap);
    d.hover(DVec3::new(1.5, 1.0, 0.0));
    assert!(label(&d), "New Loop is shown by the cursor");
    d.send(Input::AltTap);
    assert!(!label(&d));
}

// ---------- Section planes (Annotation & Sections part 1, brief 46) ----------

fn move_to(d: &mut Driver, p: DVec3) {
    let s = d.screen(p);
    d.send(Input::Move { x: s.x, y: s.y });
}

#[test]
fn the_section_tool_places_a_plane_on_a_face_then_moves_it() {
    use prism_tools::session::SECTION_TOOL;
    let mut d = Driver::new(prism_tools::scenes::pocket());
    d.tool(SECTION_TOOL);
    assert!(d.s.hint().contains("Click a face"), "{}", d.s.hint());
    // A ghost lies on the face under the cursor; nothing is added yet.
    d.hover(DVec3::new(0.4, 0.4, 2.0));
    assert!(d.s.document().assembly().sections().0.is_empty());
    // Click the top ring: a plane on it, facing the viewer.
    d.click(DVec3::new(0.4, 0.4, 2.0));
    let (id, sec) = d.s.document().assembly().active_section().expect("a plane");
    assert!((sec.origin.z - 2.0).abs() < 1e-6, "{:?}", sec.origin);
    assert_eq!(sec.normal, DVec3::Z, "its normal faces the viewer");
    assert!(d.s.hint().contains("along its normal"));
    // It follows the mouse along its normal; the next click sets it.
    move_to(&mut d, DVec3::new(0.4, 0.4, 1.0));
    d.click(DVec3::new(0.4, 0.4, 1.0));
    let (_, sec) = d.s.document().assembly().active_section().expect("a plane");
    assert!((sec.origin.z - 1.0).abs() < 0.02, "{:?}", sec.origin);
    // The pocket's walls are cut at z = 1: a ring cap, 4×4 less 2.4×2.4.
    let cut = d.s.scene().cut.as_ref().expect("the scene is cut");
    assert_eq!(cut.caps, 1);
    assert!((cut.cap_area - 10.24).abs() < 0.05, "{}", cut.cap_area);
    let _ = id;
    // Placing and moving is two undo steps; the model never changed.
    let model = d.s.document().model().clone();
    act(&mut d, "edit.undo");
    act(&mut d, "edit.undo");
    assert!(d.s.document().assembly().sections().0.is_empty());
    assert!(d.s.scene().cut.is_none());
    assert_eq!(d.s.document().model(), &model);
}

#[test]
fn esc_leaves_a_new_plane_where_it_was_placed() {
    use prism_tools::session::SECTION_TOOL;
    let mut d = Driver::new(prism_tools::scenes::pocket());
    d.tool(SECTION_TOOL);
    d.click(DVec3::new(0.4, 0.4, 2.0));
    move_to(&mut d, DVec3::new(0.4, 0.4, 0.8));
    d.send(Input::Key { key: Key::Escape });
    let (_, sec) = d.s.document().assembly().active_section().expect("a plane");
    assert!((sec.origin.z - 2.0).abs() < 1e-6, "{:?}", sec.origin);
    assert_eq!(d.s.tool(), SECTION_TOOL);
}

#[test]
fn a_plane_is_picked_up_by_its_outline_and_the_menu_commands_act_on_it() {
    use prism_tools::session::SECTION_TOOL;
    let mut d = Driver::new(prism_tools::scenes::pocket());
    d.tool(SECTION_TOOL);
    d.click(DVec3::new(0.4, 0.4, 2.0));
    move_to(&mut d, DVec3::new(0.4, 0.4, 1.0));
    d.click(DVec3::new(0.4, 0.4, 1.0));
    // Flip: the other half is cut away.
    act(&mut d, "section.flip");
    let (_, sec) = d.s.document().assembly().active_section().unwrap();
    assert_eq!(sec.normal, -DVec3::Z);
    let tol = d.s.scene().tol.dist;
    assert!(
        d.s.scene()
            .mesh
            .indices
            .iter()
            .all(|i| d.s.scene().mesh.positions[*i as usize].z >= sec.origin.z - tol),
        "only what is above the plane is left"
    );
    // Deactivate: the model shows whole again, the plane stays.
    act(&mut d, "section.active");
    assert!(d.s.document().assembly().active_section().is_none());
    assert!(d.s.scene().cut.is_none());
    assert!(d.s.has_sections());
    // Activate again, then delete.
    act(&mut d, "section.active");
    assert!(d.s.scene().cut.is_some());
    act(&mut d, "section.delete");
    assert!(!d.s.has_sections());
    assert!(d.s.scene().cut.is_none());
}

#[test]
fn section_display_off_shows_the_whole_model() {
    let mut doc = prism_tools::scenes::pocket();
    doc.run(prism_doc::Command::AddSection {
        origin: DVec3::new(0.0, 0.0, 1.0),
        normal: DVec3::Z,
    });
    let mut d = Driver::new(doc);
    assert!(d.s.scene().cut.is_some());
    let mut v = d.s.view_style().clone();
    v.section = false;
    d.s.set_view_style(v);
    assert!(d.s.scene().cut.is_none(), "Section Display is off");
    let mut v = d.s.view_style().clone();
    v.section = true;
    d.s.set_view_style(v);
    assert!(d.s.scene().cut.is_some());
}

#[test]
fn a_cut_away_face_cannot_be_clicked() {
    let mut doc = prism_tools::scenes::pocket();
    doc.run(prism_doc::Command::AddSection {
        origin: DVec3::new(0.0, 0.0, 1.0),
        normal: DVec3::Z,
    });
    let mut d = Driver::new(doc);
    // The ring on top (z = 2) is cut away: a click there finds what is
    // behind it, not the ring.
    let ring = prism_ops::face_at(d.s.document().model(), DVec3::new(0.4, 0.4, 2.0)).expect("ring");
    d.tool(SELECT);
    d.click(DVec3::new(0.4, 0.4, 2.0));
    assert!(!d.s.selection().faces.contains(&ring));
}

// ---------- Dimensions (Annotation & Sections part 2, brief 46) ----------

fn dims(d: &Driver) -> Vec<prism_doc::Dimension> {
    d.s.document()
        .assembly()
        .dimensions()
        .0
        .values()
        .copied()
        .collect()
}

#[test]
fn the_dimension_tool_takes_two_points_and_a_side() {
    use prism_tools::session::DIM_TOOL;
    let mut d = Driver::new(prism_tools::scenes::pocket());
    d.tool(DIM_TOOL);
    assert!(d.s.hint().contains("first point"), "{}", d.s.hint());
    d.click(DVec3::new(0.0, 0.0, 0.0));
    assert!(d.s.hint().contains("second point"), "{}", d.s.hint());
    d.click(DVec3::new(4.0, 0.0, 0.0));
    assert!(d.s.hint().contains("dimension line goes"), "{}", d.s.hint());
    assert!(dims(&d).is_empty(), "nothing yet");
    // The side: a point in front of the box, on the ground.
    d.click(DVec3::new(2.0, -1.5, 0.0));
    let all = dims(&d);
    assert_eq!(all.len(), 1);
    let dim = all[0];
    assert!((dim.length() - 4.0).abs() < 1e-6, "{}", dim.length());
    assert!(dim.offset.y < -0.5, "{:?}", dim.offset);
    assert!(dim.offset.x.abs() < 1e-6, "square to the measured line");
    // The label shows the length in the document's unit.
    let label =
        d.s.overlay()
            .labels
            .iter()
            .find(|l| l.text.contains('4'))
            .cloned();
    assert!(label.is_some(), "a label with the length");
    // One undo step; the model never changed.
    let model = d.s.document().model().clone();
    act(&mut d, "edit.undo");
    assert!(dims(&d).is_empty());
    assert_eq!(d.s.document().model(), &model);
}

#[test]
fn esc_cancels_a_dimension_and_a_pick_up_leaves_it() {
    use prism_tools::session::DIM_TOOL;
    let mut d = Driver::new(prism_tools::scenes::pocket());
    d.tool(DIM_TOOL);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(4.0, 0.0, 0.0));
    d.send(Input::Key { key: Key::Escape });
    d.click(DVec3::new(2.0, -1.5, 0.0));
    assert!(
        dims(&d).is_empty(),
        "the second click after Esc starts over"
    );
    d.send(Input::Key { key: Key::Escape });
    // Make one, then pick it up by its line and move it.
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(4.0, 0.0, 0.0));
    d.click(DVec3::new(2.0, -1.0, 0.0));
    let first = dims(&d)[0];
    let on_line = first.a + first.offset + (first.b - first.a) * 0.25;
    d.click(on_line);
    assert!(d.s.hint().contains("dimension line goes"));
    d.send(Input::Key { key: Key::Escape });
    assert_eq!(dims(&d)[0], first, "Esc leaves it where it was");
    d.click(on_line);
    d.click(DVec3::new(2.0, -3.0, 0.0));
    let moved = dims(&d);
    assert_eq!(moved.len(), 1, "moved, not added");
    assert!(
        moved[0].offset.y < first.offset.y - 0.5,
        "{:?}",
        moved[0].offset
    );
    assert_eq!((moved[0].a, moved[0].b), (first.a, first.b));
}

#[test]
fn delete_dimension_and_the_view_toggle() {
    let mut doc = prism_tools::scenes::pocket();
    doc.run(prism_doc::Command::AddDimension {
        a: DVec3::ZERO,
        b: DVec3::new(4.0, 0.0, 0.0),
        offset: DVec3::new(0.0, -1.0, 0.0),
    });
    let mut d = Driver::new(doc);
    d.send(Input::Move { x: 50.0, y: 50.0 });
    assert!(d.s.has_dimensions());
    let labelled = |d: &Driver| d.s.overlay().labels.iter().any(|l| l.text.contains('4'));
    assert!(labelled(&d), "drawn");
    let mut v = d.s.view_style().clone();
    v.dimensions = false;
    d.s.set_view_style(v);
    assert!(!labelled(&d), "View > Dimensions off hides them");
    act(&mut d, "dim.delete");
    assert!(!d.s.has_dimensions());
}

#[test]
fn section_planes_stay_drawn_with_any_tool() {
    let mut doc = prism_tools::scenes::pocket();
    doc.run(prism_doc::Command::AddSection {
        origin: DVec3::new(0.0, 0.0, 1.0),
        normal: DVec3::Z,
    });
    let mut d = Driver::new(doc);
    d.tool(SELECT);
    d.send(Input::Move { x: 50.0, y: 50.0 });
    let lines = d.s.overlay().lines.len();
    assert!(lines >= 5, "the plane's outline and arrow ({lines})");
    let mut v = d.s.view_style().clone();
    v.section_planes = false;
    d.s.set_view_style(v);
    assert!(
        d.s.overlay().lines.len() < lines,
        "Section Planes off hides it"
    );
}

// ---------- Text (Annotation & Sections part 3, brief 46) ----------

fn text_groups(d: &Driver) -> Vec<(String, prism_math::DAffine3)> {
    d.s.document()
        .assembly()
        .root()
        .instances
        .iter()
        .map(|(id, i)| {
            (
                d.s.document()
                    .assembly()
                    .name_of(*id)
                    .unwrap_or("")
                    .to_owned(),
                i.transform,
            )
        })
        .collect()
}

#[test]
fn the_text_tool_sets_text_down_as_a_named_group_on_the_ground() {
    use prism_tools::session::{TEXT_TOOL, TextKind, TextSet};
    let mut d = Driver::empty();
    d.tool(TEXT_TOOL);
    assert!(
        d.s.hint().contains("Click where the text goes"),
        "{}",
        d.s.hint()
    );
    d.s.text_input(TextSet::Text("Hello".into()));
    d.s.text_input(TextSet::Height("300mm".into()));
    d.s.text_input(TextSet::Kind(TextKind::Solid));
    d.s.text_input(TextSet::Depth("50mm".into()));
    let panel = d.s.text_panel().expect("the window shows with the tool");
    assert_eq!(panel.settings.text, "Hello");
    assert!((panel.settings.height - 0.3).abs() < 1e-9);
    assert_eq!(panel.missing, 0);
    d.click(DVec3::new(3.0, 2.0, 0.0));
    let all = text_groups(&d);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].0, "Text: Hello");
    let t = all[0].1;
    assert!(
        t.translation.distance(DVec3::new(3.0, 2.0, 0.0)) < 0.2,
        "{:?}",
        t.translation
    );
    assert!(
        t.matrix3.z_axis.abs_diff_eq(DVec3::Z, 1e-9),
        "flat on the ground, facing up"
    );
    assert_eq!(d.s.selected_groups().len(), 1, "the new group is selected");
    // Solid, 50 mm deep, capitals 300 mm tall.
    let (lo, hi) = d.s.document().assembly().flattened().bbox().unwrap();
    assert!((hi.z - lo.z - 0.05).abs() < 1e-6, "{lo} {hi}");
    assert!(hi.y - lo.y > 0.28 && hi.y - lo.y < 0.5, "{lo} {hi}");
    // One undo step.
    act(&mut d, "edit.undo");
    assert!(text_groups(&d).is_empty());
}

#[test]
fn text_on_a_face_lies_on_it_and_faces_out() {
    use prism_tools::session::{TEXT_TOOL, TextSet};
    let mut d = Driver::new(prism_tools::scenes::pocket());
    d.tool(TEXT_TOOL);
    d.s.text_input(TextSet::Text("Top".into()));
    d.click(DVec3::new(0.4, 0.4, 2.0));
    let all = text_groups(&d);
    assert_eq!(all.len(), 1);
    let t = all[0].1;
    assert!((t.translation.z - 2.0).abs() < 1e-6, "{:?}", t.translation);
    assert!(t.matrix3.z_axis.abs_diff_eq(DVec3::Z, 1e-9));
}

#[test]
fn empty_text_says_so_and_adds_nothing() {
    use prism_tools::session::{TEXT_TOOL, TextSet};
    let mut d = Driver::empty();
    d.tool(TEXT_TOOL);
    d.s.text_input(TextSet::Text("   ".into()));
    assert!(d.s.hint().contains("Type the text"));
    d.click(DVec3::new(3.0, 2.0, 0.0));
    assert!(text_groups(&d).is_empty());
    assert!(d.s.hint().contains("Type the text"), "{}", d.s.hint());
    // Text with only characters the font lacks draws nothing either.
    d.s.text_input(TextSet::Text("日本".into()));
    assert_eq!(d.s.text_panel().unwrap().missing, 2);
    d.click(DVec3::new(3.0, 2.0, 0.0));
    assert!(text_groups(&d).is_empty());
}

#[test]
fn the_window_ignores_what_is_not_a_length_and_cuts_long_text() {
    use prism_tools::session::{MAX_TEXT, TEXT_TOOL, TextSet};
    let mut d = Driver::empty();
    d.tool(TEXT_TOOL);
    d.s.text_input(TextSet::Height("tall".into()));
    d.s.text_input(TextSet::Height("0".into()));
    d.s.text_input(TextSet::Depth("0".into()));
    let s = d.s.text_settings().clone();
    assert_eq!((s.height, s.depth), (0.2, 0.05), "unchanged");
    d.s.text_input(TextSet::Text("x".repeat(MAX_TEXT + 50)));
    assert_eq!(d.s.text_settings().text.chars().count(), MAX_TEXT);
    // Not with another tool.
    d.tool(SELECT);
    assert!(d.s.text_panel().is_none());
}

// ---------- Construction planes (Annotation & Sections part 4, brief 46) ----------

#[test]
fn the_construction_plane_tool_places_a_plane_on_the_ground_and_uses_it() {
    let mut d = Driver::empty();
    d.tool(prism_tools::session::CPLANE_TOOL);
    assert!(d.s.hint().contains("construction plane"));
    d.click(DVec3::new(3.0, 3.0, 0.0));
    let asm = d.s.document().assembly();
    let (_, p) = asm.active_work_plane().expect("a plane in use");
    assert!(p.origin.distance(DVec3::new(3.0, 3.0, 0.0)) < 1e-6, "{p:?}");
    assert!(p.normal.abs_diff_eq(DVec3::Z, 1e-9));
    assert!(d.s.has_work_planes());
    // One undo step removes it.
    act(&mut d, "edit.undo");
    assert!(!d.s.has_work_planes());
}

#[test]
fn drawing_tools_draw_on_the_working_plane() {
    let mut d = Driver::empty();
    // A wall plane at y = 1 (square to y), then a rectangle on it.
    d.s.add_work_plane(DVec3::new(3.0, 1.0, 0.0), DVec3::Y)
        .expect("plane");
    d.tool(RECT);
    d.click(DVec3::new(2.5, 1.0, 0.5));
    assert_eq!(
        d.s.inference().map(|i| i.kind),
        Some(prism_infer::Kind::OnPlane)
    );
    d.click(DVec3::new(3.5, 1.0, 1.5));
    assert_eq!(d.faces(), 1);
    for (_, p) in d.s.model().vertices() {
        assert!((p.y - 1.0).abs() < 1e-6, "on the plane: {p}");
    }
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(3.5, 1.0, 1.5)).is_some());
}

#[test]
fn hiding_the_planes_stops_using_them() {
    let mut d = Driver::empty();
    d.s.add_work_plane(DVec3::new(3.0, 1.0, 0.0), DVec3::Y)
        .expect("plane");
    let mut v = d.s.view_style().clone();
    v.work_planes = false;
    d.s.set_view_style(v);
    d.tool(RECT);
    d.click(DVec3::new(2.0, 2.0, 0.0));
    d.click(DVec3::new(4.0, 3.0, 0.0));
    for (_, p) in d.s.model().vertices() {
        assert!(p.z.abs() < 1e-6, "on the ground: {p}");
    }
}

#[test]
fn a_face_in_front_of_the_plane_still_wins() {
    let mut d = Driver::empty();
    d.tool(RECT);
    d.click(DVec3::new(2.0, 0.0, 0.0));
    d.click(DVec3::new(4.0, 2.0, 0.0));
    // A plane well below the ground square: the rectangle is nearer.
    d.s.add_work_plane(DVec3::new(3.0, 1.0, -1.0), DVec3::Z)
        .expect("plane");
    d.hover(DVec3::new(3.0, 1.0, 0.0));
    let i = d.s.inference().expect("inference");
    assert_eq!(i.kind, prism_infer::Kind::OnFace);
}

#[test]
fn construction_planes_stay_drawn_and_the_menu_commands_work() {
    let mut d = Driver::new(prism_tools::scenes::pocket());
    d.s.add_work_plane(DVec3::new(0.0, 0.0, 1.0), DVec3::Z)
        .expect("plane");
    d.tool(SELECT);
    d.send(Input::Move { x: 50.0, y: 50.0 });
    let lines = d.s.overlay().lines.len();
    assert!(lines >= 6, "the outline and the cross ({lines})");
    // Use / stop using it, then delete it.
    act(&mut d, "cplane.active");
    assert!(d.s.document().assembly().active_work_plane().is_none());
    act(&mut d, "cplane.active");
    assert!(d.s.document().assembly().active_work_plane().is_some());
    act(&mut d, "cplane.delete");
    assert!(!d.s.has_work_planes());
    d.send(Input::Move { x: 51.0, y: 50.0 });
    assert!(d.s.overlay().lines.len() < lines);
}

// ---------- Picking annotations with Alt + click (brief 46 gaps) ----------

/// Alt + click (and release) at a screen point.
fn alt_click(d: &mut Driver, s: DVec2) {
    d.send(Input::Move { x: s.x, y: s.y });
    d.send(Input::Press {
        button: Button::Left,
        x: s.x,
        y: s.y,
        shift: false,
        ctrl: false,
        alt: true,
    });
    d.send(Input::Release {
        button: Button::Left,
    });
    d.t += 0.5;
}

/// A document with the pocket and a section plane across it at `z`.
fn cut_pocket(z: f64) -> Driver {
    let mut doc = prism_tools::scenes::pocket();
    doc.run(prism_doc::Command::AddSection {
        origin: DVec3::new(0.0, 0.0, z),
        normal: DVec3::Z,
    });
    Driver::new(doc)
}

/// The middle of the outline's longest drawn edge at height `z`.
fn outline_point(d: &mut Driver, z: f64) -> DVec2 {
    d.send(Input::Move { x: 5.0, y: 5.0 });
    let lines = d.s.overlay().lines;
    let l = lines
        .iter()
        .filter(|l| (l.a.z - z).abs() < 1e-9 && (l.b.z - z).abs() < 1e-9)
        .max_by(|a, b| (a.a - a.b).length().total_cmp(&(b.a - b.b).length()))
        .expect("the plane's outline");
    d.screen((l.a + l.b) * 0.5)
}

#[test]
fn alt_click_picks_a_plane_then_picks_it_up_and_a_click_sets_it() {
    let mut d = cut_pocket(1.0);
    let at = outline_point(&mut d, 1.0);
    // A plain click on the outline does not pick it (it is not the model).
    d.click_px(at, false, false);
    assert_eq!(d.s.picked_annotation(), None);
    alt_click(&mut d, at);
    assert_eq!(d.s.picked_annotation(), Some("section"));
    // Alt + click on the picked one picks it up; it follows the cursor.
    alt_click(&mut d, at);
    move_to(&mut d, DVec3::new(0.4, 0.4, 0.5));
    d.click(DVec3::new(0.4, 0.4, 0.5));
    let (_, sec) = d.s.document().assembly().active_section().unwrap();
    // It moved with the cursor (by how far the cursor went along its
    // normal's line from where it was picked up; typed distances are exact).
    assert!((sec.origin.z - 1.0).abs() > 0.1, "{:?}", sec.origin);
    // Alt + click on nothing lets go.
    alt_click(&mut d, DVec2::new(5.0, 5.0));
    assert_eq!(d.s.picked_annotation(), None);
}

#[test]
fn a_picked_plane_takes_a_typed_distance_and_delete_removes_it() {
    let mut d = cut_pocket(1.0);
    let at = outline_point(&mut d, 1.0);
    alt_click(&mut d, at);
    alt_click(&mut d, at);
    assert!(d.s.in_operation(), "it is picked up");
    d.typed("0.4");
    let (_, sec) = d.s.document().assembly().active_section().unwrap();
    assert!((sec.origin.z - 1.4).abs() < 1e-9, "{:?}", sec.origin);
    // Delete takes the picked plane when nothing else is selected.
    let at = outline_point(&mut d, 1.4);
    alt_click(&mut d, at);
    assert_eq!(d.s.picked_annotation(), Some("section"));
    act(&mut d, "edit.delete");
    assert!(!d.s.has_sections());
    assert_eq!(d.s.picked_annotation(), None);
    // And one undo brings it back.
    act(&mut d, "edit.undo");
    assert!(d.s.has_sections());
}

#[test]
fn right_click_on_a_plane_picks_it_but_on_the_model_does_not() {
    let mut d = cut_pocket(1.0);
    let at = outline_point(&mut d, 1.0);
    d.send(Input::Move { x: at.x, y: at.y });
    d.send(Input::Press {
        button: Button::Right,
        x: at.x,
        y: at.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Release {
        button: Button::Right,
    });
    assert_eq!(d.s.picked_annotation(), Some("section"));
    // On a face of the model: none is picked, the model is.
    let f = d.screen(DVec3::new(0.4, 0.4, 0.0));
    d.send(Input::Press {
        button: Button::Right,
        x: f.x,
        y: f.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Release {
        button: Button::Right,
    });
    assert_eq!(d.s.picked_annotation(), None);
}

#[test]
fn a_dimension_is_picked_moved_by_typing_and_deleted() {
    let mut doc = prism_tools::scenes::pocket();
    doc.run(prism_doc::Command::AddDimension {
        a: DVec3::new(-2.0, -2.0, 2.0),
        b: DVec3::new(2.0, -2.0, 2.0),
        offset: DVec3::new(0.0, -1.0, 0.0),
    });
    let mut d = Driver::new(doc);
    // Its dimension line, at y = -3.
    let at = d.screen(DVec3::new(0.0, -3.0, 2.0));
    alt_click(&mut d, at);
    assert_eq!(d.s.picked_annotation(), Some("dimension"));
    alt_click(&mut d, at);
    d.typed("2");
    let (_, dim) =
        d.s.document()
            .assembly()
            .dimensions()
            .0
            .iter()
            .next()
            .unwrap();
    assert!((dim.offset.length() - 2.0).abs() < 1e-9, "{:?}", dim.offset);
    assert!(dim.offset.y < 0.0, "still on the same side");
    act(&mut d, "edit.delete");
    assert!(!d.s.has_dimensions());
}

#[test]
fn a_construction_plane_is_picked_and_moved_along_its_normal() {
    let mut doc = prism_tools::scenes::pocket();
    doc.run(prism_doc::Command::AddWorkPlane {
        origin: DVec3::new(0.0, 0.0, 1.0),
        normal: DVec3::Z,
    });
    let mut d = Driver::new(doc);
    let at = outline_point(&mut d, 1.0);
    alt_click(&mut d, at);
    assert_eq!(d.s.picked_annotation(), Some("cplane"));
    alt_click(&mut d, at);
    d.typed("0.5");
    let (_, p) = d.s.document().assembly().active_work_plane().unwrap();
    assert!((p.origin.z - 1.5).abs() < 1e-9, "{:?}", p.origin);
}

#[test]
fn shift_click_makes_a_construction_plane_through_three_points() {
    use prism_tools::session::CPLANE_TOOL;
    let mut d = Driver::empty();
    d.tool(CPLANE_TOOL);
    let p = |x: f64, y: f64, z: f64| DVec3::new(x, y, z);
    // The ground is under the cursor, so the points are on it: draw a
    // vertical wall's three points with the arrows locking nothing, by
    // giving the points directly through inference-free clicks on the
    // ground, then a plane tilted by the third: check the first two lie
    // on the plane and a line of three is refused.
    d.send(Input::Move { x: 400.0, y: 300.0 });
    let a = d.screen(p(0.0, 0.0, 0.0));
    let b = d.screen(p(2.0, 0.0, 0.0));
    let c = d.screen(p(1.0, 1.0, 0.0));
    d.click_px(a, true, false);
    assert!(d.s.hint().contains("second point"), "{}", d.s.hint());
    d.click_px(b, false, false);
    assert!(d.s.hint().contains("third point"), "{}", d.s.hint());
    d.click_px(c, false, false);
    let (_, plane) =
        d.s.document()
            .assembly()
            .active_work_plane()
            .expect("a plane");
    assert!(
        plane.normal.dot(DVec3::Z).abs() > 0.999,
        "on the ground: {:?}",
        plane.normal
    );
    // Three points in a line make nothing.
    let n = d.s.document().assembly().work_planes().0.len();
    d.click_px(a, true, false);
    d.click_px(b, false, false);
    d.click_px(d.screen(p(4.0, 0.0, 0.0)), false, false);
    assert_eq!(d.s.document().assembly().work_planes().0.len(), n);
    d.send(Input::Key { key: Key::Escape });
    assert!(!d.s.in_operation());
}

#[test]
fn text_lands_on_the_working_plane() {
    use prism_tools::session::TEXT_TOOL;
    let mut doc = Document::new();
    // A wall plane across x = 1.
    doc.run(prism_doc::Command::AddWorkPlane {
        origin: DVec3::new(1.0, 0.0, 0.0),
        normal: DVec3::X,
    });
    let mut d = Driver::new(doc);
    d.s.text_input(prism_tools::session::TextSet::Text("A".into()));
    d.tool(TEXT_TOOL);
    d.hover(DVec3::new(1.0, 0.5, 0.5));
    d.click(DVec3::new(1.0, 0.5, 0.5));
    let g = text_groups(&d);
    assert_eq!(g.len(), 1, "{g:?}");
    // The text faces along the plane's normal (x), not up.
    let n = g[0].1.transform_vector3(DVec3::Z);
    assert!(n.dot(DVec3::X).abs() > 0.99, "{n}");
}

#[test]
fn a_dimension_made_with_the_tool_follows_the_corner_it_was_drawn_to() {
    use prism_tools::session::DIM_TOOL;
    let mut d = Driver::new(prism_tools::scenes::cube());
    let model = d.s.document().model().clone();
    let corners: Vec<_> = model.vertices().collect();
    // The two ends of the box's front-bottom edge, and a point off to the side.
    let (ia, a) = *corners
        .iter()
        .min_by(|x, y| (x.1.x + x.1.y + x.1.z).total_cmp(&(y.1.x + y.1.y + y.1.z)))
        .unwrap();
    let (ib, b) = *corners
        .iter()
        .find(|(_, p)| (p.y - a.y).abs() < 1e-9 && (p.z - a.z).abs() < 1e-9 && p.x > a.x + 1e-9)
        .unwrap();
    let _ = ia;
    d.tool(DIM_TOOL);
    d.click(a);
    d.click(b);
    d.click(a + DVec3::new(1.0, -1.5, 0.0));
    let (_, dim) =
        d.s.document()
            .assembly()
            .dimensions()
            .0
            .iter()
            .next()
            .expect("a dimension");
    let len = dim.length();
    assert_eq!(d.s.document().assembly().dim_anchors().len(), 1, "attached");
    // Both ends are on corners: each is attached, and deleting the
    // dimension forgets the attachment.
    let anchors =
        &d.s.document()
            .assembly()
            .dim_anchors()
            .values()
            .next()
            .unwrap()
            .clone();
    assert!(anchors[0].is_some() && anchors[1].is_some(), "{anchors:?}");
    assert_eq!(anchors[1].as_ref().unwrap().vertex, ib);
    assert!(len > 0.0);
    act(&mut d, "dim.delete");
    assert!(d.s.document().assembly().dim_anchors().is_empty());
}

// ---------- Snap to grid (brief 46 part 5) ----------

#[test]
fn snap_to_grid_lands_on_the_grid_only_when_on_and_near() {
    let mut d = Driver::empty();
    d.tool(LINE);
    // Off (the default): the point is where the cursor is.
    let near = DVec3::new(1.03, 2.04, 0.0);
    d.hover(near);
    let p = d.s.inference().expect("a point").point;
    assert!(p.distance(near) < 0.01, "{p}");
    // On: it goes to the grid point (1, 2), and says so.
    let mut v = d.s.view_style().clone();
    v.snap_grid = true;
    d.s.set_view_style(v);
    d.hover(near);
    let i = d.s.inference().expect("a point");
    assert!(
        i.point.distance(DVec3::new(1.0, 2.0, 0.0)) < 1e-9,
        "{}",
        i.point
    );
    assert_eq!(i.label, "On Grid");
    // A click draws from it exactly.
    d.click(near);
    d.click(DVec3::new(3.03, 4.04, 0.0));
    let m = d.s.model();
    let ps: Vec<DVec3> = m.vertices().map(|(_, p)| p).collect();
    assert!(
        ps.iter()
            .any(|p| p.distance(DVec3::new(1.0, 2.0, 0.0)) < 1e-9),
        "{ps:?}"
    );
    assert!(
        ps.iter()
            .any(|p| p.distance(DVec3::new(3.0, 4.0, 0.0)) < 1e-9),
        "{ps:?}"
    );
}

#[test]
fn an_object_snap_wins_over_the_grid() {
    let mut d = Driver::empty();
    d.tool(LINE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(2.4, 0.0, 0.0));
    d.send(Input::Key { key: Key::Escape });
    let mut v = d.s.view_style().clone();
    v.snap_grid = true;
    d.s.set_view_style(v);
    d.hover(DVec3::new(2.4, 0.01, 0.0));
    let i = d.s.inference().expect("a point");
    assert!(
        i.point.distance(DVec3::new(2.4, 0.0, 0.0)) < 1e-6,
        "the end, not 2: {}",
        i.point
    );
}

// ---------- Effects (brief 47) ----------

use prism_view::effects::Kind as Fx;

fn running(d: &Driver) -> Vec<(Fx, usize)> {
    d.s.effects()
        .iter()
        .map(|e| (e.kind.clone(), e.segs.len()))
        .collect()
}

#[test]
fn a_rectangle_echoes_then_the_effect_ends() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 1.0);
    assert_eq!(running(&d), vec![(Fx::Echo, 4)], "its four sides");
    assert!(d.s.animating(), "frames keep coming while it runs");
    let accent = d.s.style.feedback.effect;
    let ours = |d: &Driver| {
        d.s.overlay()
            .lines
            .iter()
            .filter(|l| l.color.0[..3] == accent.0[..3])
            .count()
    };
    assert!(ours(&d) >= 4, "drawn over the model");
    d.wait(1.1);
    assert!(
        running(&d).is_empty() && !d.s.animating(),
        "over after a second"
    );
    assert_eq!(ours(&d), 0);
}

#[test]
fn a_line_traces_and_push_pull_sweeps_up() {
    let mut d = Driver::empty();
    d.tool(LINE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(3.0, 0.0, 0.0));
    d.send(Input::Key { key: Key::Escape });
    assert_eq!(running(&d).last(), Some(&(Fx::Trace, 1)));
    d.wait(1.1);
    rect_on_ground(&mut d, 0.0, 2.0, 2.0, 4.0);
    d.wait(1.1);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 3.0, 0.0));
    d.hover(DVec3::new(1.0, 3.0, 0.5));
    d.typed("1.5");
    let e = d.s.effects().last().expect("an effect").clone();
    let Fx::Sweep(v) = e.kind else {
        panic!("{:?}", e.kind)
    };
    assert!(
        (v - DVec3::new(0.0, 0.0, 1.5)).length() < 1e-9,
        "travels up: {v:?}"
    );
    assert_eq!(e.segs.len(), 8, "the top and the four sides");
}

#[test]
fn erase_and_move_fade_what_went() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 1.0);
    d.wait(1.1);
    d.tool(SELECT);
    act(&mut d, "edit.selall");
    act(&mut d, "edit.delete");
    assert_eq!(running(&d), vec![(Fx::Ghost, 4)]);
    act(&mut d, "edit.undo");
    assert!(running(&d).is_empty(), "undo does not animate");
    assert_eq!(d.faces(), 1);
    act(&mut d, "edit.selall");
    d.tool(MOVE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.click(DVec3::new(0.0, 3.0, 0.0));
    let e = d.s.effects().last().expect("an effect").clone();
    let Fx::Travel(turn) = e.kind else {
        panic!("{:?}", e.kind)
    };
    assert_eq!(e.segs.len(), 4, "the old outline travels");
    assert!(e.pins.iter().all(|p| *p == [false; 2]), "all of it moved");
    let end = turn.at(1.0).transform_point3(DVec3::ZERO);
    assert!(
        (end - DVec3::new(0.0, 3.0, 0.0)).length() < 1e-9,
        "to where it landed"
    );
}

#[test]
fn moving_one_corner_pins_the_far_ends() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 1.0);
    d.wait(1.1);
    d.tool(MOVE);
    d.click(DVec3::new(2.0, 1.0, 0.0));
    d.click(DVec3::new(2.0, 1.0, 1.0));
    let e = d.s.effects().last().expect("an effect");
    assert!(matches!(e.kind, Fx::Travel(_)), "{:?}", e.kind);
    assert_eq!(e.segs.len(), 2, "the two sides at the corner");
    let pinned = e.pins.iter().flatten().filter(|p| **p).count();
    assert_eq!(pinned, 2, "their far ends stay");
}

#[test]
fn offset_grows_from_the_face() {
    use prism_tools::session::OFFSET;
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 4.0);
    d.wait(1.1);
    d.tool(OFFSET);
    d.click(DVec3::new(0.5, 2.0, 0.0));
    d.hover(DVec3::new(1.0, 2.0, 0.0));
    d.typed("1");
    let e = d.s.effects().last().expect("an effect").clone();
    assert_eq!(e.kind, Fx::Morph);
    assert_eq!(e.segs.len(), 4);
    // Each new corner starts on the old outline.
    let on = |p: DVec3| {
        p.x.abs() < 1e-9 || p.y.abs() < 1e-9 || (p.x - 4.0).abs() < 1e-9 || (p.y - 4.0).abs() < 1e-9
    };
    assert!(e.from.iter().flatten().all(|p| on(*p)), "{:?}", e.from);
}

#[test]
fn follow_me_carries_its_profile_along_the_path() {
    use prism_tools::session::FOLLOW;
    let mut m = prism_topo::Model::new();
    let c = [(-0.5, 1.5), (0.5, 1.5), (0.5, 2.5), (-0.5, 2.5)].map(|(y, z)| DVec3::new(0.0, y, z));
    for i in 0..4 {
        prism_ops::add_segment(&mut m, c[i], c[(i + 1) % 4]);
    }
    prism_ops::add_segment(&mut m, DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0));
    prism_ops::add_segment(&mut m, DVec3::new(4.0, 0.0, 0.0), DVec3::new(4.0, 3.0, 0.0));
    let mut d = Driver::new(Document::with_model(m));
    d.look(Camera {
        eye: DVec3::new(6.0, -8.0, 9.0),
        target: DVec3::new(2.0, 1.0, 1.0),
    });
    d.tool(SELECT);
    let a = d.screen(DVec3::new(2.0, 0.0, 0.0));
    d.click_px(a, false, false);
    let b = d.screen(DVec3::new(4.0, 1.5, 0.0));
    d.click_px(b, true, false);
    d.tool(FOLLOW);
    d.click(DVec3::new(0.0, 0.0, 2.0));
    let e = d.s.effects().last().expect("an effect").clone();
    let Fx::Follow(path) = &e.kind else {
        panic!("{:?}", e.kind)
    };
    assert_eq!(e.segs.len(), 4, "the profile's outline");
    assert!(
        path[0].length() < 1e-9,
        "from the end the profile is at: {path:?}"
    );
    assert!((path[path.len() - 1] - DVec3::new(4.0, 3.0, 0.0)).length() < 1e-9);
}

#[test]
fn a_loft_tiles_its_new_surfaces() {
    use prism_tools::session::LOFT;
    let mut d = arcs_round_a_corner();
    d.tool(LOFT);
    pick(&mut d, TOP_ARC);
    key(&mut d, Key::Enter);
    pick(&mut d, SIDE_ARC);
    key(&mut d, Key::Enter);
    pick(&mut d, FRONT_ARC);
    d.wait(2.5);
    key(&mut d, Key::Enter);
    let e = d.s.effects().last().expect("an effect");
    assert!(matches!(e.kind, Fx::Tile(_)), "{:?}", e.kind);
    assert!(
        e.segs.len() > 12,
        "the surfaces' grid, not only their edges"
    );
}

#[test]
fn groups_planes_and_dimensions_animate() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 1.0);
    d.wait(1.1);
    d.tool(SELECT);
    act(&mut d, "edit.selall");
    act(&mut d, "edit.group");
    assert_eq!(
        running(&d),
        vec![(Fx::Draw, 12)],
        "the group's box draws in"
    );
    d.wait(1.1);
    assert!(
        d.s.add_section_plane(DVec3::new(1.0, 0.5, 0.0), DVec3::X)
            .is_some()
    );
    assert_eq!(running(&d), vec![(Fx::Echo, 4)], "the plane's outline");
    d.s.add_work_plane(DVec3::new(0.0, 0.0, 2.0), DVec3::Z);
    assert_eq!(running(&d).len(), 2);
}

#[test]
fn a_new_dimension_traces_its_line() {
    let mut d = Driver::empty();
    d.tool("tool.dim");
    d.click(DVec3::new(0.0, 2.0, 0.0));
    d.click(DVec3::new(3.0, 2.0, 0.0));
    d.click(DVec3::new(1.5, 3.0, 0.0));
    assert_eq!(running(&d), vec![(Fx::Trace, 3)], "out, along and back");
}

#[test]
fn the_origin_snaps_while_the_axes_show() {
    let mut d = Driver::empty();
    d.tool(LINE);
    d.hover(DVec3::new(0.01, 0.01, 0.0));
    assert_eq!(d.s.inference().map(|i| i.kind), Some(Kind::Origin));
    act(&mut d, "view.axes");
    d.hover(DVec3::new(0.02, 0.01, 0.0));
    assert_ne!(
        d.s.inference().map(|i| i.kind),
        Some(Kind::Origin),
        "hidden axes: no origin"
    );
}

#[test]
fn effects_off_and_reduced_motion() {
    let mut d = Driver::empty();
    d.s.set_effects(false);
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 1.0);
    assert!(running(&d).is_empty(), "off: nothing is even worked out");
    d.s.set_effects(true);
    d.s.reduce_motion = true;
    rect_on_ground(&mut d, 3.0, 0.0, 5.0, 1.0);
    assert_eq!(running(&d).len(), 1);
    d.wait(0.45);
    assert!(running(&d).is_empty(), "a short fade with reduced motion");
    rect_on_ground(&mut d, 6.0, 0.0, 8.0, 1.0);
    d.s.set_effects(false);
    assert!(running(&d).is_empty(), "turning it off stops it");
}

#[test]
fn a_locked_axis_sets_the_section_planes_direction() {
    use prism_tools::session::SECTION_TOOL;
    let mut d = Driver::new(prism_tools::scenes::cube());
    d.tool(SECTION_TOOL);
    d.send(Input::Key { key: Key::Right }); // lock red
    d.click(DVec3::new(0.0, 0.0, 3.0));
    let (_, sec) = d.s.document().assembly().active_section().expect("a plane");
    assert_eq!(
        sec.normal.x.abs(),
        1.0,
        "square to red, not to the top face: {:?}",
        sec.normal
    );
    assert!(
        (sec.origin - DVec3::new(0.0, 0.0, 3.0)).length() < 0.05,
        "{:?}",
        sec.origin
    );
}

#[test]
fn a_moving_section_plane_snaps_level_with_a_point() {
    use prism_tools::session::SECTION_TOOL;
    let mut d = Driver::new(prism_tools::scenes::cube());
    d.tool(SECTION_TOOL);
    d.click(DVec3::new(0.0, 0.0, 3.0));
    // The middle of a vertical edge: the plane goes level with it.
    let mid = DVec3::new(1.0, -1.0, 2.0);
    move_to(&mut d, mid);
    assert_eq!(d.s.inference().map(|i| i.kind), Some(Kind::Midpoint));
    d.click(mid);
    let (_, sec) = d.s.document().assembly().active_section().expect("a plane");
    assert!(
        (sec.origin.z - 2.0).abs() < 1e-9,
        "exactly level: {:?}",
        sec.origin
    );
}

#[test]
fn a_plane_picked_up_in_select_snaps_level_with_a_point() {
    let mut d = cut_pocket(1.5);
    let at = outline_point(&mut d, 1.5);
    alt_click(&mut d, at);
    alt_click(&mut d, at);
    // A corner of the box's base: the plane goes down to z = 0.
    let corner = DVec3::new(4.0, 0.0, 0.0);
    move_to(&mut d, corner);
    assert_eq!(d.s.inference().map(|i| i.kind), Some(Kind::Endpoint));
    d.click(corner);
    let (_, sec) = d.s.document().assembly().active_section().unwrap();
    assert!(sec.origin.z.abs() < 1e-9, "exactly level: {:?}", sec.origin);
}

// ---------- Fillet: collect, then Enter (owner, 2026-11-18) ----------

#[test]
fn clicks_collect_edges_that_meet_and_enter_rounds_them_together() {
    let mut d = box_for_rounding();
    let shown = |d: &Driver| d.s.display_scene().faces.len();
    d.click(DVec3::new(2.0, 0.0, 1.0));
    d.click(DVec3::new(1.0, 0.0, 2.0));
    assert_eq!(d.s.selection().edges.len(), 2);
    assert_eq!(d.faces(), 6, "nothing made yet");
    assert!(shown(&d) > 6, "the set shows rounded");
    // A click on a collected edge takes it out, another puts it back.
    d.click(DVec3::new(1.0, 0.0, 2.0));
    assert_eq!(d.s.selection().edges.len(), 1);
    d.click(DVec3::new(1.0, 0.0, 2.0));
    d.send(Input::Key { key: Key::Enter });
    assert_eq!(cylinders(&d), 2, "both, joined where they meet");
    valid(&d);
    undo(&mut d);
    assert_eq!(d.faces(), 6, "one undo");
}

#[test]
fn a_drag_from_an_edge_sets_the_size_and_enter_rounds() {
    let mut d = box_for_rounding();
    let a = d.screen(DVec3::new(2.0, 0.0, 1.0));
    let b = d.screen(DVec3::new(1.7, 0.0, 1.0));
    d.send(Input::Move { x: a.x, y: a.y });
    d.send(Input::Press {
        button: Button::Left,
        x: a.x,
        y: a.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    for k in 1..=4 {
        let p = a.lerp(b, f64::from(k) / 4.0);
        d.send(Input::Move { x: p.x, y: p.y });
    }
    d.send(Input::Release {
        button: Button::Left,
    });
    assert_eq!(
        d.s.selection().edges.len(),
        1,
        "the dragged edge is collected"
    );
    assert_eq!(d.faces(), 6, "a drag only sets the size");
    d.send(Input::Key { key: Key::Enter });
    assert_eq!(radii(&d).len(), 1);
    assert!((radii(&d)[0] - 0.3).abs() < 1e-6, "{:?}", radii(&d));
    valid(&d);
}

#[test]
fn a_typed_size_rounds_the_set_and_esc_or_leaving_makes_nothing() {
    use prism_tools::session::FILLET;
    let mut d = box_for_rounding();
    d.click(DVec3::new(2.0, 0.0, 1.0));
    d.typed("0.25");
    assert!((radii(&d)[0] - 0.25).abs() < 1e-9, "{:?}", radii(&d));
    undo(&mut d);
    // Esc clears the set.
    d.click(DVec3::new(2.0, 0.0, 1.0));
    d.send(Input::Key { key: Key::Escape });
    assert!(d.s.selection().is_empty());
    d.send(Input::Key { key: Key::Enter });
    assert_eq!(d.faces(), 6, "nothing to round");
    // Leaving the tool makes nothing.
    d.click(DVec3::new(2.0, 0.0, 1.0));
    d.tool(PUSH);
    assert_eq!(d.faces(), 6);
    assert_eq!(d.s.display_scene().faces.len(), 6, "no preview left");
    let _ = FILLET;
}

// ---------- Inspector, part 1 (brief 48) ----------

fn inspector_text(d: &Driver, key: &str) -> (String, bool) {
    let i = d.s.inspector();
    let f = i
        .sections
        .iter()
        .flat_map(|s| &s.fields)
        .find(|f| f.key == key)
        .unwrap_or_else(|| panic!("no field {key}"));
    (f.text.clone(), f.mixed)
}

fn group_min(d: &Driver, g: prism_doc::InstanceId) -> DVec3 {
    d.s.group_box(g)
        .expect("a box")
        .iter()
        .fold(DVec3::splat(f64::MAX), |m, p| m.min(*p))
}

#[test]
fn the_inspector_shows_a_group_and_edits_it_in_one_undo_step_each() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    let i = d.s.inspector();
    assert_eq!(i.summary, "Group");
    let titles: Vec<_> = i.sections.iter().map(|s| s.title).collect();
    assert_eq!(titles, ["General", "Position", "Size"]);
    assert_eq!(inspector_text(&d, "w").0, "2.000 m");
    assert_eq!(inspector_text(&d, "volume").0, "8.000 m³");
    assert_eq!(inspector_text(&d, "faces").0, "6");
    // Position: a typed expression, the box moves, undo puts it back.
    d.s.inspector_set("x", "1m+50cm").expect("moves");
    assert!((group_min(&d, g).x - 1.5).abs() < 1e-9);
    assert_eq!(inspector_text(&d, "x").0, "1.500 m");
    undo(&mut d);
    assert!(group_min(&d, g).x.abs() < 1e-9);
    // Size: scaled about the low corner, so the position stays.
    d.s.inspector_set("w", "4").expect("scales");
    assert_eq!(inspector_text(&d, "w").0, "4.000 m");
    assert_eq!(inspector_text(&d, "d").0, "2.000 m");
    assert_eq!(inspector_text(&d, "volume").0, "16.000 m³");
    assert!(group_min(&d, g).x.abs() < 1e-9);
    undo(&mut d);
    assert_eq!(inspector_text(&d, "w").0, "2.000 m");
}

#[test]
fn typing_a_new_tag_in_the_inspector_makes_it_and_applies_it_in_one_step() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    let steps = d.s.document().log().count();
    d.s.inspector_set("tag", "Walls").expect("a new tag");
    assert_eq!(d.s.tag_rows().len(), 1);
    assert_eq!(d.s.tag_rows()[0].1, "Walls");
    assert_eq!(inspector_text(&d, "tag").0, "Walls");
    assert!(d.s.document().assembly().tag_of(g).is_some());
    assert_eq!(d.s.document().log().count(), steps + 1, "one step");
    // The same name again (any case) uses the tag that is there.
    act(&mut d, "edit.undo");
    assert!(d.s.tag_rows().is_empty(), "one undo takes both away");
    act(&mut d, "edit.redo");
    d.s.inspector_set("tag", "None").expect("off");
    d.s.inspector_set("tag", "walls").expect("on again");
    assert_eq!(d.s.tag_rows().len(), 1, "no second tag");
    assert!(d.s.document().assembly().tag_of(g).is_some());
    // Empty takes it off.
    d.s.inspector_set("tag", "").expect("off");
    assert!(d.s.document().assembly().tag_of(g).is_none());
    // The field offers the tags that exist.
    let f =
        d.s.inspector()
            .sections
            .iter()
            .flat_map(|s| s.fields.clone())
            .find(|f| f.key == "tag")
            .expect("tag row");
    assert!(
        matches!(f.kind, prism_tools::session::FieldKind::Suggest(ref o) if o == &["None", "Walls"]),
        "{:?}",
        f.kind
    );
}

#[test]
fn an_object_can_be_on_several_tags_typed_or_clicked() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    let steps = d.s.document().log().count();
    d.s.inspector_set("tag", "Walls, breakable , Stone")
        .expect("three tags");
    assert_eq!(d.s.tag_rows().len(), 3);
    assert_eq!(d.s.document().assembly().tags_of(g).len(), 3);
    assert_eq!(inspector_text(&d, "tag").0, "Walls, breakable, Stone");
    assert_eq!(d.s.document().log().count(), steps + 1, "one undo step");
    // A click puts one on or takes it off, leaving the rest.
    d.s.inspector_set("tag.toggle", "breakable").expect("off");
    assert_eq!(inspector_text(&d, "tag").0, "Walls, Stone");
    d.s.inspector_set("tag.toggle", "loose")
        .expect("a new one, on");
    assert_eq!(d.s.tag_rows().len(), 4);
    assert_eq!(inspector_text(&d, "tag").0, "Walls, Stone, loose");
    // Hidden if any of its tags is hidden.
    let walls = d.s.tag_rows()[0].0;
    assert!(d.s.document().assembly().is_shown(g));
    d.s.tag_command(prism_tools::TagOp::Show(walls, false))
        .expect("hides");
    assert!(!d.s.document().assembly().is_shown(g));
    d.s.tag_command(prism_tools::TagOp::Show(walls, true))
        .expect("shows");
    // None takes them all off.
    d.s.inspector_set("tag", "None").expect("none");
    assert!(d.s.document().assembly().tags_of(g).is_empty());
}

#[test]
fn a_beverly_word_is_put_on_the_selection_and_taken_off_by_a_second_click() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    let word = prism_tools::BEVERLY_TAGS[0].1[0];
    assert_eq!(word, "collider");
    d.s.tag_command(prism_tools::TagOp::ToggleNamed(word.into()))
        .expect("on");
    assert_eq!(d.s.tag_rows()[0].1, word);
    assert_eq!(d.s.document().assembly().tags_of(g).len(), 1);
    d.s.tag_command(prism_tools::TagOp::ToggleNamed("breakable".into()))
        .expect("a second word");
    assert_eq!(d.s.document().assembly().tags_of(g).len(), 2);
    d.s.tag_command(prism_tools::TagOp::ToggleNamed(word.into()))
        .expect("off");
    assert_eq!(d.s.document().assembly().tags_of(g).len(), 1);
    // Nothing selected: refused with words.
    act(&mut d, "edit.desel");
    assert!(
        d.s.tag_command(prism_tools::TagOp::ToggleNamed("loose".into()))
            .is_err()
    );
}

#[test]
fn tags_added_in_the_window_take_a_name_and_a_taken_name_is_refused() {
    let mut d = Driver::empty();
    grouped_box(&mut d);
    d.s.tag_command(prism_tools::TagOp::Add("Roof".into()))
        .expect("adds");
    assert_eq!(d.s.tag_rows()[0].1, "Roof");
    assert!(
        d.s.tag_command(prism_tools::TagOp::Add("  ".into()))
            .is_err(),
        "a name is needed"
    );
    let err =
        d.s.tag_command(prism_tools::TagOp::Add("roof".into()))
            .expect_err("taken, case aside");
    assert!(err.contains("already called"), "{err}");
    d.s.tag_command(prism_tools::TagOp::Add("Floor".into()))
        .expect("adds");
    let floor = d.s.tag_rows()[1].0;
    assert!(
        d.s.tag_command(prism_tools::TagOp::Rename(floor, "Roof".into()))
            .is_err()
    );
    // Renaming a tag to its own name is no clash.
    d.s.tag_command(prism_tools::TagOp::Rename(floor, "Floor".into()))
        .expect("own name");
    assert_eq!(d.s.tag_rows().len(), 2);
}

#[test]
fn the_mirror_tool_puts_a_copy_across_the_face_clicked() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    assert_eq!(d.s.document().assembly().root().instances.len(), 1);
    d.tool(prism_tools::session::MIRROR_TOOL);
    // The top face of the 2 m cube is at z = 2: the copy goes above it.
    let steps = d.s.document().log().count();
    d.click(DVec3::new(1.0, 1.0, 2.0));
    assert_eq!(d.s.document().assembly().root().instances.len(), 2);
    assert_eq!(d.s.document().log().count(), steps + 1, "one undo step");
    let (lo, hi) = d.s.document().assembly().flattened().bbox().expect("box");
    assert!(
        lo.z.abs() < 1e-9 && (hi.z - 4.0).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
    // The copy is turned over: its placement reverses handedness.
    let placed = d.s.document().assembly().placed();
    assert!(
        placed.iter().any(|p| p.world.matrix3.determinant() < 0.0),
        "one is a mirror image"
    );
    assert!(d.s.document().assembly().find_instance(g).is_some());
    act(&mut d, "edit.undo");
    assert_eq!(d.s.document().assembly().root().instances.len(), 1);
}

#[test]
fn the_mirror_tool_copies_loose_faces_across_a_face_and_the_copy_faces_out() {
    // A 2 m cube drawn loose, all selected. Mirror across its own x = 2
    // wall: a second cube beside it, sharing that wall, all valid and
    // every face still facing out.
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("2");
    d.tool(SELECT);
    act(&mut d, "edit.selall");
    let faces = d.faces();
    d.tool(prism_tools::session::MIRROR_TOOL);
    // The x = 2 wall, seen from +x (the camera is at x = 3).
    d.click(DVec3::new(2.0, 1.0, 1.0));
    let (lo, hi) = d.s.model().bbox().expect("box");
    assert!(
        lo.x.abs() < 1e-9 && (hi.x - 4.0).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
    // The two walls at x = 2 face each other and cancel (as touching solids
    // do), leaving one solid whose long sides are in two pieces: 10 faces.
    assert_eq!(d.faces(), faces * 2 - 2, "{} faces", d.faces());
    let problems = prism_ops::check::problems(d.s.model());
    assert!(problems.is_empty(), "{problems:?}");
    act(&mut d, "edit.undo");
    assert_eq!(d.faces(), faces);
}

#[test]
fn the_array_tool_makes_a_row_of_copies_with_a_ghost_and_one_undo_step() {
    use prism_tools::session::{ARRAY_TOOL, ArraySet};
    let mut d = Driver::empty();
    grouped_box(&mut d);
    d.tool(ARRAY_TOOL);
    d.s.array_input(ArraySet::Count("4".into()));
    d.s.array_input(ArraySet::Spacing("3".into()));
    let panel = d.s.array_panel().expect("the window shows with its tool");
    assert_eq!(panel.settings.count, 4);
    // The ghost is drawn but nothing is made yet.
    assert_eq!(d.s.document().assembly().root().instances.len(), 1);
    let steps = d.s.document().log().count();
    d.s.array_input(ArraySet::Make);
    assert_eq!(d.s.document().assembly().root().instances.len(), 4);
    assert_eq!(d.s.document().log().count(), steps + 1, "one undo step");
    // The cube is 2 m wide: four of them, 3 m apart, reach 11 m.
    let (lo, hi) = d.s.document().assembly().flattened().bbox().expect("box");
    assert!(
        lo.x.abs() < 1e-9 && (hi.x - 11.0).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
    act(&mut d, "edit.undo");
    assert_eq!(d.s.document().assembly().root().instances.len(), 1);
}

#[test]
fn a_polar_array_turns_copies_round_the_middle_and_loose_faces_work_too() {
    use prism_tools::session::{ARRAY_TOOL, ArrayKind, ArraySet};
    let mut d = Driver::empty();
    grouped_box(&mut d);
    d.tool(ARRAY_TOOL);
    d.s.array_input(ArraySet::Kind(ArrayKind::Polar));
    d.s.array_input(ArraySet::Count("4".into()));
    d.s.array_input(ArraySet::Axis(2));
    d.s.array_input(ArraySet::Make);
    assert_eq!(d.s.document().assembly().root().instances.len(), 4);
    // Four quarter turns of a cube about its own middle: the same box.
    let (lo, hi) = d.s.document().assembly().flattened().bbox().expect("box");
    assert!(
        lo.x.abs() < 1e-9 && (hi.x - 2.0).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
    // Loose geometry: a rectangle in a row of three.
    let mut e = Driver::empty();
    rect_on_ground(&mut e, 0.0, 0.0, 1.0, 1.0);
    e.tool(SELECT);
    act(&mut e, "edit.selall");
    e.tool(ARRAY_TOOL);
    e.s.array_input(ArraySet::Count("3".into()));
    e.s.array_input(ArraySet::Spacing("2".into()));
    e.s.array_input(ArraySet::Make);
    assert_eq!(e.faces(), 3);
    let (lo, hi) = e.s.model().bbox().expect("box");
    assert!(
        lo.x.abs() < 1e-9 && (hi.x - 5.0).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
    valid(&e);
}

#[test]
fn make_array_with_nothing_selected_says_so() {
    use prism_tools::session::{ARRAY_NOTHING, ARRAY_TOOL, ArraySet};
    let mut d = Driver::empty();
    d.tool(ARRAY_TOOL);
    d.s.array_input(ArraySet::Make);
    assert_eq!(d.s.hint(), ARRAY_NOTHING);
}

#[test]
fn mirror_copy_in_the_menu_uses_the_plane_through_the_selection() {
    let mut d = Driver::empty();
    grouped_box(&mut d);
    // The cube is 0 to 2 in every direction: across the x plane through
    // its middle (x = 1) the copy lands on top of it.
    act(&mut d, "mirror.x");
    assert_eq!(d.s.document().assembly().root().instances.len(), 2);
    // With nothing selected it says what to do.
    let mut e = Driver::empty();
    act(&mut e, "mirror.x");
    assert_eq!(e.s.document().assembly().root().instances.len(), 0);
}

#[test]
fn the_inspector_names_tags_hides_and_locks() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    d.s.inspector_set("name", "Shed").expect("names");
    assert_eq!(d.s.document().assembly().name_of(g), Some("Shed"));
    d.s.tag_command(prism_tools::TagOp::Add("Parts".into()))
        .expect("adds");
    let tag = d.s.tag_rows()[0].1.clone();
    d.s.inspector_set("tag", &tag).expect("tags");
    assert_eq!(inspector_text(&d, "tag").0, tag);
    d.s.inspector_set("tag", "None").expect("untags");
    assert_eq!(inspector_text(&d, "tag").0, "None");
    // A name that is new makes the tag (see the typed-tag tests).
    d.s.inspector_set("locked", "true").expect("locks");
    assert!(d.s.document().assembly().is_locked(g));
    // Locked: edits are refused, unlocking is not.
    assert!(d.s.inspector_set("x", "1").is_err());
    d.s.inspector_set("locked", "false").expect("unlocks");
    d.s.inspector_set("hidden", "true").expect("hides");
    assert!(d.s.document().assembly().is_hidden(g));
}

#[test]
fn the_inspector_refuses_bad_input_and_changes_nothing() {
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    let before = group_min(&d, g);
    for (k, t) in [
        ("x", "abc"),
        ("w", "0"),
        ("w", "-1"),
        ("hidden", "maybe"),
        ("volume", "1"),
    ] {
        assert!(d.s.inspector_set(k, t).is_err(), "{k} {t}");
    }
    assert_eq!(group_min(&d, g), before);
    assert_eq!(inspector_text(&d, "w").0, "2.000 m");
}

#[test]
fn with_several_groups_the_inspector_shows_mixed_values_and_edits_all() {
    let mut d = Driver::empty();
    let a = grouped_box(&mut d);
    d.tool(MOVE);
    d.click(DVec3::new(0.0, 0.0, 0.0));
    d.send(Input::CtrlTap);
    d.hover(DVec3::new(1.0, 0.0, 0.0));
    key(&mut d, Key::Right);
    d.typed("4");
    d.tool(SELECT);
    act(&mut d, "edit.desel");
    act(&mut d, "edit.selall");
    assert_eq!(d.s.selected_groups().len(), 2);
    assert_eq!(d.s.inspector().summary, "2 groups");
    // Both are 2 m wide; together they span 6 m.
    assert_eq!(inspector_text(&d, "w").0, "6.000 m");
    assert!(
        d.s.inspector()
            .sections
            .iter()
            .flat_map(|s| &s.fields)
            .all(|f| f.key != "name"),
        "a name is for one group"
    );
    // Tag one only: the tag shows as mixed.
    d.s.tag_command(prism_tools::TagOp::Add("Parts".into()))
        .expect("adds");
    let tag = d.s.tag_rows()[0].0;
    d.s.select_path(&[a]);
    d.s.tag_command(prism_tools::TagOp::PutSelected(Some(tag)))
        .expect("puts");
    act(&mut d, "edit.selall");
    assert!(inspector_text(&d, "tag").1, "the tags differ");
    // One edit moves both.
    d.s.inspector_set("y", "1").expect("moves both");
    assert!((group_min(&d, a).y - 1.0).abs() < 1e-9);
    undo(&mut d);
    assert!(group_min(&d, a).y.abs() < 1e-9);
}

#[test]
fn the_inspector_describes_selected_faces() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    assert_eq!(d.s.inspector().summary, "Nothing selected");
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    let i = d.s.inspector();
    assert_eq!(i.summary, "1 face");
    assert_eq!(inspector_text(&d, "area").0, "4.000 m²");
    assert!(
        d.s.inspector_set("x", "1").is_err(),
        "geometry has no fields to set"
    );
}

#[test]
fn a_recording_keeps_the_groups_the_open_group_and_the_selection() {
    // The owner's session 12 was recorded inside a component: the top
    // level was empty, so the recording could not be replayed.
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    d.s.open_path_to(&[g]);
    d.s.start_recording();
    d.tool(LINE);
    d.click(DVec3::new(0.5, 0.5, 2.0));
    d.click(DVec3::new(1.5, 0.5, 2.0));
    key(&mut d, Key::Escape);
    let json = d.s.stop_recording().expect("recorded");
    let r: Recording = serde_json::from_str(&json).expect("reads back");
    assert_eq!(r.open, [g]);
    assert!(r.assembly.is_some());
    assert!(r.model.face_count() == 0, "the top level is empty");
    let s = Session::replay(&r);
    assert_eq!(s.open_path(), [g], "it starts inside the group");
    assert!(
        r.ends_with_document(s.document()),
        "the replay ends where the session did"
    );
    // The same recording with the groups left out ends somewhere else.
    let mut bare = r.clone();
    bare.assembly = None;
    bare.open.clear();
    assert!(!bare.ends_with_document(Session::replay(&bare).document()));
}

#[test]
fn a_recording_keeps_the_lens_so_the_same_clicks_land_in_the_same_place() {
    // Clicks are screen points: a parallel view reads them differently
    // from a perspective one.
    let mut d = Driver::empty();
    d.s.set_lens(prism_view::Lens {
        parallel: true,
        ..d.s.lens()
    });
    d.s.start_recording();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.typed("1");
    let json = d.s.stop_recording().expect("recorded");
    let r: Recording = serde_json::from_str(&json).expect("reads back");
    assert!(
        r.ends_with_document(Session::replay(&r).document()),
        "the replay is in the same lens"
    );
    let mut bare = r.clone();
    bare.start = None;
    assert!(
        !bare.ends_with_document(Session::replay(&bare).document()),
        "in the other lens the same points land elsewhere"
    );
}

#[test]
fn a_recording_keeps_edits_made_outside_the_viewport() {
    // The Inspector, windows and the like change the model without an
    // input; the recording holds them as commands so the replay matches.
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    d.tool(SELECT);
    d.s.start_recording();
    d.s.inspector_set("name", "Crate").expect("renames");
    d.tool(LINE);
    let json = d.s.stop_recording().expect("recorded");
    let r: Recording = serde_json::from_str(&json).expect("reads back");
    assert!(
        r.events
            .iter()
            .any(|e| matches!(e.input, Input::Commands { .. }))
    );
    let s = Session::replay(&r);
    assert!(r.ends_with_document(s.document()));
    assert_eq!(s.document().assembly().name_of(g), Some("Crate"));
}

#[test]
fn offset_matches_a_point_in_another_plane_by_the_edge_it_began_near() {
    // Owner's session 12: a snapped point above the face (another part's
    // corner) sets the distance from the edge nearest the first click. It
    // used to pick whichever edge the point's shadow lay beside (here the
    // left edge, outward), so nothing lined up.
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 4.0);
    rect_on_ground(&mut d, -3.0, 1.0, -1.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(-2.0, 1.5, 0.0));
    d.typed("2");
    d.tool(OFFSET);
    d.click(DVec3::new(2.0, 0.3, 0.0));
    d.hover(DVec3::new(-1.0, 1.0, 2.0));
    assert_eq!(
        d.s.inference().map(|i| i.kind),
        Some(prism_infer::Kind::Endpoint),
        "the corner snaps"
    );
    d.click(DVec3::new(-1.0, 1.0, 2.0));
    // One metre in from the bottom edge, all round: a 4 x 4 square leaves
    // a ring of 12 round a 2 x 2 one.
    assert!(
        areas(&d).iter().any(|a| (*a - 12.0).abs() < 1e-6),
        "{:?}",
        areas(&d)
    );
    valid(&d);
}

#[test]
fn offset_highlights_the_edge_its_distance_is_matched_to() {
    // Owner's session 14: while offsetting, an edge the cursor snaps to
    // lights up, so it is clear which one stands for the distance.
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 4.0, 4.0);
    rect_on_ground(&mut d, -3.0, 1.0, -1.0, 3.0);
    d.tool(PUSH);
    d.click(DVec3::new(-2.0, 2.0, 0.0));
    d.typed("2");
    d.tool(OFFSET);
    d.click(DVec3::new(2.0, 2.0, 0.0));
    assert!(!matches!(d.s.hover(), Some(prism_view::Hit::Edge(_))));
    // The block's top edge at x = -1, 2 m up: not a corner or a midpoint.
    d.hover(DVec3::new(-1.0, 1.4, 2.0));
    assert_eq!(
        d.s.inference().map(|i| i.kind),
        Some(prism_infer::Kind::OnEdge)
    );
    assert!(
        matches!(d.s.hover(), Some(prism_view::Hit::Edge(_))),
        "{:?}",
        d.s.hover()
    );
    // Away from every edge, nothing is lit.
    d.hover(DVec3::new(3.0, 3.0, 0.0));
    assert!(!matches!(d.s.hover(), Some(prism_view::Hit::Edge(_))));
}

/// The radii of the round faces in a group's own model.
fn group_radii(d: &Driver, g: prism_doc::InstanceId) -> Vec<f64> {
    let asm = d.s.document().assembly();
    let def = asm.find_instance(g).expect("the group").1.def;
    asm.def(def)
        .expect("its definition")
        .model
        .faces()
        .filter_map(|(_, f)| match f.surface.clone() {
            prism_geom::Surface::Cylinder(c) => Some(c.radius),
            _ => None,
        })
        .collect()
}

#[test]
fn a_round_inside_a_group_stays_live_and_the_inspector_edits_it() {
    use prism_tools::session::FILLET;
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    d.s.open_path_to(&[g]);
    d.tool(FILLET);
    d.hover(DVec3::new(2.0, 0.0, 1.0));
    d.click(DVec3::new(2.0, 0.0, 1.0));
    d.send(Input::Key { key: Key::Enter });
    let rounded = group_radii(&d, g);
    assert_eq!(rounded.len(), 1, "rounded");
    // Close the group and select it: its Inspector lists the feature.
    d.tool(SELECT);
    d.s.open_path_to(&[]);
    d.s.select_path(&[g]);
    let titles: Vec<_> = d.s.inspector().sections.iter().map(|s| s.title).collect();
    assert_eq!(titles, ["General", "Features", "Position", "Size"]);
    assert_eq!(
        inspector_text(&d, "feature.0.size").0,
        d.s.number.show(rounded[0])
    );
    // A new radius: one undo step, and the face count stays the same.
    d.s.inspector_set("feature.0.size", "0.5").expect("changes");
    assert!(
        (group_radii(&d, g)[0] - 0.5).abs() < 1e-9,
        "{:?}",
        group_radii(&d, g)
    );
    undo(&mut d);
    assert!((group_radii(&d, g)[0] - rounded[0]).abs() < 1e-9);
    // Refusals say why and change nothing.
    assert!(d.s.inspector_set("feature.0.size", "0").is_err());
    assert!(d.s.inspector_set("feature.0.size", "50").is_err());
    assert!(d.s.inspector_set("feature.3.size", "1").is_err());
    assert!((group_radii(&d, g)[0] - rounded[0]).abs() < 1e-9);
    // Bake: the round stays, the Features section goes.
    d.s.inspector_set("bake", "").expect("bakes");
    assert_eq!(group_radii(&d, g).len(), 1);
    let titles: Vec<_> = d.s.inspector().sections.iter().map(|s| s.title).collect();
    assert_eq!(titles, ["General", "Position", "Size"]);
    assert!(d.s.inspector_set("bake", "").is_err(), "nothing to bake");
    valid(&d);
}

#[test]
fn bake_features_in_the_edit_menu_makes_the_round_plain() {
    use prism_tools::session::FILLET;
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    d.s.open_path_to(&[g]);
    d.tool(FILLET);
    d.hover(DVec3::new(2.0, 0.0, 1.0));
    d.click(DVec3::new(2.0, 0.0, 1.0));
    d.send(Input::Key { key: Key::Enter });
    d.tool(SELECT);
    d.s.open_path_to(&[]);
    d.s.select_path(&[g]);
    let live = |d: &Driver| {
        let asm = d.s.document().assembly();
        let def = asm.find_instance(g).expect("group").1.def;
        asm.def(def).expect("def").stack.is_some()
    };
    assert!(live(&d));
    act(&mut d, "edit.bake");
    assert!(!live(&d));
    assert_eq!(group_radii(&d, g).len(), 1, "the round is still there");
    undo(&mut d);
    assert!(live(&d), "one undo step");
}

#[test]
fn editing_what_a_live_round_made_bakes_it_and_says_so() {
    use prism_tools::session::FILLET;
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    d.s.open_path_to(&[g]);
    d.tool(FILLET);
    d.hover(DVec3::new(2.0, 0.0, 1.0));
    d.click(DVec3::new(2.0, 0.0, 1.0));
    d.send(Input::Key { key: Key::Enter });
    let live = |d: &Driver| {
        let asm = d.s.document().assembly();
        let def = asm.find_instance(g).expect("group").1.def;
        asm.def(def).expect("def").stack.is_some()
    };
    assert!(live(&d));
    // A flat wall: the round stays live, nothing is said.
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 2.0));
    d.hover(DVec3::new(1.0, 1.0, 2.5));
    d.typed("0.5");
    assert!(live(&d), "an ordinary face is edited underneath");
    let top =
        d.s.document()
            .assembly()
            .find_instance(g)
            .map(|(_, i)| i.def)
            .and_then(|def| {
                d.s.document()
                    .assembly()
                    .def(def)
                    .map(|x| x.model.vertices().map(|(_, p)| p.z).fold(0.0, f64::max))
            });
    assert_eq!(top, Some(2.5), "and it moved");
    assert!(!d.s.hint_is_notice());
    // An edge the round made: it is baked, and the hint says so.
    d.tool(ERASE);
    // The middle of a straight edge the round made.
    let mid = {
        let asm = d.s.document().assembly();
        let def = asm
            .def(asm.find_instance(g).expect("group").1.def)
            .expect("def");
        let base = &def.stack.as_ref().expect("live").base;
        def.model
            .edges()
            .filter(|(e, edge)| {
                base.edge(*e).is_none() && matches!(edge.curve, prism_geom::Curve::Line)
            })
            .map(|(_, e)| (def.model.vertex(e.a) + def.model.vertex(e.b)) / 2.0)
            .next()
            .expect("an edge the round made")
    };
    d.hover(mid);
    assert!(
        matches!(d.s.hover(), Some(prism_view::Hit::Edge(_))),
        "{:?} {:?}",
        mid,
        d.s.hover()
    );
    d.click(mid);
    assert!(!live(&d), "baked to edit its edge");
    assert!(d.s.hint_is_notice());
    assert!(d.s.hint().contains("baked"), "{}", d.s.hint());
    undo(&mut d);
    assert!(live(&d), "one undo step brings it back");
}

#[test]
fn a_round_on_loose_geometry_is_live_and_shown_in_the_inspector() {
    use prism_tools::session::FILLET;
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.5));
    d.typed("2");
    d.tool(FILLET);
    d.hover(DVec3::new(2.0, 0.0, 1.0));
    d.click(DVec3::new(2.0, 0.0, 1.0));
    d.send(Input::Key { key: Key::Enter });
    d.tool(SELECT);
    let radius = |d: &Driver| radii_of(d.s.model());
    assert_eq!(radius(&d).len(), 1, "rounded");
    // Nothing selected: the Features of the top level show.
    let titles: Vec<_> = d.s.inspector().sections.iter().map(|s| s.title).collect();
    assert_eq!(titles, ["Features"]);
    let before = radius(&d)[0];
    d.s.inspector_set("feature.0.size", "0.5").expect("changes");
    assert!((radius(&d)[0] - 0.5).abs() < 1e-9 && (before - 0.5).abs() > 1e-9);
    // Edit > Bake Features with nothing selected bakes them.
    act(&mut d, "edit.bake");
    assert!(d.s.document().assembly().root().stack.is_none());
    assert_eq!(radius(&d).len(), 1, "the round stays");
    let titles: Vec<_> = d.s.inspector().sections.iter().map(|s| s.title).collect();
    assert!(titles.is_empty(), "{titles:?}");
}

fn radii_of(m: &prism_topo::Model) -> Vec<f64> {
    m.faces()
        .filter_map(|(_, f)| match f.surface.clone() {
            prism_geom::Surface::Cylinder(c) => Some(c.radius),
            _ => None,
        })
        .collect()
}

/// A rounded box at the top level, the Select tool active. Returns where
/// the round's face is, to click.
fn rounded_box(d: &mut Driver) -> DVec3 {
    use prism_tools::session::FILLET;
    rect_on_ground(d, 0.0, 0.0, 2.0, 2.0);
    d.tool(PUSH);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.hover(DVec3::new(1.0, 1.0, 0.5));
    d.typed("2");
    d.tool(FILLET);
    d.hover(DVec3::new(2.0, 0.0, 1.0));
    d.click(DVec3::new(2.0, 0.0, 1.0));
    d.send(Input::Key { key: Key::Enter });
    d.tool(SELECT);
    round_centre(d)
}

/// The middle of the face the live round made.
fn round_centre(d: &Driver) -> DVec3 {
    let m = d.s.model();
    let base =
        d.s.document()
            .assembly()
            .root()
            .stack
            .as_ref()
            .expect("live")
            .base
            .clone();
    let (_, round) = m
        .faces()
        .find(|(f, _)| base.face(*f).is_none())
        .expect("the round's face");
    round
        .loops()
        .flatten()
        .map(|v| m.vertex(*v))
        .fold(DVec3::ZERO, |a, p| a + p)
        / round.loops().flatten().count() as f64
}

#[test]
fn clicking_a_round_selects_the_feature() {
    let mut d = Driver::empty();
    let at = rounded_box(&mut d);
    assert_eq!(d.s.selected_feature(), None);
    d.click(at);
    assert_eq!(
        d.s.selected_feature(),
        Some(0),
        "the click picked the feature"
    );
    assert_eq!(d.s.selected_feature_name(), Some("Fillet"));
    let i = d.s.inspector();
    assert_eq!(i.summary, "Fillet");
    let titles: Vec<_> = i.sections.iter().map(|s| s.title).collect();
    assert_eq!(titles, ["Feature"]);
    // Its size changes from here.
    d.s.inspector_set("feature.0.size", "0.5").expect("changes");
    assert!((radii_of(d.s.model())[0] - 0.5).abs() < 1e-9);
    // Alt + click on the round gets the plain face, not the feature.
    let at = round_centre(&d);
    d.send(Input::Held {
        ctrl: false,
        alt: true,
    });
    d.click(at);
    d.send(Input::Held {
        ctrl: false,
        alt: false,
    });
    // A round is one smooth surface, so its face selects as the same set.
    assert!(!d.s.selection().faces.is_empty());
    d.click(DVec3::new(0.0, 1.0, 1.0));
    assert_eq!(
        d.s.selected_feature(),
        None,
        "a flat wall is not the feature"
    );
}

#[test]
fn exploding_the_selected_feature_bakes_it_and_remove_takes_it_off() {
    let mut d = Driver::empty();
    let at = rounded_box(&mut d);
    d.click(at);
    assert_eq!(d.s.selected_feature(), Some(0));
    act(&mut d, "edit.explode_feature");
    assert!(d.s.document().assembly().root().stack.is_none());
    assert_eq!(
        radii_of(d.s.model()).len(),
        1,
        "the round stays as geometry"
    );
    undo(&mut d);
    assert!(d.s.document().assembly().root().stack.is_some());
    d.click(at);
    act(&mut d, "edit.remove_feature");
    assert!(d.s.document().assembly().root().stack.is_none());
    assert!(radii_of(d.s.model()).is_empty(), "the box is plain again");
}

#[test]
fn the_outliner_lists_live_features_and_selects_them() {
    let mut d = Driver::empty();
    rounded_box(&mut d);
    let top = d.s.top_features();
    assert_eq!(top.len(), 1);
    assert!(top[0].label.starts_with("Fillet "), "{}", top[0].label);
    assert!(!top[0].selected);
    d.s.select_feature_at(&[], 0);
    assert_eq!(d.s.selected_feature(), Some(0));
    assert!(d.s.top_features()[0].selected);
    // A group's features are on its row, and picking one opens the group.
    let mut d = Driver::empty();
    let g = grouped_box(&mut d);
    d.s.open_path_to(&[g]);
    {
        use prism_tools::session::FILLET;
        d.tool(FILLET);
        d.hover(DVec3::new(2.0, 0.0, 1.0));
        d.click(DVec3::new(2.0, 0.0, 1.0));
        d.send(Input::Key { key: Key::Enter });
    }
    d.tool(SELECT);
    d.s.open_path_to(&[]);
    let row =
        d.s.outline()
            .into_iter()
            .find(|r| r.path == [g])
            .expect("the group");
    assert_eq!(row.features.len(), 1);
    d.s.select_feature_at(&[g], 0);
    assert_eq!(d.s.selected_feature(), Some(0));
    assert_eq!(d.s.inspector().summary, "Fillet");
}

/// The squares in the overlay: the size handle is one.
fn squares(d: &Driver) -> Vec<DVec3> {
    d.s.overlay()
        .marks
        .iter()
        .filter(|m| m.shape == prism_view::overlay::Shape::Square)
        .map(|m| m.at)
        .collect()
}

#[test]
fn the_fillet_tool_shows_a_handle_on_the_selected_fillet_and_dragging_it_sets_the_size() {
    use prism_tools::session::FILLET;
    let mut d = Driver::empty();
    let at = rounded_box(&mut d);
    d.click(at);
    assert_eq!(d.s.selected_feature(), Some(0));
    let was = radii_of(d.s.model())[0];
    // Select shows no guide; the Fillet tool does.
    let none = squares(&d).len();
    d.tool(FILLET);
    assert_eq!(d.s.selected_feature(), Some(0), "still the feature");
    let marks = squares(&d);
    assert_eq!(marks.len(), none + 1, "a handle appears");
    let handle = *marks.last().unwrap();
    // It is across a wall from the edge, `was` away.
    let guide =
        d.s.document()
            .assembly()
            .root()
            .stack
            .as_ref()
            .unwrap()
            .size_guide(0)
            .expect("a guide");
    let mid = (guide.a + guide.b) * 0.5;
    assert!(((handle - mid).length() - was).abs() < 1e-9);
    let along = (handle - mid).normalize();
    // Drag it out to 0.6 and let go: one undo step.
    let log = d.s.document().log().count();
    let from = d.screen(handle);
    d.send(Input::Move {
        x: from.x,
        y: from.y,
    });
    d.send(Input::Press {
        button: Button::Left,
        x: from.x,
        y: from.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    let to = d.screen(mid + along * 0.6);
    d.send(Input::Move { x: to.x, y: to.y });
    assert_eq!(
        d.s.document().log().count(),
        log,
        "nothing is made until the button is let go"
    );
    d.send(Input::Release {
        button: Button::Left,
    });
    assert_eq!(d.s.document().log().count(), log + 1);
    assert!((radii_of(d.s.model())[0] - 0.6).abs() < 1e-3);
    assert_eq!(d.s.selected_feature(), Some(0), "still selected");
    act(&mut d, "edit.undo");
    assert!((radii_of(d.s.model())[0] - was).abs() < 1e-9);
}

#[test]
fn escape_while_dragging_the_handle_leaves_the_size_and_a_typed_size_sets_it() {
    use prism_tools::session::FILLET;
    let mut d = Driver::empty();
    let at = rounded_box(&mut d);
    d.click(at);
    let was = radii_of(d.s.model())[0];
    d.tool(FILLET);
    let handle = *squares(&d).last().expect("a handle");
    let from = d.screen(handle);
    d.send(Input::Press {
        button: Button::Left,
        x: from.x,
        y: from.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Move {
        x: from.x + 20.0,
        y: from.y,
    });
    d.send(Input::Key { key: Key::Escape });
    d.send(Input::Release {
        button: Button::Left,
    });
    assert!((radii_of(d.s.model())[0] - was).abs() < 1e-9);
    // A typed size is the feature's new size.
    d.typed("0.4");
    assert!((radii_of(d.s.model())[0] - 0.4).abs() < 1e-9);
    assert_eq!(d.s.selected_feature(), Some(0));
}

#[test]
fn a_subtract_by_the_tool_stays_live_in_the_inspector_until_baked() {
    use prism_tools::session::SOLID_TOOLS;
    let mut d = overlapping_groups();
    d.tool(SOLID_TOOLS[1]);
    d.click(DVec3::new(0.5, 0.0, 1.0));
    d.click(DVec3::new(2.5, 0.0, 1.0));
    d.tool(SELECT);
    d.click(DVec3::new(2.5, 0.0, 1.0));
    let i = d.s.inspector();
    let features = i
        .sections
        .iter()
        .find(|s| s.title == "Features")
        .expect("a Features section for the live subtract");
    let kind = features
        .fields
        .iter()
        .find(|f| f.key == "feature.0.kind")
        .expect("its row");
    assert_eq!(kind.text, "Subtract");
    assert!(matches!(
        kind.kind,
        prism_tools::session::FieldKind::Choice(_)
    ));
    // Bake makes it plain; the shape stays.
    act(&mut d, "edit.bake");
    let asm = d.s.document().assembly();
    assert!(
        asm.root()
            .instances
            .values()
            .all(|i| asm.def(i.def).unwrap().stack.is_none())
    );
    let (g, _) = asm
        .root()
        .instances
        .iter()
        .find(|(_, i)| i.transform.translation.x < 2.0)
        .unwrap();
    assert!((group_volume(&d, *g) - 4.0).abs() < 1e-9);
}

#[test]
fn the_cutter_of_a_live_subtract_is_moved_and_sized_in_the_inspector() {
    use prism_tools::session::SOLID_TOOLS;
    let mut d = overlapping_groups();
    d.tool(SOLID_TOOLS[1]);
    d.click(DVec3::new(0.5, 0.0, 1.0));
    d.click(DVec3::new(2.5, 0.0, 1.0));
    d.tool(SELECT);
    d.click(DVec3::new(2.5, 0.0, 1.0));
    let body = |d: &Driver| {
        *d.s.document()
            .assembly()
            .root()
            .instances
            .iter()
            .find(|(_, i)| i.transform.translation.x < 2.0)
            .unwrap()
            .0
    };
    let g = body(&d);
    assert!((group_volume(&d, g) - 4.0).abs() < 1e-9);
    let (width, _) = inspector_text(&d, "feature.0.cutter.w");
    let width: f64 = width.trim_end_matches(" m").parse().unwrap();
    assert!((width - 2.0).abs() < 1e-9, "the cutter is 2 m wide");
    // Shorter: it cuts half as deep.
    d.s.inspector_set("feature.0.cutter.w", "1.5")
        .expect("changes");
    assert_eq!(d.s.document().undo_name(), Some("Change Feature"));
    assert!((group_volume(&d, g) - 6.0).abs() < 1e-9);
    // Moved along x: it now cuts from 0.5 to 2.
    d.s.inspector_set("feature.0.cutter.x", "0.5")
        .expect("changes");
    assert!((group_volume(&d, g) - 2.0).abs() < 1e-9);
    // Nothing for a size of nothing, a name, or no change.
    assert!(d.s.inspector_set("feature.0.cutter.d", "0").is_err());
    assert!(d.s.inspector_set("feature.0.cutter.w", "wide").is_err());
    assert!(d.s.inspector_set("feature.0.cutter.x", "0.5").is_err());
    assert!(d.s.inspector_set("feature.0.cutter.q", "1").is_err());
    act(&mut d, "edit.undo");
    act(&mut d, "edit.undo");
    assert!((group_volume(&d, g) - 4.0).abs() < 1e-9);
}

#[test]
fn a_recording_says_what_it_is_at_the_top_and_old_ones_still_load() {
    let mut d = Driver::empty();
    d.s.start_recording();
    d.tool(RECT);
    let json = d.s.stop_recording().expect("recording");
    let first = json.lines().nth(1).expect("a first field");
    assert!(
        first
            .trim_start()
            .starts_with("\"about\": \"This is a recorded Prism session")
    );
    // A file saved before the note existed has none, and plays as before.
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    value.as_object_mut().unwrap().remove("about");
    let old: Recording = serde_json::from_value(value).expect("an old recording");
    assert_eq!(old.about, "");
    Session::replay(&old);
}

#[test]
fn a_live_boolean_changes_kind_and_shows_a_ghost_of_its_cutter_when_selected() {
    use prism_tools::session::SOLID_TOOLS;
    let mut d = overlapping_groups();
    d.tool(SOLID_TOOLS[1]);
    d.click(DVec3::new(0.5, 0.0, 1.0));
    d.click(DVec3::new(2.5, 0.0, 1.0));
    d.tool(SELECT);
    d.click(DVec3::new(2.5, 0.0, 1.0));
    let g =
        *d.s.document()
            .assembly()
            .root()
            .instances
            .iter()
            .find(|(_, i)| i.transform.translation.x < 2.0)
            .unwrap()
            .0;
    let dashed = |d: &Driver| {
        d.s.overlay()
            .lines
            .iter()
            .filter(|l| l.dash == [6.0, 4.0])
            .count()
    };
    assert_eq!(dashed(&d), 0, "no ghost until the feature is selected");
    // Open the body and select its cut: the cutter's box is drawn.
    d.s.open_path_to(&[g]);
    d.s.select_feature(0);
    assert_eq!(d.s.selected_feature(), Some(0));
    assert_eq!(dashed(&d), 12, "the cutter is a box");
    // Subtract becomes Intersect: what was cut away is what is left.
    assert!((group_volume(&d, g) - 4.0).abs() < 1e-9);
    d.s.inspector_set("feature.0.kind", "Intersect")
        .expect("changes");
    assert_eq!(d.s.document().undo_name(), Some("Change Feature"));
    assert!((group_volume(&d, g) - 4.0).abs() < 1e-9, "the shared part");
    assert_eq!(inspector_text(&d, "feature.0.kind").0, "Intersect");
    // Union with the cutter back in; the same kind again or a wrong word does nothing.
    d.s.inspector_set("feature.0.kind", "union")
        .expect("changes");
    assert!((group_volume(&d, g) - 12.0).abs() < 1e-9);
    assert!(d.s.inspector_set("feature.0.kind", "Union").is_err());
    assert!(d.s.inspector_set("feature.0.kind", "Merge").is_err());
    act(&mut d, "edit.undo");
    assert_eq!(inspector_text(&d, "feature.0.kind").0, "Intersect");
}

#[test]
fn a_feature_field_shows_its_result_while_typed_and_nothing_is_made() {
    use prism_tools::session::SOLID_TOOLS;
    let mut d = overlapping_groups();
    d.tool(SOLID_TOOLS[1]);
    d.click(DVec3::new(0.5, 0.0, 1.0));
    d.click(DVec3::new(2.5, 0.0, 1.0));
    d.tool(SELECT);
    d.click(DVec3::new(2.5, 0.0, 1.0));
    let g =
        *d.s.document()
            .assembly()
            .root()
            .instances
            .iter()
            .find(|(_, i)| i.transform.translation.x < 2.0)
            .unwrap()
            .0;
    d.s.open_path_to(&[g]);
    d.s.select_feature(0);
    // The area of what is drawn: the cut is 2 wide, then 1.5 wide.
    let area = |d: &Driver| -> f64 {
        d.s.display_scene()
            .faces
            .iter()
            .map(|f| {
                let shoelace = |p: &[prism_math::DVec2]| {
                    (0..p.len())
                        .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
                        .sum::<f64>()
                        .abs()
                        / 2.0
                };
                shoelace(&f.outer) - f.holes.iter().map(|h| shoelace(h)).sum::<f64>()
            })
            .sum()
    };
    let was = area(&d);
    let log = d.s.document().log().count();
    d.s.inspector_preview("feature.0.cutter.w", "1.5");
    assert!(
        (area(&d) - was).abs() > 1e-6,
        "the body is drawn with the shorter cut"
    );
    assert_eq!(d.s.document().log().count(), log, "nothing is made");
    // Text that is not a length, or a size of nothing: the body as it is.
    d.s.inspector_preview("feature.0.cutter.w", "wide");
    assert!((area(&d) - was).abs() < 1e-9);
    d.s.inspector_preview("feature.0.cutter.w", "1.5");
    d.s.inspector_preview("feature.0.cutter.w", "0");
    assert!((area(&d) - was).abs() < 1e-9);
    // Leaving the field takes the preview away.
    d.s.inspector_preview("feature.0.cutter.w", "1.5");
    d.s.inspector_preview_end();
    assert!((area(&d) - was).abs() < 1e-9);
    assert_eq!(d.s.document().log().count(), log);
}

#[test]
fn a_round_is_pointed_at_the_edges_selected_in_the_viewport() {
    let mut d = Driver::empty();
    rounded_box(&mut d);
    let refs = |d: &Driver| match &d
        .s
        .document()
        .assembly()
        .root()
        .stack
        .as_ref()
        .unwrap()
        .features[0]
    {
        prism_doc::Feature::Round { edges, .. } => edges.clone(),
        _ => panic!("a round"),
    };
    let first = refs(&d);
    assert!(
        first[0].a.x > 1.5 || first[0].b.x > 1.5,
        "the edge at x = 2"
    );
    // With nothing picked there is no such row.
    let has = |d: &Driver| {
        d.s.inspector()
            .sections
            .iter()
            .flat_map(|s| &s.fields)
            .any(|f| f.key == "feature.0.edges")
    };
    assert!(!has(&d));
    assert!(d.s.inspector_set("feature.0.edges", "").is_err());
    // Pick the edge at the near corner: the round moves to it, in one step.
    d.click(DVec3::new(0.0, 0.0, 1.0));
    assert_eq!(d.s.selection().edges.len(), 1);
    assert!(has(&d));
    let log = d.s.document().log().count();
    d.s.inspector_set("feature.0.edges", "").expect("changes");
    assert_eq!(d.s.document().log().count(), log + 1);
    let now = refs(&d);
    assert_eq!(now.len(), 1);
    assert!(now[0].a.x < 0.5 && now[0].a.y < 0.5, "{now:?}");
    assert_eq!(d.s.document().undo_name(), Some("Change Feature"));
    act(&mut d, "edit.undo");
    assert_eq!(refs(&d)[0].a, first[0].a);
}

#[test]
fn a_shell_stays_live_and_its_thickness_is_changed_in_the_inspector() {
    let mut d = Driver::empty();
    box_2m(&mut d);
    d.tool(SHELL);
    d.hover(DVec3::new(1.0, 1.0, 2.0));
    d.send(Input::Press {
        button: Button::Left,
        x: d.screen(DVec3::new(1.0, 1.0, 2.0)).x,
        y: d.screen(DVec3::new(1.0, 1.0, 2.0)).y,
        shift: true,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Release {
        button: Button::Left,
    });
    d.click(DVec3::new(1.0, 0.0, 1.0));
    d.hover(DVec3::new(1.0, 0.3, 1.2));
    d.typed("0.1");
    let tray = 8.0 - 1.8 * 1.8 * 1.9;
    assert!((model_volume(d.s.model()) - tray).abs() < 1e-6);
    // It stays a feature of the loose geometry, named in the Outliner list.
    let stack =
        d.s.document()
            .assembly()
            .root()
            .stack
            .clone()
            .expect("a stack");
    assert_eq!(stack.features.len(), 1);
    assert_eq!(stack.features[0].name(), "Shell");
    d.tool(SELECT);
    d.s.select_feature(0);
    assert_eq!(d.s.inspector().summary, "Shell");
    // A thicker wall: one undo step; the shell runs again from the box.
    d.s.inspector_set("feature.0.size", "-0.2")
        .expect("changes");
    assert_eq!(d.s.document().undo_name(), Some("Change Feature"));
    let thick = 8.0 - 1.6 * 1.6 * 1.8;
    assert!(
        (model_volume(d.s.model()) - thick).abs() < 1e-6,
        "{}",
        model_volume(d.s.model())
    );
    assert!(d.s.inspector_set("feature.0.size", "0").is_err());
    // Typed text shows the result before Enter, and makes nothing.
    let log = d.s.document().log().count();
    d.s.inspector_preview("feature.0.size", "-0.3");
    d.s.inspector_preview_end();
    assert_eq!(d.s.document().log().count(), log);
    act(&mut d, "edit.undo");
    assert!((model_volume(d.s.model()) - tray).abs() < 1e-6);
    // Bake makes it plain geometry; the shape stays.
    act(&mut d, "edit.bake");
    assert!(d.s.document().assembly().root().stack.is_none());
    assert!((model_volume(d.s.model()) - tray).abs() < 1e-6);
}

#[test]
fn a_live_shell_has_a_handle_that_drags_its_thickness() {
    let mut d = Driver::empty();
    box_2m(&mut d);
    d.tool(SHELL);
    d.hover(DVec3::new(1.0, 1.0, 2.0));
    d.send(Input::Press {
        button: Button::Left,
        x: d.screen(DVec3::new(1.0, 1.0, 2.0)).x,
        y: d.screen(DVec3::new(1.0, 1.0, 2.0)).y,
        shift: true,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Release {
        button: Button::Left,
    });
    d.click(DVec3::new(1.0, 0.0, 1.0));
    d.hover(DVec3::new(1.0, 0.3, 1.2));
    d.typed("0.1");
    let tray = 8.0 - 1.8 * 1.8 * 1.9;
    assert!((model_volume(d.s.model()) - tray).abs() < 1e-6);
    // No handle until the feature is selected.
    let none = squares(&d).len();
    d.s.select_feature(0);
    let marks = squares(&d);
    assert_eq!(marks.len(), none + 1, "a handle appears");
    let handle = *marks.last().unwrap();
    // It is on the wall: the middle of the face shelled, 0.1 in from it.
    assert!(
        (handle - DVec3::new(1.0, 0.1, 1.0)).length() < 1e-9,
        "{handle:?}"
    );
    let log = d.s.document().log().count();
    // Grab it and drag it in to 0.2: the body shows it, nothing is made yet.
    let from = d.screen(handle);
    d.send(Input::Move {
        x: from.x,
        y: from.y,
    });
    d.send(Input::Press {
        button: Button::Left,
        x: from.x,
        y: from.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    let to = d.screen(DVec3::new(1.0, 0.2, 1.0));
    d.send(Input::Move { x: to.x, y: to.y });
    assert_eq!(d.s.document().log().count(), log, "nothing until let go");
    let drawn = |s: &prism_view::Scene| -> f64 {
        s.faces
            .iter()
            .map(|f| {
                let p = &f.outer;
                (0..p.len())
                    .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
                    .sum::<f64>()
                    .abs()
                    / 2.0
            })
            .sum()
    };
    assert!(
        (drawn(d.s.display_scene()) - drawn(d.s.scene())).abs() > 1e-6,
        "the body is drawn with the new thickness"
    );
    d.send(Input::Release {
        button: Button::Left,
    });
    assert_eq!(d.s.document().log().count(), log + 1, "one step");
    assert_eq!(d.s.selected_feature(), Some(0), "still selected");
    let thick = 8.0 - 1.6 * 1.6 * 1.8;
    assert!(
        (model_volume(d.s.model()) - thick).abs() < 0.05,
        "{}",
        model_volume(d.s.model())
    );
    act(&mut d, "edit.undo");
    assert!((model_volume(d.s.model()) - tray).abs() < 1e-6);
    // Esc while dragging leaves it as it was.
    d.s.select_feature(0);
    let handle = *squares(&d).last().unwrap();
    let from = d.screen(handle);
    d.send(Input::Move {
        x: from.x,
        y: from.y,
    });
    d.send(Input::Press {
        button: Button::Left,
        x: from.x,
        y: from.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
    d.send(Input::Move {
        x: from.x + 15.0,
        y: from.y,
    });
    d.send(Input::Key { key: Key::Escape });
    d.send(Input::Release {
        button: Button::Left,
    });
    assert!((model_volume(d.s.model()) - tray).abs() < 1e-6);
}

// ---------- rim styles of a live shell (brief 49 part 3) ----------

/// Two flat quads, each a NURBS patch, side by side in the plane y = 0 (x 0
/// to 1 and 1 to 2, z 0 to 1), joined smoothly; both face -y. A sheet whose
/// rim is four straight boundaries.
fn rim_sheet() -> prism_topo::Model {
    use prism_geom::{NurbsSurface, Patch, Surface};
    use prism_topo::{Face, Model};
    let square = |x0: f64| {
        let k = vec![0.0, 0.0, 1.0, 1.0];
        NurbsSurface::new(
            [1, 1],
            [k.clone(), k],
            [2, 2],
            vec![
                DVec3::new(x0, 0.0, 0.0),
                DVec3::new(x0, 0.0, 1.0),
                DVec3::new(x0 + 1.0, 0.0, 0.0),
                DVec3::new(x0 + 1.0, 0.0, 1.0),
            ],
            vec![1.0; 4],
            [false, false],
        )
        .unwrap()
    };
    let mut m = Model::new();
    let v = |m: &mut Model, x: f64, z: f64| m.add_vertex(DVec3::new(x, 0.0, z));
    let (a0, a1, a2) = (
        v(&mut m, 0.0, 0.0),
        v(&mut m, 1.0, 0.0),
        v(&mut m, 2.0, 0.0),
    );
    let (b0, b1, b2) = (
        v(&mut m, 0.0, 1.0),
        v(&mut m, 1.0, 1.0),
        v(&mut m, 2.0, 1.0),
    );
    let (bottom_a, bottom_b) = (m.add_edge(a0, a1).unwrap(), m.add_edge(a1, a2).unwrap());
    let (right_b, top_b) = (m.add_edge(a2, b2).unwrap(), m.add_edge(b2, b1).unwrap());
    let (top_a, left_a) = (m.add_edge(b1, b0).unwrap(), m.add_edge(b0, a0).unwrap());
    let shared = m.add_edge(a1, b1).unwrap();
    m.add_face(Face::with_edges(
        vec![a0, a1, b1, b0],
        vec![],
        Surface::Nurbs(Patch::whole(square(0.0))),
        vec![vec![bottom_a, shared, top_a, left_a]],
    ));
    m.add_face(Face::with_edges(
        vec![a1, a2, b2, b1],
        vec![],
        Surface::Nurbs(Patch::whole(square(1.0))),
        vec![vec![bottom_b, right_b, top_b, shared]],
    ));
    m
}

/// The sheet shelled 0.1 as a live shell, selected with the Shell tool.
fn rim_driver() -> Driver {
    rim_driver_in(Document::with_model(rim_sheet()))
}

fn rim_driver_in(mut doc: Document) -> Driver {
    let all: std::collections::BTreeSet<prism_topo::FaceId> =
        doc.model().faces().map(|(id, _)| id).collect();
    assert!(doc.run(prism_doc::Command::Shell {
        faces: all,
        open: std::collections::BTreeSet::new(),
        thickness: 0.1,
    }));
    let mut d = Driver::new(doc);
    d.tool(SHELL);
    d.s.select_feature(0);
    d
}

fn diamonds(d: &Driver) -> Vec<(DVec3, bool)> {
    d.s.overlay()
        .marks
        .iter()
        .filter_map(|m| match m.shape {
            prism_view::overlay::Shape::Diamond => Some((m.at, true)),
            prism_view::overlay::Shape::DiamondOutline => Some((m.at, false)),
            _ => None,
        })
        .collect()
}

fn rim_press(d: &mut Driver, p: DVec3) {
    let s = d.screen(p);
    d.send(Input::Move { x: s.x, y: s.y });
    d.send(Input::Press {
        button: Button::Left,
        x: s.x,
        y: s.y,
        shift: false,
        ctrl: false,
        alt: false,
    });
}

fn let_go(d: &mut Driver) {
    d.send(Input::Release {
        button: Button::Left,
    });
}

#[test]
fn a_live_shell_has_a_handle_on_each_open_boundary() {
    let d = rim_driver();
    let marks = diamonds(&d);
    assert_eq!(marks.len(), 4, "{marks:?}");
    for want in [
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(2.0, 0.0, 0.5),
        DVec3::new(1.0, 0.0, 1.0),
        DVec3::new(0.0, 0.0, 0.5),
    ] {
        assert!(
            marks
                .iter()
                .any(|(p, flush)| !flush && p.distance(want) < 1e-9),
            "a square handle at {want}: {marks:?}"
        );
    }
}

#[test]
fn dragging_a_rim_handle_tilts_the_cut_and_one_step_sets_it() {
    let mut d = rim_driver();
    let whole = model_volume(d.s.model()).abs();
    assert!((whole - 0.2).abs() < 1e-9, "{whole}");
    let log = d.s.document().log().count();
    // Lean the wall 0.5 up the sheet for each unit of thickness.
    rim_press(&mut d, DVec3::new(1.0, 0.0, 0.0));
    let to = d.screen(DVec3::new(1.0, -1.0, 0.5));
    d.send(Input::Move { x: to.x, y: to.y });
    assert_eq!(d.s.document().log().count(), log, "nothing until let go");
    assert!(d.s.hint().contains("Tilting"), "{}", d.s.hint());
    let_go(&mut d);
    assert_eq!(d.s.document().log().count(), log + 1, "one step");
    let trimmed = 2.0 * (0.1 - 0.5 * 0.1 * 0.1 * 0.5);
    assert!(
        (model_volume(d.s.model()).abs() - trimmed).abs() < 1e-9,
        "{}",
        model_volume(d.s.model())
    );
    assert_eq!(d.s.selected_feature(), Some(0), "still selected");
    // The boundary is drawn flush now, with a filled handle.
    assert!(
        diamonds(&d)
            .iter()
            .any(|(p, flush)| *flush && p.distance(DVec3::new(1.0, 0.0, 0.0)) < 1e-9)
    );
    // One undo step puts the whole wall back.
    act(&mut d, "edit.undo");
    assert!((model_volume(d.s.model()).abs() - whole).abs() < 1e-9);
}

#[test]
fn the_tilt_snaps_to_a_guide_line_and_to_a_part_beside_it() {
    // Both give the plane that leans 0.6 up the sheet for each unit of
    // thickness, through the bottom edge: by hand, the wall trims a
    // triangle of legs d and 0.6 d from each of the two bottom edges.
    let trimmed = 2.0 * (0.1 - 0.5 * 0.1 * 0.1 * 0.6);
    let lean = DVec3::new(1.0, -1.0, 0.6);
    // A guide line through the edge's end that lies in that plane.
    let mut doc = Document::with_model(rim_sheet());
    doc.run(prism_doc::Command::AddGuide {
        guide: prism_topo::Guide::Line {
            origin: DVec3::ZERO,
            dir: DVec3::new(0.0, -1.0, 0.6).normalize(),
        },
    });
    let mut d = rim_driver_in(doc);
    rim_press(&mut d, DVec3::new(1.0, 0.0, 0.0));
    let to = d.screen(lean);
    d.send(Input::Move { x: to.x, y: to.y });
    assert!(d.s.hint().contains("guide line"), "{}", d.s.hint());
    let_go(&mut d);
    assert!((model_volume(d.s.model()).abs() - trimmed).abs() < 1e-9);
    // A box grouped and turned about the x axis, so one of its faces is
    // that plane: another part meeting the edge.
    let mut doc = Document::with_model(rim_sheet());
    let had: std::collections::BTreeSet<_> = doc.model().vertices().map(|(i, _)| i).collect();
    doc.run(prism_doc::Command::Rect {
        x0: 0.0,
        y0: 10.0,
        x1: 2.0,
        y1: 12.0,
        z: 0.0,
    });
    let f = prism_ops::face_at(doc.model(), DVec3::new(1.0, 11.0, 0.0)).expect("base");
    let up = doc.model().face(f).unwrap().plane().unwrap().normal.z > 0.0;
    doc.run(prism_doc::Command::PushPull {
        face: f,
        distance: if up { 1.0 } else { -1.0 },
        new_loop: false,
    });
    let m = doc.model();
    let selection = prism_ops::Selection {
        vertices: m
            .vertices()
            .map(|(i, _)| i)
            .filter(|i| !had.contains(i))
            .collect(),
        edges: m
            .edges()
            .filter(|(_, e)| !had.contains(&e.a))
            .map(|(i, _)| i)
            .collect(),
        faces: m
            .faces()
            .filter(|(_, f)| f.loops().flatten().all(|v| !had.contains(v)))
            .map(|(i, _)| i)
            .collect(),
        guides: Default::default(),
    };
    let g = doc.assembly().next_instance();
    assert!(doc.run(prism_doc::Command::MakeGroup {
        selection,
        instances: Default::default(),
    }));
    assert!(doc.run(prism_doc::Command::Place {
        instances: [g].into(),
        change: prism_doc::Change::Rotate {
            centre: DVec3::ZERO,
            axis: DVec3::X,
            angle: -(0.6f64).atan(),
        },
    }));
    let mut d = rim_driver_in(doc);
    rim_press(&mut d, DVec3::new(1.0, 0.0, 0.0));
    let to = d.screen(lean);
    d.send(Input::Move { x: to.x, y: to.y });
    assert!(d.s.hint().contains("another part"), "{}", d.s.hint());
    let_go(&mut d);
    assert!(
        (model_volume(d.s.model()).abs() - trimmed).abs() < 1e-9,
        "{} vs {trimmed}",
        model_volume(d.s.model())
    );
}

#[test]
fn the_tilt_snaps_to_a_plane_through_the_boundary_when_close() {
    let mut d = rim_driver();
    // Nearly along the ground plane (z = 0), which holds the bottom edge.
    rim_press(&mut d, DVec3::new(1.0, 0.0, 0.0));
    let to = d.screen(DVec3::new(1.0, -1.0, 0.03));
    d.send(Input::Move { x: to.x, y: to.y });
    assert!(d.s.hint().contains("axis plane"), "{}", d.s.hint());
    let_go(&mut d);
    // Snapped square to the sheet: the wall is the whole thickness.
    assert!((model_volume(d.s.model()).abs() - 0.2).abs() < 1e-9);
}

#[test]
fn a_tilt_the_kernel_cannot_build_says_why_and_makes_nothing() {
    let mut d = rim_driver();
    let log = d.s.document().log().count();
    // A cut that leans so far away that the surface cannot be carried on
    // to meet it.
    rim_press(&mut d, DVec3::new(1.0, 0.0, 0.0));
    let to = d.screen(DVec3::new(1.0, -1.0, -8.0));
    d.send(Input::Move { x: to.x, y: to.y });
    assert_eq!(d.s.hint(), prism_ops::shelling::FLUSH_REACH);
    let_go(&mut d);
    assert_eq!(d.s.document().log().count(), log, "nothing was made");
    // Esc in the middle of a drag leaves it as it was, too.
    rim_press(&mut d, DVec3::new(1.0, 0.0, 0.0));
    let to = d.screen(DVec3::new(1.0, -1.0, 0.5));
    d.send(Input::Move { x: to.x, y: to.y });
    key(&mut d, Key::Escape);
    let_go(&mut d);
    assert_eq!(d.s.document().log().count(), log);
    assert!((model_volume(d.s.model()).abs() - 0.2).abs() < 1e-9);
}

#[test]
fn clicking_a_rim_handle_switches_it_between_flush_and_square() {
    let mut d = rim_driver();
    let log = d.s.document().log().count();
    // Square to Flush: a plane through the bottom edge that builds (the
    // ground plane; the sheet's own plane would lie along the face).
    rim_press(&mut d, DVec3::new(1.0, 0.0, 0.0));
    let_go(&mut d);
    assert_eq!(d.s.document().log().count(), log + 1);
    let stack = d.s.document().assembly().root().stack.clone().unwrap();
    let prism_doc::Feature::Shell { rims, .. } = &stack.features[0] else {
        panic!("a shell");
    };
    assert_eq!(rims.len(), 1);
    assert!(matches!(
        rims[0].style,
        prism_ops::shelling::RimStyle::Flush(_)
    ));
    // Flush to Square.
    rim_press(&mut d, DVec3::new(1.0, 0.0, 0.0));
    let_go(&mut d);
    let stack = d.s.document().assembly().root().stack.clone().unwrap();
    let prism_doc::Feature::Shell { rims, .. } = &stack.features[0] else {
        panic!("a shell");
    };
    assert_eq!(rims[0].style, prism_ops::shelling::RimStyle::Square);
}

fn inspector_has(d: &Driver, key: &str) -> bool {
    d.s.inspector()
        .sections
        .iter()
        .flat_map(|s| &s.fields)
        .any(|f| f.key == key)
}

#[test]
fn the_inspector_lists_each_rim_and_sets_how_it_ends() {
    let mut d = rim_driver();
    for n in 0..4 {
        let key = format!("feature.0.rim.{n}");
        assert_eq!(inspector_text(&d, &key).0, "Square", "{key}");
        assert!(!inspector_has(&d, &format!("{key}.tilt")), "no tilt yet");
    }
    assert!(!inspector_has(&d, "feature.0.rim.4"), "four rims only");
    let log = d.s.document().log().count();
    d.s.inspector_set("feature.0.rim.0", "Flush")
        .expect("flush");
    assert_eq!(d.s.document().log().count(), log + 1, "one undo step");
    assert_eq!(inspector_text(&d, "feature.0.rim.0").0, "Flush");
    assert!(
        inspector_has(&d, "feature.0.rim.0.tilt"),
        "a straight flush rim has a tilt"
    );
    let label =
        d.s.inspector()
            .sections
            .iter()
            .flat_map(|s| &s.fields)
            .find(|f| f.key == "feature.0.rim.0")
            .map(|f| f.label.clone())
            .unwrap();
    assert!(label.contains("Rim 1"), "{label}");
    // Back to square, and undo and redo move between them.
    d.s.inspector_set("feature.0.rim.0", "square")
        .expect("square");
    assert_eq!(inspector_text(&d, "feature.0.rim.0").0, "Square");
    assert!((model_volume(d.s.model()).abs() - 0.2).abs() < 1e-9);
    act(&mut d, "edit.undo");
    assert_eq!(inspector_text(&d, "feature.0.rim.0").0, "Flush");
    // Words that are not a style, and rims that are not there, are refused.
    assert!(d.s.inspector_set("feature.0.rim.0", "Round").is_err());
    assert!(d.s.inspector_set("feature.0.rim.9", "Flush").is_err());
    assert!(d.s.inspector_set("feature.0.rim.x", "Flush").is_err());
    assert!(d.s.inspector_set("feature.0.rim.0.slide", "1").is_err());
}

#[test]
fn a_rims_tilt_is_typed_in_degrees_and_the_kernels_refusals_say_why() {
    let mut d = rim_driver();
    d.s.inspector_set("feature.0.rim.0", "Flush")
        .expect("flush");
    let whole = model_volume(d.s.model()).abs();
    // One way leans into the sheet (the wall is trimmed back), the other
    // away from it (the surface is carried on past its edge): both build.
    let mut volumes = Vec::new();
    for t in ["30", "-30"] {
        d.s.inspector_set("feature.0.rim.0.tilt", t).expect(t);
        let shown: f64 = inspector_text(&d, "feature.0.rim.0.tilt")
            .0
            .parse()
            .unwrap();
        assert!((shown - t.parse::<f64>().unwrap()).abs() < 0.06, "{shown}");
        volumes.push(model_volume(d.s.model()).abs());
    }
    volumes.sort_by(f64::total_cmp);
    assert!(
        volumes[0] < whole - 1e-6 && volumes[1] > whole + 1e-6,
        "{volumes:?}"
    );
    // A plane lying along the sheet is refused in the kernel's words.
    let before = model_volume(d.s.model()).abs();
    let why = d.s.inspector_set("feature.0.rim.0.tilt", "0").unwrap_err();
    assert!(why.contains("along"), "{why}");
    assert!((model_volume(d.s.model()).abs() - before).abs() < 1e-12);
    // A degree sign is allowed; words are not.
    d.s.inspector_set("feature.0.rim.0.tilt", "20°")
        .expect("a degree sign is allowed");
    assert!(d.s.inspector_set("feature.0.rim.0.tilt", "steep").is_err());
    assert!(d.s.inspector_set("feature.0.rim.0.tilt", "nan").is_err());
    // Typing shows the shell as it would be, and stops when the typing does.
    d.s.inspector_preview("feature.0.rim.0.tilt", "89");
    d.s.inspector_preview_end();
    assert!(model_volume(d.s.model()).abs() < whole - 1e-6);
}

#[test]
fn sizing_a_shell_of_joined_freeform_faces_shows_its_ghost() {
    let mut d = Driver::new(Document::with_model(rim_sheet()));
    d.tool(SHELL);
    d.click(DVec3::new(0.5, 0.0, 0.5));
    d.hover(DVec3::new(0.5, -0.1, 0.5));
    let drawn = d.s.overlay().lines.len();
    assert!(drawn > 8, "the ghost's edges ({drawn})");
    d.typed("0.1");
    assert_eq!(
        d.s.document()
            .assembly()
            .root()
            .stack
            .as_ref()
            .map(|s| s.features.len()),
        Some(1)
    );
}

#[test]
fn ctrl_before_the_first_click_makes_the_next_move_a_copy() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 1.0, 1.0);
    d.tool(SELECT);
    d.click(DVec3::new(0.5, 0.5, 0.0));
    d.tool(MOVE);
    d.send(Input::CtrlTap);
    assert!(d.s.hint().contains("COPY on"), "{}", d.s.hint());
    d.click(DVec3::ZERO);
    d.hover(DVec3::new(1.0, 0.1, 0.0));
    key(&mut d, Key::Right);
    d.typed("3");
    assert_eq!(d.faces(), 2, "a copy, the original stays");
    assert!(has_point(&d, 0.0, 0.0) && has_point(&d, 3.0, 0.0));
    // A copy is for one move: the next move carries the original.
    assert!(!d.s.hint().contains("COPY on"));
    d.click(DVec3::ZERO);
    d.hover(DVec3::new(1.0, 0.1, 0.0));
    key(&mut d, Key::Right);
    d.typed("1");
    assert_eq!(d.faces(), 2, "moved, not copied");
    // Tapping twice turns it off again.
    d.send(Input::CtrlTap);
    d.send(Input::CtrlTap);
    assert!(!d.s.hint().contains("COPY on"));
    valid(&d);
}

#[test]
fn ctrl_d_repeats_a_move_copy_in_a_chain() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 1.0, 1.0);
    d.tool(SELECT);
    d.click(DVec3::new(0.5, 0.5, 0.0));
    d.tool(MOVE);
    d.click(DVec3::ZERO);
    d.send(Input::CtrlTap);
    d.hover(DVec3::new(1.0, 0.1, 0.0));
    key(&mut d, Key::Right);
    d.typed("3");
    assert_eq!(d.faces(), 2);
    act(&mut d, "edit.duplicate");
    assert_eq!(d.faces(), 3);
    assert!(has_point(&d, 6.0, 0.0), "the same step again");
    act(&mut d, "edit.duplicate");
    act(&mut d, "edit.duplicate");
    assert_eq!(d.faces(), 5);
    assert!(has_point(&d, 9.0, 0.0) && has_point(&d, 12.0, 0.0));
    // Each press is one undo step.
    act(&mut d, "edit.undo");
    assert_eq!(d.faces(), 4);
    valid(&d);
}

#[test]
fn ctrl_d_continues_after_a_typed_array() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 1.0, 1.0);
    d.tool(SELECT);
    d.click(DVec3::new(0.5, 0.5, 0.0));
    d.tool(MOVE);
    d.click(DVec3::ZERO);
    d.send(Input::CtrlTap);
    d.hover(DVec3::new(1.0, 0.1, 0.0));
    key(&mut d, Key::Right);
    d.typed("3");
    d.typed("x3");
    assert_eq!(d.faces(), 4);
    act(&mut d, "edit.duplicate");
    assert_eq!(d.faces(), 5);
    assert!(has_point(&d, 12.0, 0.0), "the fifth step on");
}

#[test]
fn ctrl_d_repeats_a_rotate_copy_about_the_same_centre() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 1.0, 0.0, 2.0, 1.0);
    d.tool(SELECT);
    d.click(DVec3::new(1.5, 0.5, 0.0));
    d.tool(ROTATE);
    d.send(Input::CtrlTap);
    d.click(DVec3::ZERO);
    d.click(DVec3::new(2.0, 0.0, 0.0));
    d.hover(DVec3::new(0.0, 2.0, 0.0));
    d.typed("90");
    assert_eq!(d.faces(), 2);
    assert!(has_point(&d, -1.0, 2.0), "a quarter turn");
    act(&mut d, "edit.duplicate");
    assert_eq!(d.faces(), 3);
    assert!(has_point(&d, -2.0, -1.0), "a half turn");
    act(&mut d, "edit.duplicate");
    assert!(has_point(&d, 1.0, -2.0), "three quarters");
    assert_eq!(d.faces(), 4);
    valid(&d);
}

#[test]
fn ctrl_d_makes_copies_of_groups_too() {
    let mut d = Driver::empty();
    let a = grouped_box(&mut d);
    d.tool(MOVE);
    d.click(DVec3::ZERO);
    d.send(Input::CtrlTap);
    d.hover(DVec3::new(1.0, 0.0, 0.0));
    key(&mut d, Key::Right);
    d.typed("4");
    assert_eq!(d.s.document().assembly().root().instances.len(), 2);
    act(&mut d, "edit.duplicate");
    act(&mut d, "edit.duplicate");
    let asm = d.s.document().assembly();
    assert_eq!(asm.root().instances.len(), 4, "three copies in a row");
    let xs: Vec<f64> = asm
        .root()
        .instances
        .values()
        .map(|i| i.transform.translation.x)
        .collect();
    for want in [0.0, 4.0, 8.0, 12.0] {
        assert!(xs.iter().any(|x| (x - want).abs() < 1e-9), "{xs:?}");
    }
    let _ = a;
}

#[test]
fn ctrl_d_with_nothing_to_repeat_says_so() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 1.0, 1.0);
    d.tool(SELECT);
    d.click(DVec3::new(0.5, 0.5, 0.0));
    let n = d.s.document().log().count();
    act(&mut d, "edit.duplicate");
    assert_eq!(d.s.document().log().count(), n);
    assert!(d.s.hint().contains("Ctrl+D"), "{}", d.s.hint());
}

#[test]
fn ctrl_d_after_selecting_something_else_starts_a_new_chain() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 1.0, 1.0);
    rect_on_ground(&mut d, 0.0, 5.0, 1.0, 6.0);
    d.tool(SELECT);
    d.click(DVec3::new(0.5, 0.5, 0.0));
    d.tool(MOVE);
    d.click(DVec3::ZERO);
    d.send(Input::CtrlTap);
    d.hover(DVec3::new(1.0, 0.1, 0.0));
    key(&mut d, Key::Right);
    d.typed("3");
    d.tool(SELECT);
    act(&mut d, "edit.desel");
    d.click(DVec3::new(0.5, 5.5, 0.0));
    act(&mut d, "edit.duplicate");
    assert!(has_point(&d, 3.0, 5.0), "one step from the other square");
    assert!(!has_point(&d, 6.0, 5.0));
}
