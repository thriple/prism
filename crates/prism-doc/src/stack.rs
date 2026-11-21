//! Live features on a body (Inspector & Features, part 2): the geometry the
//! body is drawn from (`base`), and the features applied on top of it in
//! order. The definition's own model is always the result, so every other
//! part of the program reads it as before. A direct edit is applied to the
//! base and the features run again; a feature that cannot find its edges
//! any more is marked broken and skipped, never silently dropped.

use std::collections::{BTreeMap, BTreeSet};

use prism_math::{DAffine3, DVec3};
use prism_ops::select::Selection;
use prism_topo::{EdgeId, FaceId, Model};
use serde::{Deserialize, Serialize};

/// An edge a feature points at: its id, and where its ends were, so it can
/// be found again by place if the id is gone.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeRef {
    pub id: EdgeId,
    pub a: DVec3,
    pub b: DVec3,
}

impl EdgeRef {
    pub fn of(m: &Model, id: EdgeId) -> Option<EdgeRef> {
        let (a, b) = ends(m, id)?;
        Some(EdgeRef { id, a, b })
    }
}

/// A face a feature points at: its id, and where it was (the middle of its
/// outline and its normal), so it can be found again by place.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaceRef {
    pub id: FaceId,
    pub centre: DVec3,
    /// Zero for a face that is not flat.
    pub normal: DVec3,
}

impl FaceRef {
    pub fn of(m: &Model, id: FaceId) -> Option<FaceRef> {
        let f = m.face(id)?;
        let pts: Vec<DVec3> = f.outer.iter().map(|v| m.vertex(*v)).collect();
        if pts.is_empty() {
            return None;
        }
        let centre = pts.iter().sum::<DVec3>() / pts.len() as f64;
        let normal = f.plane().map_or(DVec3::ZERO, |p| p.normal);
        Some(FaceRef { id, centre, normal })
    }
}

fn same_face(m: &Model, id: FaceId, r: &FaceRef) -> bool {
    FaceRef::of(m, id).is_some_and(|n| {
        n.centre.distance(r.centre) <= m.tol().dist && (n.normal - r.normal).length() <= 1e-6
    })
}

/// The face of `m` where `r` was.
fn face_at(m: &Model, r: &FaceRef) -> Option<FaceId> {
    m.faces().map(|(f, _)| f).find(|f| same_face(m, *f, r))
}

fn ends(m: &Model, id: EdgeId) -> Option<(DVec3, DVec3)> {
    let e = m.edge(id)?;
    Some((m.vertex(e.a), m.vertex(e.b)))
}

fn same_place(m: &Model, id: EdgeId, r: &EdgeRef) -> bool {
    let tol = m.tol().dist;
    ends(m, id).is_some_and(|(a, b)| {
        (a.distance(r.a) <= tol && b.distance(r.b) <= tol)
            || (a.distance(r.b) <= tol && b.distance(r.a) <= tol)
    })
}

/// The style a person chose for one open boundary of a live shell (brief
/// 49 part 3): any edge on it, and how its wall ends. Boundaries with no
/// choice are left to the program (`shelling::default_rims`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RimChoice {
    pub edge: EdgeRef,
    pub style: prism_ops::shelling::RimStyle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Feature {
    /// A fillet (a radius) or chamfer (how far into each face) on edges.
    Round {
        edges: Vec<EdgeRef>,
        size: f64,
        fillet: bool,
        /// Set by the last run: the edges are gone, or the kernel refused.
        #[serde(default)]
        broken: bool,
    },
    /// A shell (or a sheet thickened): the bodies of `faces` made
    /// `thickness` thick (out along their normals for more than zero, in
    /// for less), with `open` left open.
    Shell {
        faces: Vec<FaceRef>,
        open: Vec<FaceRef>,
        thickness: f64,
        /// How the walls at open boundaries end, where chosen.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        rims: Vec<RimChoice>,
        /// Set by the last run: the faces are gone, or the kernel refused.
        #[serde(default)]
        broken: bool,
    },
    /// The body joined with, cut by, or cut down to a second solid kept
    /// inside the feature (in the body's own coordinates). The tool's
    /// geometry is saved beside the base, not in the feature's JSON.
    Boolean {
        op: BoolKind,
        #[serde(skip, default = "Model::new")]
        tool: Model,
        /// Set by the last run: the kernel refused (the shapes no longer
        /// make a solid).
        #[serde(default)]
        broken: bool,
    },
}

