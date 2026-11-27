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

/// Dragged past what can be made, the largest size that works is shown
/// (DECISIONS #45, kind 2), and the reason shows at the cursor in the
/// warning colour.
#[test]
fn a_drag_too_far_shows_the_largest_round_and_says_why_at_the_cursor() {
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
        "the largest round that can be made shows"
    );
    let warn = prism_view::Style::default().health_many;
    let o = s.overlay();
    assert!(
        o.labels
            .iter()
            .any(|l| l.color == warn && l.text.contains("the most these edges take")),
        "{:?}",
        o.labels
    );
}

/// The owner's session 24 (2026-11-26): seven edges of a top section
/// collected (six straight, one arc onto a cylinder) and filleted were
/// refused at every size. The straight ones were each in pieces, split
/// where lines drawn across their side faces met them: to the kernel, a
/// corner at each. Those lines are taken off, the pieces joined, the
/// edges rounded together and the lines drawn back.
#[test]
fn the_seven_edges_of_session_24_fillet_together() {
    let mut r: Recording =
        serde_json::from_str(include_str!("data/owner-session-24.json")).expect("recording");
    r.events.truncate(1100);
    let s = Session::replay(&r);
    let edges = s.selection().edges.clone();
    assert_eq!(edges.len(), 7);
    let mut m = s.work_model().clone();
    prism_ops::rounds::round_edges(&mut m, &edges, 0.05, true).expect("filleted");
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
    let h = prism_ops::health::check(&m);
    assert!(h.reversed.is_empty() && h.leaky == 0, "{h:?}");
}

/// The owner's Xby4 (2026-11-26): the long top edge between the top and
/// a face bevelled 12 degrees off it would not fillet at any size (the
/// kernel refuses shallow edges; lines drawn on the end faces met its
/// ends). It does now, and the lines on the end faces stay.
#[test]
fn the_xby4_top_edge_fillets() {
    let c = prism_file::read(include_bytes!("data/owner-xby4.prism")).expect("the file");
    let m = &c.assembly.root().model;
    let a = prism_ops::vertex_at(m, prism_math::DVec3::new(-0.5450, 1.2791, 2.5338));
    let e = m
        .edges()
        .find(|(_, x)| {
            let (p, q) = (m.vertex(x.a), m.vertex(x.b));
            (p.y - 1.27908).abs() < 1e-4
                && (q.y - 1.27908).abs() < 1e-4
                && (p.z - 2.53384).abs() < 1e-4
                && (q.z - 2.53384).abs() < 1e-4
                && (p.x - q.x).abs() > 1.0
        })
        .map(|(id, _)| id)
        .expect("the top edge");
    let _ = a;
    let lines = |m: &prism_topo::Model| {
        m.edges()
            .filter(|(_, x)| {
                (m.vertex(x.a).x + 0.545).abs() < 1e-3 && (m.vertex(x.b).x + 0.545).abs() < 1e-3
            })
            .count()
    };
    for r in [0.005, 0.02, 0.05] {
        let mut w = m.clone();
        prism_ops::rounds::round_edges(&mut w, &[e].into(), r, true)
            .unwrap_or_else(|why| panic!("{r}: {why}"));
        assert_eq!(prism_ops::check::problems(&w), Vec::<String>::new());
        let h = prism_ops::health::check(&w);
        assert!(h.reversed.is_empty() && h.leaky == 0, "{r}: {h:?}");
        assert!(lines(&w) >= lines(m), "{r}: the end face keeps its lines");
    }
}

/// The owner's session 29 (2026-11-26, brief 56 step 1): the body has a
/// slit (a line drawn into face 329 from its edge) and a notch 0.24 mm
/// long, so the kernel would not take it and only edges Prism's own round
/// could do rounded (16 of 192 tries, not counting lines on flat faces). With the slit taken off a copy and
/// the notch welded, the kernel takes the body: 81 round at
/// 0.093 m, and the slit is drawn back. Of the rest, some fit smaller (the
/// tool makes the largest that fits); the others are corners steps 2 and
/// 3 are for (a round running into a wall beside it).
#[test]
fn session_29_rounds_past_its_slit_and_notch() {
    let mut r: Recording =
        serde_json::from_str(include_str!("data/owner-session-29-open-fillet.json"))
            .expect("recording");
    r.events.truncate(118);
    let s = Session::replay(&r);
    let m = s.work_model().clone();
    let ef = m.edge_faces();
    let slit = |m: &prism_topo::Model| {
        m.edge_faces()
            .values()
            .filter(|f| matches!(f[..], [a, b] if a == b))
            .count()
    };
    assert_eq!(slit(&m), 3, "the slit is three edges");
    let edges: Vec<prism_topo::EdgeId> = m
        .edges()
        .filter(|(id, x)| ef.get(id).is_some_and(|f| f.len() == 2 && f[0] != f[1]) && !x.soft)
        .map(|(id, _)| id)
        .collect();
    let (mut done, mut flat) = (0, 0);
    for fillet in [true, false] {
        for e in &edges {
            let mut w = m.clone();
            let got = prism_ops::rounds::round_edges(&mut w, &[*e].into(), 0.093, fillet);
            if got == Err(prism_ops::rounds::FLAT_LINE) {
                flat += 1;
            }
            if got.is_ok() {
                assert_eq!(
                    prism_ops::check::problems(&w),
                    Vec::<String>::new(),
                    "{e:?}"
                );
                let h = prism_ops::health::check(&w);
                assert!(h.reversed.is_empty(), "{e:?}: {h:?}");
                assert!(slit(&w) >= 1, "{e:?} {fillet}: the slit is drawn back");
                done += 1;
            }
        }
    }
    eprintln!(
        "{done} rounded, {flat} lines on flat faces, of {}",
        edges.len() * 2
    );
    assert!(done >= 81, "{done} of {} rounded", edges.len() * 2);
}

#[test]
#[ignore]
fn zz_corner_look() {
    let c = prism_file::read(include_bytes!("data/owner-corner-test.prism")).expect("the file");
    for (i, g) in c.assembly.defs().enumerate() {
        let _ = (i, g);
    }
    let m = &c.assembly.root().model;
    let ef = m.edge_faces();
    eprintln!("faces {} edges {}", m.faces().count(), m.edges().count());
    for (id, f) in m.faces() {
        let s = format!("{:?}", f.surface);
        eprintln!("face {id:?} {} {:?}", &s[..s.len().min(90)], f.plane().map(|p| p.normal));
    }
    for (id, e) in m.edges() {
        eprintln!(
            "edge {id:?} {:.3?} -> {:.3?} curved {} soft {} faces {:?} concave? ",
            m.vertex(e.a), m.vertex(e.b), e.curve.is_curved(), e.soft, ef.get(&id)
        );
    }
}
