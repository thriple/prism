//! The document: the geometry, and every edit as a command in a history, so
//! undo, redo, autosave and test replay all come from one log.
//!
//! The geometry is an [`Assembly`]: the root model and its groups. Each step
//! keeps the assembly before and after; definitions it did not change are
//! shared, not copied. Undo and redo put a snapshot back exactly. Id
//! counters are never wound back, so an id is never handed out twice.

use std::collections::BTreeSet;

pub mod assembly;
pub mod image;
pub mod material;
pub mod stack;

pub use assembly::{
    Anchor, Assembly, Attributes, DefId, Definition, DimId, Dimension, Instance, InstanceId,
    NESTED, Placed, ROOT, Section, SectionId, SolidOp, TWO_SOLIDS, Tag, TagId, WorkPlane,
    WorkPlaneId,
};
pub use image::{Image, ImageId, Pixels, Texture};
pub use material::{FaceSide, Material, Principled};
use prism_math::{DAffine3, DQuat, DVec3, Plane};
use prism_ops::select::Selection;
use prism_ops::{pushpull, transform};
use prism_topo::{FaceId, Guide, Model, VertexId};
use serde::{Deserialize, Serialize};
pub use stack::{
    BoolKind, EdgeRef, FaceRef, Feature, GuideSide, RimBoundary, RimChoice, RimReport, SizeGuide,
    Stack, edge_at, resolve_faces,
};

