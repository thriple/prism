//! Edit > Preferences and Help > About (Everyday Commands part 3, brief 33).

use egui::{Context, RichText};
use prism_math::LengthUnit;
use prism_ui::theme::{self, color, size};
use prism_ui::widgets::{SoftButton, SoftDialog, SoftSelect, SoftSlider, SoftToggle};

use super::{Chrome, Source, storage};
use crate::actions::keymap::{Keymap, Preset};
use crate::prefs::{APERTURE, Prefs, RELEASE, SMART_BUDGET, WAKE};

fn heading(ui: &mut egui::Ui, t: &str) {
    ui.add_space(size::MENU_PAD);
    ui.label(
        RichText::new(t)
            .font(theme::body_strong())
            .color(color::INK),
    );
}

fn note(ui: &mut egui::Ui, t: &str) {
    ui.add(egui::Label::new(RichText::new(t).font(theme::small()).color(color::MUTED)).wrap());
}

/// Room left above and below the window, and the least it shrinks to.
const SCREEN_MARGIN: f32 = 240.0;
const MIN_HEIGHT: f32 = 200.0;

/// 1.2345 m as these units show it.
fn example(units: prism_math::DisplayUnits) -> String {
    prism_tools::units::NumberFormat {
        units,
        ..Default::default()
    }
    .show(1.2345)
}

/// The width of a slider's name, so the rails line up.
const LABEL_WIDTH: f32 = 110.0;

