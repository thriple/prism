//! Display triangles for a cone, sphere or torus face (Exact Curves 5b):
//! its loops unrolled, triangulated, then refined where the surface bulges
//! away from a triangle's edge, every point on the exact surface with the
//! exact normal.

use std::collections::BTreeMap;

use prism_geom::Turned;
use prism_math::{DVec2, DVec3};
use prism_topo::{Face, Model};

use crate::tess::triangulate;

/// A turned face ready to draw: points, normals and triangles (each turning
/// counter-clockwise about the face's normal), and its loops unrolled.
#[derive(Clone, Debug, Default)]
pub struct TurnedMesh {
    pub positions: Vec<DVec3>,
    pub normals: Vec<DVec3>,
    pub tris: Vec<[u32; 3]>,
    pub outer: Vec<DVec2>,
    pub holes: Vec<Vec<DVec2>>,
    /// The first point of the outer loop, in space (not on a pole).
    pub start: DVec3,
}

/// How far an edge's middle may stand off the surface, as a share of the
/// surface's size: about what 64 steps a turn give.
const SAG: f64 = 1.2e-3;
/// Rounds of splitting, at most.
const ROUNDS: usize = 12;

pub fn mesh(model: &Model, f: &Face, t: &Turned) -> TurnedMesh {
    let loops: Vec<Vec<(DVec3, DVec2)>> = (0..=f.holes.len())
        .map(|k| {
            let mut l = model.unrolled(f, k);
            // Repeated points (a run across a pole of no length) would
            // stop the triangulation.
            l.dedup_by(|b, a| a.1.distance(b.1) <= 1e-12);
            while l.len() > 1 && l[0].1.distance(l[l.len() - 1].1) <= 1e-12 {
                l.pop();
            }
            l
        })
        .collect();
    let flat = |l: &Vec<(DVec3, DVec2)>| l.iter().map(|x| x.1).collect::<Vec<_>>();
    let outer = flat(&loops[0]);
    let holes: Vec<Vec<DVec2>> = loops[1..].iter().map(flat).collect();
    let start = loops[0].first().map_or(t.origin, |x| x.0);
    let mut uv: Vec<DVec2> = loops.iter().flatten().map(|x| x.1).collect();
    let mut tris: Vec<[usize; 3]> = triangulate(&outer, &holes);
    let sag = SAG * t.size();
    let at = |q: DVec2| t.point(q);
    for _ in 0..ROUNDS {
        // The edges to split, each once: its middle on the surface is off
        // the chord.
        let mut mids: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        for tri in &tris {
            for k in 0..3 {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                let key = (a.min(b), a.max(b));
                if mids.contains_key(&key) {
                    continue;
                }
                let m = (uv[a] + uv[b]) * 0.5;
                if at(m).distance((at(uv[a]) + at(uv[b])) * 0.5) > sag {
                    mids.insert(key, uv.len());
                    uv.push(m);
                }
            }
        }
        if mids.is_empty() {
            break;
        }
        let len = |a: usize, b: usize| at(uv[a]).distance(at(uv[b]));
        let mut next = Vec::with_capacity(tris.len() * 3);
        for tri in tris {
            let mid = |k: usize| {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                mids.get(&(a.min(b), a.max(b))).copied()
            };
            let split: Vec<usize> = (0..3).filter(|k| mid(*k).is_some()).collect();
            match split.len() {
                0 => next.push(tri),
                1 => {
                    // Corners from the split edge's start.
                    let k = split[0];
                    let (a, b, c) = (tri[k], tri[(k + 1) % 3], tri[(k + 2) % 3]);
                    let m = mid(k).unwrap_or(a);
                    next.extend([[a, m, c], [m, b, c]]);
                }
                2 => {
                    // The edge not split runs from c to a.
                    let k = (0..3).find(|k| mid(*k).is_none()).unwrap_or(0);
                    let (c, a, b) = (tri[k], tri[(k + 1) % 3], tri[(k + 2) % 3]);
                    let (m1, m2) = (mid((k + 1) % 3).unwrap_or(a), mid((k + 2) % 3).unwrap_or(b));
                    next.push([m1, b, m2]);
                    if len(a, m2) <= len(m1, c) {
                        next.extend([[a, m1, m2], [a, m2, c]]);
                    } else {
                        next.extend([[a, m1, c], [m1, m2, c]]);
                    }
                }
                _ => {
                    let [a, b, c] = tri;
                    let (m0, m1, m2) = (
                        mid(0).unwrap_or(a),
                        mid(1).unwrap_or(b),
                        mid(2).unwrap_or(c),
                    );
                    next.extend([[a, m0, m2], [m0, b, m1], [m2, m1, c], [m0, m1, m2]]);
                }
            }
        }
        tris = next;
    }
    TurnedMesh {
        positions: uv.iter().map(|q| t.point(*q)).collect(),
        normals: uv.iter().map(|q| t.normal(*q)).collect(),
        tris: tris
            .into_iter()
            .map(|[a, b, c]| {
                // Unrolled, counter-clockwise turns about the outward normal.
                if t.outward {
                    [a as u32, b as u32, c as u32]
                } else {
                    [a as u32, c as u32, b as u32]
                }
            })
            .collect(),
        outer,
        holes,
        start,
    }
}
