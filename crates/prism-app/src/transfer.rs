//! File > Import and File > Export (brief 09): the dialogs, and what they
//! run. The formats themselves are in `prism_file::mesh`.

use egui::Context;
use prism_file::mesh::{self, ExportOptions, Format, Mesh};
use prism_math::LengthUnit;
use prism_tools::Session;
use prism_ui::widgets::{SoftButton, SoftDialog, SoftSelect, SoftToggle};

use crate::chrome::Chrome;
use crate::doc_files::Opened;

/// Files bigger than this get a note that they may take a while.
const BIG: usize = 50 * 1024 * 1024;

/// A file read and waiting in the Import dialog.
struct Importing {
    name: String,
    format: Format,
    mesh: Mesh,
    /// Metres per unit, if the file says.
    stated: Option<f64>,
    unit: LengthUnit,
    y_up: bool,
    merge: bool,
    bytes: usize,
    /// A GLB's objects (brief 35): kept as groups and components when
    /// `keep` is on.
    scene: Option<mesh::gltf::Scene>,
    keep: bool,
}

/// The Export dialog.
struct Exporting {
    formats: &'static [Format],
    title: &'static str,
    format: usize,
    selection_only: bool,
    unit: LengthUnit,
    y_up: bool,
    /// GLB: everything as one mesh (off: groups and components stay
    /// separate named objects, brief 35).
    merge: bool,
}

/// The STEP export window (Solids & CAD part 5b, brief 44).
struct StepExporting {
    unit: LengthUnit,
    selection_only: bool,
}

/// The DXF / SVG export window (Solids & CAD part 6a, brief 45).
struct DrawingExporting {
    format: prism_file::drawing::Format,
    unit: LengthUnit,
    view: prism_file::drawing::View,
    selection_only: bool,
}

/// A STEP file read and waiting in its Import window (Solids & CAD part
/// 5c, brief 44).
struct StepImporting {
    name: String,
    read: prism_file::step::Imported,
}

#[derive(Default)]
pub struct Transfer {
    importing: Option<Importing>,
    exporting: Option<Exporting>,
    step: Option<StepExporting>,
    step_in: Option<StepImporting>,
    drawing: Option<DrawingExporting>,
}

const PRINT: [Format; 2] = [Format::ThreeMf, Format::Stl];
const MEDIA: [Format; 3] = [Format::Glb, Format::Obj, Format::Ply];
/// Game engines (brief 35): GLB, metres, Y up, the structure kept.
const GAME: [Format; 1] = [Format::Glb];
const UNITS: [LengthUnit; 4] = [
    LengthUnit::Millimetres,
    LengthUnit::Centimetres,
    LengthUnit::Metres,
    LengthUnit::Inches,
];

impl Transfer {
    pub fn modal(&self) -> bool {
        self.importing.is_some()
            || self.exporting.is_some()
            || self.step.is_some()
            || self.step_in.is_some()
            || self.drawing.is_some()
    }

    /// File > Import… and the export presets.
    pub fn action(&mut self, id: &str, s: &Session, chrome: &mut Chrome) {
        match id {
            "file.import" => match crate::mesh_files::pick() {
                Ok(Some(o)) => self.open(o, chrome),
                Ok(None) => {}
                Err(e) => chrome.status_note(format!("Could not import: {e}")),
            },
            "exp.3mf" => self.start_export(&PRINT, "Export for 3D printing", s),
            "exp.step" => {
                // The model's own unit (feet and inches write inches).
                let unit = match s.number.units.length {
                    LengthUnit::FeetInches => LengthUnit::Inches,
                    u => u,
                };
                self.step = Some(StepExporting {
                    unit,
                    selection_only: !s.selection().faces.is_empty(),
                });
            }
            "exp.dxf" => {
                self.drawing = Some(DrawingExporting {
                    format: prism_file::drawing::Format::Dxf,
                    unit: prism_file::drawing::file_unit(s.number.units.length),
                    view: prism_file::drawing::View::Flat,
                    selection_only: has_choice(s),
                });
            }
            "exp.gltf" => self.start_export(&MEDIA, "Export for media and the web", s),
            "exp.game" => {
                self.start_export(&GAME, "Export for a game engine (Beverly, Godot, Unity)", s);
                if let Some(e) = &mut self.exporting {
                    e.unit = LengthUnit::Metres;
                    e.y_up = true;
                    e.selection_only = false;
                }
            }
            _ => {}
        }
    }

