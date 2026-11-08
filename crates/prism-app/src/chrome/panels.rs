//! Window > Outliner, Entity Info and Tags (Groups & Components part 3).
//! The app hands the rows in each frame and passes the requests on to the
//! session.

use egui::{Context, RichText};
use prism_doc::{InstanceId, TagId};
use prism_tools::{EntityInfo, OutlineRow};
use prism_ui::theme::{self, color, size};
use prism_ui::widgets::{SoftButton, SoftDialog, field};

use super::Chrome;

/// What the panels ask the session to do.
#[derive(Clone, Debug, PartialEq)]
pub enum PanelOp {
    Select(Vec<InstanceId>),
    Open(Vec<InstanceId>),
    Hide(InstanceId, bool),
    Name(InstanceId, String),
    Tag(prism_tools::TagOp),
    /// Model Health: select a row's items, or run its fix.
    HealthSelect(&'static str),
    HealthFix(&'static str),
}

/// The panels' data, set by the app each frame.
#[derive(Clone, Debug, Default)]
pub struct PanelData {
    pub outline: Vec<OutlineRow>,
    pub info: Option<EntityInfo>,
    /// Tag id, name, shown, instances on it.
    pub tags: Vec<(TagId, String, bool, usize)>,
    /// Model Health's rows (computed when the model changes while open).
    pub health: Vec<prism_tools::HealthRow>,
}

/// Which panels are open, and text being edited.
#[derive(Clone, Debug, Default)]
pub struct Panels {
    pub outliner: bool,
    pub info: bool,
    pub tags: bool,
    pub health: bool,
    /// Model Health's problems not drawn in the model (its switch off).
    pub health_marks_off: bool,
    /// The name field in Entity Info, and which instance it is for.
    pub name_edit: Option<(InstanceId, String)>,
    /// A tag being renamed.
    pub tag_edit: Option<(TagId, String)>,
}

impl Panels {
    pub fn any_open(&self) -> bool {
        self.outliner || self.info || self.tags || self.health
    }

    /// Close the last opened (Esc).
    pub fn close_one(&mut self) {
        if self.health {
            self.health = false;
        } else if self.tags {
            self.tags = false;
        } else if self.info {
            self.info = false;
        } else {
            self.outliner = false;
        }
    }
}

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) {
    let data = chrome.panel_data.clone();
    let mut p = chrome.panels.clone();
    let mut ops = Vec::new();
    outliner(ctx, &mut p, &data, &mut ops);
    info(ctx, &mut p, &data, &mut ops);
    tags(ctx, &mut p, &data, &mut ops);
    health(ctx, &mut p, &data, &mut ops);
    chrome.panels = p;
    chrome.panel_ops.extend(ops);
}

fn outliner(ctx: &Context, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let r = SoftDialog::new("Outliner")
        .subtitle(match data.outline.len() {
            0 => "No groups yet".to_owned(),
            1 => "1 group or component".to_owned(),
            n => format!("{n} groups and components"),
        })
        .show(ctx, p.outliner, |ui| {
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    for row in &data.outline {
                        ui.horizontal(|ui| {
                            ui.add_space(row.depth as f32 * 14.0);
                            let eye = if row.hidden { "○" } else { "●" };
                            if ui
                                .add(SoftButton::new(eye).small())
                                .on_hover_text(if row.hidden { "Show" } else { "Hide" })
                                .clicked()
                            {
                                ops.push(PanelOp::Hide(
                                    *row.path.last().expect("row"),
                                    !row.hidden,
                                ));
                            }
                            let mut text = RichText::new(&row.label);
                            text = if !row.shown {
                                text.color(color::MUTED)
                            } else if row.selected || row.open {
                                text.color(color::ACCENT)
                            } else {
                                text.color(color::INK)
                            };
                            let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                            if resp.double_clicked() {
                                ops.push(PanelOp::Open(row.path.clone()));
                            } else if resp.clicked() {
                                ops.push(PanelOp::Select(row.path.clone()));
                            }
                            if row.label != row.kind {
                                ui.label(
                                    RichText::new(&row.kind)
                                        .font(theme::mono())
                                        .color(color::MUTED),
                                );
                            }
                        });
                    }
                });
        });
    if r.close {
        p.outliner = false;
    }
}