/// A named slider with its value beside it (the kit's slider shows the
/// value only while held).
fn slider(ui: &mut egui::Ui, v: &mut f64, (lo, hi): (f64, f64), k: f64, label: &str, unit: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(LABEL_WIDTH, size::SLIDER_HANDLE),
            egui::Sense::hover(),
        );
        ui.painter().text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            theme::body(),
            color::INK,
        );
        let mut f = (*v * k) as f32;
        if ui
            .add(SoftSlider::new(&mut f, (lo * k) as f32..=(hi * k) as f32, label).suffix(unit))
            .changed()
        {
            *v = f64::from(f) / k;
        }
        let shown = *v * k;
        // Whole numbers (decimals, milliseconds) without a fraction.
        let text = if k > 1.0 || unit.is_empty() {
            format!("{shown:.0}{unit}")
        } else {
            format!("{shown:.2}{unit}")
        };
        ui.label(RichText::new(text).font(theme::mono()).color(color::MUTED));
    });
}

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) {
    let mut p = chrome.prefs;
    let mut preset = Preset::ALL
        .iter()
        .position(|x| *x == chrome.keymap.preset)
        .unwrap_or(0);
    let old_preset = preset;
    let mut shortcuts = false;
    let names: Vec<&str> = LengthUnit::ALL.iter().map(|u| u.name()).collect();
    let presets: Vec<&str> = Preset::ALL.iter().map(|x| x.label()).collect();
    let tolerance = chrome.tolerance.clone();
    let r = SoftDialog::new("Preferences")
        .subtitle("For every document")
        .show(ctx, chrome.prefs_open, |ui| {
            let tall = (ctx.content_rect().height() - SCREEN_MARGIN).max(MIN_HEIGHT);
            egui::ScrollArea::vertical()
                .max_height(tall)
                .show(ui, |ui| {
                    heading(ui, "Units");
                    let mut i = LengthUnit::ALL
                        .iter()
                        .position(|u| *u == p.units.length)
                        .unwrap_or(2);
                    ui.add(SoftSelect::new(&mut i, &names, "New documents"));
                    p.units.length = LengthUnit::ALL[i];
                    let mut d = f64::from(p.units.decimals);
                    slider(
                        ui,
                        &mut d,
                        (0.0, f64::from(prism_math::units::MAX_DECIMALS)),
                        1.0,
                        "Decimals",
                        "",
                    );
                    p.units.decimals = d.round() as u8;
                    ui.label(
                        RichText::new(format!("Example: {}", example(p.units))).color(color::MUTED),
                    );
                    note(
                        ui,
                        "The open document keeps its own units: File > Document Settings.",
                    );
                    if ui.add(SoftButton::new("Reset Units").small()).clicked() {
                        p.reset_units();
                    }

                    heading(ui, "Tolerance");
                    ui.label(RichText::new(&tolerance).color(color::INK));
                    note(
                        ui,
                        "Points closer than this are the same point. It follows the model's \
                 size, so millimetre parts and kilometre sites both work.",
                    );

                    heading(ui, "Inference");
                    let i = &mut p.inference;
                    slider(ui, &mut i.aperture, APERTURE, 1.0, "Pick aperture", "x");
                    slider(ui, &mut i.release, RELEASE, 1.0, "Snap release", "x");
                    slider(ui, &mut i.wake, WAKE, 1000.0, "Hover wake", " ms");
                    note(
                        ui,
                        "Aperture: how near the cursor must come to snap. Release: how far it \
                 can leave before the snap lets go. Wake: how long a hover wakes a point.",
                    );
                    if ui.add(SoftButton::new("Reset Inference").small()).clicked() {
                        p.inference = prism_infer::Settings::default();
                    }

                    heading(ui, "Smart Guides");
                    ui.add(SoftToggle::new(
                        &mut p.smart_guides,
                        "Line up and space objects",
                    ));
                    let ms = &mut p.smart_budget_ms;
                    slider(ui, ms, SMART_BUDGET, 1.0, "Time budget", " ms");
                    note(
                        ui,
                        "While a group or body is moved or pasted, it lines up with its \
                 neighbours and snaps to equal gaps. Over the budget, spacing pauses \
                 first. View > Smart Guides or Ctrl+U turns it off.",
                    );

                    heading(ui, "Keyboard");
                    ui.add(SoftSelect::new(&mut preset, &presets, "Preset"));
                    note(ui, "A new preset replaces keys you changed yourself.");
                    ui.add_space(size::MENU_PAD * 0.5);
                    if ui
                        .add(SoftButton::new("Keyboard Shortcuts…").small())
                        .clicked()
                    {
                        shortcuts = true;
                    }
                });
        });
    if r.close {
        chrome.prefs_open = false;
    }
    set(chrome, p);
    if preset != old_preset {
        chrome.keymap = Keymap::from_preset(Preset::ALL[preset], &chrome.reg);
        chrome.save_keymap();
    }
    if shortcuts {
        chrome.prefs_open = false;
        chrome.run("win.hotkeys", Source::Menu);
    }
}

/// Take new preferences, and save them if they changed.
pub(crate) fn set(chrome: &mut Chrome, mut p: Prefs) {
    p.clamp();
    if p != chrome.prefs {
        chrome.prefs = p;
        if chrome.persist {
            storage::save_prefs(&p.to_json());
        }
    }
}

pub(super) fn about(chrome: &mut Chrome, ctx: &Context) {
    let r = SoftDialog::new("About Prism")
        .subtitle(format!("Version {}", env!("CARGO_PKG_VERSION")))
        .show(ctx, chrome.about_open, |ui| {
            ui.add_space(size::MENU_PAD);
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "A 3D modeller with exact curves and the feel of drawing by hand.",
                    )
                    .color(color::INK),
                )
                .wrap(),
            );
            ui.add_space(size::MENU_PAD);
            note(ui, "Free software under the MIT or Apache 2.0 licence.");
            note(
                ui,
                "Built in Rust with egui, wgpu, glam and other open-source libraries; \
                 each one and its licence is listed in docs/DEPENDENCIES.md.",
            );
            ui.add_space(size::MENU_PAD);
            if ui.add(SoftButton::new("Done")).clicked() {
                chrome.about_open = false;
            }
        });
    if r.close {
        chrome.about_open = false;
    }
}
