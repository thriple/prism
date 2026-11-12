//! Window > Materials (Basic Materials 1c, brief 28): the document's
//! materials as swatches (used ones first), the built-in colours, New,
//! Duplicate, Delete and Purge Unused, and the chosen material's name,
//! colour, opacity, roughness, metalness and texture (Basic Materials 2c:
//! load an image, its size, remove it). Clicking a swatch picks the Paint
//! Bucket with it. A PNG or JPEG dropped on the window becomes a new
//! textured material ([`image_loaded`]).

use egui::{RichText, Ui};
use prism_doc::material::PALETTE;
use prism_tools::session::{Brush, MaterialOp, MaterialRow};
use prism_ui::theme::{self, color, size};
use prism_ui::widgets::{SideCard, SoftButton, SoftSlider, field, swatch};

use super::panels::{PanelData, PanelOp, Panels, after_card};

/// An image picked or dropped: onto material `target`, or as a new
/// material, which is picked for the Paint Bucket and shown in the open
/// Materials window. Says in the status bar if the image cannot be read.
pub fn image_loaded(
    chrome: &mut super::Chrome,
    s: &mut prism_tools::Session,
    target: Option<prism_topo::MaterialId>,
    o: crate::doc_files::Opened,
) {
    match s.load_image(target, &o.name, o.bytes) {
        Ok(tool) => {
            if let Some(tool) = tool {
                chrome.active_tool = tool;
            }
            chrome.panels.materials = true;
            chrome.panels.folded.remove("Materials");
            chrome.panels.texture_text = None;
            s.mark_all_dirty();
        }
        Err(why) => chrome.status_note(format!("{}: {why}", o.name)),
    }
}

