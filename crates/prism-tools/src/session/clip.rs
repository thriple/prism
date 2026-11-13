//! Cut, Copy, Paste and Paste in Place; Lock and Unlock; Intersect Faces
//! With Context (Everyday Commands part 1, brief 33).
//!
//! - A clip is an assembly of its own (`Assembly::clip`), in world
//!   coordinates: the selected loose geometry at its root, and copies of the
//!   selected groups with everything they use. The app keeps it, so every
//!   open document shares it; in the session it is set by events, so a
//!   recorded session replays.
//! - Paste holds the copy at the low corner of its box and moves it with
//!   the inference engine until a click places it; Esc cancels.

use std::collections::BTreeSet;
use std::sync::Arc;

use prism_doc::{Assembly, Command, InstanceId};
use prism_math::{DAffine3, DVec3};
use prism_ops::Selection;
use prism_view::overlay::{Line, Overlay};

use super::Session;

/// What the status bar says when an edit would change a locked group.
pub const LOCKED: &str = "Locked: Edit > Hide & Lock > Unlock it first";

/// The most edges a pasted copy shows while it follows the cursor.
const PREVIEW_EDGES: usize = 4000;

/// A paste in progress: the clip, the point of it held at the cursor, and
/// its edges (drawn where it would go).
#[derive(Clone, Debug)]
pub(super) struct Pasting {
    clip: Arc<Assembly>,
    grab: DVec3,
    lines: Vec<(DVec3, DVec3)>,
}

impl Session {
    /// The clipboard (the app shares it between documents).
    pub fn clipboard(&self) -> Option<Arc<Assembly>> {
        self.clipboard.clone()
    }

    /// Set the clipboard (another document's copy).
    pub fn set_clipboard(&mut self, clip: Option<Arc<Assembly>>) {
        self.clipboard = clip;
    }

    pub(super) fn pasting(&self) -> bool {
        self.pasting.is_some()
    }

    /// Edit > Copy: the selection and the selected groups. Returns whether
    /// anything was copied.
    pub(super) fn copy(&mut self) -> bool {
        if self.selection.is_empty() && self.groups.is_empty() {
            return false;
        }
        let asm = self.doc.assembly();
        let Some(ctx) = asm.context_def(&self.ctx) else {
            return false;
        };
        // The work model is in world coordinates already.
        let mut work = self.work_model().clone();
        let geo = prism_ops::select::take(&mut work, &self.selection);
        let clip = asm.clip(ctx, &self.to_world, geo, &self.groups);
        if clip.is_empty() {
            return false;
        }
        self.clipboard = Some(Arc::new(clip));
        true
    }

    /// Edit > Cut: copy, then delete, as one step. A locked group is not cut.
    pub(super) fn cut(&mut self) {
        let cut = Command::Cut {
            selection: self.selection.clone(),
            instances: self.groups.clone(),
        };
        if cut.locked_in(self.doc.assembly()) {
            self.notice = Some(LOCKED);
            return;
        }
        if self.copy() {
            self.cancel();
            self.selection = Selection::default();
            self.groups.clear();
            self.run(cut);
        }
    }

    /// Edit > Paste: the copy follows the cursor until a click.
    pub(super) fn paste_start(&mut self) {
        let Some(clip) = self.clipboard.clone() else {
            return;
        };
        self.cancel();
        let flat = clip.flattened();
        let Some((lo, _)) = flat.bbox() else {
            return;
        };
        let scene = prism_view::Scene::build(&flat);
        let lines = scene
            .edges
            .iter()
            .flat_map(|e| {
                let pts: Vec<DVec3> = std::iter::once(e.a)
                    .chain(e.mid.iter().copied())
                    .chain(std::iter::once(e.b))
                    .collect();
                pts.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>()
            })
            .take(PREVIEW_EDGES)
            .collect();
        self.pasting = Some(Pasting {
            clip,
            grab: lo,
            lines,
        });
        self.overlay_dirty = true;
        self.dirty = true;
        self.refresh();
    }

    /// The click that places a paste.
    pub(super) fn paste_press(&mut self) {
        let Some(to) = self.inference.as_ref().map(|i| i.point) else {
            return;
        };
        let Some(p) = self.pasting.take() else {
            return;
        };
        self.paste_at(p.clip, DAffine3::from_translation(to - p.grab));
    }

