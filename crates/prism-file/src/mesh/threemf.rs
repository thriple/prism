//! 3MF (DECISIONS #25: our own code): a zip with one model file, one object.
//! Colours (Basic Materials part 3) are base materials; textures are not
//! exported (brief 28).

use std::fmt::Write;
use std::io::{Cursor, Read};

use prism_math::{DVec3, LengthUnit};
use quick_xml::events::Event;

use super::{Mesh, MeshMaterial, damaged};

/// The base material Prism's unpainted triangles use: Prism's Default is
/// not a material, so a file read back leaves it out.
const DEFAULT: &str = "Default";

fn srgb8(x: f32) -> u8 {
    let x = x.clamp(0.0, 1.0);
    let s = if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

/// `#RRGGBB` or `#RRGGBBAA` (sRGB) as linear RGBA.
fn colour(hex: &str) -> Option<[f32; 4]> {
    let h = hex.strip_prefix('#')?;
    let byte = |k: usize| u8::from_str_radix(h.get(2 * k..2 * k + 2)?, 16).ok();
    let lin = |b: u8| {
        let s = f32::from(b) / 255.0;
        if s <= 0.040_45 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    let a = if h.len() >= 8 { byte(3)? } else { 255 };
    Some([
        lin(byte(0)?),
        lin(byte(1)?),
        lin(byte(2)?),
        f32::from(a) / 255.0,
    ])
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
use crate::FileError;

const MODEL: &str = "3D/3dmodel.model";

fn unit_name(u: LengthUnit) -> &'static str {
    match u {
        LengthUnit::Millimetres => "millimeter",
        LengthUnit::Centimetres => "centimeter",
        LengthUnit::Metres => "meter",
        LengthUnit::Inches | LengthUnit::FeetInches => "inch",
    }
}

pub fn write(m: &Mesh, unit: LengthUnit) -> Vec<u8> {
    let types = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
 <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
 <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
</Types>
"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>
"#;
    let coloured = !m.materials.is_empty() && m.polygon_materials.len() == m.polygons.len();
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"{}\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n <metadata name=\"Application\">Prism</metadata>\n <resources>\n",
        unit_name(unit)
    );
    // Colours: one base material each, then Default for unpainted ones
    // (the object's own).
    let default = m.materials.len();
    if coloured {
        s.push_str("  <basematerials id=\"2\">\n");
        for x in &m.materials {
            let b = x.base;
            let _ = writeln!(
                s,
                "   <base name=\"{}\" displaycolor=\"#{:02X}{:02X}{:02X}{:02X}\"/>",
                xml_escape(&x.name),
                srgb8(b[0]),
                srgb8(b[1]),
                srgb8(b[2]),
                (b[3].clamp(0.0, 1.0) * 255.0).round() as u8
            );
        }
        let _ = writeln!(
            s,
            "   <base name=\"{DEFAULT}\" displaycolor=\"#FFFFFFFF\"/>"
        );
        s.push_str("  </basematerials>\n");
        let _ = writeln!(
            s,
            "  <object id=\"1\" type=\"model\" pid=\"2\" pindex=\"{default}\">"
        );
    } else {
        s.push_str("  <object id=\"1\" type=\"model\">\n");
    }
    s.push_str("   <mesh>\n    <vertices>\n");
    for p in &m.positions {
        let _ = writeln!(
            s,
            "     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>",
            p.x, p.y, p.z
        );
    }
    s.push_str("    </vertices>\n    <triangles>\n");
    for (k, p) in m.polygons.iter().enumerate() {
        let colour = if coloured {
            let i = m.polygon_materials[k].map_or(default, |x| x as usize);
            format!(" pid=\"2\" p1=\"{i}\"")
        } else {
            String::new()
        };
        for i in 1..p.len().saturating_sub(1) {
            let _ = writeln!(
                s,
                "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"{colour}/>",
                p[0],
                p[i],
                p[i + 1]
            );
        }
    }
    s.push_str("    </triangles>\n   </mesh>\n  </object>\n </resources>\n <build>\n  <item objectid=\"1\"/>\n </build>\n</model>\n");
    crate::zip_files(&[
        ("[Content_Types].xml", types.as_bytes().to_vec()),
        ("_rels/.rels", rels.as_bytes().to_vec()),
        (MODEL, s.into_bytes()),
    ])
}

/// The file's unit, in metres per unit.
pub fn unit_of(bytes: &[u8]) -> Option<f64> {
    let xml = model_xml(bytes).ok()?;
    let mut r = quick_xml::Reader::from_reader(xml.as_slice());
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.local_name().as_ref() == "model" => {
                for a in e.attributes().flatten() {
                    if a.key.local_name().as_ref() == "unit" {
                        return Some(match a.value.as_ref() {
                            "micron" => 1e-6,
                            "millimeter" => 1e-3,
                            "centimeter" => 1e-2,
                            "inch" => 0.0254,
                            "foot" => 0.3048,
                            "meter" => 1.0,
                            _ => 1e-3,
                        });
                    }
                }
                return Some(1e-3);
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
}

fn model_xml(bytes: &[u8]) -> Result<Vec<u8>, FileError> {
    let mut z =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| damaged(format!("3MF: {e}")))?;
    let mut f = z
        .by_name(MODEL)
        .map_err(|_| damaged("3MF: no 3D/3dmodel.model"))?;
    let mut xml = Vec::new();
    (&mut f)
        .take(1 << 32)
        .read_to_end(&mut xml)
        .map_err(|e| damaged(format!("3MF: {e}")))?;
    Ok(xml)
}

pub fn read(bytes: &[u8]) -> Result<Mesh, FileError> {
    let xml = model_xml(bytes)?;
    let mut r = quick_xml::Reader::from_reader(xml.as_slice());
    let mut buf = Vec::new();
    let mut m = Mesh::default();
    // Each object's vertices are numbered from 0; later objects follow on.
    let mut base = 0u32;
    // Base material groups by id: each base's material (None: Prism's
    // Default), and the object's own.
    let mut groups: std::collections::BTreeMap<String, Vec<Option<u32>>> = Default::default();
    let mut group: Option<String> = None;
    let mut object: Option<(String, usize)> = None;
    let mut any = false;
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.local_name();
                let get = |k: &str| -> Option<String> {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.local_name().as_ref() == k)
                        .map(|a| a.value.to_string())
                };
                let pick = |pid: Option<String>, index: Option<usize>| -> Option<u32> {
                    groups.get(&pid?)?.get(index?).copied().flatten()
                };
                match name.as_ref() {
                    "basematerials" => {
                        group = get("id");
                        if let Some(g) = &group {
                            groups.insert(g.clone(), Vec::new());
                        }
                    }
                    "base" => {
                        if let Some(g) = group.as_ref().and_then(|g| groups.get_mut(g)) {
                            let name =
                                get("name").unwrap_or_else(|| format!("Colour {}", g.len() + 1));
                            let base = get("displaycolor").and_then(|c| colour(&c));
                            g.push(match base {
                                Some(base) if name != DEFAULT => {
                                    m.materials.push(MeshMaterial {
                                        name,
                                        base,
                                        roughness: 0.5,
                                        metalness: 0.0,
                                        texture: None,
                                    });
                                    Some((m.materials.len() - 1) as u32)
                                }
                                _ => None,
                            });
                        }
                    }
                    "object" => {
                        object = get("pid").zip(get("pindex").and_then(|i| i.parse().ok()));
                    }
                    "mesh" => base = m.positions.len() as u32,
                    "vertex" => {
                        let c = |k: &str| -> Result<f64, FileError> {
                            get(k)
                                .and_then(|v| v.parse().ok())
                                .ok_or_else(|| damaged("3MF: a bad vertex"))
                        };
                        m.positions.push(DVec3::new(c("x")?, c("y")?, c("z")?));
                    }
                    "triangle" => {
                        let i = |k: &str| -> Result<u32, FileError> {
                            get(k)
                                .and_then(|v| v.parse::<u32>().ok())
                                .map(|v| base + v)
                                .ok_or_else(|| damaged("3MF: a bad triangle"))
                        };
                        m.polygons.push(vec![i("v1")?, i("v2")?, i("v3")?]);
                        let (opid, oidx) = object.clone().unzip();
                        let mat = pick(
                            get("pid").or(opid),
                            get("p1").and_then(|i| i.parse().ok()).or(oidx),
                        );
                        any |= mat.is_some();
                        m.polygon_materials.push(mat);
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(damaged(format!("3MF: {e}"))),
            _ => {}
        }
        buf.clear();
    }
    if !any {
        m.polygon_materials.clear();
        m.materials.clear();
    }
    Ok(m)
}
