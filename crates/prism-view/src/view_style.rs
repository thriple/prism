//! A view style (PLAN 1.14): what is shown and how lines and faces look.
//! It is saved with the document. The named styles are presets: choosing
//! one sets every field, and later changes are kept.

use serde::{Deserialize, Serialize};

use crate::style::{FaceDraw, HiddenLines, Rgba, Style};

/// How faces look.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaceStyle {
    /// Lit, with a front and a back colour.
    Shaded,
    /// Lit, front and back in one colour.
    Monochrome,
    /// Unlit, in the background colour: lines on top hide what is behind.
    HiddenLine,
    /// No faces: every line shows, and faces cannot be picked.
    Wireframe,
}

/// Colour of model edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineColor {
    One,
    /// Edges along the red, green or blue axis in that colour.
    ByAxis,
}

/// The named styles.
pub const PRESETS: [&str; 5] = ["Default", "Hidden Line", "X-ray", "Technical", "Wireframe"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewStyle {
    /// The named style it started from.
    pub name: String,
    pub faces: bool,
    pub lines: bool,
    pub points: bool,
    pub axes: bool,
    pub ground: bool,
    pub grid: bool,
    pub guides: bool,
    /// Hidden objects drawn as ghosts that can be picked and unhidden.
    pub hidden: bool,
    /// Points.
    pub line_width: f32,
    /// Points; 0 turns profiles off.
    pub profile_width: f32,
    pub line_color: LineColor,
    pub hidden_lines: HiddenLines,
    pub face_style: FaceStyle,
    pub xray: bool,
    /// 0 to 1.
    pub xray_opacity: f32,
    /// Lighting: the sun (else the default light), and shadows.
    pub sun: bool,
    pub shadows: bool,
    /// When and where the sun is.
    pub month: u32,
    pub day: u32,
    pub hours: f64,
    pub utc_offset: f64,
    pub latitude: f64,
    pub longitude: f64,
    pub north: f64,
    /// How strong the lit side is, and how bright the shaded side stays.
    pub light: f32,
    pub dark: f32,
}

impl Default for ViewStyle {
    fn default() -> Self {
        Self::preset("Default").expect("the Default style")
    }
}

/// The background colour Hidden Line paints faces with.
const PAPER: Rgba = Rgba::rgb(0xff, 0xff, 0xff);

impl ViewStyle {
    /// A named style, if there is one by that name.
    pub fn preset(name: &str) -> Option<ViewStyle> {
        let base = ViewStyle {
            name: name.to_owned(),
            faces: true,
            lines: true,
            points: false,
            axes: true,
            ground: true,
            grid: true,
            guides: true,
            hidden: false,
            line_width: 1.5,
            // Profiles on by default (owner request, 2026-11-09).
            profile_width: 3.0,
            line_color: LineColor::One,
            hidden_lines: HiddenLines::Off,
            face_style: FaceStyle::Shaded,
            xray: false,
            xray_opacity: 0.5,
            sun: false,
            shadows: true,
            // Midsummer afternoon in Guam, where the owner is.
            month: 6,
            day: 21,
            hours: 13.5,
            utc_offset: 10.0,
            latitude: 13.44,
            longitude: 144.79,
            north: 0.0,
            light: 0.34,
            dark: 0.66,
        };
        Some(match name {
            "Default" => base,
            "Hidden Line" => ViewStyle {
                face_style: FaceStyle::HiddenLine,
                ..base
            },
            "X-ray" => ViewStyle { xray: true, ..base },
            "Technical" => ViewStyle {
                face_style: FaceStyle::Monochrome,
                line_width: 1.0,
                profile_width: 0.0,
                ground: false,
                ..base
            },
            "Wireframe" => ViewStyle {
                face_style: FaceStyle::Wireframe,
                profile_width: 0.0,
                ..base
            },
            _ => return None,
        })
    }

    /// The drawing style this view style gives, on top of `base`.
    pub fn apply(&self, base: &Style) -> Style {
        let mut s = *base;
        s.edge_width = self.line_width;
        s.profile_width = self.profile_width;
        s.hidden_lines = self.hidden_lines;
        s.show_lines = self.lines;
        s.show_points = self.points;
        s.show_axes = self.axes;
        s.show_grid = self.grid;
        s.show_ground = self.ground;
        s.lines_by_axis = self.line_color == LineColor::ByAxis;
        s.faces = if !self.faces || self.face_style == FaceStyle::Wireframe {
            FaceDraw::None
        } else if self.xray {
            FaceDraw::XRay
        } else {
            FaceDraw::Solid
        };
        match self.face_style {
            FaceStyle::Shaded | FaceStyle::Wireframe => {}
            FaceStyle::Monochrome => {
                s.face_back = s.face_front;
                s.materials = false;
            }
            FaceStyle::HiddenLine => {
                s.materials = false;
                s.face_front = PAPER;
                s.face_back = PAPER;
                s.shadow_alpha = 0.0;
            }
        }
        s.diffuse = self.light;
        s.ambient = self.dark;
        s.ground_shadow = self.shadows;
        s.face_shadows = false;
        if self.sun {
            let d = self.sun_direction();
            if d.z > 0.0 {
                s.light = d.to_array();
                s.shadow_dir = (-d).to_array();
                s.face_shadows = self.shadows;
            } else {
                // Night: no direct light and no shadows.
                s.diffuse = 0.0;
                s.ground_shadow = false;
            }
        }
        if self.face_style == FaceStyle::HiddenLine {
            s.ambient = 1.0;
            s.diffuse = 0.0;
            s.face_shadows = false;
        }
        if s.faces == FaceDraw::XRay {
            let a = crate::style::alpha(self.xray_opacity.clamp(0.0, 1.0));
            s.face_front = s.face_front.with_alpha(a);
            s.face_back = s.face_back.with_alpha(a);
        }
        s
    }

    /// The direction toward the sun (model coordinates).
    pub fn sun_direction(&self) -> prism_math::DVec3 {
        prism_math::sun::sun_direction(&prism_math::sun::SunPlace {
            month: self.month,
            day: self.day,
            hours: self.hours,
            utc_offset: self.utc_offset,
            latitude: self.latitude,
            longitude: self.longitude,
            north: self.north,
        })
    }

    /// Whether a View menu toggle is on.
    pub fn is_on(&self, id: &str) -> Option<bool> {
        Some(match id {
            "show.faces" => self.faces,
            "show.lines" => self.lines,
            "show.points" => self.points,
            "view.axes" => self.axes,
            "view.ground" => self.ground,
            "view.grid" => self.grid,
            "view.guides" => self.guides,
            "view.hidden" => self.hidden,
            "view.xray" => self.xray,
            "view.shadow" => self.shadows,
            "light.sun" => self.sun,
            "light.default" => !self.sun,
            _ => return None,
        })
    }

    /// Flip a View menu toggle. Returns false if `id` is not one.
    pub fn toggle(&mut self, id: &str) -> bool {
        let f = match id {
            "show.faces" => &mut self.faces,
            "show.lines" => &mut self.lines,
            "show.points" => &mut self.points,
            "view.axes" => &mut self.axes,
            "view.ground" => &mut self.ground,
            "view.grid" => &mut self.grid,
            "view.guides" => &mut self.guides,
            "view.hidden" => &mut self.hidden,
            "view.xray" => &mut self.xray,
            "view.shadow" => &mut self.shadows,
            // One or the other.
            "light.sun" | "light.default" => {
                self.sun = id == "light.sun";
                return true;
            }
            _ => return false,
        };
        *f = !*f;
        true
    }
}

/// The View menu's toggles.
pub const TOGGLES: [&str; 12] = [
    "show.faces",
    "show.lines",
    "show.points",
    "view.axes",
    "view.ground",
    "view.grid",
    "view.guides",
    "view.hidden",
    "view.xray",
    "view.shadow",
    "light.sun",
    "light.default",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_exists_and_keeps_its_name() {
        for p in PRESETS {
            assert_eq!(ViewStyle::preset(p).expect("preset").name, p);
        }
        assert!(ViewStyle::preset("Nope").is_none());
    }

    #[test]
    fn styles_map_to_drawing() {
        let base = Style::default();
        let d = ViewStyle::default().apply(&base);
        assert_eq!(d.faces, FaceDraw::Solid);
        assert!(d.profile_width > d.edge_width, "profiles on by default");
        let w = ViewStyle::preset("Wireframe").unwrap().apply(&base);
        assert_eq!(w.faces, FaceDraw::None);
        let x = ViewStyle::preset("X-ray").unwrap().apply(&base);
        assert_eq!(x.faces, FaceDraw::XRay);
        assert!(x.face_front.0[3] < 255);
        let m = ViewStyle::preset("Technical").unwrap().apply(&base);
        assert_eq!(m.face_back, m.face_front);
        let h = ViewStyle::preset("Hidden Line").unwrap().apply(&base);
        assert_eq!((h.face_front, h.diffuse), (PAPER, 0.0));
    }

    #[test]
    fn toggles_flip_and_read_back() {
        let mut v = ViewStyle::default();
        for id in TOGGLES {
            let was = v.is_on(id).expect("a toggle");
            assert!(v.toggle(id));
            assert_eq!(v.is_on(id), Some(!was));
        }
        assert!(!v.toggle("view.nothing"));
    }

    #[test]
    fn a_saved_style_reads_back_and_missing_fields_default() {
        let v = ViewStyle::preset("Technical").unwrap();
        let text = serde_json::to_string(&v).unwrap();
        assert_eq!(serde_json::from_str::<ViewStyle>(&text).unwrap(), v);
        let partial: ViewStyle = serde_json::from_str(r#"{"xray": true}"#).unwrap();
        assert!(partial.xray && partial.faces);
    }
}
