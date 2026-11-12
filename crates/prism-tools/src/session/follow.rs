//! The Follow Me tool (brief 26, steps 5c and 5d): select a path of lines
//! and arcs, then click the profile face to sweep it along the path. Alt
//! (tapped) follows a face's edge instead: the path is the outer loop of
//! the selected face.
//!
//! With no path selected it works as in SketchUp (owner, 2026-11-12):
//! press on the profile face and drag along the edges joined to it. The
//! inference engine runs while dragging, and the path ends where it snaps
//! on the path: a point on an edge or arc, a midpoint, an end, or where a
//! line from another point (along an axis) crosses the path; else at the
//! point on the edge under the cursor. The path is the shortest run of
//! edges from the face to there, cut partway along its last edge when the
//! end is inside it. The sweep is previewed as it grows. Releasing after
//! a drag makes it; a click, move and click works too; Esc cancels.

use std::collections::BTreeSet;

use prism_doc::Command;
use prism_infer::Kind;
use prism_math::{DVec2, DVec3};
use prism_topo::{EdgeId, VertexId};
use prism_view::{Hit, View};

use super::Session;

pub const FOLLOW: &str = "tool.followme";

/// Said with Alt on and not one face selected.
pub const NO_FACE: &str =
    "Alt: select one face, whose edge is the path, then click the profile face.";

