//! Window > Outliner, Entity Info and Tags (Groups & Components part 3).
//! The app hands the rows in each frame and passes the requests on to the
//! session.

use std::collections::BTreeSet;

use egui::{Context, RichText, Ui};
use prism_doc::{InstanceId, TagId};
use prism_tools::{EntityInfo, OutlineRow};
use prism_ui::theme::{self, color, size};
use prism_ui::widgets::{SideCard, SideCardResponse, SoftButton, field};

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
    /// View Settings: the new view style.
    View(prism_view::ViewStyle),
    /// The Materials window.
    Material(prism_tools::session::MaterialOp),
    /// Load an image: onto this material, or as a new one.
    LoadImage(Option<prism_topo::MaterialId>),
    /// The Fillet window: its kind, or its size as typed.
    Round(prism_tools::session::RoundOp),
    RoundSize(String),
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
    /// View Settings' style.
    pub view: prism_view::ViewStyle,
    /// The Materials window: the document's materials, the Paint Bucket's
    /// brush, and Default's colour (the style's front colour).
    pub materials: Vec<prism_tools::session::MaterialRow>,
    pub brush: prism_tools::session::Brush,
    pub default_colour: [u8; 4],
    /// The Fillet window (while its tool is active).
    pub round: Option<prism_tools::session::RoundPanel>,
}

/// Which panels are open, and text being edited.
#[derive(Clone, Debug, Default)]
pub struct Panels {
    pub outliner: bool,
    /// Help > Tool Hint Panel.
    pub hints: bool,
    pub info: bool,
    pub tags: bool,
    pub health: bool,
    /// View Settings.
    pub view: bool,
    /// The Materials window, and the chosen material's name being edited.
    pub materials: bool,
    pub material_name: Option<(prism_topo::MaterialId, String)>,
    /// The chosen material's texture width and height being typed.
    pub texture_text: Option<(prism_topo::MaterialId, [String; 2])>,
    /// Typing a texture's width leaves its height alone (the shape is
    /// kept by default).
    pub texture_free_shape: bool,
    /// On the web, the image being picked is for this material (or a new
    /// one); it arrives a few frames later.
    pub image_for: Option<Option<prism_topo::MaterialId>>,
    /// The Fillet window (shown only with its tool), and its size field
    /// while it is being typed in.
    pub fillet: bool,
    pub fillet_size: Option<String>,
    /// Model Health's problems not drawn in the model (its switch off).
    pub health_marks_off: bool,
    /// The name field in Entity Info, and which instance it is for.
    pub name_edit: Option<(InstanceId, String)>,
    /// A tag being renamed.
    pub tag_edit: Option<(TagId, String)>,
    /// The side windows folded to their titles.
    pub folded: BTreeSet<&'static str>,
}

impl Panels {
    pub fn any_open(&self) -> bool {
        self.outliner
            || self.hints
            || self.info
            || self.tags
            || self.health
            || self.view
            || self.materials
    }
}

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) {
    let Some(rect) = chrome.side_rect else {
        return;
    };
    let data = chrome.panel_data.clone();
    let mut p = chrome.panels.clone();
    let mut ops = Vec::new();
    egui::Area::new(egui::Id::unique("side-column"))
        .order(egui::Order::Middle)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            // The whole column, every frame: the app redraws only on change,
            // so a size taken from the last frame would cut new cards off.
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            egui::ScrollArea::vertical()
                .max_height(rect.height())
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(rect.width());
                    if chrome.active_tool == prism_tools::session::FILLET {
                        super::fillet::card(ui, &mut p, &data, &mut ops);
                    }
                    super::hints::card(chrome, ui, &mut p);
                    info(ui, &mut p, &data, &mut ops);
                    super::materials::card(ui, &mut p, &data, &mut ops);
                    outliner(ui, &mut p, &data, &mut ops);
                    super::components::card(chrome, ui, &mut p.folded);
                    tags(ui, &mut p, &data, &mut ops);
                    health(ui, &mut p, &data, &mut ops);
                    view(ui, &mut p, &data, &mut ops);
                    // Room for the last card's shadow.
                    ui.add_space(size::GAP);
                });
        });
    chrome.panels = p;
    chrome.panel_ops.extend(ops);
}

