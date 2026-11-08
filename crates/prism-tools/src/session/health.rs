//! Model Health in the session (PLAN 1.10): the panel's rows, selecting a
//! row's items, and running its fix, on the open group's model.

use prism_doc::Command;
use prism_ops::Selection;
use prism_ops::health::{Found, check};

use prism_view::camera::EASE_EXTENTS;

use super::Session;

/// One row of the Model Health panel.
#[derive(Clone, Debug, PartialEq)]
pub struct HealthRow {
    /// Which check (for selecting and fixing).
    pub key: &'static str,
    pub label: &'static str,
    /// What was found; zero is healthy.
    pub count: usize,
    /// More words ("2 watertight, 1 with holes").
    pub detail: String,
    /// The fix's button label, if there is one.
    pub fix: Option<&'static str>,
}

impl Session {
    /// Run every check on the open group's model, and count the groups.
    pub fn health_rows(&self) -> Vec<HealthRow> {
        let r = check(self.work_model());
        let row = |key, label, f: &Found, fix| HealthRow {
            key,
            label,
            count: f.count(),
            detail: String::new(),
            fix: (!f.is_empty()).then_some(fix).flatten(),
        };
        let mut rows = vec![
            row("duplicates", "Duplicates", &r.duplicates, None),
            row("near", "Near misses", &r.near_misses, None),
            row(
                "reversed",
                "Reversed faces",
                &r.reversed,
                Some("Orient Faces"),
            ),
            HealthRow {
                detail: format!("{} watertight, {} with holes", r.watertight, r.leaky),
                ..row("open", "Open edges", &r.open_edges, None)
            },
            row(
                "nonmanifold",
                "Edges with 3 or more faces",
                &r.non_manifold,
                None,
            ),
            row(
                "interior",
                "Interior faces",
                &r.interior,
                Some("Clean Envelope"),
            ),
            row("degenerate", "Degenerate", &r.degenerate, None),
            row("stray", "Stray edges", &r.stray, Some("Remove")),
            row(
                "crossing",
                "Crossing faces",
                &r.crossing,
                Some("Intersect Faces"),
            ),
            row("tiny", "Tiny pieces", &r.tiny, None),
        ];
        // Information only: nesting and scale.
        let asm = self.doc.assembly();
        let placed = asm.placed();
        let comps = asm.components().len();
        let groups = placed.len();
        let depth = placed.iter().map(|p| p.path.len()).max().unwrap_or(0);
        let shared = asm.defs().filter(|(id, _)| asm.uses(*id) > 1).count();
        rows.push(HealthRow {
            key: "nesting",
            label: "Nesting",
            count: 0,
            detail: format!(
                "{groups} groups and instances, {comps} components, {shared} shared, nested {depth} deep"
            ),
            fix: None,
        });
        rows.push(HealthRow {
            key: "scale",
            label: "Scale",
            count: usize::from(r.far_from_origin),
            detail: if r.far_from_origin {
                "far from the origin for its size: move it nearer".into()
            } else {
                "near the origin".into()
            },
            fix: None,
        });
        rows
    }

    /// Select a row's items and zoom to them.
    pub fn select_health(&mut self, key: &str) {
        let r = check(self.work_model());
        let f = match key {
            "duplicates" => r.duplicates,
            "near" => r.near_misses,
            "reversed" => r.reversed,
            "open" => r.open_edges,
            "nonmanifold" => r.non_manifold,
            "interior" => r.interior,
            "degenerate" => r.degenerate,
            "stray" => r.stray,
            "crossing" => r.crossing,
            "tiny" => r.tiny,
            _ => return,
        };
        let sel = Selection {
            vertices: f.vertices,
            edges: f.edges,
            faces: f.faces,
            ..Default::default()
        };
        let m = self.work_model();
        let pts: Vec<prism_math::DVec3> = sel
            .all_vertices(m)
            .into_iter()
            .map(|v| m.vertex(v))
            .collect();
        self.set_groups(Default::default());
        self.set_selection(sel);
        if let Some(first) = pts.first() {
            let (lo, hi) = pts
                .iter()
                .fold((*first, *first), |(l, h), p| (l.min(*p), h.max(*p)));
            let goal = self.camera.extents(lo, hi, self.view().aspect());
            self.ease_to(goal, EASE_EXTENTS);
        }
    }

    /// Run a row's fix, in one undo step.
    pub fn health_fix(&mut self, key: &str) {
        let only = (!self.selection.faces.is_empty()).then(|| self.selection.faces.clone());
        let c = match key {
            "reversed" => Command::OrientFaces { faces: None },
            // Clean Envelope works on the selection when there is one (PLAN 1.10).
            "interior" => Command::CleanEnvelope { faces: only },
            "stray" => Command::RemoveStray,
            "crossing" => Command::IntersectFaces { faces: None },
            _ => return,
        };
        self.cancel();
        self.run(c);
        self.refresh();
    }
}
