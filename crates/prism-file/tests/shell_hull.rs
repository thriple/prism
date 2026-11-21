//! Shell / Thicken of the boat hull as a joined body (brief 49, parts 1
//! and 2 through 2c), against the real file
//! (`docs/acceptance/models/boat-half-hull.prism`). A regression for the
//! whole pipeline on a real file, not just the hand-built fixtures
//! `shelling.rs`'s own tests already prove each piece right on.

use std::collections::BTreeSet;

use prism_doc::DefId;
use prism_ops::{boolean::is_solid, check, shelling};
use prism_topo::FaceId;

/// The half hull (three NURBS faces, two flat) thickens both ways, with
/// its own third, sliver NURBS face left open (`FaceId(2)`, its own
/// separate, pre-existing limitation: NOTES "Offset-fit issue"). Every
/// blocker the brief expected along the way is now past: the sheet
/// offsets and joins (part 1); its creases mitre where they can and
/// bridge where they cannot, inside and outside (part 2, 2b); its one
/// true corner of three faces, near the bow, splits only the face it
/// must (part 2c); and the real file's own fitted edges, not just the
/// hand-built fixtures' straight ones, stay truly on each face's own
/// offset surface throughout, not just at the ends `truly_on` itself
/// checks (part 2c, found testing this). The volume is close to area x
/// thickness, as a thin shell's always roughly is.
#[test]
fn the_hull_thickens_both_ways() {
    let bytes = std::fs::read("../../docs/acceptance/models/boat-half-hull.prism").unwrap();
    let contents = prism_file::read(&bytes).unwrap();
    let model = &contents.assembly.def(DefId(6)).unwrap().model;
    let all: BTreeSet<FaceId> = model.faces().map(|(id, _)| id).collect();
    let open: BTreeSet<FaceId> = [FaceId(2)].into();
    let area: f64 = all
        .iter()
        .filter(|f| !open.contains(f))
        .map(|f| model.face_area(*f))
        .sum();
    for d in [0.05, -0.05] {
        assert_eq!(shelling::refusal(model, &all, &open, d), None, "d={d}");
        let mut m = model.clone();
        shelling::shell(&mut m, &all, &open, d).unwrap();
        assert!(is_solid(&m), "d={d}");
        let problems = check::problems(&m);
        assert!(problems.is_empty(), "d={d}: {problems:?}");
        let v: f64 = m
            .faces()
            .map(|(id, _)| prism_ops::health::face_volume(&m, id))
            .sum();
        assert!(
            (v.abs() - area * d.abs()).abs() < 0.1 * v.abs(),
            "d={d}: {v} {area}"
        );
    }
}

/// The hull's centre-plane boundary (its three rims in the plane y = 4.863:
/// the keel, the gunwale end and the transom's edge) ends flush in that
/// plane, so a mirrored second half meets it exactly (brief 49 part 3).
/// Thickened inward the offset surface is trimmed back to the plane; the
/// other way it would have to be extended, which says so.
#[test]
fn the_hulls_centre_plane_rims_end_flush() {
    use prism_math::Plane;
    use prism_topo::EdgeId;
    let bytes = std::fs::read("../../docs/acceptance/models/boat-half-hull.prism").unwrap();
    let contents = prism_file::read(&bytes).unwrap();
    let model = &contents.assembly.def(DefId(6)).unwrap().model;
    let all: BTreeSet<FaceId> = model.faces().map(|(id, _)| id).collect();
    let open: BTreeSet<FaceId> = [FaceId(2)].into();
    let plane = Plane::new(
        prism_math::DVec3::new(0.0, 4.863037368822191, 0.0),
        prism_math::DVec3::Y,
    );
    let styles: shelling::RimStyles = [EdgeId(4), EdgeId(8), EdgeId(21)]
        .into_iter()
        .map(|e| (e, shelling::RimStyle::Flush(plane)))
        .collect();
    let d = -0.05;
    assert_eq!(
        shelling::refusal_with(model, &all, &open, d, &styles),
        None,
        "inward"
    );
    let mut m = model.clone();
    shelling::shell_with(&mut m, &all, &open, d, &styles).unwrap();
    assert!(is_solid(&m));
    let problems = check::problems(&m);
    assert!(problems.is_empty(), "{problems:?}");
    // Nothing of the new solid lies past the plane.
    let past = m
        .vertices()
        .map(|(_, p)| p.y - 4.863037368822191)
        .fold(f64::MIN, f64::max);
    assert!(past < 1e-4, "{past}");
    assert_eq!(
        shelling::refusal_with(model, &all, &open, 0.05, &styles),
        Some(shelling::FLUSH_REACH),
        "outward needs the surface extended"
    );
}