/// One edit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Segment {
        p: DVec3,
        q: DVec3,
    },
    Rect {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        z: f64,
    },
    /// A closed loop of segments through these points, in one step (a
    /// rectangle on any plane).
    Loop {
        points: Vec<DVec3>,
    },
    /// An exact circle: two half arcs with one curve id (Exact Curves).
    /// An arc (the Arc tools): from `start`, counter-clockwise about
    /// `normal` by `sweep` radians, on the circle about `centre`. A pie also
    /// has the two radius lines.
    Arc {
        centre: DVec3,
        normal: DVec3,
        radius: f64,
        start: DVec3,
        sweep: f64,
        pie: bool,
    },
    /// A NURBS curve (the Curve tool, Freeform NURBS).
    Spline {
        curve: prism_geom::Nurbs,
    },
    /// Send control points of a NURBS curve or surface through `m` (cage
    /// editing, Freeform NURBS part 3). `points` are places in
    /// `prism_ops::cage::points`.
    MoveCage {
        cage: prism_ops::cage::Cage,
        points: std::collections::BTreeSet<usize>,
        m: DAffine3,
    },
    /// Add control points to a NURBS curve or surface, keeping its shape.
    AddCagePoints {
        cage: prism_ops::cage::Cage,
    },
    /// A surface through the curves these edges are on (Loft, Freeform
    /// NURBS part 4).
    Loft {
        edges: std::collections::BTreeSet<prism_topo::EdgeId>,
    },
    /// The curves these edges are on turned about the one line among them
    /// that touches none of the others (Revolve).
    Revolve {
        edges: std::collections::BTreeSet<prism_topo::EdgeId>,
    },
    /// Two curves swept: one along the other (Sweep, Freeform NURBS part
    /// 5).
    Sweep {
        edges: std::collections::BTreeSet<prism_topo::EdgeId>,
    },
    /// The surface inside a ring of curves (Patch).
    Patch {
        edges: std::collections::BTreeSet<prism_topo::EdgeId>,
    },
    /// Shell / Thicken (brief 42): the bodies of `faces` made `thickness`
    /// thick (out along their normals for more than zero, in for less),
    /// with `open` left open.
    Shell {
        faces: std::collections::BTreeSet<FaceId>,
        open: std::collections::BTreeSet<FaceId>,
        thickness: f64,
    },
    /// The Loft tool's surface: sides picked by hand (each a run of edges,
    /// in the order picked) filled as a ring or lofted through (brief 34).
    LoftSides {
        sides: Vec<Vec<prism_topo::EdgeId>>,
        preset: prism_ops::sides::Preset,
    },
    /// Round the corner at `corner` with an arc of `radius` (the Arc tools'
    /// double-click).
    Fillet {
        corner: DVec3,
        radius: f64,
    },
    /// A polygon through `points` (the Polygon tool), its sides one curve.
    Polygon {
        points: Vec<DVec3>,
    },
    Circle {
        centre: DVec3,
        normal: DVec3,
        radius: f64,
    },
    Erase {
        selection: Selection,
    },
    PushPull {
        face: FaceId,
        distance: f64,
        new_loop: bool,
    },
    Translate {
        vertices: BTreeSet<VertexId>,
        by: DVec3,
    },
    Rotate {
        vertices: BTreeSet<VertexId>,
        centre: DVec3,
        axis: DVec3,
        angle: f64,
    },
    Scale {
        vertices: BTreeSet<VertexId>,
        centre: DVec3,
        factor: DVec3,
    },
    /// Copies of a selection, each moved or rotated one step further than
    /// the last (`count` 1 is a plain copy; more is an array), in one step.
    Copy {
        selection: Selection,
        step: Motion,
        count: u32,
    },
    /// A mirrored copy of a selection across the plane through `origin`
    /// with this `normal` (Shapes & Placement part 1).
    MirrorCopy {
        selection: Selection,
        origin: DVec3,
        normal: DVec3,
    },
    /// Offset a face's loops (positive into the face).
    Offset {
        face: FaceId,
        distance: f64,
    },
    /// Add a guide line or point.
    AddGuide {
        guide: Guide,
    },
    /// Add imported geometry beside the model (not joined to it).
    Import {
        model: Model,
    },
    /// Turn where faces cross into edges (all faces, or those crossings
    /// that involve a face in `faces`).
    IntersectFaces {
        faces: Option<BTreeSet<FaceId>>,
    },
    /// Model Health (PLAN 1.10): turn faces round so each shell agrees and
    /// faces out (all, or only these).
    OrientFaces {
        faces: Option<BTreeSet<FaceId>>,
    },
    /// Solid > Convert > Curve Chain ↔ NURBS Curve (brief 43): a chain of
    /// lines and arcs becomes one NURBS curve, or one NURBS curve a chain.
    ConvertCurve {
        edges: BTreeSet<prism_topo::EdgeId>,
    },
    /// Solid > Convert > Faces ↔ Smooth Patch (brief 43): a grid of
    /// four-sided faces becomes one freeform face, or one a grid of flat
    /// faces.
    ConvertFaces {
        faces: BTreeSet<FaceId>,
    },
    /// Edit > Weld (brief 43): these edges, joined end to end, become one
    /// curve.
    Weld {
        edges: BTreeSet<prism_topo::EdgeId>,
    },
    /// Soften and smooth these edges, or (with `soft` false) make them
    /// plain again (Exact Curves 4c).
    Soften {
        edges: BTreeSet<prism_topo::EdgeId>,
        soft: bool,
    },
    /// Turn these faces round.
    ReverseFaces {
        faces: BTreeSet<FaceId>,
    },
    /// Remove interior faces (all, or only these).
    CleanEnvelope {
        faces: Option<BTreeSet<FaceId>>,
    },
    /// Remove edges in no face, and points left on nothing.
    RemoveStray,
    /// Move the ends of arcs that left their circle back onto it.
    RepairArcs,
    /// A command run inside an open group (the path of instances to it).
    In {
        context: Vec<InstanceId>,
        command: Box<Command>,
    },
    /// Several commands as one step (geometry and groups selected together).
    Batch {
        commands: Vec<Command>,
    },
    /// Make Group from a selection and selected groups.
    MakeGroup {
        selection: Selection,
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        instances: BTreeSet<InstanceId>,
    },
    /// Explode a group into its parent.
    Explode {
        instance: InstanceId,
    },
    /// Make Component from a selection and selected groups.
    MakeComponent {
        selection: Selection,
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        instances: BTreeSet<InstanceId>,
        name: String,
    },
    /// Give each instance its own copy of its component.
    MakeUnique {
        instances: BTreeSet<InstanceId>,
    },
    /// Rename a component.
    Rename {
        def: DefId,
        name: String,
    },
    /// Hide or show instances (anywhere: instance ids are unique).
    SetHidden {
        instances: BTreeSet<InstanceId>,
        hidden: bool,
    },
    /// Name an instance (empty clears it).
    NameInstance {
        instance: InstanceId,
        name: String,
    },
    /// Add a tag.
    AddTag {
        name: String,
    },
    RenameTag {
        tag: TagId,
        name: String,
    },
    /// Show or hide everything on a tag.
    SetTagVisible {
        tag: TagId,
        visible: bool,
    },
    /// Put instances on a tag (None: on no tag).
    SetTag {
        instances: BTreeSet<InstanceId>,
        tag: Option<TagId>,
    },
    /// Put instances on a tag as well (`on`), or take them off it; their
    /// other tags stay.
    TagOn {
        instances: BTreeSet<InstanceId>,
        tag: TagId,
        on: bool,
    },
    /// Give instances exactly these tags (none: no tag).
    SetTags {
        instances: BTreeSet<InstanceId>,
        tags: BTreeSet<TagId>,
    },
    /// Give a group or component new axes (brief 36): `axes` in its
    /// definition's own coordinates. Nothing moves in the world.
    ChangeAxes {
        instance: InstanceId,
        axes: DAffine3,
    },
    /// The model's own axes (Annotation & Sections part 4): `axes`, a rigid
    /// frame in world coordinates, becomes the world's origin and
    /// directions. Everything moves with it, one undo step.
    ChangeModelAxes {
        axes: DAffine3,
    },
    /// Move, rotate or scale whole groups.
    Place {
        instances: BTreeSet<InstanceId>,
        change: Change,
    },
    /// Copies of groups, each one step further than the last.
    CopyInstances {
        instances: BTreeSet<InstanceId>,
        step: Change,
        count: u32,
    },
    /// Erase groups.
    EraseInstances {
        instances: BTreeSet<InstanceId>,
    },
    /// Cut: erase a selection and groups (after they were copied).
    Cut {
        selection: Selection,
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        instances: BTreeSet<InstanceId>,
    },
    /// Paste a clip (an assembly of its own, made by `Assembly::clip`),
    /// moved by `at` from the clip's coordinates.
    Paste {
        clip: Box<Assembly>,
        at: DAffine3,
    },
    /// A new section plane (Annotation & Sections part 1), in world
    /// coordinates; the side `normal` points to is cut away. It becomes
    /// the plane that cuts.
    AddSection {
        origin: DVec3,
        normal: DVec3,
    },
    MoveSection {
        id: SectionId,
        origin: DVec3,
    },
    /// A new construction plane (Annotation & Sections part 4), in world
    /// coordinates. It becomes the working plane the drawing tools use.
    AddWorkPlane {
        origin: DVec3,
        normal: DVec3,
    },
    MoveWorkPlane {
        id: WorkPlaneId,
        origin: DVec3,
    },
    /// Make the plane the working plane, or stop it being one.
    SetWorkPlaneActive {
        id: WorkPlaneId,
        active: bool,
    },
    RenameWorkPlane {
        id: WorkPlaneId,
        name: String,
    },
    DeleteWorkPlane {
        id: WorkPlaneId,
    },
    /// Cut away the other side.
    FlipSection {
        id: SectionId,
    },
    /// Make the plane the one that cuts, or stop it cutting.
    SetSectionActive {
        id: SectionId,
        active: bool,
    },
    RenameSection {
        id: SectionId,
        name: String,
    },
    DeleteSection {
        id: SectionId,
    },
    /// Text (Annotation & Sections part 3), set in one of the bundled
    /// fonts with capitals `height` tall and made into a new group
    /// (named after the text) whose axes are the text's: `at` places it,
    /// `x` along the line, `y` up, facing `z`.
    AddText {
        text: String,
        font: prism_text::Font,
        height: f64,
        mode: prism_ops::text::TextMode,
        at: DAffine3,
    },
    /// A new linear dimension (Annotation & Sections part 2), in world
    /// coordinates.
    AddDimension {
        a: DVec3,
        b: DVec3,
        offset: DVec3,
    },
    MoveDimension {
        id: DimId,
        offset: DVec3,
    },
    DeleteDimension {
        id: DimId,
    },
    /// Attach a dimension's ends to vertices, so they follow the model.
    AnchorDimension {
        id: DimId,
        a: Option<Anchor>,
        b: Option<Anchor>,
    },
    /// Lock or unlock groups (anywhere: instance ids are unique).
    SetLocked {
        instances: BTreeSet<InstanceId>,
        locked: bool,
    },
    /// Intersect Faces > With Context: where the open group's faces cross
    /// `others` (the geometry outside it, in its coordinates), edges are
    /// added to the open group.
    IntersectWith {
        others: Model,
    },
    /// Round a solid's edge (`fillet`, a radius) or cut it off (a chamfer,
    /// how far into each face).
    RoundEdge {
        edge: prism_topo::EdgeId,
        size: f64,
        fillet: bool,
    },
    /// Round many edges of one closed solid at once through the kernel,
    /// with clean corners where they meet (Fillet Joins, brief 32).
    RoundEdges {
        edges: BTreeSet<prism_topo::EdgeId>,
        size: f64,
        fillet: bool,
    },
    /// Change a live feature's size (Inspector part 2); the features run
    /// again on the base. `def` is the group's definition, or the root for
    /// loose geometry.
    SetFeatureSize {
        def: DefId,
        index: usize,
        size: f64,
    },
    /// Move and stretch a live boolean's second solid so its box is
    /// `min..max`, in the body's own coordinates (Inspector part 3b).
    SetCutter {
        def: DefId,
        index: usize,
        min: prism_math::DVec3,
        max: prism_math::DVec3,
    },
    /// Point a live round at other edges, given as they are now (Inspector
    /// part 4b). They are found by place before the round runs.
    SetFeatureEdges {
        def: DefId,
        index: usize,
        edges: Vec<EdgeRef>,
    },
    /// How the wall at one open boundary of a live shell ends (brief 49
    /// part 3): `edge` is any edge on that boundary, found by place; `None`
    /// hands the boundary back to the program.
    SetRimStyle {
        def: DefId,
        index: usize,
        edge: EdgeRef,
        style: Option<prism_ops::shelling::RimStyle>,
    },
    /// Make a live boolean another kind (Inspector part 3b-2).
    SetBoolKind {
        def: DefId,
        index: usize,
        op: BoolKind,
    },
    /// Take a live feature off.
    RemoveFeature {
        def: DefId,
        index: usize,
    },
    /// Turn one live feature, and those before it, into plain geometry.
    ExplodeFeature {
        def: DefId,
        index: usize,
    },
    /// Turn the live features into plain geometry.
    BakeFeatures {
        def: DefId,
    },
    /// Sweep a profile face along a path of lines (Follow Me, Exact Curves
    /// 5c).
    FollowMe {
        path: BTreeSet<prism_topo::EdgeId>,
        profile: FaceId,
        /// Stop partway along the path (Follow Me dragged to a point on an
        /// edge): the path is cut there first.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end: Option<DVec3>,
    },
    /// Add a material to the document (its name made unique).
    AddMaterial {
        material: Material,
    },
    /// Change a material.
    EditMaterial {
        id: prism_topo::MaterialId,
        material: Material,
    },
    /// Remove a material: what used it goes back to Default.
    RemoveMaterial {
        id: prism_topo::MaterialId,
    },
    /// Remove every material nothing uses (and every image no material
    /// uses).
    PurgeMaterials,
    /// Keep an image in the document (part 2), for a material's texture.
    /// An image already there (the same bytes) is kept once.
    AddImage {
        image: crate::image::Image,
    },
    /// Paint face sides (None: back to Default).
    Paint {
        sides: BTreeSet<(FaceId, FaceSide)>,
        material: Option<prism_topo::MaterialId>,
    },
    /// Place the texture on flat faces (part 3, Texture > Position): their
    /// placement, or None for the default (Reset Position).
    PlaceTexture {
        faces: BTreeSet<FaceId>,
        map: Option<prism_topo::UvMap>,
    },
    /// Paint groups and components as a whole.
    PaintInstances {
        instances: BTreeSet<InstanceId>,
        material: Option<prism_topo::MaterialId>,
    },
    /// Replace one material by another, in the open group or everywhere.
    ReplaceMaterial {
        from: Option<prism_topo::MaterialId>,
        to: Option<prism_topo::MaterialId>,
        everywhere: bool,
    },
    /// Solid > Validate and Heal's Heal (Solids & CAD part 1): the pieces
    /// holding these faces, or all of them.
    Heal {
        faces: Option<BTreeSet<FaceId>>,
    },
    /// A boolean of two groups in the open one (Solids & CAD part 2).
    Boolean {
        op: SolidOp,
        first: InstanceId,
        second: InstanceId,
    },
    /// The union of these groups, keeping only their outside (Union of three
    /// or more; once its own Outer Shell command).
    OuterShell {
        instances: BTreeSet<InstanceId>,
    },
}

