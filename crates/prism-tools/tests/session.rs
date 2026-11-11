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
            alt_loop: false,
            brush: Brush::default(),
            brush_copy: None,
            materials: Vec::new(),
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
    assert!(d.s.inference().is_none(), "no inference while scaling");
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
fn a_click_on_an_arc_selects_its_whole_circle() {
    let mut d = circle_in_a_rectangle();
    assert_eq!(d.faces(), 4);
    d.tool(SELECT);
    d.click(DVec3::new(4.0, 2.0, 0.0));
    let sel = d.s.selection().clone();
    assert_eq!(sel.edges.len(), 2, "both halves of the circle");
    for e in &sel.edges {
        assert!(d.s.model().edge(*e).unwrap().curve.is_arc());
    }
}

#[test]
fn the_eraser_takes_the_whole_circle_and_the_faces_merge() {
    let mut d = circle_in_a_rectangle();
    d.tool(ERASE);
    d.click(DVec3::new(4.0, 2.0, 0.0));
    assert_eq!(d.faces(), 2, "the line still splits the rectangle");
    assert!(d.s.model().edges().all(|(_, e)| !e.curve.is_arc()));
    same(&areas(&d), &[12.0, 12.0]);
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
            prism_geom::Curve::Line => None,
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
            .filter_map(|(_, f)| match f.surface {
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
        .filter_map(|(_, f)| match f.surface {
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
    assert!(d.s.hint().contains("Ctrl cuts it off"), "{}", d.s.hint());
    // An upright corner edge, at its middle: rounded with a tenth of the
    // face's width.
    d.click(DVec3::new(2.0, 0.0, 1.0));
    assert_eq!(d.faces(), faces + 1);
    assert_eq!(radii(&d).len(), 1);
    assert!((radii(&d)[0] - 0.2).abs() < 1e-9, "{:?}", radii(&d));
    valid(&d);
    // A typed size changes it.
    d.typed("0.5");
    assert!((radii(&d)[0] - 0.5).abs() < 1e-9, "{:?}", radii(&d));
    assert_eq!(d.faces(), faces + 1);
    valid(&d);
    // Ctrl cuts another edge off flat, with the same size.
    let s = d.screen(DVec3::new(0.0, 0.0, 1.0));
    d.click_px(s, false, true);
    assert_eq!(d.faces(), faces + 2);
    assert_eq!(radii(&d).len(), 1, "flat");
    valid(&d);
    // Too big for the faces: refused, and the hint says why.
    d.typed("5");
    assert_eq!(d.s.hint(), prism_ops::round::TOO_WIDE);
    undo(&mut d);
    undo(&mut d);
    assert_eq!(d.faces(), faces, "one undo each");
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
    d.hover(DVec3::new(4.0, 0.0, 0.0));
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
    d.hover(DVec3::new(4.0, 0.0, 0.0));
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
    d.hover(DVec3::new(4.0, 0.0, 0.0));
    d.click(DVec3::new(4.0, 0.0, 0.0));
    let v = volume(&d);
    assert!((v - 20.0).abs() < 1e-6, "{v}");
}
