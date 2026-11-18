//! Text (Annotation & Sections part 3): the Text tool.
//!
//! Type the words, pick a font, a height and what it is made of in the
//! Text window, then click: the text is set down as a new group (named
//! after it), on the face under the pointer or on the ground, reading
//! along the face's own direction. A ghost shows where it will go. Only
//! the three bundled fonts are used (no system fonts).

use prism_doc::Command;
use prism_math::{DAffine3, DVec2, DVec3};
use prism_ops::text::TextMode;
use prism_text::{Font, Laid, layout};
use prism_view::View;
use prism_view::overlay::{Line, Overlay};

use super::Session;

pub const TEXT_TOOL: &str = "tool.text";

/// The most contour pieces drawn for the ghost; more shows a box.
const GHOST_PIECES: usize = 6000;
/// The longest text set, in characters (the overlap rules make very long
/// text slow).
pub const MAX_TEXT: usize = 400;

/// What the text is made of, as the window offers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    Outline,
    Face,
    Solid,
}

/// The Text window's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSettings {
    pub text: String,
    pub font: Font,
    /// Capital letters' height, in metres.
    pub height: f64,
    pub kind: TextKind,
    /// How far a solid is pulled out, in metres.
    pub depth: f64,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            text: "Text".into(),
            font: Font::Sans,
            height: 0.2,
            kind: TextKind::Face,
            depth: 0.05,
        }
    }
}

impl TextSettings {
    pub fn mode(&self) -> TextMode {
        match self.kind {
            TextKind::Outline => TextMode::Outline,
            TextKind::Face => TextMode::Face,
            TextKind::Solid => TextMode::Solid { depth: self.depth },
        }
    }
}

/// One change in the window.
#[derive(Clone, Debug, PartialEq)]
pub enum TextSet {
    Text(String),
    Font(Font),
    /// Typed in the document's unit; text that is not a length changes
    /// nothing.
    Height(String),
    Kind(TextKind),
    Depth(String),
}

/// What the Text window shows.
#[derive(Clone, Debug, PartialEq)]
pub struct TextPanel {
    pub settings: TextSettings,
    pub height: String,
    pub depth: String,
    /// Characters the font has no glyph for in the text.
    pub missing: usize,
    pub how: &'static str,
}

/// The frame text takes on the face (or the ground) under the pointer:
/// `x` along the text, `y` up the text, facing `z`. On a face it reads
/// along the world's x (or y, on a face square to x).
pub fn frame_on(at: DVec3, normal: DVec3) -> DAffine3 {
    let n = normal.normalize_or_zero();
    let n = if n == DVec3::ZERO { DVec3::Z } else { n };
    let along = if n.cross(DVec3::X).length() > 1e-6 {
        DVec3::X
    } else {
        DVec3::Y
    };
    let mut u = (along - n * along.dot(n)).normalize();
    let mut v = n.cross(u);
    // Reads upright: y up the face, where it can, not down it.
    if v.z < -1e-9 {
        (u, v) = (-u, -v);
    }
    DAffine3::from_cols(u, v, n, at)
}

impl Session {
    pub fn text_settings(&self) -> &TextSettings {
        &self.text_set
    }

    pub fn text_panel(&self) -> Option<TextPanel> {
        (self.tool == TEXT_TOOL).then(|| TextPanel {
            settings: self.text_set.clone(),
            height: self.number.show(self.text_set.height),
            depth: self.number.show(self.text_set.depth),
            missing: self.text_laid().missing,
            how: self
                .text_hint()
                .unwrap_or("Type the text, then click where it goes"),
        })
    }

