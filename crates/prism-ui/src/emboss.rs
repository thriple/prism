//! The kit's emboss: every soft surface has four shadows, so it can move
//! smoothly from raised, through flat, to pressed in.
//!
//! Depth `d`: 1 raised, 0 flat, -1 pressed. `s` is the shadow offset in points
//! at depth 1; the blur is twice the offset, as in the kit (`Glass.tsx`).

use egui::epaint::RectShape;
use egui::{Color32, Mesh, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::theme::{self, color, depth};

/// A soft surface: outer shadows, the fill, then inner shadows on top.
pub fn soft(out: &mut Vec<Shape>, rect: Rect, radius: f32, d: f32, s: f32, fill: Color32) {
    let radius = fit(rect, radius);
    outer(out, rect, radius, d.max(0.0) * s);
    out.push(RectShape::filled(rect, theme::corner(radius), fill).into());
    inset(out, rect, radius, (-d).max(0.0) * s);
}

/// The raised half: a dark shadow down-right and a light one up-left.
pub fn outer(out: &mut Vec<Shape>, rect: Rect, radius: f32, a: f32) {
    if a <= 0.0 {
        return;
    }
    let blur = 2.0 * a * depth::BLUR_RAMP;
    for (offset, c) in [(a, color::SHADOW_DARK), (-a, color::SHADOW_LIGHT)] {
        out.push(
            RectShape::filled(
                rect.translate(vec2(offset, offset)),
                theme::corner(radius),
                c,
            )
            .with_blur_width(blur)
            .into(),
        );
    }
}

/// The pressed half: the same pair, inside the surface.
pub fn inset(out: &mut Vec<Shape>, rect: Rect, radius: f32, b: f32) {
    if b <= 0.0 {
        return;
    }
    let blur = 2.0 * b;
    inner_shadow(out, rect, radius, vec2(b, b), blur, color::SHADOW_DARK);
    inner_shadow(out, rect, radius, vec2(-b, -b), blur, color::SHADOW_LIGHT);
}

/// A floating card (window, menu, popup): a strong drop shadow, a light edge
/// up-left, the fill and a faint 1 point border.
pub fn floating(out: &mut Vec<Shape>, rect: Rect, radius: f32, fill: Color32, lift: f32) {
    let radius = fit(rect, radius);
    let o = depth::FLOAT_OFFSET * lift;
    let blur = depth::FLOAT_BLUR * depth::BLUR_RAMP * lift;
    if lift > 0.0 {
        out.push(
            RectShape::filled(
                rect.translate(vec2(o, o)),
                theme::corner(radius),
                color::SHADOW_DARK,
            )
            .with_blur_width(blur)
            .into(),
        );
        outer_light(out, rect, radius, depth::CARD * lift);
    }
    out.push(
        RectShape::new(
            rect,
            theme::corner(radius),
            fill,
            Stroke::new(theme::size::EDGE_WIDTH, color::FLOAT_EDGE),
            StrokeKind::Inside,
        )
        .into(),
    );
}

fn outer_light(out: &mut Vec<Shape>, rect: Rect, radius: f32, a: f32) {
    out.push(
        RectShape::filled(
            rect.translate(vec2(-a, -a)),
            theme::corner(radius),
            color::SHADOW_LIGHT,
        )
        .with_blur_width(2.0 * a * depth::BLUR_RAMP)
        .into(),
    );
}

/// A radius no larger than half the shorter side.
pub fn fit(rect: Rect, radius: f32) -> f32 {
    radius
        .min(rect.width() * 0.5)
        .min(rect.height() * 0.5)
        .max(0.0)
}

/// Rings of the inner-shadow mesh, and points per corner on each ring.
const RINGS: usize = 10;
const CORNER_POINTS: usize = 7;

/// An inner shadow, as CSS `inset` draws it: everything inside `rect` that a
/// copy of `rect` moved by `offset` does not cover is shadowed, blurred.
/// Built as a mesh of rings following the rounded outline, coloured per vertex.
pub fn inner_shadow(
    out: &mut Vec<Shape>,
    rect: Rect,
    radius: f32,
    offset: Vec2,
    blur: f32,
    color: Color32,
) {
    let radius = fit(rect, radius);
    let ramp = (blur * depth::BLUR_RAMP).max(1.0);
    let half_short = rect.width().min(rect.height()) * 0.5;
    let reach = (offset.x.abs().max(offset.y.abs()) + ramp).min(half_short);
    let caster = rect.translate(offset);
    let alpha = |p: Pos2| (0.5 + rounded_rect_distance(p, caster, radius) / ramp).clamp(0.0, 1.0);

    let mut mesh = Mesh::default();
    let per_ring = 4 * CORNER_POINTS;
    for k in 0..=RINGS {
        // Rings are closer together near the edge, where the shadow changes fastest.
        let s = reach * (k as f32 / RINGS as f32).powf(1.6);
        for p in outline(rect, radius, s) {
            mesh.colored_vertex(p, color.gamma_multiply(alpha(p)));
        }
        if k > 0 {
            let a = ((k - 1) * per_ring) as u32;
            let b = (k * per_ring) as u32;
            for i in 0..per_ring as u32 {
                let j = (i + 1) % per_ring as u32;
                mesh.add_triangle(a + i, a + j, b + i);
                mesh.add_triangle(a + j, b + j, b + i);
            }
        }
    }
    // Fill the middle from its centre.
    let centre = rect.center();
    let c = mesh.vertices.len() as u32;
    mesh.colored_vertex(centre, color.gamma_multiply(alpha(centre)));
    let last = (RINGS * per_ring) as u32;
    for i in 0..per_ring as u32 {
        mesh.add_triangle(last + i, last + (i + 1) % per_ring as u32, c);
    }
    out.push(Shape::mesh(mesh));
}

/// Points around a rounded rectangle shrunk inwards by `s`, clockwise from the
/// bottom-right corner. Always the same number of points.
fn outline(rect: Rect, radius: f32, s: f32) -> Vec<Pos2> {
    let s = s.min(rect.width().min(rect.height()) * 0.5);
    let r = (radius - s).max(0.0);
    let inner = rect.shrink(s);
    let corners = [
        (pos2(inner.right() - r, inner.bottom() - r), 0.0),
        (pos2(inner.left() + r, inner.bottom() - r), 90.0),
        (pos2(inner.left() + r, inner.top() + r), 180.0),
        (pos2(inner.right() - r, inner.top() + r), 270.0),
    ];
    let mut pts = Vec::with_capacity(4 * CORNER_POINTS);
    for (c, start) in corners {
        for i in 0..CORNER_POINTS {
            let deg: f32 = start + 90.0 * i as f32 / (CORNER_POINTS - 1) as f32;
            let (sin, cos) = deg.to_radians().sin_cos();
            pts.push(c + vec2(cos, sin) * r);
        }
    }
    pts
}

/// Signed distance from `p` to a rounded rectangle: negative inside.
pub fn rounded_rect_distance(p: Pos2, rect: Rect, radius: f32) -> f32 {
    let half = rect.size() * 0.5 - Vec2::splat(radius);
    let q = (p - rect.center()).abs() - half;
    let outside = vec2(q.x.max(0.0), q.y.max(0.0)).length();
    outside + q.x.max(q.y).min(0.0) - radius
}

/// The 2 point accent focus ring, 4 points out from the control.
pub fn focus_ring(out: &mut Vec<Shape>, rect: Rect, radius: f32) {
    let gap = theme::size::FOCUS_GAP;
    out.push(
        RectShape::stroke(
            rect.expand(gap),
            theme::corner(fit(rect, radius) + gap),
            Stroke::new(theme::size::FOCUS_WIDTH, color::ACCENT),
            StrokeKind::Outside,
        )
        .into(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_is_negative_inside_and_positive_outside() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 40.0));
        assert!(rounded_rect_distance(pos2(50.0, 20.0), r, 10.0) < 0.0);
        assert!((rounded_rect_distance(pos2(50.0, -5.0), r, 10.0) - 5.0).abs() < 1e-4);
        // Just outside a rounded corner, though inside the square corner.
        assert!(rounded_rect_distance(pos2(1.0, 1.0), r, 10.0) > 0.0);
    }

    #[test]
    fn outline_has_a_fixed_size_and_stays_inside() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(30.0, 30.0));
        for s in [0.0, 5.0, 15.0, 40.0] {
            let pts = outline(r, 15.0, s);
            assert_eq!(pts.len(), 4 * CORNER_POINTS);
            assert!(pts.iter().all(|p| r.expand(1e-3).contains(*p)));
        }
    }

    #[test]
    fn soft_surfaces_have_shadows_only_on_the_right_side() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(80.0, 40.0));
        let count = |d: f32| {
            let mut v = Vec::new();
            soft(&mut v, r, 16.0, d, 6.0, color::SURFACE);
            v.len()
        };
        assert_eq!(count(0.0), 1, "flat: just the fill");
        assert_eq!(count(1.0), 3, "raised: two outer shadows and the fill");
        assert_eq!(count(-1.0), 3, "pressed: the fill and two inner shadows");
    }

    #[test]
    fn inner_shadow_is_dark_where_the_offset_uncovers_the_surface() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(80.0, 40.0));
        let mut v = Vec::new();
        inner_shadow(&mut v, r, 16.0, vec2(4.0, 4.0), 8.0, color::SHADOW_DARK);
        let Shape::Mesh(mesh) = &v[0] else {
            panic!("expected a mesh")
        };
        // Strongest shadow on the outer ring along the top and along the bottom.
        let edge = |y: f32| {
            mesh.vertices
                .iter()
                .filter(|v| (v.pos.y - y).abs() < 1e-3 && (16.0..=64.0).contains(&v.pos.x))
                .map(|v| v.color.a())
                .max()
                .unwrap_or(0)
        };
        // A copy moved down uncovers the top edge (shadow), and covers the bottom.
        assert!(
            edge(0.0) > edge(40.0),
            "top {} bottom {}",
            edge(0.0),
            edge(40.0)
        );
        let centre = mesh.vertices.last().map(|v| v.color.a());
        assert_eq!(centre, Some(0), "no shadow in the middle");
    }
}