    fn start_export(&mut self, formats: &'static [Format], title: &'static str, s: &Session) {
        let f = formats[0];
        let o = ExportOptions::for_format(f);
        self.exporting = Some(Exporting {
            formats,
            title,
            format: 0,
            selection_only: !s.selection().faces.is_empty(),
            unit: o.unit,
            y_up: o.y_up,
            merge: false,
        });
    }

    /// A mesh or STEP file, picked or dropped: read it and show its Import
    /// dialog.
    pub fn open(&mut self, o: Opened, chrome: &mut Chrome) {
        let lower = o.name.to_ascii_lowercase();
        if lower.ends_with(".step") || lower.ends_with(".stp") {
            let text = String::from_utf8_lossy(&o.bytes);
            // Read at the tolerance of a new model; it is pasted in like a
            // copied group.
            match prism_file::step::import(&text, prism_topo::Model::new().tol().dist) {
                Ok(read) => {
                    self.step_in = Some(StepImporting { name: o.name, read });
                }
                Err(e) => chrome.status_note(format!("Could not import {}: {e}", o.name)),
            }
            return;
        }
        let Some(format) = Format::of(&o.name) else {
            chrome.status_note(format!(
                "{} is not a format Prism imports (STEP, STL, OBJ, 3MF, glTF, PLY)",
                o.name
            ));
            return;
        };
        let read = mesh::read(&o.bytes, format).map(|mut m| {
            // An OBJ's materials are in the .mtl beside it (natively; the
            // web has only the file itself).
            if format == Format::Obj {
                let dir = o
                    .path
                    .as_deref()
                    .and_then(|p| std::path::Path::new(p).parent())
                    .map(std::path::Path::to_path_buf);
                let sibling = |name: &str| -> Option<Vec<u8>> {
                    if cfg!(target_arch = "wasm32") {
                        return None;
                    }
                    std::fs::read(dir.as_ref()?.join(name)).ok()
                };
                mesh::obj_materials(&mut m, &o.bytes, &sibling);
            }
            m
        });
        match read {
            Ok(m) if m.polygons.is_empty() => {
                chrome.status_note(format!("{} has no faces to import", o.name));
            }
            Ok(m) => {
                let scene = (format == Format::Glb)
                    .then(|| mesh::read_structured(&o.bytes).ok())
                    .flatten()
                    .filter(mesh::has_structure);
                self.importing = Some(Importing {
                    keep: scene.is_some(),
                    scene,
                    name: o.name,
                    format,
                    stated: mesh::stated_unit(&o.bytes, format),
                    mesh: m,
                    unit: LengthUnit::Millimetres,
                    y_up: format.y_up(),
                    merge: true,
                    bytes: o.bytes.len(),
                });
            }
            Err(e) => chrome.status_note(format!("Could not import {}: {e}", o.name)),
        }
    }

    /// Every frame: finished web picks, and the dialogs.
    pub fn frame(&mut self, ctx: &Context, s: &mut Session, chrome: &mut Chrome) {
        if let Some(r) = crate::mesh_files::take_picked() {
            match r {
                Ok(o) => self.open(o, chrome),
                Err(e) => chrome.status_note(format!("Could not import: {e}")),
            }
        }
        self.import_dialog(ctx, s, chrome);
        self.export_dialog(ctx, s, chrome);
        self.step_dialog(ctx, s, chrome);
        self.step_import_dialog(ctx, s, chrome);
        self.drawing_dialog(ctx, s, chrome);
    }

