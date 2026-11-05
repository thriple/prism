//! Glass cutouts: recessed windows through the surface onto the aurora.
//!
//! Layers, bottom to top: the aurora, a frost tint, film grain, then the
//! recess's inner shadow. Controls sitting in the well are drawn after.

use egui::epaint::RectShape;
use egui::{Painter, Rect, Shape, pos2};

use crate::aurora;
use crate::emboss;
use crate::theme::{self, color, size};

/// A frost tint over part of a cutout: the area it covers and its opacity.
#[derive(Clone, Copy, Debug)]
pub struct Tint {
    pub area: Rect,
    pub frost: f32,
}

/// Paint a cutout in `rect`, frosted evenly at `frost` (0 clear, 1 opaque).
pub fn well(painter: &Painter, rect: Rect, radius: f32, frost: f32, depth: f32) {
    well_tinted(painter, rect, radius, &[Tint { area: rect, frost }], depth);
}

/// Paint a cutout with different frost over different parts (a slider rail).
pub fn well_tinted(painter: &Painter, rect: Rect, radius: f32, tints: &[Tint], depth: f32) {
    let radius = emboss::fit(rect, radius);
    let corner = theme::corner(radius);
    match aurora::current(painter.ctx()) {
        Some(a) => {
            let s = a.screen;
            let uv = Rect::from_min_max(
                pos2(
                    (rect.min.x - s.min.x) / s.width(),
                    (rect.min.y - s.min.y) / s.height(),
                ),
                pos2(
                    (rect.max.x - s.min.x) / s.width(),
                    (rect.max.y - s.min.y) / s.height(),
                ),
            );
            painter
                .add(RectShape::filled(rect, corner, color::UNTINTED).with_texture(a.texture, uv));
            for t in tints {
                let clip = t.area.intersect(painter.clip_rect());
                painter.with_clip_rect(clip).add(RectShape::filled(
                    rect,
                    corner,
                    theme::fade(color::FROST, t.frost),
                ));
            }
            let tile = size::GRAIN_TILE;
            let grain_uv = Rect::from_min_max(
                pos2(rect.min.x / tile, rect.min.y / tile),
                pos2(rect.max.x / tile, rect.max.y / tile),
            );
            painter.add(
                RectShape::filled(rect, corner, color::UNTINTED).with_texture(a.grain, grain_uv),
            );
        }
        // No aurora prepared: a plain recess.
        None => {
            painter.add(RectShape::filled(rect, corner, color::FROST));
        }
    }
    let mut shapes: Vec<Shape> = Vec::new();
    emboss::inset(&mut shapes, rect, radius, depth);
    painter.extend(shapes);
}