/// How groups are moved, turned or scaled.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Change {
    Translate {
        by: DVec3,
    },
    Rotate {
        centre: DVec3,
        axis: DVec3,
        angle: f64,
    },
    Scale {
        centre: DVec3,
        factor: DVec3,
    },
    /// A reflection in the plane through `origin` with this `normal`.
    /// Applied an even number of times it is no change.
    Mirror {
        origin: DVec3,
        normal: DVec3,
    },
}

impl Change {
    /// The change applied `k` times.
    pub fn affine(&self, k: f64) -> DAffine3 {
        match *self {
            Change::Translate { by } => DAffine3::from_translation(by * k),
            Change::Rotate {
                centre,
                axis,
                angle,
            } => {
                DAffine3::from_translation(centre)
                    * DAffine3::from_quat(DQuat::from_axis_angle(axis.normalize(), angle * k))
                    * DAffine3::from_translation(-centre)
            }
            Change::Scale { centre, factor } => {
                DAffine3::from_translation(centre)
                    * DAffine3::from_scale(factor.powf(k))
                    * DAffine3::from_translation(-centre)
            }
            Change::Mirror { origin, normal } => {
                if (k.round() as i64) % 2 == 0 {
                    return DAffine3::IDENTITY;
                }
                let n = normal.normalize_or_zero();
                // I - 2 n nT about the origin point.
                let m = prism_math::DMat3::from_cols(
                    DVec3::X - n * (2.0 * n.x),
                    DVec3::Y - n * (2.0 * n.y),
                    DVec3::Z - n * (2.0 * n.z),
                );
                DAffine3::from_translation(origin)
                    * DAffine3::from_mat3(m)
                    * DAffine3::from_translation(-origin)
            }
        }
    }
}

/// A change of groups, in a group's own coordinates.
fn change_to_local(c: &Change, to_local: &DAffine3) -> Change {
    match *c {
        Change::Translate { by } => Change::Translate {
            by: to_local.transform_vector3(by),
        },
        Change::Rotate {
            centre,
            axis,
            angle,
        } => Change::Rotate {
            centre: to_local.transform_point3(centre),
            axis: to_local.transform_vector3(axis).normalize(),
            angle,
        },
        Change::Scale { centre, factor } => Change::Scale {
            centre: to_local.transform_point3(centre),
            factor,
        },
        Change::Mirror { origin, normal } => Change::Mirror {
            origin: to_local.transform_point3(origin),
            // A normal moves by the inverse transpose.
            normal: (to_local.matrix3.inverse().transpose() * normal).normalize_or_zero(),
        },
    }
}

impl From<Motion> for Change {
    fn from(m: Motion) -> Change {
        match m {
            Motion::Translate { by } => Change::Translate { by },
            Motion::Rotate {
                centre,
                axis,
                angle,
            } => Change::Rotate {
                centre,
                axis,
                angle,
            },
        }
    }
}

/// How a copy is placed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Motion {
    Translate {
        by: DVec3,
    },
    Rotate {
        centre: DVec3,
        axis: DVec3,
        angle: f64,
    },
}