fn info(ctx: &Context, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let r = SoftDialog::new("Entity Info").show(ctx, p.info, |ui| match &data.info {
        Some(EntityInfo::Instance {
            id,
            name,
            kind,
            tag,
            hidden,
        }) => {
            ui.label(RichText::new(kind).color(color::MUTED));
            ui.add_space(size::MENU_PAD);
            ui.horizontal(|ui| {
                ui.label("Name");
                let text = match &mut p.name_edit {
                    Some((i, t)) if i == id => t,
                    _ => {
                        p.name_edit = Some((*id, name.clone()));
                        &mut p.name_edit.as_mut().expect("set").1
                    }
                };
                let resp = field(ui, text, "No name", 180.0);
                if resp.lost_focus() && *text != *name {
                    ops.push(PanelOp::Name(*id, text.clone()));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Tag");
                let none = tag.is_none();
                if ui
                    .add(SoftButton::new("None").small().selected(none))
                    .clicked()
                {
                    ops.push(PanelOp::Tag(prism_tools::TagOp::PutSelected(None)));
                }
                for (t, tname, _, _) in &data.tags {
                    if ui
                        .add(SoftButton::new(tname).small().selected(*tag == Some(*t)))
                        .clicked()
                    {
                        ops.push(PanelOp::Tag(prism_tools::TagOp::PutSelected(Some(*t))));
                    }
                }
            });
            if ui
                .add(SoftButton::new(if *hidden { "Show" } else { "Hide" }).small())
                .clicked()
            {
                ops.push(PanelOp::Hide(*id, !hidden));
            }
        }
        Some(EntityInfo::Geometry {
            groups,
            faces,
            edges,
            points,
            area,
        }) => {
            p.name_edit = None;
            let line = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
            ui.label(
                [
                    line(*groups, "group"),
                    line(*faces, "face"),
                    line(*edges, "edge"),
                    line(*points, "point"),
                ]
                .join(", "),
            );
            if *faces > 0 {
                ui.label(
                    RichText::new(format!("Area {area:.4}"))
                        .font(theme::mono())
                        .color(color::MUTED),
                );
            }
        }
        None => {
            ui.label(RichText::new("Nothing selected").color(color::MUTED));
        }
    });
    if r.close {
        p.info = false;
    }
}

fn tags(ctx: &Context, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let r = SoftDialog::new("Tags")
        .subtitle("Show or hide everything on a tag")
        .show(ctx, p.tags, |ui| {
            for (t, name, shown, count) in &data.tags {
                ui.horizontal(|ui| {
                    if ui
                        .add(SoftButton::new(if *shown { "●" } else { "○" }).small())
                        .clicked()
                    {
                        ops.push(PanelOp::Tag(prism_tools::TagOp::Show(*t, !shown)));
                    }
                    match &mut p.tag_edit {
                        Some((id, text)) if id == t => {
                            field(ui, text, "Name", 140.0);
                            if ui.add(SoftButton::new("Save").small()).clicked() {
                                ops.push(PanelOp::Tag(prism_tools::TagOp::Rename(
                                    *t,
                                    text.clone(),
                                )));
                                p.tag_edit = None;
                            }
                        }
                        _ => {
                            ui.label(RichText::new(name).color(if *shown {
                                color::INK
                            } else {
                                color::MUTED
                            }));
                            ui.label(
                                RichText::new(count.to_string())
                                    .font(theme::mono())
                                    .color(color::MUTED),
                            );
                            if ui.add(SoftButton::new("Rename").small()).clicked() {
                                p.tag_edit = Some((*t, name.clone()));
                            }
                            if ui.add(SoftButton::new("Put selected").small()).clicked() {
                                ops.push(PanelOp::Tag(prism_tools::TagOp::PutSelected(Some(*t))));
                            }
                        }
                    }
                });
            }
            ui.add_space(size::MENU_PAD);
            if ui.add(SoftButton::new("Add tag").small()).clicked() {
                ops.push(PanelOp::Tag(prism_tools::TagOp::Add));
            }
        });
    if r.close {
        p.tags = false;
    }
}

fn health(ctx: &Context, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let problems: usize = data
        .health
        .iter()
        .filter(|r| r.key != "nesting")
        .map(|r| r.count)
        .sum();
    let r = SoftDialog::new("Model Health")
        .subtitle(if problems == 0 {
            "Nothing found".to_owned()
        } else {
            format!("{problems} things to look at")
        })
        .show(ctx, p.health, |ui| {
            for row in &data.health {
                ui.horizontal(|ui| {
                    let colour = if row.count == 0 {
                        color::MUTED
                    } else {
                        color::INK
                    };
                    let name = ui.add(
                        egui::Label::new(RichText::new(row.label).color(colour))
                            .sense(egui::Sense::click()),
                    );
                    if name.clicked() && row.count > 0 {
                        ops.push(PanelOp::HealthSelect(row.key));
                    }
                    if row.key != "nesting" {
                        ui.label(
                            RichText::new(row.count.to_string())
                                .font(theme::mono())
                                .color(colour),
                        );
                    }
                    if !row.detail.is_empty() {
                        ui.label(RichText::new(&row.detail).color(color::MUTED));
                    }
                    if let Some(fix) = row.fix
                        && ui.add(SoftButton::new(fix).small()).clicked()
                    {
                        ops.push(PanelOp::HealthFix(row.key));
                    }
                });
            }
            ui.add_space(size::MENU_PAD);
            let mut show = !p.health_marks_off;
            ui.add(prism_ui::widgets::SoftToggle::new(
                &mut show,
                "Show in the model",
            ));
            p.health_marks_off = !show;
            ui.label(RichText::new("Click a row to select and zoom to it.").color(color::MUTED));
        });
    if r.close {
        p.health = false;
    }
}
