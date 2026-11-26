//! The document's precision in the session (DECISIONS #42, tolerance stage
//! 4): reading and setting it, what a new one would merge, and the warning
//! for a line drawn shorter than it.

use prism_doc::Command;
use prism_math::Tol;
use prism_math::tol::{COARSE, FINE, STANDARD};
use prism_ops::health::{WouldMerge, would_merge};

use super::Session;

/// Said when a line is asked for shorter than the document's precision: it
/// would be one point. Nothing is refused that can be drawn; this cannot.
pub const BELOW_PRECISION: &str = "Shorter than this document's precision: it would merge into one point. To draw smaller, choose Fine in File > Document Settings.";

/// The precision presets, as (name, metres).
pub const PRESETS: [(&str, f64); 3] = [
    ("Fine (0.001 mm)", FINE),
    ("Standard (0.01 mm)", STANDARD),
    ("Coarse (0.1 mm)", COARSE),
];

/// What changing the precision would do, shown before it is applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrecisionChange {
    /// The precision it would become (kept within the allowed range).
    pub to: Tol,
    /// Points and edges closer or shorter than it, in every group: they
    /// merge when an edit next touches them. Only a coarser precision has
    /// any.
    pub merges: WouldMerge,
}

impl PrecisionChange {
    /// One plain line for the dialog.
    pub fn describe(&self) -> String {
        if self.merges.is_empty() {
            return "Nothing in this document is closer than that: nothing will merge.".into();
        }
        let n =
            |k: usize, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
        let mut parts = Vec::new();
        if self.merges.points > 0 {
            parts.push(n(self.merges.points, "point is", "points are"));
        }
        if self.merges.edges > 0 {
            parts.push(n(self.merges.edges, "line is", "lines are"));
        }
        format!(
            "{} closer or shorter than that: they will merge when you next edit near them.",
            parts.join(" and ")
        )
    }
}

impl Session {
    /// The document's precision.
    pub fn precision(&self) -> Tol {
        self.doc.assembly().tol()
    }

    /// What setting the precision to `dist` metres would do (Model Health
    /// first, as DECISIONS #42 asks of a coarser setting).
    pub fn precision_change(&self, dist: f64) -> PrecisionChange {
        let to = Tol::new(dist);
        let mut merges = WouldMerge::default();
        if to.dist > self.precision().dist {
            for (_, d) in self.doc.assembly().defs() {
                merges += would_merge(&d.model, &to);
            }
        }
        PrecisionChange { to, merges }
    }

    /// Set the document's precision (an undoable command). Returns whether
    /// it changed.
    pub fn set_precision(&mut self, dist: f64) -> bool {
        let to = Tol::new(dist);
        if to == self.precision() {
            return false;
        }
        let made = self.run(Command::SetPrecision { dist: to.dist });
        if made {
            self.model_changed();
        }
        made
    }

    /// Warn when a line is asked for shorter than the precision.
    pub(super) fn warn_below_precision(&mut self, len: f64) {
        if len < self.precision().dist {
            self.notice = Some(BELOW_PRECISION);
            self.overlay_dirty = true;
        }
    }
}