    /// Set text down at `at` on a plane with this `normal` (for the
    /// assistant); the new group's id. Uses the window's settings for
    /// what `set` leaves out.
    pub fn add_text(
        &mut self,
        set: TextSettings,
        at: DVec3,
        normal: DVec3,
    ) -> Result<u32, &'static str> {
        if set.text.trim().is_empty() {
            return Err("there is no text");
        }
        if !(set.height.is_finite() && set.height > 0.0) {
            return Err("the height must be above zero");
        }
        self.cancel();
        let id = self.doc.assembly().next_instance();
        if self.run(Command::AddText {
            text: set.text.chars().take(MAX_TEXT).collect(),
            font: set.font,
            height: set.height,
            mode: set.mode(),
            at: frame_on(at, normal),
        }) {
            self.set_groups([id].into());
            Ok(id.0)
        } else {
            Err("that text has nothing to draw")
        }
    }

    /// Change a setting (the window's fields and buttons).
    pub fn text_input(&mut self, op: TextSet) {
        match op {
            TextSet::Text(t) => self.text_set.text = t.chars().take(MAX_TEXT).collect(),
            TextSet::Font(f) => self.text_set.font = f,
            TextSet::Height(t) => {
                if let Some(h) = self
                    .number
                    .length(t.trim())
                    .map(f64::abs)
                    .filter(|h| *h > 0.0)
                {
                    self.text_set.height = h;
                }
            }
            TextSet::Kind(k) => self.text_set.kind = k,
            TextSet::Depth(t) => {
                if let Some(d) = self.number.length(t.trim()).filter(|d| *d != 0.0) {
                    self.text_set.depth = d;
                }
            }
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The text laid out with the window's settings (kept until they change).
    fn text_laid(&self) -> Laid {
        let s = &self.text_set;
        layout(&s.text, s.font, s.height)
    }

    /// The frame under the pointer: the flat face it is on, else the
    /// ground at the snapped point.
    fn text_frame_at(&self, s: DVec2, view: &View) -> Option<DAffine3> {
        let (o, d) = view.ray(s);
        let on_face = self.scene.ray_face(o, d).and_then(|(id, t)| {
            let n = self.scene.face(id)?.flat_normal()?;
            Some((o + d * t, if n.dot(d) > 0.0 { -n } else { n }))
        });
        let point = self.inference.as_ref().map(|i| i.point);
        match (on_face, point) {
            (Some((at, n)), p) => Some(frame_on(p.unwrap_or(at), n)),
            (None, Some(p)) => Some(frame_on(p, DVec3::Z)),
            (None, None) => None,
        }
    }

    pub(super) fn text_hover(&mut self, s: DVec2, view: &View) {
        if self.tool != TEXT_TOOL {
            return;
        }
        let ghost = self.text_frame_at(s, view);
        if ghost != self.text_ghost {
            self.text_ghost = ghost;
        }
    }

    pub(super) fn text_press(&mut self, s: DVec2) {
        self.update_hover(s);
        let view = self.view();
        if self.text_set.text.trim().is_empty() {
            self.notice = Some("Type the text in the Text window first");
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        let Some(at) = self.text_frame_at(s, &view) else {
            return;
        };
        self.cancel();
        let id = self.doc.assembly().next_instance();
        let set = self.text_set.clone();
        if self.run(Command::AddText {
            text: set.text.clone(),
            font: set.font,
            height: set.height,
            mode: set.mode(),
            at,
        }) {
            self.set_groups([id].into());
        } else {
            self.notice = Some("That text has nothing to draw");
        }
        self.text_ghost = None;
        self.overlay_dirty = true;
        self.dirty = true;
        self.refresh();
    }

    pub(super) fn text_hint(&self) -> Option<&'static str> {
        if self.tool != TEXT_TOOL {
            return None;
        }
        Some(if self.text_set.text.trim().is_empty() {
            "Type the text in the Text window"
        } else {
            "Click where the text goes · it follows the face under the pointer"
        })
    }

    /// The ghost: the letters' outlines where a click would put them.
    pub(super) fn text_overlay(&self, o: &mut Overlay) {
        let (true, Some(at)) = (self.tool == TEXT_TOOL, self.text_ghost) else {
            return;
        };
        let laid = self.text_laid();
        let color = self.style.feedback.dimension;
        let place = |p: DVec2| at.transform_point3(DVec3::new(p.x, p.y, 0.0));
        let pieces: usize = laid.contours.iter().map(Vec::len).sum();
        let line = |a: DVec3, b: DVec3| Line {
            a,
            b,
            color,
            width: 1.0,
            dash: [0.0, 0.0],
        };
        if pieces <= GHOST_PIECES {
            for c in &laid.contours {
                for k in 0..c.len() {
                    o.lines.push(line(place(c[k]), place(c[(k + 1) % c.len()])));
                }
            }
        } else if laid.width > 0.0 {
            // Too much to draw while the pointer moves: its box.
            let h = self.text_set.height;
            let corners = [
                DVec2::new(0.0, -laid.depth),
                DVec2::new(laid.width, -laid.depth),
                DVec2::new(laid.width, h),
                DVec2::new(0.0, h),
            ];
            for k in 0..4 {
                o.lines
                    .push(line(place(corners[k]), place(corners[(k + 1) % 4])));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_on_the_ground_reads_along_x_facing_up() {
        let f = frame_on(DVec3::new(1.0, 2.0, 0.0), DVec3::Z);
        assert_eq!(f.matrix3.x_axis, DVec3::X);
        assert_eq!(f.matrix3.y_axis, DVec3::Y);
        assert_eq!(f.matrix3.z_axis, DVec3::Z);
        assert_eq!(f.translation, DVec3::new(1.0, 2.0, 0.0));
    }

    #[test]
    fn text_on_a_wall_stands_upright_and_faces_out() {
        // A wall facing -y: reads along +x, up the wall.
        let f = frame_on(DVec3::ZERO, -DVec3::Y);
        assert!(f.matrix3.x_axis.abs_diff_eq(DVec3::X, 1e-12));
        assert!(f.matrix3.y_axis.abs_diff_eq(DVec3::Z, 1e-12));
        assert!(f.matrix3.z_axis.abs_diff_eq(-DVec3::Y, 1e-12));
        // A wall facing +x reads along y... turned to read left to right
        // from outside: x along y.
        let f = frame_on(DVec3::ZERO, DVec3::X);
        assert!(f.matrix3.z_axis.abs_diff_eq(DVec3::X, 1e-12));
        assert!(f.matrix3.x_axis.abs_diff_eq(DVec3::Y, 1e-12));
        assert!(f.matrix3.y_axis.abs_diff_eq(DVec3::Z, 1e-12));
        // A wall facing -x reads along -y, upright.
        let f = frame_on(DVec3::ZERO, -DVec3::X);
        assert!(f.matrix3.x_axis.abs_diff_eq(-DVec3::Y, 1e-12));
        assert!(f.matrix3.y_axis.abs_diff_eq(DVec3::Z, 1e-12));
        // The frame is always a proper turn.
        for n in [
            DVec3::new(1.0, 1.0, 1.0),
            -DVec3::Z,
            DVec3::new(0.0, 0.3, 0.9),
        ] {
            let f = frame_on(DVec3::ZERO, n);
            assert!((f.matrix3.determinant() - 1.0).abs() < 1e-9, "{n}");
            assert!(f.matrix3.z_axis.abs_diff_eq(n.normalize(), 1e-12));
        }
    }
}
