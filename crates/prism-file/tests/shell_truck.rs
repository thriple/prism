//! The owner's truck (`owner-truck.prism`, 2026-11-26): the cab, an open
//! tray of flat faces whose front is a leaning windscreen, would not shell
//! at any thickness ("that thickness turns a face inside out"). At the top
//! of the windscreen, beside the upright side, the two open rims' square
//! walls lean differently, so their moved copies do not end at one point;
//! the corner was put at one point anyway and the side's rim came out bent.
//! Now each rim ends where its own wall puts it and a small triangle
//! closes the gap between them (`shelling::rim_ends`).

use std::collections::BTreeSet;

use prism_doc::DefId;
use prism_ops::{boolean::is_solid, check, shelling};
use prism_topo::FaceId;

fn part(def: u32) -> prism_topo::Model {
    let bytes = std::fs::read("tests/data/owner-truck.prism").unwrap();
    let contents = prism_file::read(&bytes).unwrap();
    contents.assembly.def(DefId(def)).unwrap().model.clone()
}

#[test]
fn the_truck_cab_with_a_leaning_windscreen_shells_both_ways() {
    for def in [5, 6] {
        let model = part(def);
        let all: BTreeSet<FaceId> = model.faces().map(|(id, _)| id).collect();
        let none = BTreeSet::new();
        let area: f64 = all.iter().map(|f| model.face_area(*f)).sum();
        for d in [0.05, -0.05, 0.02, -0.02] {
            assert_eq!(
                shelling::refusal(&model, &all, &none, d),
                None,
                "{def} by {d}"
            );
            let mut m = model.clone();
            let flags =
                shelling::shell_flagged(&mut m, &all, &none, d, &Default::default()).unwrap();
            assert!(flags.is_empty(), "{def} by {d}: built as asked, {flags:?}");
            assert!(is_solid(&m), "{def} by {d}");
            let problems = check::problems(&m);
            assert!(problems.is_empty(), "{def} by {d}: {problems:?}");
            let v: f64 = m
                .faces()
                .map(|(id, _)| prism_ops::health::face_volume(&m, id))
                .sum();
            assert!(
                (v.abs() - area * d.abs()).abs() < 0.05 * area * d.abs(),
                "{def} by {d}: volume {v}, area x thickness {}",
                area * d.abs()
            );
        }
    }
}
