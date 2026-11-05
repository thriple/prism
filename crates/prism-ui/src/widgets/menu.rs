//! Menu parts (prototype `.mnu`, `.mi`, kit `ContextMenu`): a floating card,
//! rows with a check, a label and a key, separators and a heading.

use egui::{Align2, Pos2, Rect, Response, Sense, Ui, pos2, vec2};

use crate::anim;
use crate::emboss;
use crate::spring;
use crate::theme::{self, color, depth, radius, size, text};

/// Paint a menu's card behind its rows.
pub fn card(shapes: &mut Vec<egui::Shape>, rect: Rect) {
    emboss::floating(shapes, rect, radius::MENU, color::SURFACE, 1.0);
}

/// What a menu row shows at its right.
pub enum RowEnd<'a> {
    None,
    Key(&'a str),
    Submenu,
}

/// One menu row: hover sinks it slightly, greyed rows are dimmed and inert.
pub struct MenuRow<'a> {
    pub label: &'a str,
    pub end: RowEnd<'a>,
    pub checked: bool,
    pub enabled: bool,
    /// Drawn as hovered (keyboard highlight or open submenu).
    pub highlight: bool,
    pub width: f32,
}

impl MenuRow<'_> {
    pub fn show(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(vec2(self.width, size::MENU_ROW_HEIGHT), Sense::click());
        let hot = self.enabled && (self.highlight || response.hovered());
        let sink = anim::spring(ui.ctx(), response.id.with("sink"), if hot { 1.0 } else { 0.0 }, spring::SPRING_SNAPPY);
        if ui.is_rect_visible(rect) {
            let mut painter = ui.painter().clone();
            if !self.enabled {
                painter.multiply_opacity(color::GREYED_OPACITY);
            }
            if sink > 0.01 {
                let mut shapes = Vec::new();
                emboss::inset(&mut shapes, rect, radius::MENU_ROW, depth::MENU_INSET * sink);
                painter.extend(shapes);
            }
            let [pad_l, pad_r] = size::MENU_ROW_PAD;
            let check = pos2(rect.left() + pad_l + size::MENU_CHECK * 0.5, rect.center().y);
            if self.checked {
                painter.circle_filled(check, size::MENU_CHECK_DOT, color::ACCENT);
            }
            let label_x = rect.left() + pad_l + size::MENU_CHECK + pad_l;
            painter.text(pos2(label_x, rect.center().y), Align2::LEFT_CENTER, self.label, theme::body(), color::INK);
            let right = pos2(rect.right() - pad_r, rect.center().y);
            match self.end {
                RowEnd::Key(k) => {
                    painter.text(right, Align2::RIGHT_CENTER, k, key_font(), color::MUTED);
                }
                RowEnd::Submenu => {
                    let a = size::MENU_ARROW;
                    painter.add(egui::Shape::convex_polygon(
                        vec![right + vec2(-a, -a), right + vec2(0.0, 0.0), right + vec2(-a, a)],
                        color::MUTED,
                        egui::Stroke::NONE,
                    ));
                }
                RowEnd::None => {}
            }
        }
        response
    }
}

/// The width a row needs for its label and key.
pub fn row_width(ui: &Ui, label: &str, key: &str) -> f32 {
    let f = ui.fonts_mut(|f| {
        let l = f.layout_no_wrap(label.to_owned(), theme::body(), color::INK).size().x;
        let k = f.layout_no_wrap(key.to_owned(), key_font(), color::MUTED).size().x;
        l + k
    });
    let [pad_l, pad_r] = size::MENU_ROW_PAD;
    pad_l * 2.0 + size::MENU_CHECK + f + size::MENU_KEY_GAP + pad_r
}

/// The font for keys at the right of menu rows.
pub fn key_font() -> egui::FontId {
    egui::FontId::new(text::MENU_KEY, egui::FontFamily::Name(text::MONO_MEDIUM.into()))
}

/// A thin line between groups of rows.
pub fn separator(ui: &mut Ui, width: f32) {
    let [v, h] = size::MENU_SEPARATOR;
    let (rect, _) = ui.allocate_exact_size(vec2(width, 2.0 * v + size::EDGE_WIDTH), Sense::hover());
    let y = rect.center().y;
    ui.painter().line_segment(
        [pos2(rect.left() + h, y), pos2(rect.right() - h, y)],
        egui::Stroke::new(size::EDGE_WIDTH, color::SEPARATOR),
    );
}

/// A small uppercase heading (the context menu says what it applies to).
pub fn heading(ui: &mut Ui, label: &str, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, size::MENU_HEAD_HEIGHT), Sense::hover());
    let job = caps(label, color::MUTED);
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let pos = Pos2::new(rect.left() + size::MENU_ROW_PAD[1], rect.center().y - galley.size().y * 0.5);
    ui.painter().galley(pos, galley, color::MUTED);
}

/// An uppercase, letter-spaced label, as the prototype's group titles.
pub fn caps(label: &str, colour: egui::Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &label.to_uppercase(),
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::new(text::LABEL, egui::FontFamily::Name(text::SANS_SEMIBOLD.into())),
            color: colour,
            extra_letter_spacing: text::LABEL_SPACING,
            ..Default::default()
        },
    );
    job
}