pub(super) fn card(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    if !p.materials {
        return;
    }
    let pick = |ops: &mut Vec<PanelOp>, b: Brush| ops.push(PanelOp::Material(MaterialOp::Pick(b)));
    let r = SideCard::new("Materials")
        .subtitle(match data.materials.len() {
            0 => "Pick a colour to paint with".to_owned(),
            1 => "1 material in the model".to_owned(),
            n => format!("{n} materials in the model"),
        })
        .show(ui, p.folded.contains("Materials"), |ui| {
            caption(ui, "In Model");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing =
                    egui::vec2(size::ITEM_GAP * 2.0, size::ITEM_GAP * 2.0);
                let def = swatch(
                    ui,
                    theme::srgba(data.default_colour),
                    data.brush == Brush::Default,
                )
                .on_hover_text("Default");
                if def.clicked() {
                    pick(ops, Brush::Default);
                }
                for m in &data.materials {
                    let resp = swatch(
                        ui,
                        theme::srgba(m.srgba),
                        data.brush == Brush::Material(m.id),
                    )
                    .on_hover_text(if m.used {
                        m.name.clone()
                    } else {
                        format!("{} (not used)", m.name)
                    });
                    if resp.clicked() {
                        pick(ops, Brush::Material(m.id));
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            caption(ui, "Colours");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing =
                    egui::vec2(size::ITEM_GAP * 2.0, size::ITEM_GAP * 2.0);
                for (i, (name, c)) in PALETTE.iter().enumerate() {
                    let resp = swatch(
                        ui,
                        theme::srgba([c[0], c[1], c[2], 255]),
                        data.brush == Brush::Palette(i),
                    )
                    .on_hover_text(*name);
                    if resp.clicked() {
                        pick(ops, Brush::Palette(i));
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            let chosen = match data.brush {
                Brush::Material(id) => data.materials.iter().find(|m| m.id == id),
                _ => None,
            };
            ui.horizontal_wrapped(|ui| {
                if ui.add(SoftButton::new("New").small()).clicked() {
                    ops.push(PanelOp::Material(MaterialOp::New));
                }
                if ui
                    .add(SoftButton::new("New from Image…").small())
                    .on_hover_text("A PNG or JPEG; you can also drop one on the window")
                    .clicked()
                {
                    ops.push(PanelOp::LoadImage(None));
                }
                if let Some(m) = chosen {
                    if ui.add(SoftButton::new("Duplicate").small()).clicked() {
                        ops.push(PanelOp::Material(MaterialOp::Duplicate(m.id)));
                    }
                    if ui
                        .add(SoftButton::new("Delete").small())
                        .on_hover_text("Faces using it go back to Default")
                        .clicked()
                    {
                        ops.push(PanelOp::Material(MaterialOp::Delete(m.id)));
                    }
                }
                let unused = data.materials.iter().filter(|m| !m.used).count();
                if unused > 0
                    && ui
                        .add(SoftButton::new(format!("Purge Unused ({unused})")).small())
                        .clicked()
                {
                    ops.push(PanelOp::Material(MaterialOp::Purge));
                }
            });
            if let Some(m) = chosen {
                edit(ui, p, m, ops);
            } else {
                p.material_name = None;
            }
        });
    if after_card(ui, &mut p.folded, "Materials", r) {
        p.materials = false;
    }
}

/// The chosen material's name, colour, roughness and metalness.
fn edit(ui: &mut Ui, p: &mut Panels, m: &MaterialRow, ops: &mut Vec<PanelOp>) {
    ui.add_space(size::MENU_PAD);
    caption(ui, "Edit");
    let base = |m: &MaterialRow| m.material.clone();
    ui.horizontal_wrapped(|ui| {
        ui.label("Name");
        let text = match &mut p.material_name {
            Some((id, t)) if *id == m.id => t,
            _ => {
                p.material_name = Some((m.id, m.name.clone()));
                &mut p.material_name.as_mut().expect("set").1
            }
        };
        let resp = field(ui, text, "Name", 160.0);
        if resp.lost_focus() && !text.trim().is_empty() && text.trim() != m.name {
            let mut x = base(m);
            x.name = text.trim().to_owned();
            ops.push(PanelOp::Material(MaterialOp::Edit(m.id, x)));
        }
    });
    // A texture replaces the colour.
    if m.texture.is_none() {
        ui.horizontal_wrapped(|ui| {
            ui.label("Colour");
            let mut rgb = [m.srgba[0], m.srgba[1], m.srgba[2]];
            if egui::widgets::color_picker::color_edit_button_srgb(ui, &mut rgb).changed() {
                let mut x = base(m);
                let lin = prism_doc::Material::colour("", rgb).surface().base;
                let s = x.surface_mut();
                s.base = [lin[0], lin[1], lin[2], s.base[3]];
                ops.push(PanelOp::Material(MaterialOp::Edit(m.id, x)));
            }
            ui.label(
                RichText::new(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]))
                    .font(theme::mono())
                    .color(color::MUTED),
            );
        });
    }
    let mut opacity = f32::from(m.srgba[3]) / 255.0 * 100.0;
    caption(ui, &format!("Opacity {opacity:.0} %"));
    if ui
        .add(SoftSlider::new(&mut opacity, 0.0..=100.0, "Opacity").suffix(" %"))
        .changed()
    {
        let mut x = base(m);
        x.surface_mut().base[3] = (opacity / 100.0).clamp(0.0, 1.0);
        ops.push(PanelOp::Material(MaterialOp::Edit(m.id, x)));
    }
    texture(ui, p, m, ops);
    let mut rough = m.roughness;
    caption(ui, &format!("Roughness {rough:.2}"));
    if ui
        .add(SoftSlider::new(&mut rough, 0.0..=1.0, "Roughness"))
        .changed()
    {
        let mut x = base(m);
        x.surface_mut().roughness = (rough * 100.0).round() / 100.0;
        ops.push(PanelOp::Material(MaterialOp::Edit(m.id, x)));
    }
    let mut metal = m.metalness;
    caption(ui, &format!("Metalness {metal:.2}"));
    if ui
        .add(SoftSlider::new(&mut metal, 0.0..=1.0, "Metalness"))
        .changed()
    {
        let mut x = base(m);
        x.surface_mut().metalness = (metal * 100.0).round() / 100.0;
        ops.push(PanelOp::Material(MaterialOp::Edit(m.id, x)));
    }
    ui.label(
        RichText::new(
            "Roughness and metalness are kept for rendering; the viewport shows the colour.",
        )
        .color(color::MUTED),
    );
}

/// The chosen material's texture: load or replace its image, its size in
/// the model (the shape kept unless unlocked), or remove it.
fn texture(ui: &mut Ui, p: &mut Panels, m: &MaterialRow, ops: &mut Vec<PanelOp>) {
    ui.add_space(size::MENU_PAD);
    let Some(t) = &m.texture else {
        caption(ui, "Texture: none");
        if ui
            .add(SoftButton::new("Load Image…").small())
            .on_hover_text("A PNG or JPEG, repeated to fill each face")
            .clicked()
        {
            ops.push(PanelOp::LoadImage(Some(m.id)));
        }
        return;
    };
    caption(
        ui,
        &format!("Texture: {} × {} pixels", t.pixels[0], t.pixels[1]),
    );
    let text = match &mut p.texture_text {
        Some((id, x)) if *id == m.id => x,
        _ => {
            p.texture_text = Some((m.id, t.size_text.clone()));
            &mut p.texture_text.as_mut().expect("set").1
        }
    };
    let mut typed = None;
    egui::Grid::new("texture size").show(ui, |ui| {
        for (k, label) in ["Width", "Height"].into_iter().enumerate() {
            ui.label(label);
            let resp = field(ui, &mut text[k], label, 120.0);
            if resp.lost_focus() && text[k].trim() != t.size_text[k] {
                typed = Some((k, text[k].clone()));
            }
            ui.end_row();
        }
    });
    if let Some((k, text)) = typed {
        ops.push(PanelOp::Material(MaterialOp::TextureSize {
            id: m.id,
            text,
            height: k == 1,
            keep_shape: !p.texture_free_shape,
        }));
        p.texture_text = None;
    } else if !ui.memory(|mem| mem.focused().is_some())
        && p.texture_text
            .as_ref()
            .is_some_and(|(_, x)| *x != t.size_text)
    {
        // The size changed elsewhere (undo, the other side): follow it.
        p.texture_text = None;
    }
    let mut keep = !p.texture_free_shape;
    if ui
        .add(prism_ui::widgets::SoftToggle::new(&mut keep, "Keep shape"))
        .changed()
    {
        p.texture_free_shape = !keep;
    }
    ui.horizontal_wrapped(|ui| {
        if ui.add(SoftButton::new("Replace Image…").small()).clicked() {
            ops.push(PanelOp::LoadImage(Some(m.id)));
        }
        if ui
            .add(SoftButton::new("Remove Texture").small())
            .on_hover_text("Back to its colour")
            .clicked()
        {
            ops.push(PanelOp::Material(MaterialOp::RemoveTexture(m.id)));
        }
    });
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(color::MUTED));
}
