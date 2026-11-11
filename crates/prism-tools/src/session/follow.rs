//! The Follow Me tool (brief 26, steps 5c and 5d): select a path of lines
//! and arcs, then click the profile face to sweep it along the path. Alt
//! (tapped) follows a face's edge instead: the path is the outer loop of
//! the selected face.
//!
//! With no path selected it works as in SketchUp (owner, 2026-11-12):
//! press on the profile face and drag along the edges joined to it. The
//! path is the shortest run of edges from the face to the edge under the
//! cursor (an arc or circle taken whole), and the sweep is previewed as
//! it grows. Releasing after a drag makes it; a click, move and click
//! works too; Esc cancels.

use std::collections::BTreeSet;

use prism_doc::Command;
use prism_math::DVec2;
use prism_view::{Hit, View};

use super::Session;

pub const FOLLOW: &str = "tool.followme";

/// Said with Alt on and not one face selected.
pub const NO_FACE: &str =
    "Alt: select one face, whose edge is the path, then click the profile face.";

impl Session {
    pub(super) fn follow_hover(&mut self, s: DVec2, view: &View) {
        if let Some(super::edit::EditOp::Follow { profile, path, .. }) = &self.edit {
            let profile = *profile;
            let old = path.clone();
            let new = self.follow_path_to(profile, s, view).unwrap_or(old.clone());
            if new != old {
                self.follow_preview(profile, new);
            }
            return;
        }
        let (o, d) = view.ray(s);
        let h = self.scene.ray_face(o, d).map(|(f, t)| prism_view::Picked {
            hit: Hit::Face(f),
            point: o + d * t,
        });
        self.set_hover_pick(h);
    }