impl Session {
    pub(super) fn follow_hover(&mut self, s: DVec2, view: &View) {
        if let Some(super::edit::EditOp::Follow {
            profile, path, end, ..
        }) = &self.edit
        {
            let profile = *profile;
            let old = (path.clone(), *end);
            if let Some(new) = self.follow_path_to(profile, s, view)
                && new != old
            {
                self.follow_preview(profile, new.0, new.1);
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
                    end: None,
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
        let c = Command::FollowMe {
            path,
            profile,
            end: None,
        };
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
        let Some(super::edit::EditOp::Follow {
            profile, path, end, ..
        }) = self.edit.take()
        else {
            return;
        };
        self.set_preview(None);
        if path.is_empty() {
            self.overlay_dirty = true;
            self.refresh();
            return;
        }
        let c = Command::FollowMe { path, profile, end };
        if let Some(why) = c.refusal(self.work_model()) {
            self.follow_notice = Some(why);
        } else {
            self.run(c);
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    /// Where the path ends, from the inference engine's snap (else the
    /// edge under the cursor), as an edge reachable from the profile and a
    /// point on it (None: its whole length).
    fn follow_end(
        &self,
        reach: &BTreeSet<EdgeId>,
        s: DVec2,
        view: &View,
    ) -> Option<(EdgeId, Option<DVec3>)> {
        let m = self.work_model();
        let tol = self.scene.tol.dist;
        if let Some(i) = &self.inference {
            // On a path edge or arc (a point on it, its midpoint).
            if let Some(e) = i.edge.filter(|e| reach.contains(e)) {
                return Some((e, Some(i.point)));
            }
            // At a path point: the edge that reaches it, whole.
            if let Some(v) = i.vertex
                && let Some(e) = reach.iter().find(|e| m.edge(**e).is_some_and(|x| x.has(v)))
            {
                return Some((*e, Some(m.vertex(v))));
            }
            // Lined up with another point: where its guide crosses the path.
            if i.kind == Kind::Guide {
                let mut best: Option<(f64, EdgeId, DVec3)> = None;
                for g in &i.guides {
                    for e in reach {
                        let Some(x) = m.edge(*e) else { continue };
                        let (pa, pb) = (m.vertex(x.a), m.vertex(x.b));
                        let off = |t: f64| {
                            let q = x.curve.point(pa, pb, t);
                            let r = q - g.origin;
                            (r - g.dir * r.dot(g.dir)).length()
                        };
                        // Sample, then narrow down round the nearest sample.
                        let n = 64;
                        let k = (0..=n)
                            .min_by(|a, b| {
                                off(f64::from(*a) / f64::from(n))
                                    .total_cmp(&off(f64::from(*b) / f64::from(n)))
                            })
                            .unwrap_or(0);
                        let (mut lo, mut hi) = (
                            (f64::from(k) - 1.0).max(0.0) / f64::from(n),
                            (f64::from(k) + 1.0).min(f64::from(n)) / f64::from(n),
                        );
                        for _ in 0..80 {
                            let (a, b) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
                            if off(a) < off(b) {
                                hi = b;
                            } else {
                                lo = a;
                            }
                        }
                        let t = (lo + hi) / 2.0;
                        let d = off(t);
                        if d <= tol * 10.0 {
                            let q = x.curve.point(pa, pb, t);
                            let screen = view.to_screen(q).map_or(f64::MAX, |(p, _)| p.distance(s));
                            if best.is_none_or(|b| screen < b.0) {
                                best = Some((screen, *e, q));
                            }
                        }
                    }
                }
                if let Some((_, e, q)) = best {
                    return Some((e, Some(q)));
                }
            }
        }
        // Else the point on the path edge under the cursor.
        let p = super::edit::edge_pick(&self.scene, view, s, self.settings.aperture)?;
        let Hit::Edge(e) = p.hit else { return None };
        reach.contains(&e).then_some((e, Some(p.point)))
    }

    /// The path from the profile to where the cursor says it ends: the
    /// shortest run of edges (not the profile's own) from one of its
    /// corners, and the point it stops at partway along the last edge
    /// (None: at an edge's end). None when the cursor says nothing.
    fn follow_path_to(
        &self,
        profile: prism_topo::FaceId,
        s: DVec2,
        view: &View,
    ) -> Option<(BTreeSet<EdgeId>, Option<DVec3>)> {
        let m = self.work_model();
        let face = m.face(profile)?;
        let own: BTreeSet<EdgeId> = face.edge_ids().collect();
        // Breadth first from the profile's corners over the other edges.
        let mut at: std::collections::BTreeMap<VertexId, Vec<(EdgeId, VertexId)>> =
            Default::default();
        for (id, e) in m.edges().filter(|(id, _)| !own.contains(id)) {
            at.entry(e.a).or_default().push((id, e.b));
            at.entry(e.b).or_default().push((id, e.a));
        }
        let starts: BTreeSet<VertexId> = face.loops().flatten().copied().collect();
        let mut came: std::collections::BTreeMap<VertexId, Option<(EdgeId, VertexId)>> =
            starts.iter().map(|v| (*v, None)).collect();
        let mut queue: std::collections::VecDeque<VertexId> = starts.iter().copied().collect();
        let mut reach = BTreeSet::new();
        while let Some(v) = queue.pop_front() {
            for (id, w) in at.get(&v).into_iter().flatten() {
                reach.insert(*id);
                if let std::collections::btree_map::Entry::Vacant(slot) = came.entry(*w) {
                    slot.insert(Some((*id, v)));
                    queue.push_back(*w);
                }
            }
        }
        let (target, end) = self.follow_end(&reach, s, view)?;
        let te = m.edge(target)?;
        // Reach the target edge by its nearer end (fewer steps).
        let steps = |v: VertexId| {
            let mut n = 0usize;
            let mut v = v;
            while let Some(Some((_, prev))) = came.get(&v).copied() {
                n += 1;
                v = prev;
            }
            came.contains_key(&v).then_some(n)
        };
        let (near, far) = match (steps(te.a), steps(te.b)) {
            (Some(x), Some(y)) if y < x => (te.b, te.a),
            (Some(_), _) => (te.a, te.b),
            (None, Some(_)) => (te.b, te.a),
            (None, None) => return None,
        };
        let mut path: BTreeSet<EdgeId> = [target].into();
        let mut v = near;
        while let Some(Some((e, prev))) = came.get(&v).copied() {
            if e == target {
                break;
            }
            path.insert(e);
            v = prev;
        }
        // An end at the edge's far point is the whole edge; at its near
        // point, the path stops before it.
        let tol = self.scene.tol.dist;
        let end = match end {
            Some(p) if p.distance(m.vertex(far)) < tol => None,
            Some(p) if p.distance(m.vertex(near)) < tol => {
                path.remove(&target);
                None
            }
            other => other,
        };
        Some((path, end))
    }

    /// Show the sweep along `path` (or why it is refused).
    fn follow_preview(
        &mut self,
        profile: prism_topo::FaceId,
        path: BTreeSet<EdgeId>,
        end: Option<DVec3>,
    ) {
        let base = self.work_model();
        let (why, preview) = if path.is_empty() {
            (None, None)
        } else {
            let c = Command::FollowMe {
                path: path.clone(),
                profile,
                end,
            };
            let why = c.refusal(base);
            let preview = why.is_none().then(|| {
                let mut m = base.clone();
                c.apply(&mut m);
                m
            });
            (why, preview)
        };
        self.follow_notice = why;
        if let Some(super::edit::EditOp::Follow {
            path: p, end: e, ..
        }) = &mut self.edit
        {
            *p = path;
            *e = end;
        }
        self.set_preview(preview);
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The path drawn over the model while dragging (its last edge only up
    /// to where it stops).
    pub(super) fn follow_overlay(&self, o: &mut prism_view::overlay::Overlay) {
        let Some(super::edit::EditOp::Follow { path, end, .. }) = &self.edit else {
            return;
        };
        let fb = &self.style.feedback;
        let m = self.work_model();
        let tol = self.scene.tol;
        let mut push = |a, b| {
            o.lines.push(prism_view::overlay::Line {
                a,
                b,
                color: fb.endpoint,
                width: fb.target_width,
                dash: [0.0, 0.0],
            });
        };
        for id in path {
            let Some(x) = m.edge(*id) else { continue };
            let (pa, pb) = (m.vertex(x.a), m.vertex(x.b));
            // The edge the path stops inside: drawn from its joined end.
            let cut = end.filter(|p| {
                let n = 64;
                (0..=n).any(|k| {
                    x.curve
                        .point(pa, pb, f64::from(k) / f64::from(n))
                        .distance(*p)
                        < pa.distance(pb) / f64::from(n)
                }) && p.distance(pa) > tol.dist
                    && p.distance(pb) > tol.dist
            });
            let pts: Vec<DVec3> = (0..=32)
                .map(|k| x.curve.point(pa, pb, f64::from(k) / 32.0))
                .collect();
            let pts: Vec<DVec3> = match cut {
                Some(p) => {
                    let joined_a = path
                        .iter()
                        .any(|o| o != id && m.edge(*o).is_some_and(|y| y.has(x.a)))
                        || path.len() == 1;
                    let k = pts
                        .iter()
                        .enumerate()
                        .min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p)))
                        .map_or(0, |(k, _)| k);
                    if joined_a {
                        let mut v = pts[..=k].to_vec();
                        v.push(p);
                        v
                    } else {
                        let mut v = vec![p];
                        v.extend_from_slice(&pts[k..]);
                        v
                    }
                }
                None => pts,
            };
            for w in pts.windows(2) {
                push(w[0], w[1]);
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
