//! Live features as things you select (Inspector part 2): a feature is
//! its own faces, picked by clicking them with Select. The selection of
//! exactly the faces a feature made *is* the feature being selected, so
//! the existing highlight, Inspector and menus work from it.

use std::cell::RefCell;
use std::collections::BTreeSet;

use prism_doc::{Command, DefId, SizeGuide};
use prism_math::{DVec2, DVec3};
use prism_ops::select::Selection;
use prism_topo::FaceId;
use prism_view::View;
use prism_view::overlay::{Line, Mark, Overlay, Shape};

use super::{Session, round};

/// The faces each feature made, for the model version and group they
/// were worked out for.
pub type FeatureCache = RefCell<Option<(u64, DefId, Vec<BTreeSet<FaceId>>)>>;

/// A selected feature's size guide, for the model version, group and
/// feature it was worked out for.
pub type GuideCache = RefCell<Option<(u64, DefId, usize, Option<SizeGuide>)>>;

/// The model the features before one make, for the document revision, group
/// and feature it was worked out for: a drag of a feature's handle runs
/// only that feature and those after it on this, not the whole stack
/// from its base (a shell's thickness handle took about 260 ms a frame on
/// the owner's session-17 body, re-running everything each move).
pub type BeforeCache = RefCell<Option<(u64, DefId, usize, prism_topo::Model)>>;

/// Said at the cursor while the handle is dragged to a size the round
/// cannot be made at.
const CANNOT_SIZE: &str = "The round cannot be this size here: a face beside it is too narrow, or it runs into a round next to it.";

/// A drag of the size handle in progress.
#[derive(Clone, Debug)]
pub(super) struct Handle {
    def: DefId,
    index: usize,
    guide: SizeGuide,
    side: usize,
    start: f64,
    size: f64,
}

/// How close (in pixels, before the aperture) the cursor must be to grab
/// the handle.
pub(super) const GRAB: f64 = 12.0;

impl Handle {
    /// Where the handle is for `size`: across the side face from the middle
    /// of the edge.
    fn at(guide: &SizeGuide, side: usize, size: f64) -> DVec3 {
        (guide.a + guide.b) * 0.5 + guide.sides[side].across * size
    }
}

impl Session {
    /// The faces each live feature of the open group (the top level for
    /// loose geometry) made, in order.
    pub(super) fn feature_faces(&self) -> Vec<BTreeSet<FaceId>> {
        let asm = self.doc.assembly();
        let Some(def) = asm.context_def(&self.ctx) else {
            return Vec::new();
        };
        if let Some((v, d, faces)) = &*self.feature_cache.borrow()
            && *v == self.version
            && *d == def
        {
            return faces.clone();
        }
        let faces = asm
            .def(def)
            .and_then(|d| d.stack.as_ref())
            .map(|s| s.made_faces())
            .unwrap_or_default();
        *self.feature_cache.borrow_mut() = Some((self.version, def, faces.clone()));
        faces
    }

    /// Which feature made this face.
    pub(super) fn feature_of_face(&self, f: FaceId) -> Option<usize> {
        self.feature_faces().iter().position(|m| m.contains(&f))
    }

    /// What a right-click on a face a live feature made selects: the
    /// feature, so the menu offers Explode and Remove.
    pub(super) fn feature_click(&self, hit: prism_view::Hit) -> Option<Selection> {
        let prism_view::Hit::Face(f) = hit else {
            return None;
        };
        let faces = self.feature_faces().get(self.feature_of_face(f)?)?.clone();
        Some(Selection {
            faces,
            ..Selection::default()
        })
    }

    /// The feature selected: the selection is exactly the faces it made.
    pub fn selected_feature(&self) -> Option<usize> {
        let sel = &self.selection;
        if sel.faces.is_empty()
            || !sel.edges.is_empty()
            || !sel.vertices.is_empty()
            || !self.groups.is_empty()
        {
            return None;
        }
        self.feature_faces()
            .iter()
            .position(|m| !m.is_empty() && *m == sel.faces)
    }