/// What a live boolean does with its tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoolKind {
    Union,
    /// The body minus the tool.
    Subtract,
    /// Only where the body and the tool overlap.
    Intersect,
}

impl BoolKind {
    fn op(self) -> prism_ops::boolean::Op {
        match self {
            BoolKind::Union => prism_ops::boolean::Op::Union,
            BoolKind::Subtract => prism_ops::boolean::Op::Subtract,
            BoolKind::Intersect => prism_ops::boolean::Op::Intersect,
        }
    }
}

impl Feature {
    pub fn name(&self) -> &'static str {
        match self {
            Feature::Round { fillet: true, .. } => "Fillet",
            Feature::Round { .. } => "Chamfer",
            Feature::Shell { .. } => "Shell",
            Feature::Boolean {
                op: BoolKind::Union,
                ..
            } => "Union",
            Feature::Boolean {
                op: BoolKind::Subtract,
                ..
            } => "Subtract",
            Feature::Boolean {
                op: BoolKind::Intersect,
                ..
            } => "Intersect",
        }
    }

    pub fn is_broken(&self) -> bool {
        match self {
            Feature::Round { broken, .. }
            | Feature::Shell { broken, .. }
            | Feature::Boolean { broken, .. } => *broken,
        }
    }

    /// Run it on `m`. `on_base`: the model is the base itself, so its ids
    /// are the ones the feature was made with; later features find their
    /// edges by place first, because the ids a feature makes can shift.
    fn run(&mut self, m: &mut Model, on_base: bool) {
        match self {
            Feature::Round {
                edges,
                size,
                fillet,
                broken,
            } => {
                let Some(found) = resolve(m, edges, on_base) else {
                    *broken = true;
                    return;
                };
                let mut t = m.clone();
                match prism_ops::rounds::round_edges(&mut t, &found, *size, *fillet) {
                    Ok(_) => {
                        *m = t;
                        *broken = false;
                    }
                    Err(_) => *broken = true,
                }
            }
            Feature::Shell {
                faces,
                open,
                thickness,
                rims,
                broken,
            } => {
                let found = resolve_faces(m, faces, on_base).filter(|s| !s.is_empty());
                let open = resolve_faces(m, open, on_base);
                let (Some(found), Some(open)) = (found, open) else {
                    *broken = true;
                    return;
                };
                let styles = rim_styles(m, rims, &found, &open, *thickness, on_base);
                let mut t = m.clone();
                match prism_ops::shelling::shell_with(&mut t, &found, &open, *thickness, &styles) {
                    Ok(()) => {
                        *m = t;
                        *broken = false;
                    }
                    Err(_) => *broken = true,
                }
            }
            Feature::Boolean { op, tool, broken } => {
                // The tool first for Union and Intersect, as the group tools
                // always ran them.
                let run = match op {
                    BoolKind::Subtract => prism_ops::boolean::boolean(m, tool, op.op()),
                    _ => prism_ops::boolean::boolean(tool, m, op.op()),
                };
                match run {
                    Ok(r) => {
                        *m = r;
                        *broken = false;
                    }
                    Err(_) => *broken = true,
                }
            }
        }
    }
}

/// The styles of a live shell's rims: what was chosen, found again in `m`,
/// and the program's own pick for every other boundary.
fn rim_styles(
    m: &Model,
    rims: &mut [RimChoice],
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    thickness: f64,
    on_base: bool,
) -> prism_ops::shelling::RimStyles {
    let mut picks = Vec::new();
    for c in rims.iter_mut() {
        if let Some(e) = resolve(m, std::slice::from_mut(&mut c.edge), on_base)
            .and_then(|s| s.into_iter().next())
        {
            picks.push((e, c.style));
        }
    }
    let chosen = prism_ops::shelling::styles_for_boundaries(m, faces, open, &picks);
    prism_ops::shelling::default_rims_from(m, faces, open, thickness, chosen)
}

