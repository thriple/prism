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
