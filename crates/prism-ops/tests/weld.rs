//! Edit > Weld (brief 43, part 4a).

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::add_segment;
use prism_ops::curves::{self, ALREADY, NOT_A_CHAIN, NOTHING};
use prism_topo::{EdgeId, Model};

fn chain(m: &mut Model, pts: &[DVec3]) -> BTreeSet<EdgeId> {
    pts.windows(2)
        .flat_map(|w| add_segment(m, w[0], w[1]).edges)
        .collect()
}

fn ids(m: &Model, edges: &BTreeSet<EdgeId>) -> BTreeSet<Option<prism_topo::CurveId>> {
    edges.iter().map(|e| m.edge(*e).unwrap().curve_id).collect()
}

#[test]
fn a_chain_of_lines_welds_into_one_curve() {
    let mut m = Model::new();
    let p = DVec3::new;
    let es = chain(
        &mut m,
        &[
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(2.0, 1.0, 0.0),
            p(3.0, 1.0, 0.5),
        ],
    );
    assert_eq!(es.len(), 3);
    curves::weld(&mut m, &es).unwrap();
    let got = ids(&m, &es);
    assert_eq!(got.len(), 1);
    let c = got.into_iter().next().unwrap().expect("a curve id");
    // Picking any of them takes them all.
    let first = *es.iter().next().unwrap();
    assert_eq!(m.curve_edges(first).len(), 3);
    assert!(m.edges().filter(|(_, e)| e.curve_id == Some(c)).count() == 3);
    assert_eq!(curves::weld_refusal(&m, &es), Some(ALREADY));
}

#[test]
fn a_closed_loop_welds() {
    let mut m = Model::new();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    let es = chain(
        &mut m,
        &[
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(1.0, 1.0),
            p(0.0, 1.0),
            p(0.0, 0.0),
        ],
    );
    assert_eq!(m.face_count(), 1);
    curves::weld(&mut m, &es).unwrap();
    assert_eq!(ids(&m, &es).len(), 1);
    assert_eq!(m.face_count(), 1, "the face is left alone");
}

#[test]
fn branches_and_edges_apart_are_refused() {
    let mut m = Model::new();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    let mut es = chain(&mut m, &[p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)]);
    es.extend(chain(&mut m, &[p(1.0, 0.0), p(1.0, 1.0)]));
    assert_eq!(curves::weld_refusal(&m, &es), Some(NOT_A_CHAIN), "a branch");
    let mut apart = chain(&mut m, &[p(5.0, 0.0), p(6.0, 0.0)]);
    apart.extend(chain(&mut m, &[p(8.0, 0.0), p(9.0, 0.0)]));
    assert_eq!(curves::weld_refusal(&m, &apart), Some(NOT_A_CHAIN), "apart");
    assert_eq!(curves::weld_refusal(&m, &BTreeSet::new()), Some(NOTHING));
}

#[test]
fn a_welded_run_past_a_boxs_corners_stays_whole() {
    let mut m = Model::new();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    let f = chain(
        &mut m,
        &[
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(1.0, 1.0),
            p(0.0, 1.0),
            p(0.0, 0.0),
        ],
    );
    let face = m.faces().next().map(|(id, _)| id).unwrap();
    let up = if m.face(face).unwrap().plane().unwrap().normal.z > 0.0 {
        1.0
    } else {
        -1.0
    };
    prism_ops::pushpull::push_pull(&mut m, face, up, false);
    // The bottom loop, now meeting the box's upright edges at its corners.
    let bottom: BTreeSet<EdgeId> = f.into_iter().filter(|e| m.edge(*e).is_some()).collect();
    assert_eq!(bottom.len(), 4);
    curves::weld(&mut m, &bottom).unwrap();
    // A line drawn out from a corner only touches the curve.
    add_segment(&mut m, DVec3::ZERO, DVec3::new(-1.0, -1.0, 0.0));
    assert_eq!(ids(&m, &bottom).len(), 1, "still one curve");
}