/// After a side window's card: fold or unfold it, and space before the
/// next. Returns true if it was closed.
pub(super) fn after_card(
    ui: &mut Ui,
    folded: &mut BTreeSet<&'static str>,
    name: &'static str,
    r: SideCardResponse,
) -> bool {
    if r.toggle && !folded.remove(name) {
        folded.insert(name);
    }
    ui.add_space(size::SIDE_GAP);
    r.close
}

fn outliner(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    if !p.outliner {
        return;
    }
    let r = SideCard::new("Outliner")
        .subtitle(match data.outline.len() {
            0 => "No groups yet".to_owned(),
            1 => "1 group or component".to_owned(),
            n => format!("{n} groups and components"),
        })
        .show(ui, p.folded.contains("Outliner"), |ui| {
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
    if after_card(ui, &mut p.folded, "Outliner", r) {
        p.outliner = false;
    }
}

fn info(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    if !p.info {
        return;
    }
    let r =
        SideCard::new("Entity Info").show(ui, p.folded.contains("Entity Info"), |ui| {
            match &data.info {
                Some(EntityInfo::Instance {
                    id,
                    name,
                    kind,
                    tag,
                    hidden,
                    locked,
                    material,
                }) => {
                    let kind = if *locked {
                        format!("{kind} (locked)")
                    } else {
                        kind.clone()
                    };
                    ui.label(RichText::new(kind).color(color::MUTED));
                    ui.add_space(size::MENU_PAD);
                    ui.horizontal_wrapped(|ui| {
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
                    ui.horizontal_wrapped(|ui| {
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
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Material");
                        ui.label(RichText::new(material).color(color::MUTED));
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
                    materials,
                }) => {
                    p.name_edit = None;
                    let line =
                        |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
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
                    if let Some((front, back)) = materials {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Front");
                            ui.label(RichText::new(front).color(color::MUTED));
                            ui.label("Back");
                            ui.label(RichText::new(back).color(color::MUTED));
                        });
                    }
                }
                None => {
                    ui.label(RichText::new("Nothing selected").color(color::MUTED));
                }
            }
        });
    if after_card(ui, &mut p.folded, "Entity Info", r) {
        p.info = false;
    }
}

fn tags(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    if !p.tags {
        return;
    }
    let r = SideCard::new("Tags")
        .subtitle("Show or hide everything on a tag")
        .show(ui, p.folded.contains("Tags"), |ui| {
            for (t, name, shown, count) in &data.tags {
                ui.horizontal_wrapped(|ui| {
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
    if after_card(ui, &mut p.folded, "Tags", r) {
        p.tags = false;
    }
}

fn health(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    if !p.health {
        return;
    }
    let problems: usize = data
        .health
        .iter()
        // Nesting and approximate curves are information, not problems.
        .filter(|r| r.key != "nesting" && r.key != "approx")
        .map(|r| r.count)
        .sum();
    let r = SideCard::new("Model Health")
        .subtitle(if problems == 0 {
            "Nothing found".to_owned()
        } else {
            format!("{problems} things to look at")
        })
        .show(ui, p.folded.contains("Model Health"), |ui| {
            for row in &data.health {
                ui.horizontal_wrapped(|ui| {
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
    if after_card(ui, &mut p.folded, "Model Health", r) {
        p.health = false;
    }
}

fn view(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    use prism_view::style::HiddenLines;
    use prism_view::view_style::{FaceStyle, LineColor, PRESETS};
    if !p.view {
        return;
    }
    let mut v = data.view.clone();
    let r = SideCard::new("View Settings")
        .subtitle(v.name.clone())
        .show(ui, p.folded.contains("View Settings"), |ui| {
            let mut preset = PRESETS.iter().position(|n| *n == v.name).unwrap_or(0);
            if ui
                .add(prism_ui::widgets::SoftSelect::new(
                    &mut preset,
                    &PRESETS,
                    "Style",
                ))
                .changed()
                && let Some(s) = prism_view::ViewStyle::preset(PRESETS[preset])
            {
                v = s;
            }
            ui.add_space(size::MENU_PAD);
            ui.label(RichText::new("Show").color(color::MUTED));
            ui.horizontal_wrapped(|ui| {
                for (on, label) in [
                    (&mut v.faces, "Faces"),
                    (&mut v.lines, "Lines"),
                    (&mut v.points, "Points"),
                    (&mut v.axes, "Axes"),
                    (&mut v.ground, "Ground"),
                    (&mut v.grid, "Grid"),
                    (&mut v.guides, "Guides"),
                    (&mut v.hidden, "Hidden"),
                ] {
                    ui.add(prism_ui::widgets::SoftToggle::new(on, label));
                }
            });
            ui.add_space(size::MENU_PAD);
            const FACES: [&str; 5] = [
                "Colour/Material",
                "Base Colour",
                "Monochrome",
                "Hidden Line",
                "Wireframe",
            ];
            const FACE_STYLES: [FaceStyle; 5] = [
                FaceStyle::Shaded,
                FaceStyle::BaseColour,
                FaceStyle::Monochrome,
                FaceStyle::HiddenLine,
                FaceStyle::Wireframe,
            ];
            let mut f = FACE_STYLES
                .iter()
                .position(|s| *s == v.face_style)
                .unwrap_or(0);
            if ui
                .add(prism_ui::widgets::SoftSelect::new(&mut f, &FACES, "Faces"))
                .changed()
            {
                v.face_style = FACE_STYLES[f.min(FACE_STYLES.len() - 1)];
            }
            ui.horizontal_wrapped(|ui| {
                ui.add(prism_ui::widgets::SoftToggle::new(
                    &mut v.textures,
                    "Textures",
                ));
                ui.add(prism_ui::widgets::SoftToggle::new(&mut v.xray, "X-ray"));
            });
            ui.horizontal_wrapped(|ui| {
                ui.add(prism_ui::widgets::SoftToggle::new(
                    &mut v.zebra,
                    "Zebra stripes",
                ));
                ui.add(prism_ui::widgets::SoftToggle::new(
                    &mut v.combs,
                    "Curvature combs",
                ));
            });
            if v.xray {
                caption(ui, format!("Opacity {:.0} %", v.xray_opacity * 100.0));
                let mut o = v.xray_opacity * 100.0;
                if ui
                    .add(
                        prism_ui::widgets::SoftSlider::new(&mut o, 10.0..=90.0, "Opacity")
                            .suffix(" %"),
                    )
                    .changed()
                {
                    v.xray_opacity = o / 100.0;
                }
            }
            ui.add_space(size::MENU_PAD);
            caption(ui, format!("Line thickness {:.1} pt", v.line_width));
            ui.add(
                prism_ui::widgets::SoftSlider::new(&mut v.line_width, 0.5..=4.0, "Line thickness")
                    .suffix(" pt"),
            );
            caption(
                ui,
                if v.profile_width > 0.0 {
                    format!("Profile thickness {:.1} pt", v.profile_width)
                } else {
                    "Profiles off".to_owned()
                },
            );
            ui.add(
                prism_ui::widgets::SoftSlider::new(
                    &mut v.profile_width,
                    0.0..=6.0,
                    "Profile thickness",
                )
                .suffix(" pt"),
            );
            const COLORS: [&str; 2] = ["One colour", "By axis"];
            let mut c = usize::from(v.line_color == LineColor::ByAxis);
            if ui
                .add(prism_ui::widgets::SoftSelect::new(
                    &mut c,
                    &COLORS,
                    "Line colour",
                ))
                .changed()
            {
                v.line_color = if c == 1 {
                    LineColor::ByAxis
                } else {
                    LineColor::One
                };
            }
            const HIDDEN: [&str; 3] = ["Not drawn", "Dashed", "Faint"];
            const HIDDEN_LINES: [HiddenLines; 3] =
                [HiddenLines::Off, HiddenLines::Dashed, HiddenLines::Faint];
            let mut h = HIDDEN_LINES
                .iter()
                .position(|x| *x == v.hidden_lines)
                .unwrap_or(0);
            if ui
                .add(prism_ui::widgets::SoftSelect::new(
                    &mut h,
                    &HIDDEN,
                    "Hidden lines",
                ))
                .changed()
            {
                v.hidden_lines = HIDDEN_LINES[h.min(2)];
            }
            lighting(ui, &mut v);
        });
    if v != data.view {
        ops.push(PanelOp::View(v));
    }
    if after_card(ui, &mut p.folded, "View Settings", r) {
        p.view = false;
    }
}

/// A slider's name and value, above it.
fn caption(ui: &mut Ui, text: String) {
    ui.label(RichText::new(text).color(color::MUTED));
}

/// View Settings' Lighting section (part 2): the sun and shadows.
fn lighting(ui: &mut Ui, v: &mut prism_view::ViewStyle) {
    use prism_ui::widgets::{SoftSlider, SoftToggle};
    ui.add_space(size::MENU_PAD);
    ui.label(RichText::new("Lighting").color(color::MUTED));
    ui.horizontal_wrapped(|ui| {
        ui.add(SoftToggle::new(&mut v.sun, "Sun"));
        ui.add(SoftToggle::new(&mut v.shadows, "Shadows"));
    });
    // Each value is written back only when its slider moves, so a value
    // that f32 cannot hold exactly is not "changed" every frame.
    let slider = |ui: &mut Ui, text: String, value: f64, range| -> Option<f64> {
        caption(ui, text);
        let mut x = value as f32;
        ui.add(SoftSlider::new(&mut x, range, ""))
            .changed()
            .then_some(f64::from(x))
    };
    if v.sun {
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let m = MONTHS[(v.month.clamp(1, 12) - 1) as usize];
        if let Some(x) = slider(ui, format!("Month {m}"), f64::from(v.month), 1.0..=12.0) {
            v.month = x.round() as u32;
        }
        if let Some(x) = slider(ui, format!("Day {}", v.day), f64::from(v.day), 1.0..=31.0) {
            v.day = x.round() as u32;
        }
        let (h, min) = (
            v.hours.floor() as u32,
            (v.hours.fract() * 60.0).round() as u32,
        );
        if let Some(x) = slider(ui, format!("Time {h:02}:{min:02}"), v.hours, 0.0..=24.0) {
            v.hours = (x * 12.0).round() / 12.0;
        }
        if let Some(x) = slider(
            ui,
            format!("Latitude {:.2}°", v.latitude),
            v.latitude,
            -90.0..=90.0,
        ) {
            v.latitude = (x * 100.0).round() / 100.0;
        }
        if let Some(x) = slider(
            ui,
            format!("Longitude {:.2}°", v.longitude),
            v.longitude,
            -180.0..=180.0,
        ) {
            v.longitude = (x * 100.0).round() / 100.0;
        }
        if let Some(x) = slider(
            ui,
            format!("UTC {:+.1} h", v.utc_offset),
            v.utc_offset,
            -12.0..=14.0,
        ) {
            v.utc_offset = (x * 2.0).round() / 2.0;
        }
        if let Some(x) = slider(ui, format!("North {:.0}°", v.north), v.north, 0.0..=360.0) {
            v.north = x.round();
        }
    }
    if let Some(x) = slider(
        ui,
        format!("Light {:.0} %", v.light * 100.0),
        f64::from(v.light),
        0.0..=1.0,
    ) {
        v.light = ((x * 100.0).round() / 100.0) as f32;
    }
    if let Some(x) = slider(
        ui,
        format!("Dark {:.0} %", v.dark * 100.0),
        f64::from(v.dark),
        0.0..=1.0,
    ) {
        v.dark = ((x * 100.0).round() / 100.0) as f32;
    }
}