    /// File > Export > DXF / SVG: the format, the unit, the view and the
    /// selection only.
    fn drawing_dialog(&mut self, ctx: &Context, s: &mut Session, chrome: &mut Chrome) {
        use prism_file::drawing::{self, Format as DFormat, View};
        let open = self.drawing.is_some();
        let has_selection = has_choice(s);
        let mut go = None;
        let r = SoftDialog::new("Export DXF / SVG")
            .subtitle("For routers, lasers and CNC programs")
            .show(ctx, open, |ui| {
                let Some(e) = &mut self.drawing else { return };
                let formats = [DFormat::Dxf, DFormat::Svg];
                let mut k = formats.iter().position(|f| *f == e.format).unwrap_or(0);
                ui.add(SoftSelect::new(&mut k, &["DXF", "SVG"], "Format"));
                e.format = formats[k];
                let unames: Vec<&str> = UNITS.iter().map(|u| u.name()).collect();
                let mut k = UNITS.iter().position(|u| *u == e.unit).unwrap_or(0);
                ui.add(SoftSelect::new(&mut k, &unames, "Unit"));
                e.unit = UNITS[k];
                let vnames: Vec<&str> = View::ALL.iter().map(|v| v.name()).collect();
                let mut k = View::ALL.iter().position(|v| *v == e.view).unwrap_or(0);
                ui.add(SoftSelect::new(&mut k, &vnames, "View"));
                e.view = View::ALL[k];
                if has_selection {
                    ui.add(SoftToggle::new(&mut e.selection_only, "Only the selection"));
                }
                ui.label(match e.view {
                    View::Flat => "Each part's largest flat face (or each selected flat face) laid flat, outside up, in a row. Arcs stay arcs.",
                    _ => "The edges seen straight on from this side. Hidden lines are not removed yet. Arcs stay arcs.",
                });
                ui.horizontal(|ui| {
                    if ui.add(SoftButton::new("Export…")).clicked() {
                        go = Some(true);
                    }
                    if ui.add(SoftButton::new("Cancel")).clicked() {
                        go = Some(false);
                    }
                });
            });
        if r.close && go.is_none() && open {
            go = Some(false);
        }
        if let Some(yes) = go
            && let Some(e) = self.drawing.take()
            && yes
        {
            let none_f = Default::default();
            let none_e = Default::default();
            let none_g = Default::default();
            let only = e.selection_only && has_selection;
            let scope = drawing::Scope {
                path: s.open_path(),
                faces: if only { &s.selection().faces } else { &none_f },
                edges: if only { &s.selection().edges } else { &none_e },
                groups: if only { s.selected_groups() } else { &none_g },
            };
            let made = drawing::export(s.document().assembly(), scope, e.view, e.unit, e.format);
            let made = match made {
                Ok(m) => m,
                Err(why) => {
                    chrome.status_note(format!("Not exported. {why}"));
                    return;
                }
            };
            let ext = e.format.extension();
            let text = made.text.clone();
            let make = |stem: &str| vec![(format!("{stem}.{ext}"), text.clone().into_bytes())];
            match crate::mesh_files::save_all("Prism drawing", ext, &make) {
                Ok(Some(to)) => chrome.status_note(format!("Exported {} to {to}", made.summary())),
                Ok(None) => {}
                Err(err) => chrome.status_note(format!("Not exported: {err}")),
            }
        }
    }

    /// File > Import of a STEP file: what it holds and its size, then
    /// Import (one undo step) or Cancel.
    fn step_import_dialog(&mut self, ctx: &Context, s: &mut Session, chrome: &mut Chrome) {
        let open = self.step_in.is_some();
        let mut go = None;
        let r = SoftDialog::new("Import STEP")
            .subtitle("Exact solids and surfaces from another CAD program")
            .show(ctx, open, |ui| {
                let Some(i) = &self.step_in else { return };
                ui.label(format!("{}: {}", i.name, i.read.summary()));
                if let Some((lo, hi)) = i.read.assembly.flattened().bbox() {
                    let size = hi - lo;
                    ui.label(format!(
                        "Size: {} × {} × {}",
                        s.number.show(size.x),
                        s.number.show(size.y),
                        s.number.show(size.z)
                    ));
                }
                ui.label("Each part comes in as a group named after it; a part used more than once is a component.");
                ui.horizontal(|ui| {
                    if ui.add(SoftButton::new("Import")).clicked() {
                        go = Some(true);
                    }
                    if ui.add(SoftButton::new("Cancel")).clicked() {
                        go = Some(false);
                    }
                });
            });
        if r.close && go.is_none() && open {
            go = Some(false);
        }
        if let Some(yes) = go
            && let Some(i) = self.step_in.take()
            && yes
        {
            let summary = i.read.summary();
            if s.import_assembly(i.read.assembly) {
                chrome.status_note(format!("Imported {} ({summary})", i.name));
            } else {
                chrome.status_note(format!("{} had nothing to import", i.name));
            }
        }
    }

