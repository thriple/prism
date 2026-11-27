//! What the tools say about curved faces (Exact Curves 4a, 4d): the reasons
//! edits are refused come from `Command::refusal`.

use prism_doc::Command;
use prism_math::DVec3;

#[test]
fn edits_of_a_cylinder_say_why_they_are_refused() {
    let m = prism_tools::scenes::cylinder(1.0, 2.0);
    let side = m
        .faces()
        .find(|(_, f)| f.is_curved())
        .map(|(id, _)| id)
        .expect("a side");
    // One point of the top: the whole top moves since 2026-11-25 (the side
    // follows, `bands.rs`), but one point of it would bend its arcs.
    let top = m
        .vertices()
        .filter(|(_, p)| p.z > 1.0)
        .map(|(id, _)| id)
        .take(1)
        .collect();
    let cases = [
        (
            Command::Offset {
                face: side,
                distance: 0.1,
            },
            prism_ops::offset::CURVED_OFFSET,
        ),
        (
            Command::PushPull {
                face: side,
                distance: -1.0,
                new_loop: false,
            },
            prism_ops::radius::RADIUS_GONE,
        ),
        (
            Command::Translate {
                vertices: top,
                by: DVec3::Z,
            },
            prism_ops::transform::ARC_WOULD_BEND,
        ),
    ];
    for (c, why) in cases {
        assert_eq!(c.refusal(&m), Some(why), "{}", c.name());
        let mut after = m.clone();
        c.apply(&mut after);
        assert_eq!(after.to_json(), m.to_json(), "{} changes nothing", c.name());
    }
}

/// The owner's recorded test (2026-11-14): a face lofted between four
/// curves, then the flat back face beside it pulled out, once plainly and
/// twice with Alt's new loop. Every pull was refused ("would bend a curved
/// face"); now each one adds faces and the model stays valid. The face a
/// new loop leaves behind crosses the loft, so the two cut each other
/// (2026-11-25; before, Model Health found them crossing).
#[test]
fn the_back_of_a_lofted_face_pulls_out_in_the_recorded_session() {
    let r: prism_tools::Recording =
        serde_json::from_str(include_str!("../data/pull-beside-loft.json")).expect("recording");
    let s = prism_tools::Session::replay(&r);
    let m = s.model();
    assert_eq!(m.face_count(), 21, "three pulls, each adding faces");
    assert_eq!(prism_ops::check::problems(m), Vec::<String>::new());
    let h = prism_ops::health::check(m);
    assert!(
        h.crossing.faces.is_empty(),
        "crossing: {:?}",
        h.crossing.faces
    );
}

/// The owner's session 9 (2026-11-17): closing a loft's end with Patch.
/// The ring's bottom was two lines end to end, one side was a spline with
/// a piece erased, and that spline left the bottom almost straight on (no
/// real corner). Patch did nothing; now it fills the three-sided ring, with
/// no fold (the first try, as four sides, had a lip by that joint).
#[test]
fn patch_closes_the_lofts_end_in_the_recorded_session() {
    let r: prism_tools::Recording =
        serde_json::from_str(include_str!("../data/patch-closing-loft.json")).expect("recording");
    let before: std::collections::BTreeSet<_> = r.model.faces().map(|(id, _)| id).collect();
    let s = prism_tools::Session::replay(&r);
    let m = s.model();
    assert_eq!(prism_ops::check::problems(m), Vec::<String>::new());
    let new: Vec<_> = m.faces().filter(|(id, _)| !before.contains(id)).collect();
    assert!(new.len() >= 3, "a three-sided fill: {}", new.len());
    // All of it faces one way (the end is nearly flat, across x).
    let mut side = 0.0;
    for (_, f) in &new {
        let p = f.surface.patch().expect("freeform");
        for i in 0..=8 {
            for j in 0..=8 {
                let u = p.u.0 + (p.u.1 - p.u.0) * f64::from(i) / 8.0;
                let v = p.v.0 + (p.v.1 - p.v.0) * f64::from(j) / 8.0;
                let Some(n) = p.surface.normal(u, v).try_normalize() else {
                    continue;
                };
                let n = if p.flipped { -n } else { n };
                assert!(n.x.abs() > 0.8, "bent: {n} at {u}, {v}");
                if side == 0.0 {
                    side = n.x.signum();
                }
                assert_eq!(n.x.signum(), side, "folded at {u}, {v}");
            }
        }
    }
}

