//! The aurora seen through glass cutouts: a base colour and seven soft blobs.
//!
//! It is drawn once, at one eighth of the window's resolution (it is meant to
//! be blurry, and upscaling does the blurring), and redrawn only when the
//! window size changes, or every frame if "animate aurora" is on. Cutouts
//! show the part of it that sits behind them, so they all look through to
//! one layer. The blob paths follow the kit's `aurora.ts`.

use egui::{Color32, ColorImage, Context, Id, Rect, TextureHandle, TextureId, TextureOptions};

use crate::anim::motion;
use crate::theme::{self, color};

/// A blob's centre, how far it wanders, its periods in seconds and its radius,
/// all as fractions of the image.
struct Blob {
    cx: f32,
    cy: f32,
    ax: f32,
    ay: f32,
    px: f32,
    py: f32,
    r: f32,
}

const BLOBS: [Blob; 7] = [
    Blob {
        cx: 0.20,
        cy: 0.25,
        ax: 0.20,
        ay: 0.18,
        px: 37.0,
        py: 53.0,
        r: 0.30,
    },
    Blob {
        cx: 0.70,
        cy: 0.20,
        ax: 0.22,
        ay: 0.20,
        px: 47.0,
        py: 31.0,
        r: 0.28,
    },
    Blob {
        cx: 0.50,
        cy: 0.70,
        ax: 0.30,
        ay: 0.18,
        px: 59.0,
        py: 41.0,
        r: 0.34,
    },
    Blob {
        cx: 0.15,
        cy: 0.75,
        ax: 0.16,
        ay: 0.22,
        px: 43.0,
        py: 67.0,
        r: 0.26,
    },
    Blob {
        cx: 0.85,
        cy: 0.65,
        ax: 0.14,
        ay: 0.26,
        px: 71.0,
        py: 49.0,
        r: 0.28,
    },
    Blob {
        cx: 0.45,
        cy: 0.35,
        ax: 0.32,
        ay: 0.24,
        px: 53.0,
        py: 61.0,
        r: 0.22,
    },
    Blob {
        cx: 0.80,
        cy: 0.90,
        ax: 0.24,
        ay: 0.14,
        px: 67.0,
        py: 29.0,
        r: 0.24,
    },
];

/// The aurora image is this fraction of the window's size in pixels.
pub const SCALE: f32 = 1.0 / 8.0;
const MIN_SIZE: [usize; 2] = [64, 48];
/// Grain texture size in pixels.
const GRAIN: usize = 64;

/// Draw the aurora at time `t` (seconds). Pure: the same inputs give the same image.
pub fn render(w: usize, h: usize, t: f64) -> ColorImage {
    let rgb = |c: Color32| [f32::from(c.r()), f32::from(c.g()), f32::from(c.b())];
    let base = rgb(color::AURORA_BASE);
    let size = w.max(h) as f32;
    let tau = std::f64::consts::TAU;
    let blobs: Vec<([f32; 3], f32, f32, f32)> = BLOBS
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let fi = i as f64;
            let x = (b.cx + b.ax * (tau * t / f64::from(b.px) + fi).sin() as f32) * w as f32;
            let y = (b.cy + b.ay * (tau * t / f64::from(b.py) + fi * 2.0).cos() as f32) * h as f32;
            let breathe = 0.85 + 0.15 * (tau * t / (f64::from(b.px) * 0.7) + fi).sin() as f32;
            let r = b.r * size * breathe;
            (rgb(color::AURORA[i % color::AURORA.len()]), x, y, r)
        })
        .collect();

    let mut pixels = Vec::with_capacity(w * h);
    for py in 0..h {
        for px in 0..w {
            let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
            let mut c = base;
            for (col, x, y, r) in &blobs {
                let d = ((fx - x).powi(2) + (fy - y).powi(2)).sqrt() / r;
                // The kit's radial gradient: .95 at the centre, .45 halfway, 0 at the edge.
                let a = if d < 0.5 {
                    0.95 - d
                } else if d < 1.0 {
                    0.45 * (1.0 - d) * 2.0
                } else {
                    0.0
                };
                for k in 0..3 {
                    c[k] += (col[k] - c[k]) * a;
                }
            }
            pixels.push(Color32::from_rgb(c[0] as u8, c[1] as u8, c[2] as u8));
        }
    }
    ColorImage::new([w, h], pixels)
}