    pub(super) fn follow_press(&mut self, s: DVec2) {
        // The second click of click-move-click makes the sweep.
        if let Some(super::edit::EditOp::Follow { .. }) = &self.edit {
            self.follow_commit();
            return;
        }
        self.follow_notice = None;
        let Some(Hit::Face(profile)) = self.hover.map(|p| p.hit) else {
            return;
        };
        // Nothing selected to follow: drag along the edges, as in SketchUp.
        if !self.follow_perimeter && self.selection().edges.is_empty() {
            if self.work_model().face(profile).is_some() {
                self.edit = Some(super::edit::EditOp::Follow {
                    profile,
                    path: BTreeSet::new(),
                    pressed: Some(s),
                });
                self.overlay_dirty = true;
                self.refresh();
            }
            return;
        }
        let path: BTreeSet<_> = if self.follow_perimeter {
            match self.perimeter() {
                Some(p) => p,
                None => {
                    self.follow_notice = Some(NO_FACE);
                    self.overlay_dirty = true;
                    return;
                }
            }
        } else {
            self.selection().edges.iter().copied().collect()
        };
        let c = Command::FollowMe { path, profile };
        if let Some(why) = c.refusal(self.work_model()) {
            self.follow_notice = Some(why);
        } else if self.run(c) {
            self.set_selection(Default::default());
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    /// The button let go: after a drag, the sweep is made.
    pub(super) fn follow_release(&mut self, s: DVec2) {
        let Some(super::edit::EditOp::Follow { pressed, .. }) = &mut self.edit else {
            return;
        };
        if pressed
            .take()
            .is_some_and(|p| p.distance(s) >= super::edit::DRAG_MIN)
        {
            self.follow_commit();
        }
    }

    /// Make the sweep previewed (or say why not).
    fn follow_commit(&mut self) {
        let Some(super::edit::EditOp::Follow { profile, path, .. }) = self.edit.take() else {
            return;
        };
        self.set_preview(None);
        if path.is_empty() {
            self.overlay_dirty = true;
            self.refresh();
            return;
        }
        let c = Command::FollowMe { path, profile };
        if let Some(why) = c.refusal(self.work_model()) {
            self.follow_notice = Some(why);
        } else {
            self.run(c);
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    /// The path from the profile to the edge under the cursor: the shortest
    /// run of edges (not the profile's own) from one of its corners, with
    /// the hovered edge's whole curve. None when no edge is under it.
    fn follow_path_to(
        &self,
        profile: prism_topo::FaceId,
        s: DVec2,
        view: &View,
    ) -> Option<BTreeSet<prism_topo::EdgeId>> {
        let m = self.work_model();
        let face = m.face(profile)?;
        let own: BTreeSet<prism_topo::EdgeId> = face.edge_ids().collect();
        let p = super::edit::edge_pick(&self.scene, view, s, self.settings.aperture)?;
        let Hit::Edge(target) = p.hit else {
            return None;
        };
        if own.contains(&target) || m.edge(target).is_none() {
            return None;
        }
        // Breadth first from the profile's corners over the other edges.
        let starts: BTreeSet<prism_topo::VertexId> = face.loops().flatten().copied().collect();
        let mut came: std::collections::BTreeMap<
            prism_topo::VertexId,
            Option<(prism_topo::EdgeId, prism_topo::VertexId)>,
        > = starts.iter().map(|v| (*v, None)).collect();
        let mut queue: std::collections::VecDeque<prism_topo::VertexId> =
            starts.iter().copied().collect();
        let te = m.edge(target)?;
        let mut reached = None;
        while let Some(v) = queue.pop_front() {
            if v == te.a || v == te.b {
                reached = Some(v);
                break;
            }
            for (id, e) in m.edges().filter(|(id, e)| e.has(v) && !own.contains(id)) {
                let w = e.other(v);
                if let std::collections::btree_map::Entry::Vacant(slot) = came.entry(w) {
                    slot.insert(Some((id, v)));
                    queue.push_back(w);
                }
            }
        }
        let mut v = reached?;
        let mut path: BTreeSet<prism_topo::EdgeId> = [target].into();
        while let Some(Some((e, prev))) = came.get(&v).copied() {
            path.insert(e);
            v = prev;
        }
        // An arc or circle is followed whole, if the path stays a path.
        let whole: BTreeSet<_> = path
            .iter()
            .flat_map(|e| m.curve_edges(*e))
            .chain(path.iter().copied())
            .collect();
        if whole != path
            && !matches!(
                prism_ops::follow::refusal(m, &whole, profile),
                Some(prism_ops::follow::NOT_A_PATH)
            )
        {
            return Some(whole);
        }
        Some(path)
    }

    /// Show the sweep along `path` (or why it is refused).
    fn follow_preview(&mut self, profile: prism_topo::FaceId, path: BTreeSet<prism_topo::EdgeId>) {
        let c = Command::FollowMe {
            path: path.clone(),
            profile,
        };
        let base = self.work_model();
        let why = c.refusal(base);
        let preview = why.is_none().then(|| {
            let mut m = base.clone();
            c.apply(&mut m);
            m
        });
        self.follow_notice = why;
        if let Some(super::edit::EditOp::Follow { path: p, .. }) = &mut self.edit {
            *p = path;
        }
        self.set_preview(preview);
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The path drawn over the model while dragging.
    pub(super) fn follow_overlay(&self, o: &mut prism_view::overlay::Overlay) {
        let Some(super::edit::EditOp::Follow { path, .. }) = &self.edit else {
            return;
        };
        let fb = &self.style.feedback;
        for e in path.iter().filter_map(|e| self.scene.edge(*e)) {
            for (a, b) in e.segments() {
                o.lines.push(prism_view::overlay::Line {
                    a,
                    b,
                    color: fb.endpoint,
                    width: fb.target_width,
                    dash: [0.0, 0.0],
                });
            }
        }
    }

    /// The outer loop of the one selected face, as a path.
    fn perimeter(&self) -> Option<BTreeSet<prism_topo::EdgeId>> {
        let sel = self.selection();
        let [face] = sel.faces.iter().copied().collect::<Vec<_>>()[..] else {
            return None;
        };
        let f = self.work_model().face(face)?;
        Some(f.loop_edge_ids(0).iter().copied().collect())
    }

    /// Alt tapped: follow the selected face's edge, or the selected path.
    pub(super) fn toggle_follow_perimeter(&mut self) {
        self.follow_perimeter = !self.follow_perimeter;
        self.follow_notice = None;
        self.overlay_dirty = true;
    }

    pub(super) fn follow_hint(&self) -> &'static str {
        if let Some(why) = self.follow_notice {
            return why;
        }
        if self.follow_perimeter {
            return if self.perimeter().is_some() {
                "Click the profile face to sweep it round the selected face's edge (Alt: along a selected path)"
            } else {
                NO_FACE
            };
        }
        if let Some(super::edit::EditOp::Follow { path, .. }) = &self.edit {
            return if path.is_empty() {
                "Drag along the edges joined to the face to set the path · release (or click) to make it · Esc cancels"
            } else {
                "Follow the path as far as it should go · release (or click) to make it · Esc cancels"
            };
        }
        if self.selection().edges.is_empty() {
            "Press on the profile face and drag along the edges joined to it (or select a path first, then click the face) · Alt: round a selected face's edge"
        } else {
            "Click the profile face to sweep it along the selected path (Alt: round a selected face's edge)"
        }
    }
}