    /// "Fillet" or "Chamfer" for the selected feature.
    pub fn selected_feature_name(&self) -> Option<&'static str> {
        let i = self.selected_feature()?;
        let asm = self.doc.assembly();
        let def = asm.context_def(&self.ctx)?;
        Some(asm.def(def)?.stack.as_ref()?.features.get(i)?.name())
    }

    /// Select a feature of the open group (or the top level).
    pub fn select_feature(&mut self, index: usize) {
        let Some(faces) = self.feature_faces().get(index).cloned() else {
            return;
        };
        self.cancel();
        self.set_groups(BTreeSet::new());
        self.set_selection(Selection {
            faces,
            ..Selection::default()
        });
    }

    /// Explode the selected feature (and those before it) in one step.
    pub(super) fn explode_selected_feature(&mut self) {
        if let Some(index) = self.selected_feature()
            && let Some(def) = self.doc.assembly().context_def(&self.ctx)
        {
            self.cancel();
            self.run(Command::ExplodeFeature { def, index });
        }
    }

    /// Take the selected feature off, in one step.
    pub(super) fn remove_selected_feature(&mut self) {
        if let Some(index) = self.selected_feature()
            && let Some(def) = self.doc.assembly().context_def(&self.ctx)
        {
            self.cancel();
            self.run(Command::RemoveFeature { def, index });
        }
    }

    /// The selected feature's size guide and size, while the Fillet tool is
    /// active (a feature shows no guides under any other tool).
    fn handle_guide(&self) -> Option<(DefId, usize, SizeGuide, f64)> {
        if self.tool != round::FILLET {
            return None;
        }
        let index = self.selected_feature()?;
        let asm = self.doc.assembly();
        let def = asm.context_def(&self.ctx)?;
        let stack = asm.def(def)?.stack.as_ref()?;
        let size = stack.features.get(index)?.size()?;
        if let Some((v, d, i, g)) = &*self.guide_cache.borrow()
            && *v == self.version
            && *d == def
            && *i == index
        {
            return g.clone().map(|g| (def, index, g, size));
        }
        let guide = stack.size_guide(index);
        *self.guide_cache.borrow_mut() = Some((self.version, def, index, guide.clone()));
        guide.map(|g| (def, index, g, size))
    }

    /// The side face the handle is measured across: the one facing the view most.
    fn handle_side(guide: &SizeGuide, view: &View) -> usize {
        let to_eye = view.eye - (guide.a + guide.b) * 0.5;
        (0..guide.sides.len())
            .max_by(|x, y| {
                let f = |k: usize| guide.sides[k].normal.dot(to_eye).abs();
                f(*x).total_cmp(&f(*y))
            })
            .unwrap_or(0)
    }

    /// A press on the size handle starts dragging it. True if it did.
    pub(super) fn handle_press(&mut self, s: DVec2) -> bool {
        let Some((def, index, guide, size)) = self.handle_guide() else {
            return false;
        };
        let view = self.view();
        let side = Self::handle_side(&guide, &view);
        let at = Handle::at(&guide, side, size);
        let near = view
            .to_screen(at)
            .is_some_and(|(q, _)| q.distance(s) < GRAB * self.settings.aperture);
        if near {
            self.round_handle = Some(Handle {
                def,
                index,
                guide,
                side,
                start: size,
                size,
            });
        }
        near
    }

    /// While the handle is held: the size is how far across the side face
    /// the cursor is from the edge, shown on the body. True while dragging.
    pub(super) fn handle_drag(&mut self, s: DVec2, view: &View) -> bool {
        let Some(h) = self.round_handle.clone() else {
            return false;
        };
        let side = h.guide.sides[h.side];
        let (o, d) = view.ray(s);
        let den = side.normal.dot(d);
        if den.abs() > 1e-9 {
            let p = o + d * (side.normal.dot(side.origin - o) / den);
            let size = (p - h.guide.a).dot(side.across);
            if size > self.scene.tol.dist && size != h.size {
                match self.feature_at_size(h.def, h.index, size) {
                    Some(m) => {
                        if let Some(h) = &mut self.round_handle {
                            h.size = size;
                        }
                        self.round_refused = None;
                        self.set_preview(Some(m));
                    }
                    // The handle stays at the last size that worked, and
                    // says why at the cursor.
                    None => {
                        self.round_refused = Some(CANNOT_SIZE);
                        self.round_warning = false;
                    }
                }
            }
        }
        self.overlay_dirty = true;
        self.dirty = true;
        true
    }

    /// The body with feature `index` at `size`, or None if the kernel
    /// refuses that size.
    fn feature_at_size(&self, def: DefId, index: usize, size: f64) -> Option<prism_topo::Model> {
        let mut stack = self.doc.assembly().def(def)?.stack.clone()?;
        match stack.features.get_mut(index)? {
            prism_doc::Feature::Round { size: s, .. } => *s = size,
            _ => return None,
        }
        let m = self.evaluate_from(def, index, &mut stack);
        (!stack.features[index].is_broken()).then_some(m)
    }

    /// `stack` (group `def`'s, changed at feature `index`) run from the
    /// model before that feature, kept across a drag: the same model as
    /// running it whole.
    pub(super) fn evaluate_from(
        &self,
        def: DefId,
        index: usize,
        stack: &mut prism_doc::Stack,
    ) -> prism_topo::Model {
        let before = self.before(def, index, stack);
        stack.evaluate_from(&before, index)
    }

    /// The model the features of `def` before `index` make, kept while
    /// the document does not change.
    pub(super) fn before(
        &self,
        def: DefId,
        index: usize,
        stack: &prism_doc::Stack,
    ) -> std::cell::Ref<'_, prism_topo::Model> {
        // The document's own count of changes (the session's version also
        // moves with every preview, which a drag makes each frame).
        let version = self.doc.revision();
        {
            let mut cache = self.before_cache.borrow_mut();
            let fresh = cache
                .as_ref()
                .is_some_and(|(v, d, i, _)| *v == version && *d == def && *i == index);
            if !fresh {
                *cache = Some((version, def, index, stack.before(index)));
            }
        }
        std::cell::Ref::map(self.before_cache.borrow(), |c| {
            &c.as_ref().expect("just made").3
        })
    }

    /// The button let go on the handle: the size it was dragged to is
    /// set, in one undo step.
    pub(super) fn handle_release(&mut self) -> bool {
        let Some(h) = self.round_handle.take() else {
            return false;
        };
        self.round_refused = None;
        self.set_preview(None);
        if h.size != h.start {
            self.run(Command::SetFeatureSize {
                def: h.def,
                index: h.index,
                size: h.size,
            });
            // The feature's faces are new ones: keep it selected.
            self.select_feature(h.index);
        }
        self.overlay_dirty = true;
        self.refresh();
        true
    }

    /// Esc while dragging the handle: the size stays as it was.
    pub(super) fn handle_cancel(&mut self) -> bool {
        if self.round_handle.take().is_none() {
            return false;
        }
        self.round_refused = None;
        self.set_preview(None);
        self.overlay_dirty = true;
        self.refresh();
        true
    }

    /// A dashed outline of the second solid of the selected live boolean,
    /// where the Inspector's cutter fields put it.
    pub(super) fn cutter_overlay(&self, o: &mut Overlay) {
        let Some(index) = self.selected_feature() else {
            return;
        };
        let asm = self.doc.assembly();
        let Some(def) = asm.context_def(&self.ctx) else {
            return;
        };
        let Some(prism_doc::Feature::Boolean { tool, .. }) = asm
            .def(def)
            .and_then(|d| d.stack.as_ref())
            .and_then(|s| s.features.get(index))
        else {
            return;
        };
        let fb = &self.style.feedback;
        for (_, e) in tool.edges() {
            let (a, b) = (tool.vertex(e.a), tool.vertex(e.b));
            let steps = if e.curve.is_curved() { 16 } else { 1 };
            let mut from = a;
            for k in 1..=steps {
                let to = if k == steps {
                    b
                } else {
                    e.curve.point(a, b, f64::from(k) / f64::from(steps))
                };
                o.lines.push(Line {
                    a: from,
                    b: to,
                    color: fb.guide,
                    width: fb.target_width,
                    dash: [6.0, 4.0],
                });
                from = to;
            }
        }
    }

    /// The guide: the edge the feature works on (dashed), and the handle
    /// across a face from it.
    pub(super) fn handle_overlay(&self, o: &mut Overlay) {
        let Some((_, _, guide, size)) = self.handle_guide() else {
            return;
        };
        let view = self.view();
        let drag = self.round_handle.as_ref();
        let side = drag.map_or_else(|| Self::handle_side(&guide, &view), |h| h.side);
        let size = drag.map_or(size, |h| h.size);
        let at = Handle::at(&guide, side, size);
        let mid = (guide.a + guide.b) * 0.5;
        let fb = &self.style.feedback;
        let over = drag.is_some()
            || self.round_at.is_some_and(|s| {
                view.to_screen(at)
                    .is_some_and(|(q, _)| q.distance(s) < GRAB * self.settings.aperture)
            });
        let color = if over { self.style.hover } else { fb.guide };
        o.lines.push(Line {
            a: guide.a,
            b: guide.b,
            color: fb.guide,
            width: fb.target_width,
            dash: [6.0, 4.0],
        });
        o.lines.push(Line {
            a: mid,
            b: at,
            color,
            width: fb.target_width,
            dash: [0.0, 0.0],
        });
        o.marks.push(Mark {
            at,
            shape: Shape::Square,
            color,
            size: fb.glyph_size,
        });
    }
}
