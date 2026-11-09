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
            row("arcs", "Off their curve", &r.off_circle, Some("Repair")),
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
            "arcs" => r.off_circle,
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
            "arcs" => Command::RepairArcs,
            "crossing" => Command::IntersectFaces { faces: None },
            _ => return,
        };
        self.cancel();
        self.run(c);
        self.refresh();
    }
}

impl Session {
    /// Draw Model Health's problems in the model (while its window is open
    /// and its switch is on).
    pub fn set_health_view(&mut self, on: bool) {
        if on != self.health_view {
            self.health_view = on;
            self.refresh_health();
        }
    }

    /// Recheck after a change, if the problems are being drawn.
    pub(super) fn refresh_health(&mut self) {
        self.health_report = self.health_view.then(|| check(self.work_model()));
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The problems, as lines and tinted triangles (PLAN 1.10).
    pub(super) fn health_marks(&self, o: &mut prism_view::overlay::Overlay) {
        use prism_view::overlay::{Line, Tri};
        let Some(r) = &self.health_report else { return };
        let m = self.work_model();
        let w = self.style.highlight_width;
        let mut edge_lines = |found: &Found, color| {
            for e in &found.edges {
                if let Some(ed) = m.edge(*e) {
                    // Along an arc's drawn points.
                    let (a, b) = (m.vertex(ed.a), m.vertex(ed.b));
                    let pts: Vec<_> = std::iter::once(a)
                        .chain(ed.curve.inner_points(a, b))
                        .chain(std::iter::once(b))
                        .collect();
                    for p in pts.windows(2) {
                        o.lines.push(Line {
                            a: p[0],
                            b: p[1],
                            color,
                            width: w,
                            dash: [0.0, 0.0],
                        });
                    }
                }
            }
        };
        edge_lines(&r.open_edges, self.style.health_open);
        edge_lines(&r.non_manifold, self.style.health_many);
        edge_lines(&r.stray, self.style.health_stray);
        edge_lines(&r.off_circle, self.style.health_many);
        for f in r.interior.faces.iter().chain(&r.crossing.faces) {
            if let Some(face) = m.face(*f) {
                for l in face.loops() {
                    for i in 0..l.len() {
                        o.tested.push(Line {
                            a: m.vertex(l[i]),
                            b: m.vertex(l[(i + 1) % l.len()]),
                            color: self.style.health_outline,
                            width: w,
                            dash: [4.0, 3.0],
                        });
                    }
                }
            }
        }
        for f in &r.reversed.faces {
            if let Some(sf) = self.scene.face(*f) {
                let idx = &self.scene.mesh.indices[sf.tris.start as usize..sf.tris.end as usize];
                for t in idx.as_chunks::<3>().0 {
                    o.tris.push(Tri {
                        p: t.map(|i| self.scene.mesh.positions[i as usize]),
                        color: self.style.health_reversed,
                    });
                }
            }
        }
    }
}