    /// Edit > Paste in Place: where it was copied from.
    pub(super) fn paste_in_place(&mut self) {
        if let Some(clip) = self.clipboard.clone() {
            self.cancel();
            self.paste_at(clip, DAffine3::IDENTITY);
        }
    }

    /// Paste and select what was pasted.
    fn paste_at(&mut self, clip: Arc<Assembly>, at: DAffine3) {
        let here = |s: &Session| -> (BTreeSet<InstanceId>, Selection) {
            let m = s.work_model();
            (
                s.groups_in_open(),
                Selection {
                    edges: m.edges().map(|(id, _)| id).collect(),
                    faces: m.faces().map(|(id, _)| id).collect(),
                    ..Default::default()
                },
            )
        };
        let (g0, s0) = here(self);
        if self.run(Command::Paste {
            clip: Box::new((*clip).clone()),
            at,
        }) {
            let (g1, s1) = here(self);
            self.set_groups(g1.difference(&g0).copied().collect());
            self.set_selection(Selection {
                edges: s1.edges.difference(&s0.edges).copied().collect(),
                faces: s1.faces.difference(&s0.faces).copied().collect(),
                ..Default::default()
            });
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    pub(super) fn paste_cancel(&mut self) {
        if self.pasting.take().is_some() {
            self.overlay_dirty = true;
            self.dirty = true;
        }
    }

    pub(super) fn paste_hint(&self) -> Option<&'static str> {
        self.pasting
            .is_some()
            .then_some("Click to place the copy · it snaps like Move · Esc cancels")
    }

    /// The copy drawn where a click would put it.
    pub(super) fn paste_overlay(&self, o: &mut Overlay) {
        let (Some(p), Some(i)) = (&self.pasting, &self.inference) else {
            return;
        };
        let d = i.point - p.grab;
        let fb = &self.style.feedback;
        for (a, b) in &p.lines {
            o.lines.push(Line {
                a: *a + d,
                b: *b + d,
                color: fb.rubber_band,
                width: self.style.edge_width,
                dash: [0.0, 0.0],
            });
        }
    }

    /// Edit > Lock and Unlock: the selected groups.
    pub(super) fn set_locked(&mut self, locked: bool) {
        let asm = self.doc.assembly();
        let change: BTreeSet<InstanceId> = self
            .groups
            .iter()
            .copied()
            .filter(|g| asm.is_locked(*g) != locked)
            .collect();
        if !change.is_empty() {
            self.cancel();
            self.run(Command::SetLocked {
                instances: change,
                locked,
            });
            self.overlay_dirty = true;
        }
    }

    /// Whether a selected group is locked (an edit tool then does not
    /// start, and says why).
    pub(super) fn locked_selected(&mut self) -> bool {
        let asm = self.doc.assembly();
        let locked = self.groups.iter().any(|g| asm.is_locked(*g));
        if locked {
            self.notice = Some(LOCKED);
        }
        locked
    }

    /// Intersect Faces > With Context: the open group's faces against the
    /// visible geometry outside it.
    pub(super) fn intersect_with_context(&mut self) {
        if self.ctx.is_empty() {
            return;
        }
        let asm = self.doc.assembly();
        let mine = self.ctx.clone();
        // Everything shown, in the world, except the open group itself.
        let mut others = prism_topo::Model::new();
        for p in asm.placed_visible() {
            // The open group and what is in it, and the groups it is in
            // (their own geometry is added below).
            if p.path.starts_with(&mine) || mine.starts_with(&p.path) {
                continue;
            }
            if let Some(d) = asm.def(p.def) {
                let mut m = d.model.clone();
                prism_ops::transform::affine(&mut m, &p.world);
                others.append(&m);
            }
        }
        // The geometry of each definition on the way to the open group
        // (the root's, and each parent's) is outside it too.
        let mut path = Vec::new();
        for i in std::iter::once(None).chain(mine.iter().map(Some)) {
            if let Some(i) = i {
                path.push(*i);
            }
            if path.len() == mine.len() {
                break;
            }
            if let (Some(d), Some(w)) = (
                asm.context_def(&path).and_then(|d| asm.def(d)),
                asm.world_of(&path),
            ) {
                let mut m = d.model.clone();
                prism_ops::transform::affine(&mut m, &w);
                others.append(&m);
            }
        }
        if others.faces().next().is_none() {
            return;
        }
        self.cancel();
        self.run(Command::IntersectWith { others });
        self.refresh();
    }
}
