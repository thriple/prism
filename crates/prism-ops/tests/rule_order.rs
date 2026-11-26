//! The order of geometry rules (DECISIONS #44, tolerance stage 3): one test
//! per rule of what each clean-up step may change. Each edit runs, in
//! order: weld, split, rebuild faces on touched planes, cut curved faces,
//! remove duplicates, merge coplanar seams, soften, then what the user
//! asked for directly.

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::{add_rect, add_segment};
use prism_topo::{EdgeId, FaceId, Model, VertexId};

fn p(x: f64, y: f64) -> DVec3 {
    DVec3::new(x, y, 0.0)
}

/// Edges lying along the segment from `a` to `b`.
fn along(m: &Model, a: DVec3, b: DVec3) -> Vec<EdgeId> {
    let tol = m.tol();
    let on = |q: DVec3| {
        let t = (q - a).dot(b - a) / (b - a).length_squared();
        (-1e-9..=1.0 + 1e-9).contains(&t) && (a + (b - a) * t).distance(q) < tol.dist
    };
    m.edges()
        .filter(|(_, e)| on(m.vertex(e.a)) && on(m.vertex(e.b)))
        .map(|(id, _)| id)
        .collect()
}

/// A 2 by 1 face with a soft, smooth line across it at x = 1, the line
/// one curve (a click selects it whole).
fn strip_with_a_soft_line() -> (Model, EdgeId) {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 2.0, 1.0, 0.0);
    let soft = add_segment(&mut m, p(1.0, 0.0), p(1.0, 1.0)).edges[0];
    m.set_soft(soft, true, true);
    let cid = m.new_curve_id();
    m.set_curve_id(soft, Some(cid));
    (m, soft)
}

/// Rule 4a: splitting never changes how a line looks or which curve it
/// belongs to. A point put on a soft line splits it; both halves stay soft
/// and smooth, and one curve.
#[test]
fn splitting_a_line_keeps_its_look_and_its_curve() {
    let (mut m, soft) = strip_with_a_soft_line();
    let cid = m.edge(soft).unwrap().curve_id;
    let tol = m.tol();
    prism_ops::insert_point(&mut m, p(1.0, 0.5), &tol);
    let pieces = along(&m, p(1.0, 0.0), p(1.0, 1.0));
    assert_eq!(pieces.len(), 2, "split in two");
    for e in pieces {
        let x = m.edge(e).unwrap();
        assert!(x.soft && x.smooth, "{e:?} stays soft");
        assert_eq!(x.curve_id, cid, "{e:?} stays one curve");
    }
}

/// Rule 3 (parting curves): a hard line drawn across a curve parts it
/// there into two curves, each picked alone, as in SketchUp (brief 43);
/// that is its own step after splitting, and it still keeps the look.
#[test]
fn a_line_drawn_across_a_curve_parts_it_and_keeps_its_look() {
    let (mut m, _) = strip_with_a_soft_line();
    add_segment(&mut m, p(0.0, 0.5), p(2.0, 0.5));
    let pieces = along(&m, p(1.0, 0.0), p(1.0, 1.0));
    assert_eq!(pieces.len(), 2, "split in two");
    let ids: BTreeSet<_> = pieces
        .iter()
        .map(|e| m.edge(*e).unwrap().curve_id)
        .collect();
    assert_eq!(ids.len(), 2, "two curves");
    for e in pieces {
        let x = m.edge(e).unwrap();
        assert!(x.soft && x.smooth, "{e:?} stays soft");
    }
}

/// Rule 4b, first half: only the softening step makes a line soft, and only
/// the user makes a soft line hard. A line an edit adds over a soft line
/// (not drawn by the user) leaves it soft.
#[test]
fn a_line_an_edit_adds_over_a_soft_line_leaves_it_soft() {
    let (mut m, _) = strip_with_a_soft_line();
    add_segment(&mut m, p(1.0, 0.0), p(1.0, 1.0));
    for e in along(&m, p(1.0, 0.0), p(1.0, 1.0)) {
        assert!(m.edge(e).unwrap().soft, "{e:?}");
    }
}

