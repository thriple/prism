//! The viewport's look, in one table (the start of PLAN 1.14's styles).
//! These colours belong to the viewport, not to the Soft theme. Values are
//! sRGB bytes with alpha, taken from the feel test.

/// An sRGB colour with alpha, 0–255 each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }
    pub const fn with_alpha(self, a: u8) -> Self {
        let [r, g, b, _] = self.0;
        Self([r, g, b, a])
    }
    /// As 0–1 floats (still sRGB-encoded).
    pub fn to_f32(self) -> [f32; 4] {
        self.0.map(|c| f32::from(c) / 255.0)
    }
}

/// Alpha from a fraction (0–1).
pub const fn alpha(f: f32) -> u8 {
    (f * 255.0 + 0.5) as u8
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub sky_top: Rgba,
    pub sky_horizon: Rgba,
    pub ground_near_horizon: Rgba,
    pub ground_bottom: Rgba,
    pub grid_minor: Rgba,
    pub grid_major: Rgba,
    pub grid_minor_alpha: f32,
    pub grid_major_alpha: f32,
    pub grid_minor_width: f32,
    pub grid_major_width: f32,
    /// Grid spacing in model units, lines out to `grid_lines` each way, a
    /// darker line every `grid_major_every`.
    pub grid_spacing: f64,
    pub grid_lines: i32,
    pub grid_major_every: i32,
    /// The grid fades out between these distances from the eye (prototype: 1.25 - d / 45).
    pub grid_fade: f32,
    pub axes: [Rgba; 3],
    pub axis_length: f64,
    pub axis_width: f32,
    pub axis_alpha: f32,
    pub axis_negative_alpha: f32,
    /// Dash and gap lengths in pixels.
    pub axis_negative_dash: [f32; 2],
    pub face_front: Rgba,
    pub face_back: Rgba,
    /// Light direction for shading (towards the light).
    pub light: [f64; 3],
    /// Shading: ambient + diffuse * max(0, n · light).
    pub ambient: f32,
    pub diffuse: f32,
    pub edge: Rgba,
    pub edge_width: f32,
    /// How opaque edges outside the open group are (0 to 255).
    pub faded_alpha: u8,
    /// The box around the open group: colour, width and dash (points).
    pub open_box: Rgba,
    pub open_box_dash: [f32; 2],
    /// Model Health in the model: open edges, edges with 3 or more faces,
    /// stray edges, the tint on reversed faces, and the outline of interior
    /// and crossing faces.
    pub health_open: Rgba,
    pub health_many: Rgba,
    pub health_stray: Rgba,
    pub health_reversed: Rgba,
    pub health_outline: Rgba,
    /// 0 turns the profile (silhouette) lines off.
    pub profile_width: f32,
    pub hover: Rgba,
    pub hover_face_alpha: f32,
    pub selected: Rgba,
    pub selected_face_alpha: f32,
    pub highlight_width: f32,
    /// The selected-face stipple: a dot pattern in screen pixels.
    pub stipple: Rgba,
    pub stipple_tile: f32,
    pub stipple_dot: f32,
    pub vertex_dot_radius: f32,
    pub vertex_ring: Rgba,
    pub vertex_ring_width: f32,
    pub shadow_alpha: f32,
    /// Direction the light travels for the ground shadow.
    pub shadow_dir: [f64; 3],
    /// How lines hidden behind faces look: not drawn, dashed or faint.
    pub hidden_lines: HiddenLines,
    pub hidden_alpha: f32,
    pub hidden_dash: [f32; 2],
    /// Inference and tool feedback.
    pub feedback: Feedback,
}

/// The colours and sizes of inference and tool feedback (feel test:
/// `GLYPH`, `drawOverlay(`, `boxDraw(`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feedback {
    pub endpoint: Rgba,
    pub midpoint: Rgba,
    pub on_edge: Rgba,
    pub on_face: Rgba,
    pub on_ground: Rgba,
    pub intersection: Rgba,
    pub guide_cross: Rgba,
    pub free: Rgba,
    /// Parallel guides and woken edges.
    pub parallel: Rgba,
    /// Woken points' diamonds.
    pub woken: Rgba,
    pub glyph_size: f32,
    pub woken_size: f32,
    pub guide_width: f32,
    pub guide_dash: [f32; 2],
    pub rubber_band: Rgba,
    pub rubber_band_width: f32,
    /// The rectangle preview's fill.
    pub preview_fill: Rgba,
    /// Snap target highlight: a wide faint glow and a line.
    pub target_glow_width: f32,
    pub target_glow_alpha: f32,
    pub target_width: f32,
    pub target_ring: f32,
    /// Label pills.
    pub pill: Rgba,
    pub pill_text: Rgba,
    pub box_window: Rgba,
    pub box_crossing: Rgba,
    pub box_fill_alpha: f32,
    pub box_dash: [f32; 2],
    /// Edges the Eraser will remove.
    pub erase: Rgba,
    /// The Rotate tool's protractor.
    pub protractor: Rgba,
    /// Guide lines and points stored in the model (Tape Measure, Protractor).
    pub guide: Rgba,
}

