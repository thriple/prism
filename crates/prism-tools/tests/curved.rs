//! What the tools say about curved faces (Exact Curves 4a): the reasons
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
    let cap = m
        .faces()
        .find(|(_, f)| !f.is_curved())
        .map(|(id, _)| id)
        .expect("a cap");
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
                distance: 0.5,
                new_loop: false,
            },
            prism_ops::pushpull::CURVED_PULL,
        ),
        (
            Command::PushPull {
                face: cap,
                distance: 0.5,
                new_loop: false,
            },
            prism_ops::pushpull::CURVED_PULL,
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
