//! The boat hull's own NURBS faces (`docs/acceptance/models/boat-half-hull.prism`),
//! Push/Pulled alone (Solids & CAD part 3c), each a regression for a knot
//! landing exactly on a trimmed patch's own edge (NOTES "offset-fit issue").

use prism_doc::DefId;
use prism_ops::thicken;
use prism_topo::FaceId;

#[test]
fn two_of_the_three_hull_faces_thicken_both_ways() {
    let bytes = std::fs::read("../../docs/acceptance/models/boat-half-hull.prism").unwrap();
    let contents = prism_file::read(&bytes).unwrap();
    let model = &contents.assembly.def(DefId(6)).unwrap().model;
    for raw in [1, 3] {
        for d in [0.05, -0.05] {
            assert!(
                thicken::slab(model, FaceId(raw), d).is_ok(),
                "face {raw} at d={d}"
            );
        }
    }
}

/// Not yet reachable (2026-11-19): a thin sliver face between two
/// knuckles, whose own rim lies exactly along one of them. Left as a
/// known, separate limitation (NOTES "offset-fit issue"), not something
/// this test expects to pass; it changes on purpose, not by surprise, the
/// day it starts passing.
#[test]
fn the_third_hull_face_still_refuses() {
    let bytes = std::fs::read("../../docs/acceptance/models/boat-half-hull.prism").unwrap();
    let contents = prism_file::read(&bytes).unwrap();
    let model = &contents.assembly.def(DefId(6)).unwrap().model;
    for d in [0.05, -0.05] {
        assert_eq!(
            thicken::slab(model, FaceId(2), d).err(),
            Some(thicken::NO_OFFSET)
        );
    }
}