/// The edges a feature points at in `m`: by id, then by place. They are
/// brought up to date, so a moved edge is followed.
fn resolve(m: &Model, refs: &mut [EdgeRef], on_base: bool) -> Option<BTreeSet<EdgeId>> {
    let mut out = BTreeSet::new();
    for r in refs.iter_mut() {
        let id = if m.edge(r.id).is_some() && (on_base || same_place(m, r.id, r)) {
            Some(r.id)
        } else {
            m.edges().map(|(e, _)| e).find(|e| same_place(m, *e, r))
        }?;
        if let Some(now) = EdgeRef::of(m, id) {
            *r = now;
        }
        out.insert(id);
    }
    (!out.is_empty()).then_some(out)
}

/// The faces a feature points at in `m`: by id, then by place. They are
/// brought up to date, so a moved face is followed. None if one is gone.
fn resolve_faces(m: &Model, refs: &mut [FaceRef], on_base: bool) -> Option<BTreeSet<FaceId>> {
    let mut out = BTreeSet::new();
    for r in refs.iter_mut() {
        let id = if m.face(r.id).is_some() && (on_base || same_face(m, r.id, r)) {
            Some(r.id)
        } else {
            face_at(m, r)
        }?;
        if let Some(now) = FaceRef::of(m, id) {
            *r = now;
        }
        out.insert(id);
    }
    Some(out)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub base: Model,
    pub features: Vec<Feature>,
}

impl Stack {
    pub fn new(base: Model) -> Self {
        Stack {
            base,
            features: Vec::new(),
        }
    }

    /// Point the round at `index` at other edges (given as they look now:
    /// by place, in any model the features have run on). They are looked up
    /// in the model as it is just before that feature. False if it is not a
    /// round, there are none, or one is not there at that point.
    pub fn set_edges(&mut self, index: usize, picks: &[EdgeRef]) -> bool {
        if !matches!(self.features.get(index), Some(Feature::Round { .. })) {
            return false;
        }
        let mut before = Stack {
            base: self.base.clone(),
            features: self.features[..index].to_vec(),
        };
        let m = before.evaluate();
        let found: Option<Vec<EdgeRef>> = picks
            .iter()
            .map(|r| edge_at(&m, r).and_then(|e| EdgeRef::of(&m, e)))
            .collect();
        let Some(found) = found.filter(|f| !f.is_empty()) else {
            return false;
        };
        if let Some(Feature::Round { edges, .. }) = self.features.get_mut(index) {
            *edges = found;
        }
        true
    }

    /// Choose how the wall at the open boundary `edge` is on ends, for the
    /// live shell at `index` (`None` hands it back to the program). `edge`
    /// is looked up in the model as it is just before that feature. False
    /// if it is not a shell, or the edge is not on one of its boundaries.
    pub fn set_rim(
        &mut self,
        index: usize,
        edge: &EdgeRef,
        style: Option<prism_ops::shelling::RimStyle>,
    ) -> bool {
        let Some(Feature::Shell { faces, open, .. }) = self.features.get(index) else {
            return false;
        };
        let (mut faces, mut open) = (faces.clone(), open.clone());
        let mut before = Stack {
            base: self.base.clone(),
            features: self.features[..index].to_vec(),
        };
        let m = before.evaluate();
        let (Some(found), Some(open)) = (
            resolve_faces(&m, &mut faces, index == 0),
            resolve_faces(&m, &mut open, index == 0),
        ) else {
            return false;
        };
        let Some(e) = edge_at(&m, edge) else {
            return false;
        };
        let boundaries = prism_ops::shelling::boundaries(&m, &found, &open);
        let Some(on) = boundaries.iter().find(|b| b.contains(&e)) else {
            return false;
        };
        let Some(Feature::Shell { rims, .. }) = self.features.get_mut(index) else {
            return false;
        };
        // One choice for a boundary: drop any earlier one on it.
        rims.retain(|c| edge_at(&m, &c.edge).is_none_or(|x| !on.contains(&x)));
        if let (Some(style), Some(now)) = (style, EdgeRef::of(&m, e)) {
            rims.push(RimChoice { edge: now, style });
        }
        true
    }