    /// File > Export > STEP: the unit, and the selection only.
    fn step_dialog(&mut self, ctx: &Context, s: &mut Session, chrome: &mut Chrome) {
        let open = self.step.is_some();
        let has_selection = !s.selection().faces.is_empty();
        let mut go = None;
        // Its own title: windows are told apart by it.
        let r = SoftDialog::new("Export STEP")
            .subtitle("For machining, CAM and other CAD programs")
            .show(ctx, open, |ui| {
                let Some(e) = &mut self.step else { return };
                let unames: Vec<&str> = UNITS.iter().map(|u| u.name()).collect();
                let mut k = UNITS.iter().position(|u| *u == e.unit).unwrap_or(0);
                ui.add(SoftSelect::new(&mut k, &unames, "Unit"));
                e.unit = UNITS[k];
                if has_selection {
                    ui.add(SoftToggle::new(&mut e.selection_only, "Only the selected faces"));
                }
                ui.label(
                    "STEP keeps exact curves and solids. Each group and component is a named part; hidden ones are left out.",
                );
                ui.horizontal(|ui| {
                    if ui.add(SoftButton::new("Export…")).clicked() {
                        go = Some(true);
                    }
                    if ui.add(SoftButton::new("Cancel")).clicked() {
                        go = Some(false);
                    }
                });
            });
        if r.close && go.is_none() && open {
            go = Some(false);
        }
        if let Some(yes) = go
            && let Some(e) = self.step.take()
            && yes
        {
            let faces = (e.selection_only && has_selection).then(|| s.selection().faces.clone());
            let made = prism_file::step::export(
                s.document().assembly(),
                "Prism model",
                e.unit,
                faces.as_ref(),
            );
            let made = match made {
                Ok(m) => m,
                Err(why) => {
                    chrome.status_note(format!("Not exported. {why}"));
                    return;
                }
            };
            let text = made.text.clone();
            let make = |stem: &str| vec![(format!("{stem}.step"), text.clone().into_bytes())];
            match crate::mesh_files::save_all("Prism model", "step", &make) {
                Ok(Some(to)) => chrome.status_note(format!("Exported {} to {to}", made.summary())),
                Ok(None) => {}
                Err(err) => chrome.status_note(format!("Not exported: {err}")),
            }
        }
    }

    fn import_dialog(&mut self, ctx: &Context, s: &mut Session, chrome: &mut Chrome) {
        let open = self.importing.is_some();
        let mut go = None;
        let title = self
            .importing
            .as_ref()
            .map_or(String::new(), |i| i.name.clone());
        let r = SoftDialog::new("Import")
            .subtitle(title)
            .show(ctx, open, |ui| {
                let Some(i) = &mut self.importing else { return };
                ui.label(format!(
                    "{} · {} triangles",
                    i.format.name(),
                    i.mesh.triangle_count()
                ));
                if i.bytes > BIG {
                    ui.label("A big file: importing may take a while.");
                }
                match i.stated {
                    Some(_) => {
                        ui.label("The file gives its own unit.");
                    }
                    None => {
                        let names: Vec<&str> = UNITS.iter().map(|u| u.name()).collect();
                        let mut k = UNITS.iter().position(|u| *u == i.unit).unwrap_or(0);
                        ui.add(SoftSelect::new(&mut k, &names, "The file's unit"));
                        i.unit = UNITS[k];
                    }
                }
                let scale = i.stated.unwrap_or(i.unit.metres());
                let size = i.mesh.size() * scale;
                ui.label(format!(
                    "Size: {} × {} × {}",
                    s.number.show(size.x),
                    s.number.show(size.y),
                    s.number.show(size.z)
                ));
                ui.add(SoftToggle::new(&mut i.y_up, "Y is up in the file"));
                if let Some(sc) = &i.scene {
                    let n = sc.nodes.len();
                    ui.add(SoftToggle::new(
                        &mut i.keep,
                        format!("Keep its {n} objects as groups and components"),
                    ));
                }
                ui.add(SoftToggle::new(
                    &mut i.merge,
                    "Merge coplanar triangles into faces",
                ));
                ui.horizontal(|ui| {
                    if ui.add(SoftButton::new("Import")).clicked() {
                        go = Some(true);
                    }
                    if ui.add(SoftButton::new("Cancel")).clicked() {
                        go = Some(false);
                    }
                });
            });
        if r.close && go.is_none() && open {
            go = Some(false);
        }
        if let Some(yes) = go
            && let Some(i) = self.importing.take()
            && yes
        {
            let scale = i.stated.unwrap_or(i.unit.metres());
            // A GLB's objects as groups and components (brief 35).
            if let Some(sc) = i.scene.as_ref().filter(|_| i.keep) {
                let asm = mesh::assembly_of(sc, scale, i.y_up, i.merge);
                let n = asm.placed().len();
                if s.import_assembly(asm) {
                    chrome.status_note(format!("Imported {} ({n} groups and components)", i.name));
                } else {
                    chrome.status_note(format!("{} had nothing to import", i.name));
                }
                return;
            }
            // The file's materials come in too (Basic Materials part 3).
            let (model, sizes) = mesh::build_painted(&i.mesh, scale, i.y_up, i.merge);
            let materials: Vec<_> = i
                .mesh
                .materials
                .iter()
                .zip(sizes)
                .map(|(m, size)| m.to_doc(size))
                .collect();
            let (faces, n_mat) = (model.face_count(), materials.len());
            if s.import_painted(model, materials) {
                chrome.status_note(match n_mat {
                    0 => format!("Imported {} ({faces} faces)", i.name),
                    n => format!(
                        "Imported {} ({faces} faces, {n} material{})",
                        i.name,
                        if n == 1 { "" } else { "s" }
                    ),
                });
            } else {
                chrome.status_note(format!("{} had nothing to import", i.name));
            }
        }
    }

