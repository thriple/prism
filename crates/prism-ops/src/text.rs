//! Text as geometry (Annotation & Sections part 3): glyph contours made
//! into faces (a letter's holes become holes in its face), a solid, or just
//! the outline.
//!
//! The model is built in the text's own frame: `x` along the line, `y` up,
//! faces facing `+z`. The caller places it (a group's axes).

use prism_geom::Surface;
use prism_math::predicates::{Inside, point_in_polygon, signed_area2};
use prism_math::{DVec2, DVec3, Plane};
use prism_text::Laid;
use prism_topo::{Face, Model, VertexId};
use serde::{Deserialize, Serialize};

/// What the text is made of.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum TextMode {
    /// Edges only: the letters' outlines.
    Outline,
    /// Flat faces.
    Face,
    /// Faces pulled out `depth` along their normal, into solids. A
    /// negative depth pulls the other way.
    Solid { depth: f64 },
}

/// One face's worth of contours: an outline and the holes in it.
struct Region {
    outer: usize,
    holes: Vec<usize>,
}

/// Which contours bound a face and which are holes in them: a contour
/// inside an even number of others bounds a face, inside an odd number it
/// is a hole in the one just outside it.
fn regions(contours: &[Vec<DVec2>]) -> Vec<Region> {
    let inside = |outer: usize, inner: usize| {
        outer != inner && point_in_polygon(contours[inner][0], &contours[outer]) == Inside::Inside
    };
    let depth: Vec<usize> = (0..contours.len())
        .map(|i| (0..contours.len()).filter(|&j| inside(j, i)).count())
        .collect();
    (0..contours.len())
        .filter(|&i| depth[i].is_multiple_of(2))
        .map(|i| Region {
            outer: i,
            holes: (0..contours.len())
                .filter(|&j| depth[j] == depth[i] + 1 && inside(i, j))
                .collect(),
        })
        .collect()
}

/// The text as a model (empty if there is nothing to draw).
pub fn text_model(laid: &Laid, mode: TextMode) -> Model {
    let mut m = Model::new();
    let contours = &laid.contours;
    let plane = Plane::new(DVec3::ZERO, DVec3::Z);
    let mut faces = Vec::new();
    for r in regions(contours) {
        // Outer loops turn counter-clockwise about +z, holes clockwise.
        let mut ring = |k: usize, ccw: bool| -> Vec<VertexId> {
            let c = &contours[k];
            let mut ids: Vec<VertexId> = c
                .iter()
                .map(|p| m.add_vertex(DVec3::new(p.x, p.y, 0.0)))
                .collect();
            if (signed_area2(c) > 0.0) != ccw {
                ids.reverse();
            }
            for i in 0..ids.len() {
                m.add_edge(ids[i], ids[(i + 1) % ids.len()]);
            }
            ids
        };
        let outer = ring(r.outer, true);
        let holes: Vec<Vec<VertexId>> = r.holes.iter().map(|&h| ring(h, false)).collect();
        faces.push(m.add_face(Face::new(outer, holes, Surface::Plane(plane))));
    }
    match mode {
        TextMode::Face => {}
        TextMode::Outline => {
            for f in faces {
                m.remove_face(f);
            }
        }
        TextMode::Solid { depth } => {
            if depth != 0.0 && depth.is_finite() {
                for f in faces {
                    crate::pushpull::push_pull(&mut m, f, depth, false);
                }
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_text::{Font, layout};

    fn area(m: &Model) -> f64 {
        m.faces()
            .filter(|(_, f)| f.plane().is_some_and(|p| p.normal.z > 0.9))
            .map(|(id, _)| m.face_area(id))
            .sum()
    }

    #[test]
    fn letters_become_faces_with_their_holes() {
        // An O is one face with one hole: its area is the ring's.
        let o = layout("O", Font::Sans, 1.0);
        let m = text_model(&o, TextMode::Face);
        assert_eq!(m.face_count(), 1);
        let f = m.faces().next().unwrap().1;
        assert_eq!(f.holes.len(), 1, "the hole is a hole, not a second face");
        assert!(prism_ops_check(&m));
        let a = area(&m);
        assert!(a > 0.2 && a < 0.8, "the ring of an O: {a}");
        // Faces face +z.
        assert!(m.faces().all(|(_, f)| f.plane().unwrap().normal.z > 0.99));
        // An i is two faces (dot and stem); a B is one face with two holes.
        assert_eq!(
            text_model(&layout("i", Font::Sans, 1.0), TextMode::Face).face_count(),
            2
        );
        let b = text_model(&layout("B", Font::Serif, 1.0), TextMode::Face);
        assert_eq!(
            (b.face_count(), b.faces().next().unwrap().1.holes.len()),
            (1, 2)
        );
    }

    fn prism_ops_check(m: &Model) -> bool {
        let p = crate::check::problems(m);
        assert!(p.is_empty(), "{p:?}");
        true
    }

    #[test]
    fn a_whole_word_is_a_valid_model() {
        for font in Font::ALL {
            let m = text_model(
                &layout("Prism 3D: Hello, World!", font, 0.2),
                TextMode::Face,
            );
            assert!(m.face_count() > 15, "{font:?}");
            assert!(prism_ops_check(&m));
        }
    }

    #[test]
    fn outline_leaves_edges_and_no_faces() {
        let m = text_model(&layout("Hi", Font::Sans, 1.0), TextMode::Outline);
        assert_eq!(m.face_count(), 0);
        assert!(m.edge_count() > 20);
    }

    #[test]
    fn solid_text_is_closed_and_as_deep_as_asked() {
        let m = text_model(
            &layout("Hi", Font::Sans, 1.0),
            TextMode::Solid { depth: 0.25 },
        );
        let (lo, hi) = m.bbox().unwrap();
        assert!((hi.z - 0.25).abs() < 1e-9 && lo.z.abs() < 1e-9, "{lo} {hi}");
        assert!(prism_ops_check(&m));
        let r = crate::health::check(&m);
        assert!(
            r.open_edges.is_empty() && r.non_manifold.is_empty(),
            "{r:?}"
        );
        assert!(
            r.watertight >= 2,
            "two letters, two closed shells: {}",
            r.watertight
        );
        // Backwards.
        let m = text_model(
            &layout("Hi", Font::Sans, 1.0),
            TextMode::Solid { depth: -0.25 },
        );
        let (lo, hi) = m.bbox().unwrap();
        assert!((lo.z + 0.25).abs() < 1e-9 && hi.z.abs() < 1e-9);
    }

    #[test]
    fn nothing_to_draw_is_an_empty_model() {
        let m = text_model(&Laid::default(), TextMode::Face);
        assert_eq!((m.face_count(), m.edge_count()), (0, 0));
        // Zero depth is flat faces.
        let m = text_model(
            &layout("A", Font::Sans, 1.0),
            TextMode::Solid { depth: 0.0 },
        );
        assert!(m.face_count() >= 1 && m.bbox().unwrap().1.z == 0.0);
    }
}