    /// The base with every feature applied in order.
    pub fn evaluate(&mut self) -> Model {
        let mut m = self.base.clone();
        for (i, f) in self.features.iter_mut().enumerate() {
            f.run(&mut m, i == 0);
        }
        m
    }
}

/// A flat face beside the edge a feature works on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuideSide {
    pub normal: DVec3,
    pub origin: DVec3,
    /// Unit, in the face's plane, from the edge into the face.
    pub across: DVec3,
}

/// The edge a feature works on and the flat faces beside it: how far from
/// the edge a point lies across a face is the feature's size.
#[derive(Clone, Debug, PartialEq)]
pub struct SizeGuide {
    pub a: DVec3,
    pub b: DVec3,
    pub sides: Vec<GuideSide>,
}

/// The elements of a model an edit names.
#[derive(Clone, Debug, Default)]
pub struct Targets {
    pub vertices: BTreeSet<prism_topo::VertexId>,
    pub edges: BTreeSet<EdgeId>,
    pub faces: BTreeSet<prism_topo::FaceId>,
}

impl Targets {
    fn all_in(&self, m: &Model) -> bool {
        self.vertices.iter().all(|v| m.has_vertex(*v))
            && self.edges.iter().all(|e| m.edge(*e).is_some())
            && self.faces.iter().all(|f| m.face(*f).is_some())
    }
}

/// What a model looks like, whatever its ids: where its corners are, how
/// many edges and faces it has, and how many edges are soft or smooth and
/// how many faces are painted. Two runs of the same edit that give the
/// same shape give the same signature.
type Signature = (Vec<[i64; 3]>, [usize; 5]);

fn signature(m: &Model) -> Signature {
    let q = |x: f64| (x * 1e6).round() as i64;
    let mut v: Vec<[i64; 3]> = m
        .vertices()
        .map(|(_, p)| [q(p.x), q(p.y), q(p.z)])
        .collect();
    v.sort_unstable();
    let soft = m.edges().filter(|(_, e)| e.soft).count();
    let smooth = m.edges().filter(|(_, e)| e.smooth).count();
    let painted = m
        .faces()
        .filter(|(_, f)| f.paint.front.is_some() || f.paint.back.is_some())
        .count();
    (v, [m.edge_count(), m.face_count(), soft, smooth, painted])
}

impl Stack {
    /// What a drag handle for feature `index`'s size needs: the first edge
    /// it works on (where it was, before the feature ran) and the flat faces
    /// beside that edge. None for a broken feature, a curved edge, or an
    /// edge with no flat face beside it.
    pub fn size_guide(&self, index: usize) -> Option<SizeGuide> {
        let feature = self.features.get(index)?;
        if feature.is_broken() {
            return None;
        }
        let mut m = self.base.clone();
        for (i, f) in self.features.iter().take(index).enumerate() {
            f.clone().run(&mut m, i == 0);
        }
        let e = edge_at(&m, feature.refs().first()?)?;
        if m.edge(e)?.curve.is_curved() {
            return None;
        }
        let (a, b) = ends(&m, e)?;
        let dir = (b - a).try_normalize()?;
        let mid = (a + b) * 0.5;
        let by_edge = m.edge_faces();
        let sides: Vec<GuideSide> = by_edge
            .get(&e)?
            .iter()
            .filter_map(|f| {
                let face = m.face(*f)?;
                let plane = face.plane()?;
                let pts = m.loop_points(face, 0);
                let centre = pts.iter().copied().sum::<DVec3>() / pts.len().max(1) as f64;
                let w = centre - mid;
                let across = (w - dir * w.dot(dir)).try_normalize()?;
                Some(GuideSide {
                    normal: plane.normal,
                    origin: plane.origin,
                    across,
                })
            })
            .collect();
        (!sides.is_empty()).then_some(SizeGuide { a, b, sides })
    }

