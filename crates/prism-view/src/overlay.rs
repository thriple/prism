//! Tool feedback drawn over the model: guides, glyphs, rubber bands,
//! previews and labels. The tools describe it; the renderer draws the lines,
//! marks and triangles, and the app draws the label text.

use prism_math::{DVec2, DVec3};

use crate::style::Rgba;

/// Mark shapes (the feel test's glyphs, plus rings for snap targets).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Circle,
    Triangle,
    Square,
    Diamond,
    Cross,
    /// A filled disc with a ring (the vertex being snapped to).
    Ring,
    /// An outline only (a woken point).
    DiamondOutline,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub a: DVec3,
    pub b: DVec3,
    pub color: Rgba,
    /// Points.
    pub width: f32,
    /// Dash and gap in points; zero for solid.
    pub dash: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub at: DVec3,
    pub shape: Shape,
    pub color: Rgba,
    /// Half size in points.
    pub size: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tri {
    pub p: [DVec3; 3],
    pub color: Rgba,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub at: DVec3,
    /// Offset from the point on screen, in points.
    pub offset: [f32; 2],
    pub text: String,
    /// The pill's colour.
    pub color: Rgba,
}

/// A rectangle on screen (box select), in viewport points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenBox {
    pub min: DVec2,
    pub max: DVec2,
    /// Crossing (right to left): dashed and green.
    pub crossing: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overlay {
    pub lines: Vec<Line>,
    pub marks: Vec<Mark>,
    pub tris: Vec<Tri>,
    pub labels: Vec<Label>,
    pub screen_box: Option<ScreenBox>,
    /// A label for the box ("Window", "Crossing  add").
    pub box_label: Option<String>,
}