impl Command {
    /// A short name for the Edit menu ("Undo Push/Pull").
    pub fn name(&self) -> &'static str {
        match self {
            Command::Segment { .. } => "Line",
            Command::Rect { .. } => "Rectangle",
            Command::Loop { points } if points.len() == 4 => "Rectangle",
            Command::Loop { .. } => "Shape",
            Command::Circle { .. } => "Circle",
            Command::Spline { .. } => "Curve",
            Command::MoveCage { .. } => "Move Control Points",
            Command::Loft { .. } => "Loft",
            Command::Shell { .. } => "Shell / Thicken",
            Command::Revolve { .. } => "Revolve",
            Command::Sweep { .. } => "Sweep",
            Command::Patch { .. } => "Patch",
            Command::LoftSides { .. } => "Loft",
            Command::AddCagePoints { .. } => "Add Control Points",
            Command::Polygon { .. } => "Polygon",
            Command::Arc { pie: true, .. } => "Pie",
            Command::Arc { .. } => "Arc",
            Command::Fillet { .. } => "Fillet",
            Command::Erase { .. } => "Erase",
            Command::PushPull { .. } => "Push/Pull",
            Command::Translate { .. } => "Move",
            Command::Rotate { .. } => "Rotate",
            Command::Scale { .. } => "Scale",
            Command::Copy { count, .. } if *count > 1 => "Array",
            Command::Copy { .. } => "Copy",
            Command::MirrorCopy { .. } => "Mirror Copy",
            Command::Offset { .. } => "Offset",
            Command::AddGuide { .. } => "Guide",
            Command::Import { .. } => "Import",
            Command::IntersectFaces { .. } => "Intersect Faces",
            Command::In { command, .. } => command.name(),
            Command::OrientFaces { .. } => "Orient Faces",
            Command::ReverseFaces { .. } => "Reverse Faces",
            Command::Weld { .. } => "Weld",
            Command::ConvertCurve { .. } => "Convert Curve",
            Command::ConvertFaces { .. } => "Convert Faces",
            Command::Soften { soft: true, .. } => "Soften/Smooth",
            Command::Soften { .. } => "Unsoften",
            Command::CleanEnvelope { .. } => "Clean Envelope",
            Command::RemoveStray => "Remove Stray Edges",
            Command::RepairArcs => "Repair Arcs",
            Command::Heal { .. } => "Heal",
            Command::Boolean { op, .. } => match op {
                SolidOp::Union => "Union",
                SolidOp::Intersect => "Intersect",
                SolidOp::Subtract => "Subtract",
                SolidOp::Trim => "Trim",
                SolidOp::Split => "Split",
            },
            Command::OuterShell { .. } => "Union",
            // Materials and images added for the step are named by the
            // step (a material on its own by the material).
            Command::Batch { commands } => {
                let adding = |c: &&Command| {
                    matches!(c, Command::AddMaterial { .. } | Command::AddImage { .. })
                };
                commands
                    .iter()
                    .find(|c| !adding(c))
                    .or_else(|| {
                        commands
                            .iter()
                            .find(|c| matches!(c, Command::AddMaterial { .. }))
                    })
                    .or(commands.first())
                    .map_or("Edit", |c| c.name())
            }
            Command::MakeGroup { .. } => "Make Group",
            Command::Explode { .. } => "Explode",
            Command::MakeComponent { .. } => "Make Component",
            Command::MakeUnique { .. } => "Make Unique",
            Command::ChangeAxes { .. } => "Change Axes",
            Command::ChangeModelAxes { .. } => "Change Model Axes",
            Command::Rename { .. } | Command::RenameTag { .. } => "Rename",
            Command::SetHidden { hidden: true, .. } => "Hide",
            Command::SetHidden { .. } => "Unhide",
            Command::NameInstance { .. } => "Name",
            Command::AddTag { .. } => "Add Tag",
            Command::SetTagVisible { .. } => "Tag Visibility",
            Command::SetTag { .. } | Command::TagOn { .. } | Command::SetTags { .. } => "Tag",
            Command::Place { change, .. } => match change {
                Change::Translate { .. } => "Move",
                Change::Rotate { .. } => "Rotate",
                Change::Scale { .. } => "Scale",
                Change::Mirror { .. } => "Mirror",
            },
            Command::CopyInstances {
                step: Change::Mirror { .. },
                ..
            } => "Mirror Copy",
            Command::CopyInstances { count, .. } if *count > 1 => "Array",
            Command::CopyInstances { .. } => "Copy",
            Command::EraseInstances { .. } => "Erase",
            Command::Cut { .. } => "Cut",
            Command::Paste { .. } => "Paste",
            Command::AddSection { .. } => "Section Plane",
            Command::MoveSection { .. } => "Move Section Plane",
            Command::AddWorkPlane { .. } => "Construction Plane",
            Command::MoveWorkPlane { .. } => "Move Construction Plane",
            Command::SetWorkPlaneActive { active: true, .. } => "Use Construction Plane",
            Command::SetWorkPlaneActive { .. } => "Stop Using Construction Plane",
            Command::RenameWorkPlane { .. } => "Rename Construction Plane",
            Command::DeleteWorkPlane { .. } => "Delete Construction Plane",
            Command::FlipSection { .. } => "Flip Section Plane",
            Command::SetSectionActive { active: true, .. } => "Activate Section Plane",
            Command::SetSectionActive { .. } => "Deactivate Section Plane",
            Command::RenameSection { .. } => "Rename Section Plane",
            Command::DeleteSection { .. } => "Delete Section Plane",
            Command::AddText { .. } => "Text",
            Command::AddDimension { .. } => "Dimension",
            Command::MoveDimension { .. } => "Move Dimension",
            Command::DeleteDimension { .. } => "Delete Dimension",
            Command::AnchorDimension { .. } => "Attach Dimension",
            Command::SetLocked { locked: true, .. } => "Lock",
            Command::SetLocked { .. } => "Unlock",
            Command::IntersectWith { .. } => "Intersect Faces",
            Command::RoundEdge { fillet: true, .. } => "Fillet",
            Command::RoundEdge { .. } => "Chamfer",
            Command::SetFeatureSize { .. }
            | Command::SetCutter { .. }
            | Command::SetBoolKind { .. }
            | Command::SetRimStyle { .. }
            | Command::SetFeatureEdges { .. } => "Change Feature",
            Command::ExplodeFeature { .. } => "Explode Feature",
            Command::RemoveFeature { .. } => "Remove Feature",
            Command::BakeFeatures { .. } => "Bake Features",
            Command::RoundEdges { fillet: true, .. } => "Fillet",
            Command::RoundEdges { .. } => "Chamfer",
            Command::FollowMe { .. } => "Follow Me",
            Command::AddMaterial { .. } => "New Material",
            Command::EditMaterial { .. } => "Edit Material",
            Command::RemoveMaterial { .. } => "Delete Material",
            Command::PurgeMaterials => "Purge Materials",
            Command::AddImage { .. } => "Add Image",
            Command::Paint { .. } | Command::PaintInstances { .. } => "Paint",
            Command::ReplaceMaterial { .. } => "Replace Material",
            Command::PlaceTexture { map: Some(_), .. } => "Position Texture",
            Command::PlaceTexture { .. } => "Reset Texture",
        }
    }

    /// The command in a group's own coordinates: tools work in the world,
    /// and `to_local` maps the world into the open group (`model` is the
    /// group's model, for the faces a distance is measured along).
    pub fn to_local(&self, to_local: &DAffine3, model: &Model) -> Command {
        let p = |v: DVec3| to_local.transform_point3(v);
        let dir = |v: DVec3| to_local.transform_vector3(v);
        // A distance along a face's normal, in the group's units.
        let along = |face: FaceId, d: f64| -> f64 {
            let Some(n) = model.face(face).and_then(|f| f.plane()).map(|p| p.normal) else {
                return d;
            };
            // The world normal is the local one through the inverse
            // transpose; a world step d along it, brought back.
            let world_n = to_local.matrix3.transpose().mul_vec3(n).normalize();
            dir(world_n * d).dot(n)
        };
        let scale = to_local.matrix3.determinant().abs().cbrt();
        match self {
            Command::Segment { p: a, q: b } => Command::Segment { p: p(*a), q: p(*b) },
            Command::FollowMe { path, profile, end } => Command::FollowMe {
                path: path.clone(),
                profile: *profile,
                end: end.map(p),
            },
            Command::Rect { x0, y0, x1, y1, z } => Command::Loop {
                points: [
                    DVec3::new(*x0, *y0, *z),
                    DVec3::new(*x1, *y0, *z),
                    DVec3::new(*x1, *y1, *z),
                    DVec3::new(*x0, *y1, *z),
                ]
                .into_iter()
                .map(p)
                .collect(),
            },
            Command::Circle {
                centre,
                normal,
                radius,
            } => {
                let n = dir(*normal);
                let side = Plane::new(*centre, *normal).frame().u * *radius;
                Command::Circle {
                    centre: p(*centre),
                    normal: n.normalize_or_zero(),
                    radius: dir(side).length(),
                }
            }
            Command::Loop { points } => Command::Loop {
                points: points.iter().map(|v| p(*v)).collect(),
            },
            Command::Spline { curve } => Command::Spline {
                curve: curve.transformed(to_local),
            },
            Command::MoveCage { cage, points, m } => Command::MoveCage {
                cage: *cage,
                points: points.clone(),
                m: *to_local * *m * to_local.inverse(),
            },
            Command::Polygon { points } => Command::Polygon {
                points: points.iter().map(|v| p(*v)).collect(),
            },
            Command::Shell {
                faces,
                open,
                thickness,
            } => Command::Shell {
                faces: faces.clone(),
                open: open.clone(),
                thickness: thickness * scale,
            },
            Command::Fillet { corner, radius } => Command::Fillet {
                corner: p(*corner),
                // Groups scale evenly where fillets are made.
                radius: dir(DVec3::X * *radius).length(),
            },
            Command::Arc {
                centre,
                normal,
                radius,
                start,
                sweep,
                pie,
            } => {
                // A mirror turns the arc the other way about its normal.
                let flip = to_local.matrix3.determinant().signum();
                Command::Arc {
                    centre: p(*centre),
                    normal: (dir(*normal) * flip).normalize_or_zero(),
                    radius: dir(Plane::new(*centre, *normal).frame().u * *radius).length(),
                    start: p(*start),
                    sweep: *sweep,
                    pie: *pie,
                }
            }
            Command::PushPull {
                face,
                distance,
                new_loop,
            } => Command::PushPull {
                face: *face,
                distance: along(*face, *distance),
                new_loop: *new_loop,
            },
            Command::Translate { vertices, by } => Command::Translate {
                vertices: vertices.clone(),
                by: dir(*by),
            },
            Command::Rotate {
                vertices,
                centre,
                axis,
                angle,
            } => Command::Rotate {
                vertices: vertices.clone(),
                centre: p(*centre),
                axis: dir(*axis).normalize(),
                angle: *angle,
            },
            // Scale works along the group's own axes, as in SketchUp.
            Command::Scale {
                vertices,
                centre,
                factor,
            } => Command::Scale {
                vertices: vertices.clone(),
                centre: p(*centre),
                factor: *factor,
            },
            Command::MirrorCopy {
                selection,
                origin,
                normal,
            } => Command::MirrorCopy {
                selection: selection.clone(),
                origin: p(*origin),
                // A normal moves by the inverse transpose.
                normal: (to_local.matrix3.inverse().transpose() * *normal).normalize_or_zero(),
            },
            Command::Copy {
                selection,
                step,
                count,
            } => Command::Copy {
                selection: selection.clone(),
                step: match *step {
                    Motion::Translate { by } => Motion::Translate { by: dir(by) },
                    Motion::Rotate {
                        centre,
                        axis,
                        angle,
                    } => Motion::Rotate {
                        centre: p(centre),
                        axis: dir(axis).normalize(),
                        angle,
                    },
                },
                count: *count,
            },
            Command::Offset { face, distance } => Command::Offset {
                face: *face,
                distance: distance * scale,
            },
            Command::RoundEdge { edge, size, fillet } => Command::RoundEdge {
                edge: *edge,
                size: size * scale,
                fillet: *fillet,
            },
            Command::RoundEdges {
                edges,
                size,
                fillet,
            } => Command::RoundEdges {
                edges: edges.clone(),
                size: size * scale,
                fillet: *fillet,
            },
            Command::AddText {
                text,
                font,
                height,
                mode,
                at,
            } => {
                // Placed by origin and axes (a group scaled evenly grows
                // the text with it).
                let (u, v) = (dir(at.matrix3.x_axis), dir(at.matrix3.y_axis));
                let k = u.length();
                let (u, v) = (u.normalize_or_zero(), v.normalize_or_zero());
                Command::AddText {
                    text: text.clone(),
                    font: *font,
                    height: height * k,
                    mode: match mode {
                        prism_ops::text::TextMode::Solid { depth } => {
                            prism_ops::text::TextMode::Solid { depth: depth * k }
                        }
                        m => *m,
                    },
                    at: DAffine3::from_cols(u, v, u.cross(v), p(at.translation)),
                }
            }
            Command::AddGuide { guide } => Command::AddGuide {
                guide: match *guide {
                    Guide::Line { origin, dir: d } => Guide::Line {
                        origin: p(origin),
                        dir: dir(d).normalize(),
                    },
                    Guide::Point { at } => Guide::Point { at: p(at) },
                },
            },
            Command::Place { instances, change } => Command::Place {
                instances: instances.clone(),
                change: change_to_local(change, to_local),
            },
            Command::CopyInstances {
                instances,
                step,
                count,
            } => Command::CopyInstances {
                instances: instances.clone(),
                step: change_to_local(step, to_local),
                count: *count,
            },
            Command::Batch { commands } => Command::Batch {
                commands: commands
                    .iter()
                    .map(|c| c.to_local(to_local, model))
                    .collect(),
            },
            Command::PlaceTexture { faces, map } => Command::PlaceTexture {
                faces: faces.clone(),
                map: map.map(|m| m.transformed(to_local)),
            },
            Command::Paste { clip, at } => Command::Paste {
                clip: clip.clone(),
                at: *to_local * *at,
            },
            Command::IntersectWith { others } => {
                let mut others = others.clone();
                transform::affine(&mut others, to_local);
                Command::IntersectWith { others }
            }
            other => other.clone(),
        }
    }

    /// Run the command inside definition `ctx` of an assembly.
    pub fn apply_in(&self, asm: &mut Assembly, ctx: DefId) {
        // Nothing changes a locked group (Everyday Commands part 1).
        if self.locked_in(asm) {
            return;
        }
        match self {
            Command::In { context, command } => {
                if let Some(def) = asm.context_def(context) {
                    command.apply_in(asm, def);
                }
            }
            Command::Cut {
                selection,
                instances,
            } => {
                if let Some(d) = asm.def_mut_live(ctx) {
                    let t = stack::Targets {
                        vertices: selection.vertices.clone(),
                        edges: selection.edges.clone(),
                        faces: selection.faces.clone(),
                    };
                    d.edit_direct(Some(t), |m| prism_ops::remove(m, selection));
                }
                asm.erase(ctx, instances);
            }
            Command::Paste { clip, at } => {
                asm.paste(ctx, clip, at);
            }
            Command::SetLocked { instances, locked } => asm.set_locked(instances, *locked),
            Command::AddText {
                text,
                font,
                height,
                mode,
                at,
            } => {
                let laid = prism_text::layout(text, *font, *height);
                let model = prism_ops::text::text_model(&laid, *mode);
                let first = text.lines().next().unwrap_or("").trim();
                let short: String = first.chars().take(24).collect();
                asm.add_group_model(ctx, model, &format!("Text: {short}"), *at);
            }
            Command::AddDimension { a, b, offset } => {
                asm.add_dimension(Dimension {
                    a: *a,
                    b: *b,
                    offset: *offset,
                });
            }
            Command::MoveDimension { id, offset } => asm.move_dimension(*id, *offset),
            Command::DeleteDimension { id } => asm.delete_dimension(*id),
            Command::AnchorDimension { id, a, b } => {
                asm.anchor_dimension(*id, a.clone(), b.clone())
            }
            Command::AddSection { origin, normal } => {
                asm.add_section(*origin, *normal);
            }
            Command::MoveSection { id, origin } => asm.move_section(*id, *origin),
            Command::AddWorkPlane { origin, normal } => {
                asm.add_work_plane(*origin, *normal);
            }
            Command::MoveWorkPlane { id, origin } => asm.move_work_plane(*id, *origin),
            Command::SetWorkPlaneActive { id, active } => asm.set_active_work_plane(*id, *active),
            Command::RenameWorkPlane { id, name } => asm.rename_work_plane(*id, name),
            Command::DeleteWorkPlane { id } => asm.delete_work_plane(*id),
            Command::FlipSection { id } => asm.flip_section(*id),
            Command::SetSectionActive { id, active } => asm.set_active_section(*id, *active),
            Command::RenameSection { id, name } => asm.rename_section(*id, name),
            Command::DeleteSection { id } => asm.delete_section(*id),
            Command::Batch { commands } => {
                for c in commands {
                    c.apply_in(asm, ctx);
                }
            }
            Command::SetFeatureSize { def, index, size } => {
                asm.set_feature_size(*def, *index, *size);
            }
            Command::SetCutter {
                def,
                index,
                min,
                max,
            } => {
                asm.set_cutter(*def, *index, *min, *max);
            }
            Command::SetFeatureEdges { def, index, edges } => {
                asm.set_feature_edges(*def, *index, edges);
            }
            Command::SetRimStyle {
                def,
                index,
                edge,
                style,
            } => {
                asm.set_rim_style(*def, *index, edge, *style);
            }
            Command::SetBoolKind { def, index, op } => {
                asm.set_bool_kind(*def, *index, *op);
            }
            Command::RemoveFeature { def, index } => {
                asm.remove_feature(*def, *index);
            }
            Command::BakeFeatures { def } => {
                asm.bake(*def);
            }
            Command::ExplodeFeature { def, index } => {
                asm.explode_feature(*def, *index);
            }
            // A shell stays live too.
            Command::Shell {
                faces,
                open,
                thickness,
            } => {
                asm.shell_live(ctx, faces, open, *thickness);
            }
            // A round stays live, in a group and on loose geometry alike.
            Command::RoundEdges {
                edges,
                size,
                fillet,
            } => {
                asm.round_live(ctx, edges, *size, *fillet);
            }
            Command::MakeGroup {
                selection,
                instances,
            } => {
                asm.make_group(ctx, selection, instances);
            }
            Command::Explode { instance } => {
                asm.explode(ctx, *instance);
            }
            Command::Boolean { op, first, second } => {
                let _ = asm.boolean(ctx, *op, *first, *second);
            }
            Command::OuterShell { instances } => {
                let _ = asm.outer_shell(ctx, instances);
            }
            Command::MakeComponent {
                selection,
                instances,
                name,
            } => {
                asm.make_component(ctx, selection, instances, name);
            }
            Command::MakeUnique { instances } => asm.make_unique(ctx, instances),
            Command::ChangeAxes { instance, axes } => {
                if let Some((_, inst)) = asm.find_instance(*instance) {
                    asm.change_axes(inst.def, axes);
                }
            }
            Command::ChangeModelAxes { axes } => {
                asm.change_model_axes(axes);
            }
            Command::Rename { def, name } => {
                asm.rename(*def, name);
            }
            Command::SetHidden { instances, hidden } => asm.set_hidden(instances, *hidden),
            Command::NameInstance { instance, name } => asm.set_name(*instance, name),
            Command::AddTag { name } => {
                asm.add_tag(name);
            }
            Command::RenameTag { tag, name } => {
                asm.rename_tag(*tag, name);
            }
            Command::SetTagVisible { tag, visible } => asm.set_tag_visible(*tag, *visible),
            Command::SetTag { instances, tag } => asm.set_tag(instances, *tag),
            Command::TagOn { instances, tag, on } => asm.tag_on(instances, *tag, *on),
            Command::SetTags { instances, tags } => asm.set_tags(instances, tags),
            Command::Place { instances, change } => {
                asm.place(ctx, instances, &change.affine(1.0));
            }
            Command::CopyInstances {
                instances,
                step,
                count,
            } => {
                let steps: Vec<DAffine3> =
                    (1..=*count).map(|k| step.affine(f64::from(k))).collect();
                asm.copy(ctx, instances, &steps);
            }
            Command::EraseInstances { instances } => asm.erase(ctx, instances),
            Command::AddMaterial { material } => {
                asm.add_material(material.clone());
            }
            Command::EditMaterial { id, material } => asm.edit_material(*id, material.clone()),
            Command::RemoveMaterial { id } => asm.remove_material(*id),
            Command::PurgeMaterials => asm.purge_materials(),
            Command::AddImage { image } => asm.add_image(image.clone()),
            Command::Paint { sides, material } => {
                // Only a material the document has.
                if material.is_none_or(|m| asm.material(m).is_some())
                    && let Some(d) = asm.def_mut_live(ctx)
                {
                    // Paint on a face a live feature made would be lost the
                    // next time it runs, so that feature is baked first
                    // (the same rule as any other edit of what it made).
                    let t = stack::Targets {
                        faces: sides.iter().map(|(f, _)| *f).collect(),
                        ..stack::Targets::default()
                    };
                    d.edit_direct(Some(t), |m| material::paint(m, sides, *material));
                }
            }
            Command::PaintInstances {
                instances,
                material,
            } => asm.paint_instances(instances, *material),
            Command::ReplaceMaterial {
                from,
                to,
                everywhere,
            } => {
                if to.is_none_or(|m| asm.material(m).is_some()) {
                    asm.replace_material((!*everywhere).then_some(ctx), *from, *to);
                }
            }
            // A direct edit of a body with live features goes to its base,
            // and the features run again on it. One that names what a
            // feature made bakes that feature first (the Inspector part 2
            // notes).
            geometry => {
                if let Some(d) = asm.def_mut_live(ctx) {
                    d.edit_direct(geometry.targets(), |m| geometry.apply(m));
                }
            }
        }
    }

    /// The elements this edit names, for the commands that name them (a
    /// face to push, the corners to move, the edges to erase); None for
    /// the ones that work by place (a line drawn across a face). Live
    /// features use it to tell an edit of the original shape from an edit
    /// of what a feature made.
    pub fn targets(&self) -> Option<stack::Targets> {
        let mut t = stack::Targets::default();
        match self {
            Command::Erase { selection }
            | Command::Copy { selection, .. }
            | Command::MirrorCopy { selection, .. } => {
                t.vertices.extend(&selection.vertices);
                t.edges.extend(&selection.edges);
                t.faces.extend(&selection.faces);
            }
            Command::PushPull { face, .. } | Command::Offset { face, .. } => {
                t.faces.insert(*face);
            }
            Command::Translate { vertices, .. }
            | Command::Rotate { vertices, .. }
            | Command::Scale { vertices, .. } => t.vertices.extend(vertices),
            Command::ConvertCurve { edges }
            | Command::Weld { edges }
            | Command::Soften { edges, .. }
            | Command::Loft { edges, .. }
            | Command::Revolve { edges, .. }
            | Command::Sweep { edges, .. }
            | Command::Patch { edges, .. }
            | Command::RoundEdges { edges, .. } => t.edges.extend(edges),
            Command::RoundEdge { edge, .. } => {
                t.edges.insert(*edge);
            }
            Command::ConvertFaces { faces }
            | Command::ReverseFaces { faces }
            | Command::PlaceTexture { faces, .. } => {
                t.faces.extend(faces);
            }
            Command::CleanEnvelope { faces: Some(faces) }
            | Command::Heal { faces: Some(faces) } => t.faces.extend(faces),
            Command::Shell { faces, open, .. } => {
                t.faces.extend(faces);
                t.faces.extend(open);
            }
            _ => return None,
        }
        Some(t)
    }

    /// Whether this command would change a locked group (and so does
    /// nothing): it moves, erases, explodes, paints or regroups one, or it
    /// runs inside one. Copying, hiding, tagging and naming are allowed.
    pub fn locked_in(&self, asm: &Assembly) -> bool {
        let any = |s: &BTreeSet<InstanceId>| s.iter().any(|i| asm.is_locked(*i));
        match self {
            Command::In { context, command } => asm.path_locked(context) || command.locked_in(asm),
            Command::Batch { commands } => commands.iter().any(|c| c.locked_in(asm)),
            Command::Place { instances, .. }
            | Command::EraseInstances { instances }
            | Command::Cut { instances, .. }
            | Command::MakeUnique { instances }
            | Command::PaintInstances { instances, .. }
            | Command::MakeGroup { instances, .. }
            | Command::MakeComponent { instances, .. } => any(instances),
            Command::Explode { instance } | Command::ChangeAxes { instance, .. } => {
                asm.is_locked(*instance)
            }
            Command::Boolean { first, second, .. } => {
                asm.is_locked(*first) || asm.is_locked(*second)
            }
            Command::OuterShell { instances } => any(instances),
            _ => false,
        }
    }

    /// Run a geometry command on a model (commands that change groups do
    /// nothing here; see [`Command::apply_in`]).
    /// Why this edit leaves `m` as it is, when it is refused because of
    /// arcs or curved faces (what the tools say).
    pub fn refusal(&self, m: &Model) -> Option<&'static str> {
        use prism_ops::transform::{arcs_refusal, scaling, turn};
        match self {
            Command::MoveCage {
                cage,
                points,
                m: by,
            } => prism_ops::cage::moved(m, *cage, points, by).err(),
            Command::AddCagePoints { cage } => {
                let mut w = m.clone();
                prism_ops::cage::add_points(&mut w, *cage).err()
            }
            Command::Loft { edges } => prism_ops::loft::lofted(m, edges).err(),
            Command::Revolve { edges } => prism_ops::loft::revolved(m, edges).err(),
            Command::Sweep { edges } => prism_ops::sweep::swept(m, edges).err(),
            Command::Patch { edges } => prism_ops::sweep::patched(m, edges).err(),
            Command::Weld { edges } => prism_ops::curves::weld_refusal(m, edges),
            Command::ConvertCurve { edges } => prism_ops::convert::refusal(m, edges),
            Command::ConvertFaces { faces } => prism_ops::convert::faces_refusal(m, faces),
            Command::Shell {
                faces,
                open,
                thickness,
            } => prism_ops::shelling::refusal(m, faces, open, *thickness),
            Command::LoftSides { sides, preset } => {
                prism_ops::sides::made_from(m, sides, *preset).err()
            }
            Command::PushPull {
                face,
                distance,
                new_loop,
            } => prism_ops::pushpull::refusal_with(m, *face, *distance, *new_loop),
            Command::Fillet { corner, radius } => prism_ops::fillet::refusal(m, *corner, *radius),
            Command::RoundEdge { edge, size, fillet } => {
                prism_ops::round::refusal(m, *edge, *size, *fillet)
            }
            Command::RoundEdges {
                edges,
                size,
                fillet,
            } => prism_ops::rounds::refusal(m, edges, *size, *fillet),
            Command::FollowMe { path, profile, end } => match end {
                None => prism_ops::follow::refusal(m, path, *profile),
                Some(p) => {
                    let mut m = m.clone();
                    let path = prism_ops::follow::cut_path(&mut m, path, *profile, *p);
                    prism_ops::follow::refusal(&m, &path, *profile)
                }
            },
            Command::Offset { face, distance } => prism_ops::offset::refusal(m, *face, *distance),
            Command::Translate { vertices, by } => {
                arcs_refusal(m, vertices, &prism_math::DAffine3::from_translation(*by))
            }
            Command::Rotate {
                vertices,
                centre,
                axis,
                angle,
            } => arcs_refusal(m, vertices, &turn(*centre, *axis, *angle)),
            Command::Scale {
                vertices,
                centre,
                factor,
            } => arcs_refusal(m, vertices, &scaling(*centre, *factor)),
            _ => None,
        }
    }

    pub fn apply(&self, m: &mut Model) {
        match self {
            Command::Segment { p, q } => {
                prism_ops::add_segment(m, *p, *q);
            }
            Command::Rect { x0, y0, x1, y1, z } => {
                prism_ops::add_rect(m, *x0, *y0, *x1, *y1, *z);
            }
            Command::Loop { points } => {
                for i in 0..points.len() {
                    prism_ops::add_segment(m, points[i], points[(i + 1) % points.len()]);
                }
            }
            Command::Circle {
                centre,
                normal,
                radius,
            } => {
                prism_ops::add_circle(m, *centre, *normal, *radius);
            }
            Command::Polygon { points } => prism_ops::add_polygon(m, points),
            Command::Spline { curve } => {
                prism_ops::add_spline(m, curve);
            }
            Command::MoveCage {
                cage,
                points,
                m: by,
            } => {
                let _ = prism_ops::cage::move_points(m, *cage, points, by);
            }
            Command::AddCagePoints { cage } => {
                let _ = prism_ops::cage::add_points(m, *cage);
            }
            Command::Loft { edges } => {
                let _ = prism_ops::loft::loft(m, edges);
            }
            Command::Shell {
                faces,
                open,
                thickness,
            } => {
                let _ = prism_ops::shelling::shell(m, faces, open, *thickness);
            }
            Command::Revolve { edges } => {
                let _ = prism_ops::loft::revolve(m, edges);
            }
            Command::Sweep { edges } => {
                let _ = prism_ops::sweep::sweep(m, edges);
            }
            Command::Patch { edges } => {
                let _ = prism_ops::sweep::patch(m, edges);
            }
            Command::LoftSides { sides, preset } => {
                let _ = prism_ops::sides::make(m, sides, *preset);
            }
            Command::Fillet { corner, radius } => {
                prism_ops::fillet::fillet(m, *corner, *radius);
            }
            Command::RoundEdge { edge, size, fillet } => {
                prism_ops::round::round(m, *edge, *size, *fillet);
            }
            Command::RoundEdges {
                edges,
                size,
                fillet,
            } => {
                let _ = prism_ops::rounds::round_edges(m, edges, *size, *fillet);
            }
            Command::FollowMe { path, profile, end } => {
                let path = match end {
                    Some(p) => prism_ops::follow::cut_path(m, path, *profile, *p),
                    None => path.clone(),
                };
                prism_ops::follow::follow(m, &path, *profile);
            }
            Command::Arc {
                centre,
                normal,
                radius,
                start,
                sweep,
                pie,
            } => {
                if *pie {
                    prism_ops::add_pie(m, *centre, *normal, *radius, *start, *sweep);
                } else {
                    prism_ops::add_arc(m, *centre, *normal, *radius, *start, *sweep);
                }
            }
            Command::Erase { selection } => prism_ops::remove(m, selection),
            Command::PushPull {
                face,
                distance,
                new_loop,
            } => {
                pushpull::push_pull(m, *face, *distance, *new_loop);
            }
            Command::Translate { vertices, by } => transform::translate(m, vertices, *by),
            Command::Rotate {
                vertices,
                centre,
                axis,
                angle,
            } => transform::rotate(m, vertices, *centre, *axis, *angle),
            Command::Scale {
                vertices,
                centre,
                factor,
            } => transform::scale(m, vertices, *centre, *factor),
            Command::Copy {
                selection,
                step,
                count,
            } => {
                // Place every copy first, then run the coplanar rule once,
                // so overlapping copies split each other, not the original
                // before the next copy is taken.
                let mut placed: Vec<BTreeSet<VertexId>> = Vec::new();
                for k in 1..=*count {
                    let copy = prism_ops::duplicate(m, selection);
                    let vs = copy.all_vertices(m);
                    let k = f64::from(k);
                    match *step {
                        Motion::Translate { by } => transform::translate_only(m, &vs, by * k),
                        Motion::Rotate {
                            centre,
                            axis,
                            angle,
                        } => transform::rotate_only(m, &vs, centre, axis, angle * k),
                    }
                    placed.push(vs);
                }
                // Copies that landed on each other were welded (the lower id
                // stays), so the vertices left are the ones to settle.
                for g in &mut placed {
                    g.retain(|v| m.has_vertex(*v));
                }
                prism_ops::settle::settle_rigid(m, &placed);
            }
            Command::MirrorCopy {
                selection,
                origin,
                normal,
            } => {
                let copy = prism_ops::duplicate(m, selection);
                let mut vs = copy.all_vertices(m);
                let across = Change::Mirror {
                    origin: *origin,
                    normal: *normal,
                }
                .affine(1.0);
                transform::mirror_only(m, &vs, &across);
                // Points that landed on the original's were welded.
                vs.retain(|v| m.has_vertex(*v));
                prism_ops::settle::settle_rigid(m, &[vs]);
            }
            Command::Offset { face, distance } => {
                prism_ops::offset::offset(m, *face, *distance);
            }
            Command::AddGuide { guide } => {
                m.add_guide(*guide);
            }
            Command::Import { model } => {
                m.append(model);
            }
            Command::IntersectFaces { faces } => {
                prism_ops::intersect::intersect_faces(m, faces.as_ref());
            }
            Command::IntersectWith { others } => {
                prism_ops::intersect::intersect_with(m, others);
            }
            Command::OrientFaces { faces } => {
                prism_ops::health::orient(m, faces.as_ref());
            }
            Command::ReverseFaces { faces } => prism_ops::health::reverse(m, faces),
            Command::Weld { edges } => {
                let _ = prism_ops::curves::weld(m, edges);
            }
            Command::ConvertCurve { edges } => {
                let _ = prism_ops::convert::convert(m, edges);
            }
            Command::ConvertFaces { faces } => {
                let _ = prism_ops::convert::convert_faces(m, faces);
            }
            Command::Soften { edges, soft } => {
                for e in edges {
                    m.set_soft(*e, *soft, *soft);
                }
            }
            Command::CleanEnvelope { faces } => {
                prism_ops::health::clean_envelope(m, faces.as_ref());
            }
            Command::RemoveStray => {
                prism_ops::health::remove_stray(m);
            }
            Command::RepairArcs => {
                prism_ops::health::repair_arcs(m);
            }
            Command::Heal { faces } => {
                prism_ops::validate::heal(m, faces.as_ref());
            }
            // These change groups: run them with `apply_in`.
            Command::In { .. }
            | Command::Batch { .. }
            | Command::MakeGroup { .. }
            | Command::Boolean { .. }
            | Command::OuterShell { .. }
            | Command::Explode { .. }
            | Command::MakeComponent { .. }
            | Command::MakeUnique { .. }
            | Command::ChangeAxes { .. }
            | Command::ChangeModelAxes { .. }
            | Command::Rename { .. }
            | Command::SetHidden { .. }
            | Command::NameInstance { .. }
            | Command::AddTag { .. }
            | Command::RenameTag { .. }
            | Command::SetTagVisible { .. }
            | Command::SetTag { .. }
            | Command::TagOn { .. }
            | Command::SetTags { .. }
            | Command::Place { .. }
            | Command::CopyInstances { .. }
            | Command::EraseInstances { .. }
            | Command::SetFeatureSize { .. }
            | Command::SetCutter { .. }
            | Command::SetBoolKind { .. }
            | Command::SetFeatureEdges { .. }
            | Command::SetRimStyle { .. }
            | Command::RemoveFeature { .. }
            | Command::ExplodeFeature { .. }
            | Command::BakeFeatures { .. }
            | Command::Cut { .. }
            | Command::Paste { .. }
            | Command::SetLocked { .. }
            | Command::AddText { .. }
            | Command::AddDimension { .. }
            | Command::MoveDimension { .. }
            | Command::DeleteDimension { .. }
            | Command::AnchorDimension { .. }
            | Command::AddSection { .. }
            | Command::MoveSection { .. }
            | Command::AddWorkPlane { .. }
            | Command::MoveWorkPlane { .. }
            | Command::SetWorkPlaneActive { .. }
            | Command::RenameWorkPlane { .. }
            | Command::DeleteWorkPlane { .. }
            | Command::FlipSection { .. }
            | Command::SetSectionActive { .. }
            | Command::RenameSection { .. }
            | Command::DeleteSection { .. }
            | Command::AddMaterial { .. }
            | Command::EditMaterial { .. }
            | Command::RemoveMaterial { .. }
            | Command::PurgeMaterials
            | Command::AddImage { .. }
            | Command::PaintInstances { .. }
            | Command::ReplaceMaterial { .. } => {}
            Command::Paint { sides, material } => material::paint(m, sides, *material),
            Command::PlaceTexture { faces, map } => {
                for f in faces {
                    if m.face(*f).is_some_and(|x| x.plane().is_some()) {
                        m.edit_face(*f, |x| x.paint.uv = *map);
                    }
                }
            }
        }
    }
}