    /// The faces each feature made (ids in the model as drawn): what each
    /// one adds when it runs. A broken feature made none.
    pub fn made_faces(&self) -> Vec<BTreeSet<FaceId>> {
        let mut m = self.base.clone();
        let mut out = Vec::new();
        for (i, f) in self.features.iter().enumerate() {
            let before: BTreeSet<FaceId> = m.faces().map(|(id, _)| id).collect();
            f.clone().run(&mut m, i == 0);
            out.push(
                m.faces()
                    .map(|(id, _)| id)
                    .filter(|id| !before.contains(id))
                    .collect(),
            );
        }
        out
    }

    /// Bake feature `index`, and the ones before it (they made what it
    /// works on).
    pub fn explode(&self, index: usize) -> Stack {
        self.bake_first((index + 1).min(self.features.len()))
    }

    /// The first `j` features turned into geometry: the base is what they
    /// make, and the rest still run on it.
    fn bake_first(&self, j: usize) -> Stack {
        let mut head = Stack {
            base: self.base.clone(),
            features: self.features[..j].to_vec(),
        };
        Stack {
            base: head.evaluate(),
            features: self.features[j..].to_vec(),
        }
    }

    /// A direct edit of the body as drawn (`shown`). It goes to the base
    /// and the features run again. If that does not give what the edit
    /// does to the drawn model (it names a face, edge or corner a feature
    /// made, which the base does not have), the fewest features that
    /// must be baked first are baked. Returns the stack, the model to
    /// draw, and how many features were baked.
    pub fn edit(
        &self,
        shown: &Model,
        targets: Option<Targets>,
        edit: impl Fn(&mut Model),
    ) -> (Stack, Model, usize) {
        let n = self.features.len();
        let clean = !self.features.iter().any(Feature::is_broken);
        // An edit that names its elements goes under the fewest features
        // whose base has them all.
        if let Some(t) = targets {
            for j in 0..=n {
                let mut s = if j == 0 {
                    self.clone()
                } else {
                    self.bake_first(j)
                };
                if j == n || t.all_in(&s.base) {
                    edit(&mut s.base);
                    if j == n {
                        let m = s.base.clone();
                        return (Stack::new(m.clone()), m, n);
                    }
                    let m = s.evaluate();
                    // A feature that cannot follow the edit is baked too,
                    // rather than lost.
                    if clean && s.features.iter().any(Feature::is_broken) {
                        continue;
                    }
                    return (s, m, j);
                }
            }
        }
        let mut want = shown.clone();
        edit(&mut want);
        let sig = signature(&want);
        for j in 0..n {
            let mut s = if j == 0 {
                self.clone()
            } else {
                self.bake_first(j)
            };
            edit(&mut s.base);
            let m = s.evaluate();
            if signature(&m) == sig {
                return (s, m, j);
            }
        }
        (Stack::new(want.clone()), want, n)
    }
}

/// The faces of each connected solid of `m`: faces that share an edge are
/// in the same one.
fn shells(m: &Model) -> Vec<BTreeSet<FaceId>> {
    let by_edge = m.edge_faces();
    let mut seen: BTreeSet<FaceId> = BTreeSet::new();
    let mut out = Vec::new();
    for (start, _) in m.faces() {
        if seen.contains(&start) {
            continue;
        }
        let mut shell = BTreeSet::new();
        let mut todo = vec![start];
        while let Some(f) = todo.pop() {
            if !shell.insert(f) {
                continue;
            }
            if let Some(face) = m.face(f) {
                for e in face.edge_ids() {
                    todo.extend(by_edge.get(&e).into_iter().flatten().copied());
                }
            }
        }
        seen.extend(&shell);
        out.push(shell);
    }
    out
}

/// The edge of `m` whose ends are where `r`'s were.
fn edge_at(m: &Model, r: &EdgeRef) -> Option<EdgeId> {
    m.edges().map(|(e, _)| e).find(|e| same_place(m, *e, r))
}

