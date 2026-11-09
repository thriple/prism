//! The two panels drawn over the viewport: "Prism debug" and "Theme demo".

use egui::{Context, RichText};
use prism_ui::theme::{self, color};
use prism_ui::widgets::{SoftButton, SoftPanel, SoftSelect, SoftSlider, SoftToggle, groove};

use crate::frame_stats::FrameStats;
use crate::input_log::{InputLog, Modifiers};
use crate::settings::{REDUCE_MOTION_CHOICES, Settings};
use crate::viewport::Viewport;

/// Which build this is, set by the release workflow (for example "build 12").
/// Local builds say "local".
pub const BUILD: &str = match option_env!("PRISM_BUILD") {
    Some(b) => b,
    None => "local",
};

/// What the debug panel shows.
pub struct DebugInfo<'a> {
    pub stats: &'a FrameStats,
    pub log: &'a InputLog,
    pub cursor: Option<(f64, f64)>,
    pub modifiers: Modifiers,
    pub size: [u32; 2],
    pub scale: f64,
    pub adapter: String,
    pub reduce_motion: bool,
    pub aurora_redraws: u64,
}

/// The demo controls' values.
pub struct Demo {
    lights: bool,
    level: f32,
    scene: usize,
    saves: u32,
}

impl Default for Demo {
    fn default() -> Self {
        Self {
            lights: true,
            level: 64.0,
            scene: 0,
            saves: 0,
        }
    }
}

const SCENES: [&str; 4] = ["Reading", "Cinema", "Dinner", "Night"];
const DEBUG_WIDTH: f32 = 400.0;
/// Panel positions, from the viewport's corner.
const DEBUG_POS: [f32; 2] = [24.0, 24.0];
const DEMO_FROM_RIGHT: f32 = 380.0;

/// Which panels are showing (Help > Debug Panel, Help > Theme Demo).
pub struct Shown {
    pub debug: bool,
    pub demo: bool,
}

/// Draw the panels that are on, starting inside the viewport.
pub fn draw(
    ctx: &Context,
    viewport: egui::Rect,
    shown: Shown,
    info: &DebugInfo<'_>,
    settings: &mut Settings,
    demo: &mut Demo,
    model_view: &mut Viewport,
) {
    let at = |p: [f32; 2]| viewport.min + egui::vec2(p[0], p[1]);
    if shown.debug {
        SoftPanel::new("Prism debug")
            .default_pos(at(DEBUG_POS))
            .width(DEBUG_WIDTH)
            .show(ctx, |ui| debug_panel(ui, info, model_view));
    }
    if shown.demo {
        let right = viewport.width() - DEMO_FROM_RIGHT;
        SoftPanel::new("Theme demo")
            .default_pos(at([right.max(DEBUG_POS[0]), DEBUG_POS[1]]))
            .show(ctx, |ui| demo_panel(ui, settings, demo));
    }
}

fn mono(ui: &mut egui::Ui, text: String) {
    ui.label(RichText::new(text).font(theme::mono()).color(color::INK));
}

fn debug_panel(ui: &mut egui::Ui, info: &DebugInfo<'_>, viewport: &mut Viewport) {
    let fps = info
        .stats
        .fps()
        .map_or_else(|| "…".to_string(), |f| format!("{f:.0}"));
    mono(ui, format!("build {BUILD}"));
    mono(ui, format!("frame {}   fps {fps}", info.stats.frames()));
    mono(
        ui,
        format!(
            "size {}x{} px   scale {:.2}",
            info.size[0], info.size[1], info.scale
        ),
    );
    mono(ui, format!("gpu {}", info.adapter));
    let cursor = info.cursor.map_or_else(
        || "outside".to_string(),
        |(x, y)| format!("({x:.0}, {y:.0})"),
    );
    mono(ui, format!("cursor {cursor}   mods {}", info.modifiers));
    mono(
        ui,
        format!(
            "reduce motion {}   aurora drawn {}x",
            if info.reduce_motion { "on" } else { "off" },
            info.aurora_redraws
        ),
    );
    groove(ui);
    viewport_controls(ui, viewport);
    groove(ui);
    ui.label(
        RichText::new(format!("Input events ({} total)", info.log.total())).color(color::MUTED),
    );
    for line in info.log.lines() {
        mono(ui, line);
    }
}

/// The viewport's tuning (the prototype's Feel Panel sliders) and counts.
fn viewport_controls(ui: &mut egui::Ui, v: &mut Viewport) {
    let s = &mut v.session;
    let m = s.model();
    mono(
        ui,
        format!(
            "model {} faces, {} edges, {} points",
            m.face_count(),
            m.edge_count(),
            m.vertex_count()
        ),
    );
    let t = s.timings();
    mono(
        ui,
        format!(
            "pick {:.2} ms   infer {:.2} ms   scene {:.1} ms",
            t.pick_ms, t.infer_ms, t.scene_ms
        ),
    );
    let mut ap = s.settings.aperture as f32;
    if ui
        .add(SoftSlider::new(&mut ap, 0.5..=2.5, "Pick aperture").suffix("x"))
        .changed()
    {
        s.settings.aperture = f64::from(ap);
    }
    let mut rel = s.settings.release as f32;
    if ui
        .add(SoftSlider::new(&mut rel, 1.0..=3.0, "Snap release").suffix("x"))
        .changed()
    {
        s.settings.release = f64::from(rel);
    }
    let mut wake = (s.settings.wake * 1000.0) as f32;
    if ui
        .add(SoftSlider::new(&mut wake, 50.0..=800.0, "Hover wake").suffix(" ms"))
        .changed()
    {
        s.settings.wake = f64::from(wake) / 1000.0;
    }
    ui.add(SoftToggle::new(&mut s.ease, "Ease zoom"));
}

fn demo_panel(ui: &mut egui::Ui, settings: &mut Settings, demo: &mut Demo) {
    ui.horizontal(|ui| {
        if ui.add(SoftButton::new("Save scene")).clicked() {
            demo.saves += 1;
        }
        let saved = match demo.saves {
            0 => String::new(),
            1 => "Saved once".to_owned(),
            n => format!("Saved {n} times"),
        };
        ui.label(RichText::new(saved).color(color::MUTED));
    });
    ui.add(SoftToggle::new(&mut demo.lights, "Lights"));
    ui.add(SoftSlider::new(&mut demo.level, 0.0..=100.0, "Brightness").suffix("%"));
    ui.add(SoftSelect::new(&mut demo.scene, &SCENES, "Scene"));
    ui.add(SoftButton::new("Not built yet").enabled(false));
    groove(ui);
    ui.label(
        RichText::new("Settings")
            .font(theme::body_strong())
            .color(color::INK),
    );
    ui.add(SoftSelect::new(
        &mut settings.reduce_motion,
        &REDUCE_MOTION_CHOICES,
        "Reduce motion",
    ));
    ui.add(SoftToggle::new(
        &mut settings.animate_aurora,
        "Animate aurora",
    ));
    ui.add(SoftToggle::new(
        &mut settings.continuous_redraw,
        "Continuous redraw",
    ));
}