/// A command and the geometry before and after it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Step {
    command: Command,
    before: Assembly,
    after: Assembly,
}

/// The geometry and its history.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    asm: Assembly,
    done: Vec<Step>,
    undone: Vec<Step>,
    /// Counts changes (runs, undos and redos), so the app can tell whether
    /// the document changed since it was saved.
    #[serde(default)]
    revision: u64,
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start from an existing model (with an empty history).
    pub fn with_model(model: Model) -> Self {
        Self::with_assembly(Assembly::with_model(model))
    }

    /// Start from existing geometry and groups (with an empty history).
    pub fn with_assembly(asm: Assembly) -> Self {
        Self {
            asm,
            ..Self::default()
        }
    }

    /// The root model: the geometry outside every group.
    pub fn model(&self) -> &Model {
        &self.asm.root().model
    }

    /// The geometry and its groups.
    pub fn assembly(&self) -> &Assembly {
        &self.asm
    }

    /// Run a command (wrapped in [`Command::In`] to run it inside a group).
    /// A command that changes nothing is not recorded. Returns whether it
    /// changed anything.
    pub fn run(&mut self, command: Command) -> bool {
        let before = self.asm.clone();
        command.apply_in(&mut self.asm, ROOT);
        self.asm.follow_dimensions();
        if self.asm == before {
            // Put it back exactly: a command can change a value without
            // changing it (0.0 to -0.0 compare equal), and the geometry must
            // stay what the history says it is.
            self.asm = before;
            return false;
        }
        self.done.push(Step {
            command,
            before,
            after: self.asm.clone(),
        });
        self.undone.clear();
        self.revision += 1;
        true
    }

    /// Run a command inside the group at `context` (the root for none).
    pub fn run_in(&mut self, context: &[InstanceId], command: Command) -> bool {
        if context.is_empty() {
            self.run(command)
        } else {
            self.run(Command::In {
                context: context.to_vec(),
                command: Box::new(command),
            })
        }
    }

    /// The group the last step was in (Undo reopens it), and the next.
    pub fn undo_context(&self) -> Vec<InstanceId> {
        self.done
            .last()
            .map(|s| context_of(&s.command))
            .unwrap_or_default()
    }

    pub fn redo_context(&self) -> Vec<InstanceId> {
        self.undone
            .last()
            .map(|s| context_of(&s.command))
            .unwrap_or_default()
    }

    /// Goes up with every change, undo and redo.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// The command Undo would undo.
    pub fn undo_name(&self) -> Option<&'static str> {
        self.done.last().map(|s| s.command.name())
    }

    /// The command Undo would undo.
    pub fn undo_command(&self) -> Option<&Command> {
        self.done.last().map(|s| &s.command)
    }

    /// The command Redo would run again.
    pub fn redo_command(&self) -> Option<&Command> {
        self.undone.last().map(|s| &s.command)
    }

    pub fn redo_name(&self) -> Option<&'static str> {
        self.undone.last().map(|s| s.command.name())
    }

    pub fn undo(&mut self) -> bool {
        let Some(step) = self.done.pop() else {
            return false;
        };
        let now = std::mem::replace(&mut self.asm, step.before.clone());
        self.asm.keep_counters_from(&now);
        self.undone.push(step);
        self.revision += 1;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(step) = self.undone.pop() else {
            return false;
        };
        let now = std::mem::replace(&mut self.asm, step.after.clone());
        self.asm.keep_counters_from(&now);
        self.done.push(step);
        self.revision += 1;
        true
    }

    /// The commands run so far, oldest first (for replay and autosave).
    pub fn log(&self) -> impl Iterator<Item = &Command> {
        self.done.iter().map(|s| &s.command)
    }

    /// Run a log of commands from an empty model.
    pub fn replay<'a>(commands: impl IntoIterator<Item = &'a Command>) -> Document {
        let mut d = Document::new();
        for c in commands {
            d.run(c.clone());
        }
        d
    }
}