/// What the program picks for the hull's rims by itself: every boundary
/// that lies in a plane a part would meet there (here the centre plane and
/// the gunwale's level) and can end flush in it, and no other. Outward it
/// cannot be built anywhere, so every rim stays Square, as before.
#[test]
fn the_hulls_default_rims() {
    use prism_topo::EdgeId;
    let bytes = std::fs::read("../../docs/acceptance/models/boat-half-hull.prism").unwrap();
    let contents = prism_file::read(&bytes).unwrap();
    let model = &contents.assembly.def(DefId(6)).unwrap().model;
    let all: BTreeSet<FaceId> = model.faces().map(|(id, _)| id).collect();
    let open: BTreeSet<FaceId> = [FaceId(2)].into();
    let styles = shelling::default_rims(model, &all, &open, -0.05);
    let flush: Vec<EdgeId> = styles.keys().copied().collect();
    assert_eq!(
        flush,
        [1, 4, 8, 13, 21].map(EdgeId),
        "the gunwale and the centre line, not the edges of the open face"
    );
    let mut m = model.clone();
    shelling::shell_with(&mut m, &all, &open, -0.05, &styles).unwrap();
    assert!(is_solid(&m));
    let problems = check::problems(&m);
    assert!(problems.is_empty(), "{problems:?}");
    assert!(shelling::default_rims(model, &all, &open, 0.05).is_empty());
}

/// The live shell keeps its rim styles (brief 49 part 3): the program picks
/// them at first, a choice for one boundary holds while the thickness
/// changes, and handing the boundary back returns it to the program.
#[test]
fn a_live_shell_keeps_a_chosen_rim_style() {
    use prism_doc::{Command, Document, EdgeRef};
    use prism_ops::shelling::RimStyle;
    use prism_topo::EdgeId;
    let bytes = std::fs::read("../../docs/acceptance/models/boat-half-hull.prism").unwrap();
    let contents = prism_file::read(&bytes).unwrap();
    let mut d = Document::with_assembly(contents.assembly);
    let hull = DefId(6);
    let placed = d.assembly().placed();
    let path = placed
        .iter()
        .find(|p| p.def == hull)
        .expect("a hull")
        .path
        .clone();
    let model = d.assembly().def(hull).unwrap().model.clone();
    let all: BTreeSet<FaceId> = model.faces().map(|(id, _)| id).collect();
    assert!(d.run_in(
        &path,
        Command::Shell {
            faces: all,
            open: [FaceId(2)].into(),
            thickness: -0.05,
        }
    ));
    let past = |d: &Document| {
        let m = &d.assembly().def(hull).unwrap().model;
        m.vertices()
            .map(|(_, p)| p.y - 4.863037368822191)
            .fold(f64::MIN, f64::max)
    };
    assert!(past(&d) < 1e-4, "the centre line ends flush by itself");
    let live = |d: &Document| {
        let stack = d.assembly().def(hull).unwrap().stack.clone().expect("live");
        stack.features
    };
    // Choose Square for the centre line: the wall leans past the plane.
    let edge = EdgeRef::of(&model, EdgeId(8)).unwrap();
    assert!(d.run(Command::SetRimStyle {
        def: hull,
        index: 0,
        edge,
        style: Some(RimStyle::Square),
    }));
    assert!(past(&d) > 1e-3, "Square leans past the plane: {}", past(&d));
    let prism_doc::Feature::Shell { rims, .. } = &live(&d)[0] else {
        panic!("a shell");
    };
    assert_eq!(rims.len(), 1);
    // The thickness changes, the choice stays.
    assert!(d.run(Command::SetFeatureSize {
        def: hull,
        index: 0,
        size: -0.04,
    }));
    assert!(past(&d) > 1e-4);
    // Hand it back to the program: flush again.
    assert!(d.run(Command::SetRimStyle {
        def: hull,
        index: 0,
        edge,
        style: None,
    }));
    assert!(past(&d) < 1e-4);
    // One undo step each.
    assert!(d.undo());
    assert!(past(&d) > 1e-4);
}
