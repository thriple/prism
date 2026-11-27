//! The boat hull's own NURBS faces (`docs/acceptance/models/boat-half-hull.prism`),
//! Push/Pulled alone (Solids & CAD part 3c), each a regression for a knot
//! landing exactly on a trimmed patch's own edge (NOTES "offset-fit issue").

use prism_doc::DefId;
use prism_ops::{boolean::is_solid, check, thicken};
use prism_topo::FaceId;

/// All three, the thin sliver between the two knuckles (face 2, whose own
/// rim runs along one of them) too: it refused until 2026-11-23, when its
/// rim's projected points, a few ULPs to the far side of the knuckle's
/// knot, stopped reading the next face's normal (owner recording 17).
#[test]
fn the_three_hull_faces_thicken_both_ways() {
    let bytes = std::fs::read("../../docs/acceptance/models/boat-half-hull.prism").unwrap();
    let contents = prism_file::read(&bytes).unwrap();
    let model = &contents.assembly.def(DefId(6)).unwrap().model;
    for raw in [1, 2, 3] {
        for d in [0.05, -0.05] {
            let slab = thicken::slab(model, FaceId(raw), d)
                .unwrap_or_else(|e| panic!("face {raw} at d={d}: {e}"));
            assert!(is_solid(&slab.model), "face {raw} at d={d}");
            assert_eq!(
                check::problems(&slab.model),
                Vec::<String>::new(),
                "face {raw} at d={d}"
            );
        }
    }
}