impl Feature {
    /// Make a live boolean a different one (Union, Subtract, Intersect).
    /// False if it is not a boolean or already is that.
    pub fn set_bool_kind(&mut self, new: BoolKind) -> bool {
        match self {
            Feature::Boolean { op, .. } if *op != new => {
                *op = new;
                true
            }
            _ => false,
        }
    }

    /// The box round a boolean's second solid, in the body's own
    /// coordinates.
    pub fn tool_box(&self) -> Option<(DVec3, DVec3)> {
        match self {
            Feature::Boolean { tool, .. } => tool.bbox(),
            Feature::Round { .. } | Feature::Shell { .. } => None,
        }
    }

    /// Move and stretch a boolean's second solid so its box is `min..max`.
    /// False if it is not a boolean, has no volume, or the new box has none.
    pub fn set_tool_box(&mut self, min: DVec3, max: DVec3) -> bool {
        let Some((lo, hi)) = self.tool_box() else {
            return false;
        };
        let (from, to) = (hi - lo, max - min);
        if from.min_element() <= 0.0 || to.min_element() <= 0.0 {
            return false;
        }
        let by = DAffine3::from_translation(min)
            * DAffine3::from_scale(to / from)
            * DAffine3::from_translation(-lo);
        if let Feature::Boolean { tool, .. } = self {
            prism_ops::transform::affine(tool, &by);
        }
        true
    }

    /// The feature's size: a fillet's radius or a chamfer's distance. None
    /// for a feature with no size.
    pub fn size(&self) -> Option<f64> {
        match self {
            Feature::Round { size, .. } => Some(*size),
            Feature::Shell { thickness, .. } => Some(*thickness),
            Feature::Boolean { .. } => None,
        }
    }

    fn refs(&self) -> &[EdgeRef] {
        match self {
            Feature::Round { edges, .. } => edges,
            Feature::Boolean { .. } | Feature::Shell { .. } => &[],
        }
    }

    fn refs_mut(&mut self) -> &mut [EdgeRef] {
        match self {
            Feature::Round { edges, .. } => edges,
            Feature::Boolean { .. } | Feature::Shell { .. } => &mut [],
        }
    }

    /// The faces it points at (a shell's).
    fn face_refs_mut(&mut self) -> Vec<&mut FaceRef> {
        match self {
            Feature::Shell { faces, open, .. } => faces.iter_mut().chain(open.iter_mut()).collect(),
            Feature::Round { .. } | Feature::Boolean { .. } => Vec::new(),
        }
    }
}

impl Stack {
    /// Move the base and where the features point, as a group is placed.
    pub fn moved(&mut self, by: &DAffine3) {
        prism_ops::transform::affine(&mut self.base, by);
        for f in &mut self.features {
            for r in f.refs_mut() {
                r.a = by.transform_point3(r.a);
                r.b = by.transform_point3(r.b);
            }
            for r in f.face_refs_mut() {
                r.centre = by.transform_point3(r.centre);
                r.normal = by.transform_vector3(r.normal).normalize_or_zero();
            }
            if let Feature::Boolean { tool, .. } = f {
                prism_ops::transform::affine(tool, by);
            }
        }
    }

    /// Take on features from another body, whose base has been joined to
    /// this one's: their edges are found again here by place.
    pub fn adopt(&mut self, features: Vec<Feature>) {
        for mut f in features {
            for r in f.refs_mut() {
                if let Some(id) = edge_at(&self.base, r) {
                    r.id = id;
                }
            }
            for r in f.face_refs_mut() {
                if let Some(id) = face_at(&self.base, r) {
                    r.id = id;
                }
            }
            self.features.push(f);
        }
    }

    /// Where each feature's edges are in `m` (found by place): their ids
    /// there. None if one is not.
    fn reref(features: &[Feature], m: &Model) -> Option<Vec<Feature>> {
        let mut out = features.to_vec();
        for f in &mut out {
            for r in f.refs_mut() {
                r.id = edge_at(m, r)?;
            }
            for r in f.face_refs_mut() {
                r.id = face_at(m, r)?;
            }
        }
        Some(out)
    }