    fn export_dialog(&mut self, ctx: &Context, s: &mut Session, chrome: &mut Chrome) {
        let open = self.exporting.is_some();
        let title = self.exporting.as_ref().map_or("Export", |e| e.title);
        let has_selection = !s.selection().faces.is_empty();
        let mut go = None;
        let r = SoftDialog::new("Export")
            .subtitle(title)
            .show(ctx, open, |ui| {
                let Some(e) = &mut self.exporting else { return };
                let names: Vec<&str> = e.formats.iter().map(|f| f.name()).collect();
                let before = e.format;
                ui.add(SoftSelect::new(&mut e.format, &names, "Format"));
                if e.format != before {
                    let o = ExportOptions::for_format(e.formats[e.format]);
                    e.unit = o.unit;
                    e.y_up = o.y_up;
                }
                let unames: Vec<&str> = UNITS.iter().map(|u| u.name()).collect();
                let mut k = UNITS.iter().position(|u| *u == e.unit).unwrap_or(0);
                ui.add(SoftSelect::new(&mut k, &unames, "Unit"));
                e.unit = UNITS[k];
                ui.add(SoftToggle::new(&mut e.y_up, "Y up (glTF and most media programs)"));
                if has_selection {
                    ui.add(SoftToggle::new(&mut e.selection_only, "Only the selected faces"));
                }
                if e.formats[e.format] == Format::Glb && !(has_selection && e.selection_only) {
                    ui.add(SoftToggle::new(&mut e.merge, "Merge into one mesh"));
                    if !e.merge {
                        ui.label(
                            "Each group and component is its own named object, with its own axes; a component's mesh is stored once.",
                        );
                    }
                }
                ui.label(
                    "A mesh keeps the shape as triangles: exact curves and faces with holes become triangles.",
                );
                ui.horizontal(|ui| {
                    if ui.add(SoftButton::new("Export…")).clicked() {
                        go = Some(true);
                    }
                    if ui.add(SoftButton::new("Cancel")).clicked() {
                        go = Some(false);
                    }
                });
            });
        if r.close && go.is_none() && open {
            go = Some(false);
        }
        if let Some(yes) = go
            && let Some(e) = self.exporting.take()
            && yes
        {
            let f = e.formats[e.format];
            let o = ExportOptions {
                format: f,
                unit: e.unit,
                y_up: e.y_up,
            };
            let faces = (e.selection_only && has_selection).then(|| s.selection().faces.clone());
            // With groups, what is shown is exported, groups placed
            // (selection-only export works on loose geometry at the root).
            let whole = (faces.is_none() && s.document().assembly().has_instances())
                .then(|| s.document().assembly().flattened_visible());
            let asm = Some(s.document().assembly());
            let model = whole.as_ref().unwrap_or(s.model());
            let only = if whole.is_some() {
                None
            } else {
                faces.as_ref()
            };
            // GLB keeps groups and components as named objects unless
            // merged (brief 35); OBJ with materials is several files (its
            // .mtl and images).
            let structured = f == Format::Glb && !e.merge && faces.is_none();
            let doc = s.document().assembly();
            let make = |stem: &str| {
                if structured {
                    vec![(
                        format!("{stem}.glb"),
                        mesh::export_structured(doc, &o, stem, true),
                    )]
                } else {
                    mesh::export_files(model, only, &o, asm, stem)
                }
            };
            // Check what was written (PLAN 1.3): printing formats must be
            // watertight. The export still happens.
            let printing = matches!(f.extension(), "stl" | "3mf");
            let note =
                prism_ops::health::export_note(whole.as_ref().unwrap_or(s.model()), printing);
            match crate::mesh_files::save_all("Prism model", f.extension(), &make) {
                Ok(Some(to)) => chrome.status_note(match note {
                    Some(n) => format!("Exported to {to}. {n}"),
                    None => format!("Exported to {to}"),
                }),
                Ok(None) => {}
                Err(err) => chrome.status_note(format!("Not exported: {err}")),
            }
        }
    }
}

/// Whether anything is selected that a drawing can be made of.
fn has_choice(s: &Session) -> bool {
    let sel = s.selection();
    !sel.faces.is_empty() || !sel.edges.is_empty() || !s.selected_groups().is_empty()
}
