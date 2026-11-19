//! Live features as things you select (Inspector part 2): a feature is
//! its own faces, picked by clicking them with Select. The selection of
//! exactly the faces a feature made *is* the feature being selected, so
//! the existing highlight, Inspector and menus work from it.

use std::cell::RefCell;
use std::collections::BTreeSet;

use prism_doc::{Command, DefId};
use prism_ops::select::Selection;
use prism_topo::FaceId;

use super::Session;

/// The faces each feature made, for the model version and group they
/// were worked out for.
pub type FeatureCache = RefCell<Option<(u64, DefId, Vec<BTreeSet<FaceId>>)>>;

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
}
