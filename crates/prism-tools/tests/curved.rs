//! What the tools say about curved faces (Exact Curves 4a, 4d): the reasons
//! edits are refused come from `Command::refusal`.

use prism_doc::Command;
use prism_math::DVec3;

#[test]
fn edits_of_a_cylinder_say_why_they_are_refused() {
    let m = prism_tools::scenes::cylinder(1.0, 2.0);
    let side = m
        .faces()
        .find(|(_, f)| f.is_curved())
        .map(|(id, _)| id)
        .expect("a side");
    let top = m
        .vertices()
        .filter(|(_, p)| p.z > 1.0)
        .map(|(id, _)| id)
        .collect();
    let cases = [
        (
            Command::Offset {
                face: side,
                distance: 0.1,
            },
            prism_ops::offset::CURVED_OFFSET,
        ),
        (
            Command::PushPull {
                face: side,
                distance: -1.0,
                new_loop: false,
            },
            prism_ops::radius::RADIUS_GONE,
        ),
        (
            Command::Translate {
                vertices: top,
                by: DVec3::Z,
            },
            prism_ops::transform::CURVED_WOULD_BEND,
        ),
    ];
    for (c, why) in cases {
        assert_eq!(c.refusal(&m), Some(why), "{}", c.name());
        let mut after = m.clone();
        c.apply(&mut after);
        assert_eq!(after.to_json(), m.to_json(), "{} changes nothing", c.name());
    }
}

/// The owner's recorded test (2026-11-14): a face lofted between four
/// curves, then the flat back face beside it pulled out, once plainly and
/// twice with Alt's new loop. Every pull was refused ("would bend a curved
/// face"); now each one adds faces and the model stays valid.
#[test]
fn the_back_of_a_lofted_face_pulls_out_in_the_recorded_session() {
    let r: prism_tools::Recording =
        serde_json::from_str(include_str!("data/pull-beside-loft.json")).expect("recording");
    let s = prism_tools::Session::replay(&r);
    let m = s.model();
    assert_eq!(m.face_count(), 19, "three pulls, each adding faces");
    assert_eq!(prism_ops::check::problems(m), Vec::<String>::new());
}

/// The owner's session 9 (2026-11-17): closing a loft's end with Patch.
/// The ring's bottom was two lines end to end, one side was a spline with
/// a piece erased, and that spline left the bottom almost straight on (no
/// real corner). Patch did nothing; now it fills the three-sided ring, with
/// no fold (the first try, as four sides, had a lip by that joint).
#[test]
fn patch_closes_the_lofts_end_in_the_recorded_session() {
    let r: prism_tools::Recording =
        serde_json::from_str(include_str!("data/patch-closing-loft.json")).expect("recording");
    let before: std::collections::BTreeSet<_> = r.model.faces().map(|(id, _)| id).collect();
    let s = prism_tools::Session::replay(&r);
    let m = s.model();
    assert_eq!(prism_ops::check::problems(m), Vec::<String>::new());
    let new: Vec<_> = m.faces().filter(|(id, _)| !before.contains(id)).collect();
    assert!(new.len() >= 3, "a three-sided fill: {}", new.len());
    // All of it faces one way (the end is nearly flat, across x).
    let mut side = 0.0;
    for (_, f) in &new {
        let p = f.surface.patch().expect("freeform");
        for i in 0..=8 {
            for j in 0..=8 {
                let u = p.u.0 + (p.u.1 - p.u.0) * f64::from(i) / 8.0;
                let v = p.v.0 + (p.v.1 - p.v.0) * f64::from(j) / 8.0;
                let Some(n) = p.surface.normal(u, v).try_normalize() else {
                    continue;
                };
                let n = if p.flipped { -n } else { n };
                assert!(n.x.abs() > 0.8, "bent: {n} at {u}, {v}");
                if side == 0.0 {
                    side = n.x.signum();
                }
                assert_eq!(n.x.signum(), side, "folded at {u}, {v}");
            }
        }
    }
}