impl Default for Feedback {
    fn default() -> Self {
        Self {
            endpoint: Rgba::rgb(0x1b, 0x9a, 0x52),
            midpoint: Rgba::rgb(0x0b, 0x87, 0xa8),
            on_edge: Rgba::rgb(0xd4, 0x35, 0x1c),
            on_face: Rgba::rgb(0x2b, 0x57, 0xd6),
            on_ground: Rgba::rgb(0x6b, 0x72, 0x80),
            intersection: Rgba::rgb(0x11, 0x11, 0x11),
            guide_cross: Rgba::rgb(0xb2, 0x1f, 0xc2),
            free: Rgba::rgb(0x6b, 0x72, 0x80),
            parallel: Rgba::rgb(0xb2, 0x1f, 0xc2),
            woken: Rgba::rgb(0x3d, 0x40, 0x46),
            glyph_size: 6.0,
            woken_size: 7.0,
            guide_width: 1.6,
            guide_dash: [5.0, 4.0],
            rubber_band: Rgba::rgb(0x16, 0x18, 0x1c),
            rubber_band_width: 1.8,
            preview_fill: Rgba::rgb(31, 95, 224).with_alpha(alpha(0.14)),
            target_glow_width: 7.0,
            target_glow_alpha: 0.28,
            target_width: 2.8,
            target_ring: 9.0,
            pill: Rgba::rgb(0x1c, 0x1e, 0x22),
            pill_text: Rgba::rgb(255, 255, 255),
            box_window: Rgba::rgb(0x1f, 0x5f, 0xe0),
            box_crossing: Rgba::rgb(0x2e, 0xa0, 0x5a),
            box_fill_alpha: 0.13,
            box_dash: [6.0, 4.0],
            erase: Rgba::rgb(0xff, 0x3b, 0x4d),
            guide: Rgba::rgb(0x3a, 0x40, 0x4c).with_alpha(alpha(0.85)),
            protractor: Rgba::rgb(0x1c, 0x1e, 0x22),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HiddenLines {
    Off,
    Dashed,
    Faint,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            sky_top: Rgba::rgb(0xc9, 0xd9, 0xea),
            sky_horizon: Rgba::rgb(0xee, 0xf2, 0xf5),
            ground_near_horizon: Rgba::rgb(0xe9, 0xeb, 0xe9),
            ground_bottom: Rgba::rgb(0xd9, 0xdc, 0xd9),
            grid_minor: Rgba::rgb(0xb4, 0xba, 0xbf),
            grid_major: Rgba::rgb(0x8f, 0x95, 0x9b),
            grid_minor_alpha: 0.55,
            grid_major_alpha: 0.8,
            grid_minor_width: 0.8,
            grid_major_width: 1.0,
            grid_spacing: 1.0,
            grid_lines: 30,
            grid_major_every: 5,
            grid_fade: 45.0,
            axes: [
                Rgba::rgb(0xd6, 0x38, 0x2d),
                Rgba::rgb(0x2a, 0x94, 0x48),
                Rgba::rgb(0x2b, 0x57, 0xd6),
            ],
            axis_length: 60.0,
            axis_width: 1.4,
            axis_alpha: 0.9,
            axis_negative_alpha: 0.55,
            axis_negative_dash: [2.0, 4.0],
            face_front: Rgba::rgb(248, 247, 244),
            face_back: Rgba::rgb(150, 168, 198),
            light: [0.3, -0.5, 0.8],
            ambient: 0.66,
            diffuse: 0.34,
            edge: Rgba::rgb(0x16, 0x18, 0x1c),
            edge_width: 1.5,
            faded_alpha: 70,
            open_box: Rgba::rgb(0x55, 0x5b, 0x66),
            open_box_dash: [5.0, 4.0],
            health_open: Rgba::rgb(0xe0, 0x2f, 0x2f),
            health_many: Rgba::rgb(0xf0, 0x8c, 0x1a),
            health_stray: Rgba::rgb(0x8a, 0x8f, 0x99),
            health_reversed: Rgba([0xd0, 0x3c, 0xc8, 0x70]),
            health_outline: Rgba::rgb(0x9b, 0x4d, 0xf0),
            profile_width: 0.0,
            hover: Rgba::rgb(0xff, 0x8a, 0x00),
            hover_face_alpha: 0.24,
            selected: Rgba::rgb(0x1f, 0x5f, 0xe0),
            selected_face_alpha: 0.12,
            highlight_width: 2.8,
            stipple: Rgba::rgb(31, 95, 224).with_alpha(alpha(0.9)),
            stipple_tile: 6.0,
            stipple_dot: 1.4,
            vertex_dot_radius: 4.5,
            vertex_ring: Rgba::rgb(255, 255, 255),
            vertex_ring_width: 1.5,
            shadow_alpha: 0.17,
            shadow_dir: [0.35, 0.3, -1.0],
            hidden_lines: HiddenLines::Off,
            hidden_alpha: 0.35,
            hidden_dash: [4.0, 4.0],
            feedback: Feedback::default(),
        }
    }
}
