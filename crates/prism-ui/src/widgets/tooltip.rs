//! One shared tooltip that slides between the buttons it describes (kit
//! `TooltipToolbar`), instead of a new tooltip popping up for each.

use egui::{Context, Id, LayerId, Order, Pos2, Rect, pos2, vec2};

use crate::anim;
use crate::emboss;
use crate::spring;
use crate::theme::{self, color, depth, radius, size};

/// Show the tooltip `id` to the right of `anchor` (a button's rect), or hide
/// it when `anchor` is None. It keeps its last text while fading out.
pub fn shared(ctx: &Context, id: Id, anchor: Option<Rect>, text: &str) {
    let mem_id = id.with("text");
    let text = if anchor.is_some() {
        ctx.data_mut(|d| d.insert_temp(mem_id, text.to_owned()));
        text.to_owned()
    } else {
        ctx.data(|d| d.get_temp::<String>(mem_id)).unwrap_or_default()
    };
    let shown = anim::spring(ctx, id.with("shown"), if anchor.is_some() { 1.0 } else { 0.0 }, spring::SPRING_SNAPPY);
    let last: Option<Pos2> = ctx.data(|d| d.get_temp(id.with("at")));
    let target = anchor.map(|r| pos2(r.right() + size::TIP_GAP, r.center().y)).or(last);
    let Some(target) = target else { return };
    ctx.data_mut(|d| d.insert_temp(id.with("at"), target));
    // Slide from the previous button rather than jumping.
    let y = anim::spring(ctx, id.with("y"), target.y, spring::SPRING_SNAPPY);
    if shown < 0.01 || text.is_empty() {
        return;
    }
    let mut painter = ctx.layer_painter(LayerId::new(Order::Tooltip, id));
    painter.multiply_opacity(shown.clamp(0.0, 1.0));
    let galley = painter.layout_no_wrap(text, egui::FontId::new(theme::text::SMALL, theme::body_medium().family), color::INK);
    let [px, py] = size::TIP_PAD;
    let box_size = galley.size() + vec2(2.0 * px, 2.0 * py);
    let r = Rect::from_min_size(pos2(target.x, y - box_size.y * 0.5), box_size);
    let mut shapes = Vec::new();
    emboss::soft(&mut shapes, r, radius::TOOLTIP, 1.0, depth::TOOLTIP, color::SURFACE);
    painter.extend(shapes);
    painter.galley(r.min + vec2(px, py), galley, color::INK);
}
