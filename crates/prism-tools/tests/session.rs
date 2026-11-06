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
            model: self.s.model().clone(),
            camera: c,
            tool: self.s.tool().to_owned(),
            size: (1000.0, 700.0),
            settings: self.s.settings,
            events: vec![],
            result: String::new(),
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
            assert_eq!(
                Session::replay(&r).model().geometry_json(),
                r.result,
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
    assert_eq!(d.faces(), 5, "four copies between");
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

#[test]
fn scale_by_a_typed_factor() {
    let mut d = Driver::empty();
    rect_on_ground(&mut d, 0.0, 0.0, 2.0, 2.0);
    d.tool(SELECT);
    d.click(DVec3::new(1.0, 1.0, 0.0));
    d.tool(SCALE);
    d.click(DVec3::new(2.0, 2.0, 0.0));
    d.typed("1.5");
    assert!(
        prism_ops::vertex_at(d.s.model(), DVec3::new(2.5, 2.5, 0.0)).is_some(),
        "scaled about the centre"
    );
    assert!(prism_ops::vertex_at(d.s.model(), DVec3::new(-0.5, -0.5, 0.0)).is_some());
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
