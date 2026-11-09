//! Screen-space picking (PLAN 1 items 1–2, 1.2): an aperture in pixels,
//! vertices beat edges, edges beat faces, and nothing hidden behind a face
//! can be picked. Runs on the CPU in f64 from the same camera that draws.

use prism_math::{DVec2, DVec3};
use prism_topo::{EdgeId, FaceId, VertexId};

use crate::camera::View;
use crate::scene::Scene;

/// Capture radius for a vertex, in pixels, at aperture 1 (the prototype's).
pub const VERTEX_RADIUS: f64 = 14.0;
/// Capture radius for an edge, in pixels, at aperture 1.
pub const EDGE_RADIUS: f64 = 9.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Hit {
    Vertex(VertexId),
    Edge(EdgeId),
    Face(FaceId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picked {
    pub hit: Hit,
    /// The point in the model that was picked.
    pub point: DVec3,
}

/// What is under the screen position `s` (logical pixels). `aperture`
/// scales the capture radii (the prototype's slider, 0.5 to 2.5).
pub fn pick(scene: &Scene, view: &View, s: DVec2, aperture: f64) -> Option<Picked> {
    let rv = VERTEX_RADIUS * aperture;
    let re = EDGE_RADIUS * aperture;
    let mut best: Option<(f64, VertexId, DVec3)> = None;
    for &(id, p) in &scene.vertices {
        let Some((q, _)) = view.to_screen(p) else {
            continue;
        };
        let d = q.distance(s);
        if d < rv
            && best.is_none_or(|b| d < b.0 || (d == b.0 && id < b.1))
            && !scene.occluded(view.eye, p)
        {
            best = Some((d, id, p));
        }
    }
    if let Some((_, id, p)) = best {
        return Some(Picked {
            hit: Hit::Vertex(id),
            point: p,
        });
    }
    let (o, dir) = view.ray(s);
    let mut best: Option<(f64, EdgeId, DVec3)> = None;
    for e in &scene.edges {
        // An arc is picked along its drawn pieces.
        for (a, b) in e.segments() {
            let Some(t) = closest_on_segment(o, dir, a, b) else {
                continue;
            };
            let p = a.lerp(b, t);
            let Some((q, _)) = view.to_screen(p) else {
                continue;
            };
            let d = q.distance(s);
            if d < re
                && best.is_none_or(|b| d < b.0 || (d == b.0 && e.id < b.1))
                && !scene.occluded(view.eye, p)
            {
                best = Some((d, e.id, p));
            }
        }
    }
    if let Some((_, id, p)) = best {
        return Some(Picked {
            hit: Hit::Edge(id),
            point: p,
        });
    }
    scene.ray_face(o, dir).map(|(id, t)| Picked {
        hit: Hit::Face(id),
        point: o + dir * t,
    })
}

/// The point on segment `a`–`b` closest to the ray, as a parameter in 0..=1.
fn closest_on_segment(o: DVec3, d: DVec3, a: DVec3, b: DVec3) -> Option<f64> {
    let u = b - a;
    let w = a - o;
    let (uu, ud, dd) = (u.dot(u), u.dot(d), d.dot(d));
    let den = uu * dd - ud * ud;
    if uu <= 0.0 {
        return Some(0.0);
    }
    let t = if den.abs() <= 1e-15 * uu * dd {
        // Parallel: the end nearest the ray.
        0.0
    } else {
        (ud * w.dot(d) - dd * w.dot(u)) / den
    };
    Some(t.clamp(0.0, 1.0))
}

/// The model point under the cursor for navigation: the nearest face along
/// the ray (vertices and edges do not matter here).
pub fn model_point(scene: &Scene, view: &View, s: DVec2) -> Option<DVec3> {
    let (o, d) = view.ray(s);
    scene.ray_face(o, d).map(|(_, t)| o + d * t)
}

/// A distance to fall back on when nothing is under the cursor: to the
/// middle of the model, or to the camera's target.
pub fn fallback_distance(scene: &Scene, view: &View, target: DVec3) -> f64 {
    match scene.bounds {
        Some((lo, hi)) => {
            let c = lo.lerp(hi, 0.5);
            (c - view.eye)
                .dot(view.f)
                .max(lo.distance(hi) * 0.5)
                .max(1e-300)
        }
        None => view.eye.distance(target).max(1e-300),
    }
}
