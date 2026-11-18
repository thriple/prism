//! Text for Prism (Annotation & Sections part 3): three bundled fonts,
//! shaping, and glyph outlines as closed contours.
//!
//! Pure and deterministic: the same text, font and size give the same
//! contours on every machine, because the fonts are bundled (IBM Plex Sans,
//! Serif and Mono, SIL OFL; DECISIONS #12) and no system font is read.
//! Shaping (kerning, ligatures, marks) is `rustybuzz`'s; outlines come
//! from `ttf-parser`, with curves flattened into short straight pieces.

use prism_math::DVec2;
use serde::{Deserialize, Serialize};

const SANS: &[u8] = include_bytes!("../../prism-ui/assets/fonts/IBMPlexSans-Regular.ttf");
const SERIF: &[u8] = include_bytes!("../../prism-ui/assets/fonts/IBMPlexSerif-Regular.ttf");
const MONO: &[u8] = include_bytes!("../../prism-ui/assets/fonts/IBMPlexMono-Regular.ttf");

/// Pieces a quadratic and a cubic curve are cut into.
const QUAD_STEPS: usize = 8;
const CUBIC_STEPS: usize = 12;
/// A line of text is this many cap heights from the next, when the font
/// gives no better.
const LINE_GAP_CAPS: f64 = 1.9;

/// The bundled fonts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Font {
    #[default]
    Sans,
    Serif,
    Mono,
}

impl Font {
    pub const ALL: [Font; 3] = [Font::Sans, Font::Serif, Font::Mono];

    pub fn name(self) -> &'static str {
        match self {
            Font::Sans => "IBM Plex Sans",
            Font::Serif => "IBM Plex Serif",
            Font::Mono => "IBM Plex Mono",
        }
    }

    /// The short word the window and typed settings use.
    pub fn word(self) -> &'static str {
        match self {
            Font::Sans => "Sans",
            Font::Serif => "Serif",
            Font::Mono => "Mono",
        }
    }

    pub fn from_word(w: &str) -> Option<Font> {
        Font::ALL
            .into_iter()
            .find(|f| f.word().eq_ignore_ascii_case(w.trim()))
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            Font::Sans => SANS,
            Font::Serif => SERIF,
            Font::Mono => MONO,
        }
    }
}

/// Text laid out: every glyph's closed contours in one plane, `x` along the
/// line from the start, `y` up from the first line's baseline (later lines
/// are below, at negative `y`). Lengths are in the unit `cap_height` was
/// given in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Laid {
    /// Closed contours (the last point joins the first), the font's own
    /// winding: an outer boundary and its holes wind opposite ways.
    pub contours: Vec<Vec<DVec2>>,
    /// The widest line's advance.
    pub width: f64,
    /// How far the lines reach below the first baseline (zero for one
    /// line), plus the descender of the last.
    pub depth: f64,
    /// Characters the font has no glyph for (spaces and controls are not
    /// counted).
    pub missing: usize,
}

/// Flatten one glyph's outline into contours, scaled and moved to `at`.
struct Flatten {
    scale: f64,
    at: DVec2,
    done: Vec<Vec<DVec2>>,
    cur: Vec<DVec2>,
    last: DVec2,
}

impl Flatten {
    fn p(&self, x: f32, y: f32) -> DVec2 {
        self.at + DVec2::new(f64::from(x), f64::from(y)) * self.scale
    }

    /// End the contour being walked, if it is one.
    fn end(&mut self) {
        let mut c = std::mem::take(&mut self.cur);
        if c.len() > 1 && c.first() == c.last() {
            c.pop();
        }
        if c.len() >= 3 {
            self.done.push(c);
        }
    }

    fn push(&mut self, q: DVec2) {
        if self.cur.last() != Some(&q) {
            self.cur.push(q);
        }
        self.last = q;
    }
}

