//! The owner's curved wall (recording 17, 2026-11-23): six freeform
//! panels a loft cut from one surface, at the corners of its profile.
//! The loft found its cuts by projecting points, so one panel's trim runs
//! a few ULPs past both of the knots it should end on (0.12588313320923394
//! against 0.12588313320923405). Its lower edge read the panel below's
//! normal, so its offset could not be fitted: Push/Pull refused it and
//! Shell / Thicken refused the whole body as "too thick" at any thickness.

use std::collections::BTreeSet;

use prism_geom::Surface;
use prism_ops::shelling::{self, CORNER};
use prism_ops::{boolean::is_solid, check, thicken};
use prism_topo::{FaceId, Model};

fn wall() -> Model {
    Model::from_json(include_str!("data/owner-17-wall.json")).expect("the model")
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

/// Every offset fits now, so the body's only refusal is the true one: at
/// one corner four faces meet (two flat, two panels either side of a fold
/// in the profile) whose moved copies miss one point, which Shell does
/// not do yet. When it does, this becomes a test that the shell succeeds.
#[test]
fn shelling_the_owners_wall_names_its_four_face_corner_not_the_thickness() {
    let m = wall();
    let all: BTreeSet<FaceId> = m.faces().map(|(id, _)| id).collect();
    for d in [0.25, 1.0, -1.0, -0.25, -0.025] {
        assert_eq!(
            shelling::refusal(&m, &all, &BTreeSet::new(), d),
            Some(CORNER),
            "by {d}"
        );
    }
}
