//! 3MF (DECISIONS #25: our own code): a zip with one model file, one object.

use std::fmt::Write;
use std::io::{Cursor, Read};

use prism_math::{DVec3, LengthUnit};
use quick_xml::events::Event;

use super::{Mesh, damaged};
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
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"{}\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n <metadata name=\"Application\">Prism</metadata>\n <resources>\n  <object id=\"1\" type=\"model\">\n   <mesh>\n    <vertices>\n",
        unit_name(unit)
    );
    for p in &m.positions {
        let _ = writeln!(
            s,
            "     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>",
            p.x, p.y, p.z
        );
    }
    s.push_str("    </vertices>\n    <triangles>\n");
    for p in &m.polygons {
        for i in 1..p.len().saturating_sub(1) {
            let _ = writeln!(
                s,
                "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>",
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
                match name.as_ref() {
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
    Ok(m)
}
