//! Shell / Thicken of the boat hull as a joined body (brief 49 part 2b),
//! against the real file (`docs/acceptance/models/boat-half-hull.prism`).
//! A regression for the outside-corner bridge itself: `shelling.rs`'s
//! own fixture already proves the bridge is right (hand-checked volume),
//! so this just watches the real file's own, different limit stay where
//! it is, named and on purpose.

use std::collections::BTreeSet;

use prism_doc::DefId;
use prism_ops::shelling;
use prism_topo::FaceId;

/// The hull's three outside corners (part 2b) all share one end, at a
/// true corner of three faces near the bow, where an outside crease
/// meets another kind at the same point: refused by the analytic path's
/// own name for an unreachable multi-face corner, not built wrong (NOTES
/// "Brief 49 part 2b"). Left as a known, separate limitation, the way
/// `the_third_hull_face_still_refuses` is in `thicken_hull.rs`; it
/// changes on purpose, not by surprise, the day a further part reaches
/// that corner too.
#[test]
fn the_hull_still_refuses_at_its_three_face_corner() {
    let bytes = std::fs::read("../../docs/acceptance/models/boat-half-hull.prism").unwrap();
    let contents = prism_file::read(&bytes).unwrap();
    let model = &contents.assembly.def(DefId(6)).unwrap().model;
    let all: BTreeSet<FaceId> = model.faces().map(|(id, _)| id).collect();
    for d in [0.05, -0.05] {
        assert_eq!(
            shelling::refusal(model, &all, &BTreeSet::new(), d),
            Some(shelling::CORNER),
            "d={d}"
        );
    }
}
