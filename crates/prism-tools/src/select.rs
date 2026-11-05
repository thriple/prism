//! Box select (feel test `commitBox(`): left to right selects what is wholly
//! inside (window), right to left what the box touches (crossing). Points,
//! edges and faces: the points are what join edges and faces, so they come
//! along (the owner's decision, DECISIONS #21).

use std::collections::BTreeSet;

use prism_math::DVec2;
use prism_topo::{EdgeId, FaceId};
use prism_view::{Scene, View};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoxHits {
    pub vertices: BTreeSet<prism_topo::VertexId>,
    pub edges: BTreeSet<EdgeId>,
    pub faces: BTreeSet<FaceId>,
}

fn inside(p: DVec2, lo: DVec2, hi: DVec2) -> bool {
    p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y
}

/// Whether segment a–b touches the rectangle (Liang–Barsky).
fn touches(a: DVec2, b: DVec2, lo: DVec2, hi: DVec2) -> bool {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let d = b - a;
    for (p, q) in [
        (-d.x, a.x - lo.x),
        (d.x, hi.x - a.x),
        (-d.y, a.y - lo.y),
        (d.y, hi.y - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return false;
                }
                t0 = t0.max(r);
            } else {
                if r < t0 {
                    return false;
                }
                t1 = t1.min(r);
            }
        }
    }
    true
}

fn in_polygon(p: DVec2, poly: &[DVec2]) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            c = !c;
        }
        j = i;
    }
    c
}

/// What a box from `lo` to `hi` (viewport points) selects.
pub fn box_select(scene: &Scene, view: &View, lo: DVec2, hi: DVec2, crossing: bool) -> BoxHits {
    let scr = |p| view.to_screen(p).map(|(s, _)| s);
    let mut out = BoxHits::default();
    // Points: inside the box, for both kinds (hidden ones too, as for
    // edges and faces, so nothing is left behind).
    for &(id, p) in &scene.vertices {
        if let Some(s) = scr(p)
            && inside(s, lo, hi)
        {
            out.vertices.insert(id);
        }
    }
    for e in &scene.edges {
        let (Some(a), Some(b)) = (scr(e.a), scr(e.b)) else {
            continue;
        };
        let hit = if crossing {
            touches(a, b, lo, hi)
        } else {
            inside(a, lo, hi) && inside(b, lo, hi)
        };
        if hit {
            out.edges.insert(e.id);
        }
    }
    for f in &scene.faces {
        let pts: Option<Vec<DVec2>> = f
            .outer
            .iter()
            .map(|q| scr(f.origin + f.u * q.x + f.v * q.y))
            .collect();
        let Some(ps) = pts else {
            continue;
        };
        let hit = if crossing {
            ps.iter().any(|p| inside(*p, lo, hi))
                || (0..ps.len()).any(|i| touches(ps[i], ps[(i + 1) % ps.len()], lo, hi))
                || in_polygon(lo, &ps)
        } else {
            ps.iter().all(|p| inside(*p, lo, hi))
        };
        if hit {
            out.faces.insert(f.id);
        }
    }
    out
}
