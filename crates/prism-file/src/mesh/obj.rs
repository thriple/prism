//! OBJ: vertices and faces (polygons). Materials (Basic Materials part 3)
//! go in an `.mtl` beside it (`Kd`, `d`, `map_Kd`), with the images
//! copied beside it too.

use std::fmt::Write;

use prism_math::{DVec2, DVec3};

use super::{Mesh, MeshImage, MeshMaterial, damaged};
use crate::FileError;

/// A material's name as OBJ allows it (no spaces).
fn obj_name(s: &str) -> String {
    let n: String = s
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    if n.is_empty() { "material".into() } else { n }
}

fn srgb(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

fn linear(s: f32) -> f32 {
    let s = s.clamp(0.0, 1.0);
    if s <= 0.040_45 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// An OBJ with its materials: the `.obj`, the `.mtl` and the images, as
/// (file name, bytes), all named after `stem`.
pub fn write_painted(m: &Mesh, stem: &str) -> Vec<(String, Vec<u8>)> {
    // The files the OBJ names have no spaces (some readers split on them).
    let side = obj_name(stem);
    let mtl_name = format!("{side}.mtl");
    let mut s = format!("# Prism\nmtllib {mtl_name}\no prism\n");
    for p in &m.positions {
        let _ = writeln!(s, "v {} {} {}", p.x, p.y, p.z);
    }
    let uv = m.uvs.len() == m.positions.len();
    if uv {
        // OBJ's v runs up the image.
        for q in &m.uvs {
            let _ = writeln!(s, "vt {} {}", q.x, -q.y);
        }
    }
    // Unique names, in order.
    let mut names: Vec<String> = Vec::new();
    for x in &m.materials {
        let base = obj_name(&x.name);
        let mut n = base.clone();
        let mut k = 2;
        while names.contains(&n) {
            n = format!("{base}_{k}");
            k += 1;
        }
        names.push(n);
    }
    let mut current: Option<Option<u32>> = None;
    for (k, f) in m.polygons.iter().enumerate() {
        let mat = m.polygon_materials.get(k).copied().flatten();
        if current != Some(mat) {
            match mat {
                Some(i) => {
                    let _ = writeln!(s, "usemtl {}", names[i as usize]);
                }
                None => s.push_str("usemtl Default\n"),
            }
            current = Some(mat);
        }
        s.push('f');
        for i in f {
            if uv {
                let _ = write!(s, " {}/{}", i + 1, i + 1);
            } else {
                let _ = write!(s, " {}", i + 1);
            }
        }
        s.push('\n');
    }
    let mut mtl = String::from("# Prism\nnewmtl Default\nKd 1 1 1\n");
    let mut files = Vec::new();
    for (x, n) in m.materials.iter().zip(&names) {
        let b = x.base;
        let _ = writeln!(
            mtl,
            "\nnewmtl {n}\nKd {} {} {}\nd {}\nNs {}",
            srgb(b[0]),
            srgb(b[1]),
            srgb(b[2]),
            b[3],
            ((1.0 - x.roughness).clamp(0.0, 1.0) * 1000.0).round()
        );
        if let Some(t) = &x.texture {
            let file = format!("{side}_{n}.{}", t.kind.extension());
            if let Some(size) = t.size {
                let _ = writeln!(mtl, "# prism_texture_size_m {} {}", size[0], size[1]);
            }
            let _ = writeln!(mtl, "map_Kd {file}");
            files.push((file, t.bytes.to_vec()));
        }
    }
    let mut out = vec![
        (format!("{stem}.obj"), s.into_bytes()),
        (mtl_name, mtl.into_bytes()),
    ];
    out.extend(files);
    out
}

/// The `.mtl` files an OBJ names.
pub fn mtl_libs(b: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(b)
        .lines()
        .filter_map(|l| l.trim().strip_prefix("mtllib "))
        .map(|n| n.trim().to_owned())
        .collect()
}

/// Fill in the mesh's materials (named by `usemtl`) from an `.mtl`;
/// `sibling` reads a file named in it (the images), if it can.
pub fn apply_mtl(m: &mut Mesh, mtl: &[u8], sibling: &dyn Fn(&str) -> Option<Vec<u8>>) {
    let text = String::from_utf8_lossy(mtl);
    let mut at: Option<usize> = None;
    let mut size: Option<[f64; 2]> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(r) = line.strip_prefix("# prism_texture_size_m ") {
            let v: Vec<f64> = r
                .split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect();
            size = (v.len() == 2 && v[0] > 0.0 && v[1] > 0.0).then(|| [v[0], v[1]]);
            continue;
        }
        let line = line.split('#').next().unwrap_or("");
        let mut w = line.split_whitespace();
        let Some(key) = w.next() else { continue };
        let rest: Vec<&str> = w.collect();
        let nums: Vec<f32> = rest.iter().filter_map(|x| x.parse().ok()).collect();
        match key {
            "newmtl" => {
                let name = rest.join(" ");
                at = m.materials.iter().position(|x| x.name == name);
                size = None;
            }
            "Kd" if nums.len() >= 3 => {
                if let Some(x) = at.map(|k| &mut m.materials[k]) {
                    x.base = [linear(nums[0]), linear(nums[1]), linear(nums[2]), x.base[3]];
                }
            }
            "d" if !nums.is_empty() => {
                if let Some(x) = at.map(|k| &mut m.materials[k]) {
                    x.base[3] = nums[0].clamp(0.0, 1.0);
                }
            }
            "Tr" if !nums.is_empty() => {
                if let Some(x) = at.map(|k| &mut m.materials[k]) {
                    x.base[3] = (1.0 - nums[0]).clamp(0.0, 1.0);
                }
            }
            "map_Kd" => {
                // The file comes after any options (which start with -);
                // without options it may have spaces.
                let file = if rest.first().is_some_and(|w| w.starts_with('-')) {
                    rest.last().map(|f| (*f).to_owned())
                } else {
                    Some(rest.join(" "))
                };
                let image = file
                    .and_then(|f| sibling(&f))
                    .and_then(|b| prism_doc::Image::read(b).ok());
                if let (Some(x), Some(i)) = (at.map(|k| &mut m.materials[k]), image) {
                    x.texture = Some(MeshImage {
                        kind: i.kind,
                        bytes: i.bytes.clone(),
                        size,
                    });
                }
            }
            _ => {}
        }
    }
}

pub fn write(m: &Mesh) -> Vec<u8> {
    let mut s = String::from("# Prism\no prism\n");
    for p in &m.positions {
        let _ = writeln!(s, "v {} {} {}", p.x, p.y, p.z);
    }
    for f in &m.polygons {
        s.push('f');
        for i in f {
            let _ = write!(s, " {}", i + 1);
        }
        s.push('\n');
    }
    s.into_bytes()
}

pub fn read(b: &[u8]) -> Result<Mesh, FileError> {
    let text = String::from_utf8_lossy(b);
    let mut m = Mesh::default();
    // Positions as written, texture places, and the mesh's corners made
    // of the pairs faces use.
    let (mut vs, mut vts): (Vec<DVec3>, Vec<DVec2>) = (Vec::new(), Vec::new());
    let mut pairs: std::collections::BTreeMap<(usize, Option<usize>), u32> = Default::default();
    let mut mat: Option<u32> = None;
    let mut any_mat = false;
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("");
        let mut w = line.split_whitespace();
        match w.next() {
            Some("v") => {
                let mut c = [0.0; 3];
                for x in &mut c {
                    *x = w
                        .next()
                        .and_then(|s| s.parse::<f64>().ok())
                        .ok_or_else(|| {
                            damaged(format!("line {}: a vertex needs three numbers", n + 1))
                        })?;
                }
                vs.push(DVec3::from_array(c));
            }
            Some("vt") => {
                let c: Vec<f64> = w.filter_map(|s| s.parse().ok()).collect();
                // OBJ's v runs up the image; the mesh's runs down.
                vts.push(DVec2::new(
                    c.first().copied().unwrap_or(0.0),
                    -c.get(1).copied().unwrap_or(0.0),
                ));
            }
            Some("usemtl") => {
                let name = w.collect::<Vec<_>>().join(" ");
                mat = (name != "Default" && !name.is_empty()).then(|| {
                    let k = m
                        .materials
                        .iter()
                        .position(|x| x.name == name)
                        .unwrap_or_else(|| {
                            m.materials.push(MeshMaterial {
                                name,
                                base: [1.0; 4],
                                roughness: 0.5,
                                metalness: 0.0,
                                texture: None,
                            });
                            m.materials.len() - 1
                        });
                    k as u32
                });
            }
            Some("f") => {
                let count = vs.len() as i64;
                let mut poly = Vec::new();
                for t in w {
                    let mut parts = t.split('/');
                    let i: i64 = parts
                        .next()
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| damaged(format!("line {}: a bad face index", n + 1)))?;
                    // 1-based, or negative from the end.
                    let i = if i < 0 { count + i } else { i - 1 };
                    if i < 0 || i >= count {
                        return Err(damaged(format!(
                            "line {}: a face uses a missing vertex",
                            n + 1
                        )));
                    }
                    let vt = parts
                        .next()
                        .and_then(|s| s.parse::<i64>().ok())
                        .map(|j| if j < 0 { vts.len() as i64 + j } else { j - 1 })
                        .filter(|j| *j >= 0 && (*j as usize) < vts.len())
                        .map(|j| j as usize);
                    let k = *pairs.entry((i as usize, vt)).or_insert_with(|| {
                        m.positions.push(vs[i as usize]);
                        m.uvs.push(vt.map_or(DVec2::ZERO, |j| vts[j]));
                        (m.positions.len() - 1) as u32
                    });
                    poly.push(k);
                }
                if poly.len() >= 3 {
                    m.polygons.push(poly);
                    m.polygon_materials.push(mat);
                    any_mat |= mat.is_some();
                }
            }
            _ => {}
        }
    }
    if !any_mat {
        m.materials.clear();
        m.polygon_materials.clear();
    }
    if vts.is_empty() {
        m.uvs.clear();
    }
    Ok(m)
}
