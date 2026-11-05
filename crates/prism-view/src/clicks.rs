//! The Select tool's clicks, exactly as in the feel test (`toolDown(`):
//! click selects one thing, double-click on a face adds its outer edges,
//! triple-click selects everything connected. Pure: the caller passes the
//! time of each press.

use prism_math::DVec2;
use prism_ops::select::{Selection, connected};
use prism_topo::{Model, loop_pairs};

use crate::pick::Hit;

/// A press continues a series of clicks within this many seconds...
pub const SERIES_SECONDS: f64 = 0.35;
/// ...and this many pixels of the previous press.
pub const SERIES_PIXELS: f64 = 6.0;

/// Counts clicks in a series (1 = single, 2 = double, 3 or more = triple).
#[derive(Clone, Copy, Debug, Default)]
pub struct ClickSeries {
    last: Option<(f64, DVec2)>,
    n: u32,
}

impl ClickSeries {
    /// A press at time `t` (seconds) and position `s` (pixels).
    pub fn press(&mut self, t: f64, s: DVec2) -> u32 {
        self.n = match self.last {
            Some((lt, ls)) if t - lt < SERIES_SECONDS && ls.distance(s) < SERIES_PIXELS => {
                self.n + 1
            }
            _ => 1,
        };
        self.last = Some((t, s));
        self.n
    }
}

/// The selection after the `n`th click of a series on `hit` (no Shift or Ctrl).
pub fn click_select(model: &Model, hit: Option<Hit>, n: u32) -> Selection {
    let mut sel = Selection::default();
    let Some(hit) = hit else {
        return sel;
    };
    if n >= 3 {
        let start = match hit {
            Hit::Vertex(v) => Some(v),
            Hit::Edge(e) => model.edge(e).map(|e| e.a),
            Hit::Face(f) => model.face(f).and_then(|f| f.outer.first().copied()),
        };
        return start.map(|v| connected(model, v)).unwrap_or_default();
    }
    match hit {
        Hit::Vertex(v) => {
            sel.vertices.insert(v);
        }
        Hit::Edge(e) => {
            sel.edges.insert(e);
        }
        Hit::Face(f) => {
            sel.faces.insert(f);
            if n == 2
                && let Some(face) = model.face(f)
            {
                for (a, b) in loop_pairs(&face.outer) {
                    if let Some(e) = model.edge_between(a, b) {
                        sel.edges.insert(e);
                    }
                }
            }
        }
    }
    sel
}