    /// Make Group: take the solids `sel` selects out of the body, with
    /// their features. Returns the shape taken (as drawn) and, if features
    /// went with it, the stack for the new group. What stays keeps its
    /// features and is not drawn from the taken part. Features a solid is
    /// only partly selected for are baked first. `self.base` and
    /// `self.features` are what stays (the caller draws them again).
    pub fn take(&mut self, shown: &Model, sel: &Selection) -> (Model, Option<Stack>) {
        // Which solid each feature is on, and how much of it is selected.
        let (solid_of, carried_solids) = loop {
            let solids = shells(&self.base);
            let in_solid = |r: &EdgeRef| {
                let e = self
                    .base
                    .edge(r.id)
                    .filter(|_| same_place(&self.base, r.id, r));
                let e = e
                    .is_some()
                    .then_some(r.id)
                    .or_else(|| edge_at(&self.base, r))?;
                let faces = self.base.edge_faces();
                let f = faces.get(&e)?.first().copied()?;
                solids.iter().position(|s| s.contains(&f))
            };
            let solid_of: Vec<Option<usize>> = self
                .features
                .iter()
                .map(|f| {
                    let mut which = f.refs().iter().map(in_solid);
                    let first = which.next().flatten()?;
                    which.all(|w| w == Some(first)).then_some(first)
                })
                .collect();
            let status = |k: usize| {
                let shown_here: BTreeSet<FaceId> = solids[k]
                    .iter()
                    .copied()
                    .filter(|f| shown.face(*f).is_some())
                    .collect();
                let chosen = shown_here.iter().filter(|f| sel.faces.contains(f)).count();
                (chosen, shown_here.len())
            };
            let bad = solid_of.iter().rposition(|w| match w {
                None => true,
                Some(k) => {
                    let (chosen, all) = status(*k);
                    chosen != 0 && chosen != all
                }
            });
            match bad {
                Some(i) => *self = self.bake_first(i + 1),
                None => {
                    let carried: BTreeSet<usize> = solids
                        .iter()
                        .enumerate()
                        .filter(|(k, _)| {
                            solid_of.contains(&Some(*k)) && {
                                let (chosen, all) = status(*k);
                                chosen == all && all > 0
                            }
                        })
                        .map(|(k, _)| k)
                        .collect();
                    break (
                        solid_of,
                        carried
                            .into_iter()
                            .map(|k| (k, solids[k].clone()))
                            .collect::<BTreeMap<_, _>>(),
                    );
                }
            }
        };
        let mut base_sel = Selection {
            vertices: sel
                .vertices
                .iter()
                .copied()
                .filter(|v| self.base.has_vertex(*v))
                .collect(),
            edges: sel
                .edges
                .iter()
                .copied()
                .filter(|e| self.base.edge(*e).is_some())
                .collect(),
            faces: sel
                .faces
                .iter()
                .copied()
                .filter(|f| self.base.face(*f).is_some())
                .collect(),
            guides: sel.guides.clone(),
        };
        for faces in carried_solids.values() {
            base_sel.faces.extend(faces);
        }
        let taken = prism_ops::select::take(&mut self.base, &base_sel);
        let (moved, stay): (Vec<_>, Vec<_>) = self
            .features
            .drain(..)
            .zip(solid_of)
            .partition(|(_, k)| k.is_some_and(|k| carried_solids.contains_key(&k)));
        self.features = stay.into_iter().map(|(f, _)| f).collect();
        let moved: Vec<Feature> = moved.into_iter().map(|(f, _)| f).collect();
        if moved.is_empty() {
            return (taken, None);
        }
        match Stack::reref(&moved, &taken) {
            Some(features) => {
                let mut s = Stack {
                    base: taken,
                    features,
                };
                let model = s.evaluate();
                (model, Some(s))
            }
            // Not found in what was taken: it goes without them.
            None => (taken, None),
        }
    }
}