/// The owner's session 17 Shell clicks (2026-11-26). The first click
/// (0.25 m by the pointer) makes the shell, cleanly. The next (the
/// window's 1 m outside) lands on the shelled wall itself: too thick to
/// shell as one piece, it is built face by face, every one of its 62
/// faces joined (owner: no limit on the pieces), and so it is made a
/// face at a time, a step each frame, with how far it is in the box and
/// Esc to stop it (DECISIONS #41, #48). Made to the end it takes about
/// 30 s in a release build: `the_three_recorded_shell_clicks_made_to_the_end`
/// runs it, by hand.
#[test]
fn shell_is_made_at_the_first_click_of_the_recorded_session() {
    let mut r: prism_tools::Recording =
        serde_json::from_str(include_str!("../data/owner-session-17-shell.json"))
            .expect("recording");
    // Step it as the app does (the recording predates jobs).
    r.jobs = true;
    let upto = |n: usize| {
        let mut r = r.clone();
        r.events.truncate(n);
        prism_tools::Session::replay(&r)
    };
    // Just before the first press, the ghost of the shell builds.
    let before = upto(367);
    assert_eq!(before.shell_panel().expect("the Shell tool").refused, None);
    // The first press makes the shell, cleanly, at once.
    let first = upto(368);
    assert_eq!(first.version(), before.version() + 1);
    assert_eq!(
        prism_ops::check::problems(first.work_model()),
        Vec::<String>::new()
    );
    // The next starts making the shell of the shell, a face at a time.
    let mut second = upto(591);
    assert_eq!(second.hint(), prism_tools::session::JOB_RUNNING);
    let (label, value, _) = second.measurement();
    assert_eq!(label, "Shelling");
    assert!(value.starts_with("0 of 62"), "{value}");
    let t = r.events[590].t;
    for k in 1..=3 {
        second.handle(&prism_tools::Event {
            t: t + 0.02 * f64::from(k),
            input: prism_tools::Input::Time,
        });
    }
    assert!(second.measurement().1.starts_with("3 of 62"));
    // Esc stops it: nothing is made.
    second.handle(&prism_tools::Event {
        t: t + 0.1,
        input: prism_tools::Input::Key {
            key: prism_tools::Key::Escape,
        },
    });
    assert_eq!(
        second.document().log().count(),
        first.document().log().count()
    );
    assert_eq!(second.hint(), prism_tools::session::JOB_STOPPED);
}

/// [`shell_is_made_at_the_first_click_of_the_recorded_session`] made to
/// the end: all three clicks, every piece joined (by hand, in a release
/// build: `cargo test --release -p prism-tools --test curved -- --ignored
/// the_three_recorded`).
#[test]
#[ignore = "minutes: every piece of two face-by-face shells joined"]
fn the_three_recorded_shell_clicks_made_to_the_end() {
    let r: prism_tools::Recording =
        serde_json::from_str(include_str!("../data/owner-session-17-shell.json"))
            .expect("recording");
    let upto = |n: usize| {
        let mut r = r.clone();
        r.events.truncate(n);
        prism_tools::Session::replay(&r)
    };
    let before = upto(367);
    let second = upto(591);
    assert_eq!(second.version(), before.version() + 2, "built, not refused");
    let panel = second.shell_panel().expect("the Shell tool");
    assert_eq!(panel.refused, None);
    assert!(panel.warning.is_some(), "and says what to check");
    let third = upto(670);
    assert_eq!(third.version(), before.version() + 3, "built, not refused");
}

/// A drag of a live shell's thickness keeps the rim styles picked where it
/// started (picking them builds the shell many times: on the owner's
/// session-17 shell about four fifths of each frame), and letting go picks
/// afresh. Across the sizes a drag of that shell reaches, the kept pick is
/// the fresh one, so each frame shows exactly what letting go makes.
#[test]
fn a_thickness_drag_keeps_the_rim_pick_and_shows_what_is_made() {
    let r: prism_tools::Recording =
        serde_json::from_str(include_str!("../data/owner-session-17-shell.json"))
            .expect("recording");
    let mut r = r.clone();
    r.events.truncate(368);
    let s = prism_tools::Session::replay(&r);
    let a = s.document().assembly();
    let (_, def) = a
        .defs()
        .find(|(_, d)| d.model.faces().count() > 20 && d.stack.is_some())
        .expect("the shelled body");
    let stack = def.stack.clone().expect("live");
    let before = stack.before(0);
    let kept = stack.shell_rims(&before, 0).expect("a shell");
    assert!(
        kept.values()
            .any(|st| matches!(st, prism_ops::shelling::RimStyle::Flush(_))),
        "the program ends some rims flush"
    );
    for t in [0.2, 0.3, 0.4] {
        let mut st = stack.clone();
        let Some(prism_doc::Feature::Shell { thickness, .. }) = st.features.get_mut(0) else {
            unreachable!()
        };
        *thickness = t;
        let mut drag = st.clone();
        let shown = drag.evaluate_from_kept(&before, 0, &kept);
        let made = st.evaluate();
        assert!(!drag.features[0].is_broken() && !st.features[0].is_broken());
        assert_eq!(st.shell_rims(&before, 0).as_ref(), Some(&kept), "at {t}");
        assert_eq!(shown.faces().count(), made.faces().count());
        let points =
            |m: &prism_topo::Model| -> Vec<DVec3> { m.vertices().map(|(_, p)| p).collect() };
        assert_eq!(points(&shown), points(&made), "at {t}");
    }
}

