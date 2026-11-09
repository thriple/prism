//! The Check for Updates window (brief 20): what the check found, and the
//! button that downloads and installs a newer build.

use egui::{Context, RichText, Sense, vec2};
use prism_ui::emboss;
use prism_ui::theme::{self, color, depth, radius, size};
use prism_ui::widgets::{SoftButton, SoftDialog};

use super::Chrome;
use crate::update::{Status, this_build};

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) {
    let status = chrome.updater.status();
    if chrome.updates_open && status.busy() {
        ctx.request_repaint();
    }
    let this = this_build().map_or("a build made by hand".to_owned(), |n| format!("build {n}"));
    let mut install = false;
    let mut again = false;
    let r = SoftDialog::new("Check for Updates")
        .subtitle(format!("This is {this}"))
        .show(ctx, chrome.updates_open, |ui| {
            ui.add_space(size::MENU_PAD);
            let say = |ui: &mut egui::Ui, text: &str| {
                ui.add(egui::Label::new(RichText::new(text).color(color::INK)).wrap());
            };
            match &status {
                Status::Idle | Status::Checking => say(ui, "Looking for a newer build…"),
                Status::Local => say(
                    ui,
                    "This build was made by hand, so there is nothing to compare it with. \
                     Updates are for the builds from the Releases page.",
                ),
                Status::Web => say(
                    ui,
                    "The web version updates itself on every push. Reload the page for the newest.",
                ),
                Status::Unsupported => say(ui, "Updates are for the Windows and macOS builds."),
                Status::UpToDate(n) => say(ui, &format!("Build {n} is the newest.")),
                Status::Available { latest, .. } => {
                    say(
                        ui,
                        &format!(
                            "Build {latest} is available. Prism downloads it, checks it, and \
                             puts it in place of this one. It runs from the next start."
                        ),
                    );
                    ui.add_space(size::MENU_PAD);
                    if ui.add(SoftButton::new("Download and Install")).clicked() {
                        install = true;
                    }
                }
                Status::Downloading {
                    latest,
                    done,
                    total,
                } => {
                    let mb = |b: u64| b as f64 / 1_048_576.0;
                    let text = match total {
                        Some(t) => format!(
                            "Downloading build {latest}: {:.1} of {:.1} MB",
                            mb(*done),
                            mb(*t)
                        ),
                        None => format!("Downloading build {latest}: {:.1} MB", mb(*done)),
                    };
                    say(ui, &text);
                    ui.add_space(size::MENU_PAD);
                    // Progress: an inset well with an accent fill.
                    let w = ui.available_width();
                    let (bar, _) =
                        ui.allocate_exact_size(vec2(w, size::PROGRESS_HEIGHT), Sense::hover());
                    let mut v = Vec::new();
                    emboss::soft(
                        &mut v,
                        bar,
                        radius::BAR,
                        -1.0,
                        depth::BAR_WELL,
                        color::SURFACE,
                    );
                    ui.painter().extend(v);
                    if let Some(t) = total.filter(|t| *t > 0) {
                        let f = (*done as f32 / t as f32).clamp(0.0, 1.0);
                        let fill = bar.with_max_x(bar.left() + w * f);
                        ui.painter().rect_filled(fill, radius::BAR, color::ACCENT);
                    }
                }
                Status::Installed(n) => say(
                    ui,
                    &format!("Installed build {n}. Restart Prism to use it."),
                ),
                Status::Failed(why) => {
                    say(ui, why);
                    ui.add_space(size::MENU_PAD);
                    if ui.add(SoftButton::new("Try Again")).clicked() {
                        again = true;
                    }
                }
            }
            ui.add_space(size::MENU_PAD);
            ui.label(
                RichText::new("Older builds stay on the Releases page.")
                    .font(theme::small())
                    .color(color::MUTED),
            );
        });
    if r.close {
        chrome.updates_open = false;
    }
    if install {
        chrome.updater.install();
    }
    if again {
        chrome.updater.check();
    }
}