impl ttf_parser::OutlineBuilder for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        self.end();
        let q = self.p(x, y);
        self.push(q);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let q = self.p(x, y);
        self.push(q);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (a, b, c) = (self.last, self.p(x1, y1), self.p(x, y));
        for k in 1..=QUAD_STEPS {
            let t = k as f64 / QUAD_STEPS as f64;
            let u = 1.0 - t;
            self.push(a * (u * u) + b * (2.0 * u * t) + c * (t * t));
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b, c, d) = (self.last, self.p(x1, y1), self.p(x2, y2), self.p(x, y));
        for k in 1..=CUBIC_STEPS {
            let t = k as f64 / CUBIC_STEPS as f64;
            let u = 1.0 - t;
            self.push(
                a * (u * u * u) + b * (3.0 * u * u * t) + c * (3.0 * u * t * t) + d * (t * t * t),
            );
        }
    }

    fn close(&mut self) {
        self.end();
    }
}

/// Lay `text` out in `font`, with capital letters `cap_height` tall.
/// Lines are split at newlines. Empty text, or a non-positive height,
/// gives nothing.
pub fn layout(text: &str, font: Font, cap_height: f64) -> Laid {
    let mut out = Laid::default();
    if text.is_empty() || cap_height.is_nan() || cap_height <= 0.0 {
        return out;
    }
    let Some(buzz) = rustybuzz::Face::from_slice(font.bytes(), 0) else {
        return out;
    };
    let units = f64::from(buzz.units_per_em());
    let caps = buzz
        .capital_height()
        .map_or(0.7 * units, |c| f64::from(c).max(1.0));
    let scale = cap_height / caps;
    let metric_gap = f64::from(buzz.ascender() - buzz.descender() + buzz.line_gap()) * scale;
    let line_step = metric_gap.max(LINE_GAP_CAPS * cap_height * 0.8);
    for (n, line) in text.split('\n').enumerate() {
        let y = -(n as f64) * line_step;
        let mut buf = rustybuzz::UnicodeBuffer::new();
        buf.push_str(line);
        buf.guess_segment_properties();
        let shaped = rustybuzz::shape(&buzz, &[], buf);
        let mut x = 0.0;
        for (info, pos) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
            let glyph = ttf_parser::GlyphId(info.glyph_id as u16);
            if info.glyph_id == 0
                && line
                    .get(info.cluster as usize..)
                    .and_then(|s| s.chars().next())
                    .is_some_and(|c| !c.is_whitespace() && !c.is_control())
            {
                out.missing += 1;
            }
            let mut flat = Flatten {
                scale,
                at: DVec2::new(
                    x + f64::from(pos.x_offset) * scale,
                    y + f64::from(pos.y_offset) * scale,
                ),
                done: Vec::new(),
                cur: Vec::new(),
                last: DVec2::ZERO,
            };
            // A character the font lacks draws nothing (not the font's box).
            if info.glyph_id != 0 && buzz.outline_glyph(glyph, &mut flat).is_some() {
                flat.end();
                out.contours.extend(flat.done);
            }
            x += f64::from(pos.x_advance) * scale;
        }
        out.width = out.width.max(x);
        out.depth = -y + f64::from(-buzz.descender()) * scale;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(l: &Laid) -> (DVec2, DVec2) {
        let mut lo = DVec2::splat(f64::INFINITY);
        let mut hi = DVec2::splat(f64::NEG_INFINITY);
        for p in l.contours.iter().flatten() {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        (lo, hi)
    }

    #[test]
    fn capital_letters_are_the_height_asked_for() {
        for font in Font::ALL {
            let l = layout("H", font, 0.5);
            let (lo, hi) = bounds(&l);
            assert!((hi.y - 0.5).abs() < 1e-3, "{font:?} top {}", hi.y);
            assert!(lo.y.abs() < 1e-3, "{font:?} sits on the baseline: {}", lo.y);
            assert_eq!(l.contours.len(), 1, "{font:?}: an H is one contour");
            assert!(l.width > 0.3, "{font:?}");
        }
    }

    #[test]
    fn letters_with_holes_and_dots_have_more_contours() {
        assert_eq!(
            layout("O", Font::Sans, 1.0).contours.len(),
            2,
            "outer and hole"
        );
        assert_eq!(
            layout("i", Font::Sans, 1.0).contours.len(),
            2,
            "dot and stem"
        );
        assert_eq!(
            layout("B", Font::Serif, 1.0).contours.len(),
            3,
            "outer and two holes"
        );
        assert!(
            layout("   ", Font::Sans, 1.0).contours.is_empty(),
            "spaces draw nothing"
        );
    }

    #[test]
    fn text_grows_to_the_right_and_scales_with_height() {
        let one = layout("Prism", Font::Sans, 1.0);
        let two = layout("Prism Prism", Font::Sans, 1.0);
        assert!(two.width > 1.9 * one.width && two.width < 2.5 * one.width);
        let half = layout("Prism", Font::Sans, 0.5);
        assert!((half.width * 2.0 - one.width).abs() < 1e-9);
        assert_eq!(half.contours.len(), one.contours.len());
    }

    #[test]
    fn a_mono_font_gives_every_letter_the_same_width() {
        let a = layout("iiii", Font::Mono, 1.0).width;
        let b = layout("WWWW", Font::Mono, 1.0).width;
        assert!((a - b).abs() < 1e-9, "{a} {b}");
        let s = layout("iiii", Font::Sans, 1.0).width;
        let w = layout("WWWW", Font::Sans, 1.0).width;
        assert!(w > 1.5 * s, "a sans font does not");
    }

    #[test]
    fn lines_go_down_and_the_widest_sets_the_width() {
        let l = layout("Hi\nWide line", Font::Sans, 1.0);
        let (lo, _) = bounds(&l);
        assert!(lo.y < -1.5, "the second line is below the first: {}", lo.y);
        assert!(l.depth > 1.5);
        let wide = layout("Wide line", Font::Sans, 1.0).width;
        assert!((l.width - wide).abs() < 1e-9);
        assert_eq!(layout("\n", Font::Sans, 1.0).contours.len(), 0);
    }

    #[test]
    fn nothing_for_empty_text_or_a_bad_height() {
        assert_eq!(layout("", Font::Sans, 1.0), Laid::default());
        assert_eq!(layout("A", Font::Sans, 0.0), Laid::default());
        assert_eq!(layout("A", Font::Sans, -1.0), Laid::default());
        assert_eq!(layout("A", Font::Sans, f64::NAN), Laid::default());
    }

    #[test]
    fn characters_the_font_lacks_are_counted() {
        let l = layout("a日b", Font::Sans, 1.0);
        assert_eq!(l.missing, 1);
        assert_eq!(
            layout("日", Font::Sans, 1.0).contours.len(),
            0,
            "no box drawn"
        );
        assert_eq!(layout("ab", Font::Sans, 1.0).missing, 0);
        assert_eq!(layout("a b\tc", Font::Sans, 1.0).missing, 0);
    }

    #[test]
    fn the_same_text_gives_the_same_contours() {
        assert_eq!(
            layout("Prism 1.0", Font::Serif, 0.2),
            layout("Prism 1.0", Font::Serif, 0.2)
        );
    }

    #[test]
    fn contours_are_closed_loops_without_repeats() {
        let l = layout("Rgs&", Font::Serif, 1.0);
        for c in &l.contours {
            assert!(c.len() >= 3);
            assert_ne!(c.first(), c.last(), "the last point is not the first again");
            assert!(c.windows(2).all(|w| w[0] != w[1]));
        }
    }

    #[test]
    fn fonts_are_named_and_found_by_their_word() {
        assert_eq!(Font::from_word(" serif "), Some(Font::Serif));
        assert_eq!(Font::from_word("MONO"), Some(Font::Mono));
        assert_eq!(Font::from_word("comic"), None);
        assert_eq!(Font::Sans.name(), "IBM Plex Sans");
    }
}
