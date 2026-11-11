//! A face that closes a hole in a solid faces out, the same way as the
//! faces round it (owner, 2026-11-12: such faces came out reversed at
//! random).

use prism_doc::{Command, Document};
use prism_math::{DQuat, DVec3};
use prism_ops::Selection;
use prism_topo::Model;

/// A 2 m cube, turned by `turn`, with its centre.
fn cube(turn: DQuat) -> (Document, DVec3) {
    let mut d = Document::with_model(Model::new());
    d.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 2.0,
        y1: 2.0,
        z: 0.0,
    });
    let f = prism_ops::face_at(d.model(), DVec3::new(1.0, 1.0, 0.0)).expect("rect");
    let n = d
        .model()
        .face(f)
        .and_then(|x| x.plane())
        .map_or(1.0, |p| p.normal.z);
    d.run(Command::PushPull {
        face: f,
        distance: 2.0 * n.signum(),
        new_loop: false,
    });
    let (axis, angle) = turn.to_axis_angle();
    if angle.abs() > 1e-12 {
        d.run(Command::Rotate {
            vertices: d.model().vertices().map(|(id, _)| id).collect(),
            centre: DVec3::ZERO,
            axis,
            angle,
        });
    }
    (d, turn * DVec3::new(1.0, 1.0, 1.0))
}

#[test]
fn a_face_closing_a_hole_in_a_box_faces_out() {
    let turns = [
        DQuat::IDENTITY,
        DQuat::from_axis_angle(DVec3::new(1.0, 2.0, 3.0).normalize(), 0.7),
        DQuat::from_axis_angle(DVec3::new(-2.0, 1.0, 0.5).normalize(), 2.3),
    ];
    let dirs = [
        DVec3::X,
        -DVec3::X,
        DVec3::Y,
        -DVec3::Y,
        DVec3::Z,
        -DVec3::Z,
    ];
    for turn in turns {
        for dir in dirs {
            let (mut d, centre) = cube(turn);
            let out = turn * dir;
            let at = centre + out;
            let f = prism_ops::face_at(d.model(), at).expect("the face");
            // Two corners of that face, along one of its edges.
            let face = d.model().face(f).expect("face");
            let (a, b) = (
                d.model().vertex(face.outer[0]),
                d.model().vertex(face.outer[1]),
            );
            let mut sel = Selection::default();
            sel.faces.insert(f);
            d.run(Command::Erase { selection: sel });
            assert!(prism_ops::face_at(d.model(), at).is_none(), "a hole");
            // Redraw one of its edges: the hole closes.
            d.run(Command::Segment { p: a, q: b });
            let g = prism_ops::face_at(d.model(), at).expect("closed again");
            let n = d
                .model()
                .face(g)
                .and_then(|x| x.plane())
                .expect("flat")
                .normal;
            assert!(
                n.dot(out) > 0.9,
                "faces out: {dir:?} turned {turn:?}: {n:?}"
            );
        }
    }
}

/// Two rectangles sharing an edge: the one pulled up becomes a closed box
/// with a base facing down, as in SketchUp (owner, 2026-11-12), and the
/// other stays as it was. Pulled the other way (down), the same.
#[test]
fn a_rectangle_beside_another_pulled_up_gets_a_base() {
    for up in [true, false] {
        let mut d = Document::with_model(Model::new());
        for (x0, x1) in [(0.0, 2.0), (2.0, 4.0)] {
            d.run(Command::Rect {
                x0,
                y0: 0.0,
                x1,
                y1: 2.0,
                z: 0.0,
            });
        }
        assert_eq!(d.model().face_count(), 2);
        let f = prism_ops::face_at(d.model(), DVec3::new(1.0, 1.0, 0.0)).expect("left");
        let other = prism_ops::face_at(d.model(), DVec3::new(3.0, 1.0, 0.0)).expect("right");
        let n = d.model().face(f).and_then(|x| x.plane()).map_or(1.0, |p| p.normal.z);
        let dz = if up { 1.0 } else { -1.0 };
        d.run(Command::PushPull {
            face: f,
            distance: 1.5 * dz * n.signum(),
            new_loop: false,
        });
        assert_eq!(d.model().face_count(), 7, "a closed box and the other rectangle");
        let base = prism_ops::face_at(d.model(), DVec3::new(1.0, 1.0, 0.0)).expect("a base");
        let bn = d.model().face(base).and_then(|x| x.plane()).expect("flat").normal;
        assert!(bn.z * dz < -0.9, "faces out of the box: {bn:?}, up {up}");
        assert!(d.model().face(other).is_some(), "the other stays");
        // The box is closed: every edge of it has two faces, except the
        // shared one, which has the other rectangle too.
        let r = prism_ops::health::check(d.model());
        assert_eq!(r.open_edges.edges.len(), 3, "only the other rectangle's free sides");
        assert!(r.reversed.is_empty(), "no face turned wrong: {:?}", r.reversed);
    }
}