/// Rule 3, last: what the user asked for directly comes last. A line the
/// user draws over a soft line shows.
#[test]
fn a_line_the_user_draws_over_a_soft_line_shows() {
    let (mut m, _) = strip_with_a_soft_line();
    prism_ops::sticky::add_drawn_segment(&mut m, p(1.0, 0.0), p(1.0, 1.0));
    let es = along(&m, p(1.0, 0.0), p(1.0, 1.0));
    assert!(!es.is_empty());
    for e in es {
        assert!(!m.edge(e).unwrap().soft, "{e:?}");
    }
}

/// Rule 4a for welding: a hard line moved onto a soft one where the faces
/// meet smoothly (in one plane) leaves it soft; the softening rule, not
/// whichever line landed last, decides.
#[test]
fn welding_a_hard_line_onto_a_soft_one_leaves_it_soft() {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 1.0, 1.0, 0.0);
    let side = along(&m, p(1.0, 0.0), p(1.0, 1.0))[0];
    m.set_soft(side, true, true);
    add_rect(&mut m, 2.0, 0.0, 3.0, 1.0, 0.0);
    let right: BTreeSet<VertexId> = m
        .vertices()
        .filter(|(_, q)| q.x > 1.5)
        .map(|(id, _)| id)
        .collect();
    prism_ops::transform::translate(&mut m, &right, DVec3::new(-1.0, 0.0, 0.0));
    let es = along(&m, p(1.0, 0.0), p(1.0, 1.0));
    assert_eq!(es.len(), 1, "welded into one line");
    assert!(m.edge(es[0]).unwrap().soft);
    assert!(prism_ops::check::problems(&m).is_empty());
}

/// Rule 4c: removing duplicates keeps the face the edit did not make. A
/// square moved exactly onto another (painted) one: the one that stayed
/// put is kept, with its paint, whichever was drawn first.
#[test]
fn a_duplicate_keeps_the_face_the_edit_did_not_move() {
    let mut m = Model::new();
    let moved = add_rect(&mut m, 0.0, 0.0, 1.0, 1.0, 0.0).faces[0];
    let still = add_rect(&mut m, 5.0, 0.0, 6.0, 1.0, 0.0).faces[0];
    let red = Some(prism_topo::MaterialId(7));
    m.edit_face(still, |f| f.paint.front = red);
    let vs: BTreeSet<VertexId> = m.face(moved).unwrap().outer.iter().copied().collect();
    prism_ops::transform::translate(&mut m, &vs, DVec3::new(5.0, 0.0, 0.0));
    let left: Vec<FaceId> = m.faces().map(|(id, _)| id).collect();
    assert_eq!(left, vec![still], "the face that stayed put is kept");
    assert_eq!(m.face(still).unwrap().paint.front, red);
    assert!(prism_ops::check::problems(&m).is_empty());
}

/// The same the other way round: the square drawn second moved onto the
/// first keeps the first.
#[test]
fn a_duplicate_keeps_the_older_face_when_the_newer_moves() {
    let mut m = Model::new();
    let still = add_rect(&mut m, 0.0, 0.0, 1.0, 1.0, 0.0).faces[0];
    let moved = add_rect(&mut m, 5.0, 0.0, 6.0, 1.0, 0.0).faces[0];
    let vs: BTreeSet<VertexId> = m.face(moved).unwrap().outer.iter().copied().collect();
    prism_ops::transform::translate(&mut m, &vs, DVec3::new(-5.0, 0.0, 0.0));
    let left: Vec<FaceId> = m.faces().map(|(id, _)| id).collect();
    assert_eq!(left, vec![still]);
}

/// Rule 4d: no step deletes a face another step just made, except a
/// duplicate. A rectangle drawn on a box's top makes a face there that
/// survives the clean-up, and pulling it up makes four new sides that do.
#[test]
fn no_step_deletes_a_face_another_just_made() {
    let mut m = Model::new();
    let base = add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0).faces[0];
    prism_ops::push_pull(&mut m, base, -1.0, false);
    let top_z = m.vertices().map(|(_, q)| q.z).fold(f64::MIN, f64::max);
    let made = add_rect(&mut m, 0.5, 0.5, 1.5, 1.5, top_z);
    let inner = *made.faces.first().expect("a face on the top");
    assert!(m.face(inner).is_some());
    let before = m.face_count();
    prism_ops::push_pull(&mut m, inner, 0.5, false);
    assert!(m.face(inner).is_some(), "the pulled face stays");
    assert_eq!(m.face_count(), before + 4, "four new sides");
    assert!(prism_ops::check::problems(&m).is_empty());
}
