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
}

/// The Export dialog.
struct Exporting {
    formats: &'static [Format],
    title: &'static str,
    format: usize,
    selection_only: bool,
    unit: LengthUnit,
    y_up: bool,
}

#[derive(Default)]
pub struct Transfer {
    importing: Option<Importing>,
    exporting: Option<Exporting>,
}

const PRINT: [Format; 2] = [Format::ThreeMf, Format::Stl];
const MEDIA: [Format; 3] = [Format::Glb, Format::Obj, Format::Ply];
const UNITS: [LengthUnit; 4] = [
    LengthUnit::Millimetres,
    LengthUnit::Centimetres,
    LengthUnit::Metres,
    LengthUnit::Inches,
];

impl Transfer {
    pub fn modal(&self) -> bool {
        self.importing.is_some() || self.exporting.is_some()
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
            "exp.gltf" => self.start_export(&MEDIA, "Export for media and the web", s),
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
        });
    }

    /// A mesh file, picked or dropped: read it and show the Import dialog.
    pub fn open(&mut self, o: Opened, chrome: &mut Chrome) {
        let Some(format) = Format::of(&o.name) else {
            chrome.status_note(format!(
                "{} is not a format Prism imports (STL, OBJ, 3MF, glTF, PLY)",
                o.name
            ));
            return;
        };
        match mesh::read(&o.bytes, format) {
            Ok(m) if m.polygons.is_empty() => {
                chrome.status_note(format!("{} has no faces to import", o.name));
            }
            Ok(m) => {
                self.importing = Some(Importing {
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
            let bytes = match &whole {
                Some(m) => mesh::export_with(m, None, &o, asm),
                None => mesh::export_with(s.model(), faces.as_ref(), &o, asm),
            };
            // Check what was written (PLAN 1.3): printing formats must be
            // watertight. The export still happens.
            let printing = matches!(f.extension(), "stl" | "3mf");
            let note =
                prism_ops::health::export_note(whole.as_ref().unwrap_or(s.model()), printing);
            let name = format!("Prism model.{}", f.extension());
            match crate::mesh_files::save(&name, f.extension(), &bytes) {
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
