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
