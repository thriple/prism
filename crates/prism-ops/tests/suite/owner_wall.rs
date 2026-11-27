//! The owner's curved wall (recording 17, 2026-11-23): six freeform
//! panels a loft cut from one surface, at the corners of its profile.
//! The loft found its cuts by projecting points, so one panel's trim runs
//! a few ULPs past both of the knots it should end on (0.12588313320923394
//! against 0.12588313320923405). Its lower edge read the panel below's
//! normal, so its offset could not be fitted: Push/Pull refused it and
//! Shell / Thicken refused the whole body as "too thick" at any thickness.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::Surface;
use prism_math::DVec3;
use prism_ops::shelling::{self, TOO_THICK};
use prism_ops::{boolean::is_solid, check, thicken};
use prism_topo::{EdgeId, FaceId, Model, VertexId};

fn wall() -> Model {
    Model::from_json(include_str!("../data/owner-17-wall.json")).expect("the model")
}

#[test]
fn every_panel_of_the_owners_curved_wall_pulls_out_and_in() {
    let m = wall();
    let panels: Vec<FaceId> = m
        .faces()
        .filter(|(_, f)| matches!(f.surface, Surface::Nurbs(_)))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(panels.len(), 6);
    for id in panels {
        for d in [0.25, -0.25, 0.025, -0.025] {
            let slab = thicken::slab(&m, id, d).unwrap_or_else(|e| panic!("{id:?} by {d}: {e}"));
            assert!(is_solid(&slab.model), "{id:?} by {d}");
            assert_eq!(
                check::problems(&slab.model),
                Vec::<String>::new(),
                "{id:?} by {d}"
            );
        }
    }
}

/// A sound slab: closed, no problems, and every edge walked once each way
/// by the two faces that use it (so Shell can be used on it again).
fn sound(s: &Model) {
    assert!(is_solid(s), "closed");
    assert_eq!(check::problems(s), Vec::<String>::new());
    let mut from: BTreeMap<EdgeId, Vec<VertexId>> = BTreeMap::new();
    for (_, f) in s.faces() {
        for side in f.sides() {
            from.entry(side.edge).or_default().push(side.from);
        }
    }
    for (e, ends) in from {
        assert!(
            ends.len() != 2 || ends[0] != ends[1],
            "{e:?} walked one way twice"
        );
    }
}

/// The owner's wall with only the faces `keep` left.
fn part(keep: &[FaceId]) -> Model {
    let mut m = wall();
    let all: Vec<FaceId> = m.faces().map(|(id, _)| id).collect();
    for id in all {
        if !keep.contains(&id) {
            m.remove_face(id);
        }
    }
    m
}

/// The flat triangles of slab `s` (of model `m`) whose corners are all
/// within `r` of `p` and all moved (none where a point of `m` is): the
/// faces closing the gap at a split corner there. A rim's walls and its
/// own corner triangles start at points of `m` itself, and the moved copy
/// of a flat face of `m` keeps its corners.
fn caps_near(m: &Model, s: &Model, p: DVec3, r: f64) -> usize {
    let tol = m.tol();
    s.faces()
        .filter(|(_, f)| f.plane().is_some() && f.outer.len() == 3)
        .filter(|(_, f)| {
            f.outer.iter().all(|v| {
                let x = s.vertex(*v);
                x.distance(p) < r && m.vertices().all(|(_, q)| !tol.same_point(q, x))
            })
        })
        .count()
}

/// The corner where four faces meet (a cylinder, a plane, and the two
/// panels either side of a fold in the profile): their moved copies miss
/// one point. Shell used to refuse it (`CORNER`). Now, as the 2D Offset
/// tool lets its lines cross rather than refuse (NOTES.md, "Shell/Thicken
/// research, and a correction"), each run of faces between outside
/// creases there keeps its own corner and a flat face closes the gap.
const TORN: VertexId = VertexId(11);

/// The four faces round that corner on their own: it is away from any rim
/// there, so the gap between the three runs (the cylinder and plane, which
/// meet at an inside crease, and each panel) is closed by one flat
/// triangle facing out of the slab.
#[test]
fn the_four_faces_round_the_owners_torn_corner_shell_with_a_flat_cap() {
    let m = part(&[FaceId(3), FaceId(9), FaceId(13), FaceId(14)]);
    let all: BTreeSet<FaceId> = m.faces().map(|(id, _)| id).collect();
    let p = m.vertex(TORN);
    for d in [0.25, 1.0, 0.025, -0.025] {
        let (s, _) =
            shelling::slab(&m, &all, &BTreeSet::new(), d).unwrap_or_else(|e| panic!("by {d}: {e}"));
        sound(&s);
        assert_eq!(caps_near(&m, &s, p, 3.0 * d.abs()), 1, "by {d}");
    }
}

/// Four faces at a corner (two flat, two panels fading to tangent with
/// each other there) where two of them miss the shared corner: the two
/// panels, between two outside creases, split away together to their
/// own corner (two points: the bridges either side share the one edge
/// between them, so no cap is needed). Shell used to refuse it (`CORNER`).
#[test]
fn two_faces_missing_a_four_face_corner_split_away_together() {
    let m = part(&[FaceId(2), FaceId(8), FaceId(10), FaceId(11)]);
    let all: BTreeSet<FaceId> = m.faces().map(|(id, _)| id).collect();
    for d in [0.25, 1.0, 0.025] {
        let (s, _) =
            shelling::slab(&m, &all, &BTreeSet::new(), d).unwrap_or_else(|e| panic!("by {d}: {e}"));
        sound(&s);
    }
}

/// The whole wall: outward, it shells to a sound solid, both on its own
/// and joined back into the model. Inward it no longer names the corner;
/// it is refused as "too thick" for two other reasons the corner used to
/// hide (not fixed here): an offset rim edge at a two-face rim corner
/// (vertex 12) that will not fit, and a crease between two panels so
/// nearly tangent (5e-5 rad at most) that their offsets' meeting line
/// cannot be fitted to the corner it must end at.
#[test]
fn shelling_the_owners_wall_now_closes_its_four_face_corner() {
    let m = wall();
    let all: BTreeSet<FaceId> = m.faces().map(|(id, _)| id).collect();
    let p = m.vertex(TORN);
    for d in [0.25, 1.0] {
        let (s, _) =
            shelling::slab(&m, &all, &BTreeSet::new(), d).unwrap_or_else(|e| panic!("by {d}: {e}"));
        sound(&s);
        assert_eq!(caps_near(&m, &s, p, 3.0 * d), 1, "by {d}");
        let mut joined = m.clone();
        shelling::shell(&mut joined, &all, &BTreeSet::new(), d).unwrap();
        assert_eq!(check::problems(&joined), Vec::<String>::new(), "by {d}");
    }
    for d in [-1.0, -0.25, -0.025] {
        assert_eq!(
            shelling::refusal(&m, &all, &BTreeSet::new(), d),
            Some(TOO_THICK),
            "by {d}"
        );
    }
}
