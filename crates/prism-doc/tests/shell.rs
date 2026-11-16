//! Shell / Thicken as a command (brief 42, part 3a).

use prism_doc::{Command, Document};
use prism_math::DVec3;

#[test]
fn a_shell_is_one_undo_step_and_replays() {
    let mut d = Document::new();
    d.run(Command::Loop {
        points: vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
        ],
    });
    let before = d.model().geometry_json();
    let face = d.model().faces().map(|(id, _)| id).next().unwrap();
    let shell = Command::Shell {
        faces: [face].into(),
        open: Default::default(),
        thickness: 0.2,
    };
    assert_eq!(shell.refusal(d.model()), None);
    d.run(shell);
    assert_eq!(d.model().face_count(), 6, "a slab");
    assert_eq!(d.undo_name(), Some("Shell / Thicken"));
    let again = Document::replay(d.log());
    assert_eq!(again.model().to_json(), d.model().to_json());
    d.undo();
    assert_eq!(d.model().geometry_json(), before);
}