/// The owner's session 25 (2026-11-26): a disc (two half discs) selected
/// and copied turned 10 degrees about its centre, for a 10-degree slice.
/// The copy's arcs lie on the original circle; split at each other's
/// ends, the pieces were left twice over, no face could be rebuilt, and
/// every line was left loose (Remove Stray then took them all). Now the
/// twins become one and the disc is four slices: two of 10 degrees.
#[test]
fn a_disc_copied_and_turned_on_itself_makes_slices() {
    let mut r: prism_tools::Recording =
        serde_json::from_str(include_str!("../data/owner-session-25-arc.json")).expect("recording");
    r.events.truncate(562);
    let s = prism_tools::Session::replay(&r);
    let m = s.work_model();
    let h = prism_ops::health::check(m);
    assert!(h.stray.edges.is_empty(), "{:?}", h.stray.edges);
    assert_eq!(prism_ops::check::problems(m), Vec::<String>::new());
    let mut areas: Vec<f64> = m.faces().map(|(id, _)| m.face_area(id)).collect();
    areas.sort_by(f64::total_cmp);
    assert_eq!(areas.len(), 4, "{areas:?}");
    let r2 = areas.iter().sum::<f64>() / std::f64::consts::PI;
    let slice = |deg: f64| std::f64::consts::PI * r2 * deg / 360.0;
    for (got, deg) in areas.iter().zip([10.0, 10.0, 170.0, 170.0]) {
        assert!(
            (got - slice(deg)).abs() < 1e-9 * r2.max(1.0),
            "{got} {}",
            slice(deg)
        );
    }
}

/// The owner's session 27 (2026-11-26): a shape of arcs grouped, copied,
/// the copy turned and moved over it, and both exploded. The arcs cross
/// and are split where they do, but each kept being one curve, so
/// erasing the piece of an arc inside the shape erased all of it, the
/// outline's pieces too. Now an arc crossed partway by another parts
/// there, as when a line is drawn across it: the click erases that
/// piece, and the two faces beside it become one.
#[test]
fn an_arc_crossed_by_another_after_an_explode_erases_piece_by_piece() {
    let mut r: prism_tools::Recording =
        serde_json::from_str(include_str!("../data/owner-session-27-explode.json"))
            .expect("recording");
    let upto = |r: &prism_tools::Recording, n: usize| {
        let mut r = r.clone();
        r.events.truncate(n);
        prism_tools::Session::replay(&r)
    };
    let before = upto(&r, 2036);
    let m0 = before.work_model().clone();
    // The owner's first click with Erase, inside the shape.
    r.events.truncate(2072);
    let after = prism_tools::Session::replay(&r);
    let m1 = after.work_model();
    let gone: Vec<_> = m0
        .edges()
        .filter(|(id, _)| m1.edge(*id).is_none())
        .collect();
    assert_eq!(gone.len(), 1, "only the piece clicked: {gone:?}");
    let ef = m0.edge_faces();
    assert_eq!(
        ef.get(&gone[0].0).map_or(0, |f| f.len()),
        2,
        "an inside piece"
    );
    assert_eq!(
        m1.faces().count(),
        m0.faces().count() - 1,
        "two faces became one"
    );
    let area = |m: &prism_topo::Model| m.faces().map(|(id, _)| m.face_area(id)).sum::<f64>();
    assert!(
        (area(m1) - area(&m0)).abs() < 1e-9,
        "{} {}",
        area(m1),
        area(&m0)
    );
}
