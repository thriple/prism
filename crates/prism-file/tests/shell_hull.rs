//! Shell / Thicken of the boat hull as a joined body (brief 49 part 2b
//! and 2c), against the real file (`docs/acceptance/models/boat-half-hull.prism`).
//! A regression for the outside-corner bridge and the three-face corner
//! themselves: `shelling.rs`'s own fixtures already prove each is right
//! on its own (hand-checked volumes), so this just watches the real
//! file's own, different limit stay where it is, named and on purpose.

use std::collections::BTreeSet;

use prism_doc::DefId;
use prism_ops::{check, shelling};
use prism_topo::FaceId;

/// The hull's three outside corners (part 2b) all share one end, at a
/// true corner of three faces near the bow (part 2c): no longer refused
/// there (NOTES "Brief 49 part 2c"), since only the bow panel's own
/// mitres reach past its edge and need to split away; the two flat
/// faces either side keep the one corner they already shared. Past
/// that, the hull still is not genuinely shellable: fitting the real
/// file's own outside corner (not the hand-built one in `shelling.rs`,
/// which is clean) turns up a face with a bad winding, a further,
/// separate limitation, found but not chased this time (NOTES "Brief 49
/// part 2c"). Left as a known, separate issue the way
/// `the_third_hull_face_still_refuses` is in `thicken_hull.rs`; it
/// changes on purpose, not by surprise, the day a further part reaches
/// it too.
#[test]
fn the_hull_reaches_its_three_face_corner_but_not_further() {
    let bytes = std::fs::read("../../docs/acceptance/models/boat-half-hull.prism").unwrap();
    let contents = prism_file::read(&bytes).unwrap();
    let model = &contents.assembly.def(DefId(6)).unwrap().model;
    let all: BTreeSet<FaceId> = model.faces().map(|(id, _)| id).collect();
    let open: BTreeSet<FaceId> = [FaceId(2)].into();
    for d in [0.05, -0.05] {
        assert_eq!(shelling::refusal(model, &all, &open, d), None, "d={d}");
        let (slab, _) = shelling::slab(model, &all, &open, d).unwrap();
        assert!(
            !check::problems(&slab).is_empty(),
            "d={d}: expected the real hull's own outside corner to still turn up a problem"
        );
    }
}