/// A tile of film grain: neutral grey noise at low opacity.
pub fn grain() -> ColorImage {
    let a = color::GRAIN_STRENGTH;
    let pixels = (0..GRAIN * GRAIN)
        .map(|i| {
            // A small integer hash, so the grain is the same every run.
            let mut x = i as u32 ^ 0x9e37_79b9;
            x = (x ^ (x >> 16)).wrapping_mul(0x7feb_352d);
            x = (x ^ (x >> 15)).wrapping_mul(0x846c_a68b);
            x ^= x >> 16;
            let g = (x & 0xff) as f32 / 255.0;
            let v = (g * a * 255.0) as u8;
            Color32::from_rgba_premultiplied(v, v, v, (a * 255.0) as u8)
        })
        .collect();
    ColorImage::new([GRAIN, GRAIN], pixels)
}

/// What cutouts need to draw: the textures and the window area they span.
#[derive(Clone, Copy)]
pub struct AuroraRef {
    pub texture: TextureId,
    pub grain: TextureId,
    pub screen: Rect,
}

/// Owns the aurora texture and decides when to redraw it.
#[derive(Default)]
pub struct Aurora {
    texture: Option<TextureHandle>,
    grain: Option<TextureHandle>,
    size: [usize; 2],
    t: f64,
    last_time: Option<f64>,
    redraws: u64,
}

impl Aurora {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many times the image has been drawn (for tests and the debug panel).
    pub fn redraws(&self) -> u64 {
        self.redraws
    }

    /// Call once per frame before drawing widgets.
    pub fn prepare(&mut self, ctx: &Context) {
        let screen = ctx.viewport_rect();
        let px = screen.size() * ctx.pixels_per_point() * SCALE;
        let size = [
            (px.x.round() as usize).max(MIN_SIZE[0]),
            (px.y.round() as usize).max(MIN_SIZE[1]),
        ];
        let settings = motion(ctx);
        let animate = settings.animate_aurora && !settings.reduce_motion;
        let now = ctx.input(|i| i.time);
        if animate {
            let dt = self
                .last_time
                .map_or(0.0, |l| (now - l).clamp(0.0, theme::motion::MAX_STEP));
            self.t += dt;
            ctx.request_repaint();
        }
        self.last_time = Some(now);

        if self.texture.is_none() || size != self.size || animate {
            let image = render(size[0], size[1], self.t);
            match &mut self.texture {
                Some(tex) => tex.set(image, TextureOptions::LINEAR),
                None => {
                    self.texture = Some(ctx.load_texture("aurora", image, TextureOptions::LINEAR))
                }
            }
            self.size = size;
            self.redraws += 1;
        }
        let grain = self.grain.get_or_insert_with(|| {
            ctx.load_texture("grain", grain(), TextureOptions::NEAREST_REPEAT)
        });
        let r = AuroraRef {
            texture: self
                .texture
                .as_ref()
                .map_or(TextureId::default(), |t| t.id()),
            grain: grain.id(),
            screen,
        };
        ctx.data_mut(|d| d.insert_temp(id(), r));
    }
}

fn id() -> Id {
    Id::NULL.with("prism-ui-aurora")
}

/// The aurora prepared this frame, if any.
pub fn current(ctx: &Context) -> Option<AuroraRef> {
    ctx.data(|d| d.get_temp(id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_is_deterministic_and_coloured() {
        let a = render(80, 50, 0.0);
        let b = render(80, 50, 0.0);
        assert_eq!(a.pixels, b.pixels);
        assert_eq!(a.size, [80, 50]);
        // The blobs lift it well above the dark base somewhere.
        let brightest = a
            .pixels
            .iter()
            .map(|p| u32::from(p.r()) + u32::from(p.g()) + u32::from(p.b()))
            .max();
        assert!(brightest.unwrap_or(0) > 200);
    }

    #[test]
    fn time_moves_the_blobs() {
        assert_ne!(render(40, 30, 0.0).pixels, render(40, 30, 10.0).pixels);
    }

    #[test]
    fn redrawn_only_when_the_size_changes() {
        let ctx = Context::default();
        let mut aurora = Aurora::new();
        let frame = |aurora: &mut Aurora, time: f64, w: f32| {
            let input = egui::RawInput {
                time: Some(time),
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, 600.0))),
                ..Default::default()
            };
            ctx.run_pass(input, |c| aurora.prepare(c))
                .textures_delta
                .clear();
        };
        frame(&mut aurora, 0.0, 800.0);
        frame(&mut aurora, 0.1, 800.0);
        frame(&mut aurora, 0.2, 800.0);
        assert_eq!(aurora.redraws(), 1);
        frame(&mut aurora, 0.3, 1000.0);
        assert_eq!(aurora.redraws(), 2);
    }
}
