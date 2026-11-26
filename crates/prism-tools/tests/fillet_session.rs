//! The owner's session 22 (2026-11-26): dragging a round on the edge where
//! a ceiling meets a wall stopped at 0.028 m, where 0.2 m fits, and past
//! that the preview went blank with only small text in the window to say
//! why.

use prism_tools::{Recording, Session};

fn recording() -> Recording {
    serde_json::from_str(include_str!("data/owner-session-22-chamfer.json")).expect("recording")
}

fn upto(n: usize) -> Session {
    let mut r = recording();
    r.events.truncate(n);
    Session::replay(&r)
}

/// The rounded edge's ends had a point where an edge was once split and
/// a line drawn on the end face beside the ceiling: corners to the
/// kernel, though not to the shape. Every size the owner dragged to, and
/// the 0.25 m typed, is now shown.
#[test]
fn the_owners_drags_show_every_size_they_reach() {
    for n in [211, 241, 354, 606] {
        let s = upto(n);
        let panel = s.round_panel().expect("the Fillet tool");
        assert_eq!(panel.refused, None, "at event {n}, {}", panel.size);
        assert!(
            !std::ptr::eq(s.display_scene(), s.scene()),
            "a preview at event {n}"
        );
    }
}

/// The round at 0.2 m on the model as recorded: the corner filled by the
/// area a quarter circle leaves, along the edge, and the line on the end
/// face still there.
#[test]
fn the_round_the_owner_wanted_is_made_and_keeps_the_line_on_the_end_face() {
    let s = upto(202);
    let m = s.work_model();
    let e = prism_topo::EdgeId(230);
    let ed = m.edge(e).expect("the edge");
    let len = m.vertex(ed.a).distance(m.vertex(ed.b));
    let vol = |m: &prism_topo::Model| -> f64 {
        m.faces()
            .map(|(id, _)| prism_ops::health::face_volume(m, id))
            .sum()
    };
    let r = 0.2;
    for (fillet, area) in [
        (true, r * r * (1.0 - std::f64::consts::FRAC_PI_4)),
        (false, r * r / 2.0),
    ] {
        let mut w = m.clone();
        prism_ops::rounds::round_edges(&mut w, &[e].into(), r, fillet).expect("rounded");
        assert_eq!(prism_ops::check::problems(&w), Vec::<String>::new());
        let added = vol(&w) - vol(m);
        assert!((added - area * len).abs() < 1e-6, "{added} {}", area * len);
        let h = prism_ops::health::check(&w);
        assert!(h.crossing.is_empty() && h.stray.is_empty() && h.open_edges.is_empty());
        // The line up the end face, where two faces of one plane meet.
        let line = w.edges().any(|(_, x)| {
            let (a, b) = (w.vertex(x.a), w.vertex(x.b));
            (a.x - 1.38626).abs() < 1e-4
                && (b.x - 1.38626).abs() < 1e-4
                && (a.y - 1.27908).abs() < 1e-4
                && (b.y - 1.27908).abs() < 1e-4
                && (a.z - b.z).abs() > 0.3
        });
        assert!(line, "the line on the end face is kept");
    }
}

/// Dragged past what can be made, the last size that could stays on
/// screen, and the reason shows at the cursor in the warning colour.
#[test]
fn a_drag_too_far_keeps_the_last_round_and_says_why_at_the_cursor() {
    let mut r = recording();
    r.events.truncate(217);
    let t = r.events.last().expect("an event").t;
    for (i, (x, y)) in [(900.0, 300.0), (700.0, 150.0), (500.0, 50.0)]
        .into_iter()
        .enumerate()
    {
        r.events.push(prism_tools::Event {
            t: t + 0.05 * (i + 1) as f64,
            input: prism_tools::Input::Move { x, y },
        });
    }
    let s = Session::replay(&r);
    let panel = s.round_panel().expect("the Fillet tool");
    assert!(panel.refused.is_some(), "too large: {}", panel.size);
    assert!(
        !std::ptr::eq(s.display_scene(), s.scene()),
        "the last round that could be made stays"
    );
    let warn = prism_view::Style::default().health_many;
    let o = s.overlay();
    assert!(
        o.labels
            .iter()
            .any(|l| l.color == warn && l.text.contains("Showing the last size that worked")),
        "{:?}",
        o.labels
    );
}