fn context_of(c: &Command) -> Vec<InstanceId> {
    match c {
        Command::In { context, .. } => context.clone(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmds() -> Vec<Command> {
        vec![
            Command::Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 4.0,
                y1: 4.0,
                z: 0.0,
            },
            Command::Rect {
                x0: 2.0,
                y0: 2.0,
                x1: 6.0,
                y1: 6.0,
                z: 0.0,
            },
            Command::Segment {
                p: DVec3::new(0.0, 1.0, 0.0),
                q: DVec3::new(6.0, 1.0, 0.0),
            },
        ]
    }

    #[test]
    fn undo_and_redo_restore_the_model_exactly() {
        let mut d = Document::new();
        for c in cmds() {
            let before = d.model().geometry_json();
            assert!(d.run(c));
            let after = d.model().to_json();
            assert!(d.undo());
            assert_eq!(
                d.model().geometry_json(),
                before,
                "undo restores the geometry"
            );
            assert!(d.redo());
            assert_eq!(
                d.model().to_json(),
                after,
                "redo restores everything, ids included"
            );
        }
    }

    #[test]
    fn ids_are_not_handed_out_twice_after_undo() {
        let mut d = Document::new();
        d.run(cmds()[0].clone());
        let first: Vec<_> = d.model().faces().map(|(id, _)| id).collect();
        d.undo();
        d.run(cmds()[1].clone());
        let second: Vec<_> = d.model().faces().map(|(id, _)| id).collect();
        assert!(first.iter().all(|f| !second.contains(f)));
    }

    #[test]
    fn a_command_that_changes_nothing_is_not_recorded() {
        let mut d = Document::new();
        d.run(cmds()[0].clone());
        assert!(!d.run(cmds()[0].clone()));
        assert_eq!(d.log().count(), 1);
        assert_eq!(d.undo_name(), Some("Rectangle"));
    }

    #[test]
    fn replaying_the_log_gives_the_same_model() {
        let mut d = Document::new();
        for c in cmds() {
            d.run(c);
        }
        let again = Document::replay(d.log());
        assert_eq!(again.model().to_json(), d.model().to_json());
    }

    #[test]
    fn a_new_command_clears_redo() {
        let mut d = Document::new();
        d.run(cmds()[0].clone());
        d.undo();
        assert!(d.can_redo());
        d.run(cmds()[1].clone());
        assert!(!d.can_redo());
    }

    #[test]
    fn the_document_saves_and_loads() {
        let mut d = Document::new();
        for c in cmds() {
            d.run(c);
        }
        let text = serde_json::to_string(&d).expect("saves");
        let back: Document = serde_json::from_str(&text).expect("loads");
        assert_eq!(back, d);
    }
}
