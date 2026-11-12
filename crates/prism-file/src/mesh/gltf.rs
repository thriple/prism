//! glTF: binary GLB to write; GLB or `.gltf` with embedded buffers to read.
//! Only triangle primitives, with each node's transform applied.

use prism_math::{DMat4, DQuat, DVec3};
use serde_json::{Value, json};

use prism_doc::image::ImageKind;

use super::{Mesh, MeshImage, MeshMaterial, damaged};
use crate::FileError;

const GLB: u32 = 0x4654_6C67; // "glTF"
const JSON: u32 = 0x4E4F_534A;
const BIN: u32 = 0x004E_4942;

fn pad(v: &mut Vec<u8>, with: u8) {
    while !v.len().is_multiple_of(4) {
        v.push(with);
    }
}

pub fn write(m: &Mesh) -> Vec<u8> {
    let mut bin = Vec::new();
    let mut views: Vec<Value> = Vec::new();
    let mut accessors: Vec<Value> = Vec::new();
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &m.positions {
        for (k, c) in p.to_array().iter().enumerate() {
            let c = *c as f32;
            lo[k] = lo[k].min(c);
            hi[k] = hi[k].max(c);
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    if m.positions.is_empty() {
        (lo, hi) = ([0.0; 3], [0.0; 3]);
    }
    views.push(json!({"buffer": 0, "byteOffset": 0, "byteLength": bin.len(), "target": 34962}));
    accessors.push(
        json!({"bufferView": 0, "componentType": 5126, "count": m.positions.len(),
        "type": "VEC3", "min": lo, "max": hi}),
    );
    let mut attributes = json!({"POSITION": 0});
    if !m.uvs.is_empty() && m.uvs.len() == m.positions.len() {
        let at = bin.len();
        for q in &m.uvs {
            bin.extend_from_slice(&(q.x as f32).to_le_bytes());
            bin.extend_from_slice(&(q.y as f32).to_le_bytes());
        }
        views.push(
            json!({"buffer": 0, "byteOffset": at, "byteLength": bin.len() - at, "target": 34962}),
        );
        accessors.push(json!({"bufferView": views.len() - 1, "componentType": 5126,
            "count": m.uvs.len(), "type": "VEC2"}));
        attributes["TEXCOORD_0"] = json!(accessors.len() - 1);
    }
    // One primitive per material (polygons without one together).
    let mut groups: std::collections::BTreeMap<Option<u32>, Vec<u32>> = Default::default();
    for (k, p) in m.polygons.iter().enumerate() {
        let mat = m.polygon_materials.get(k).copied().flatten();
        let idx = groups.entry(mat).or_default();
        for i in 1..p.len().saturating_sub(1) {
            idx.extend([p[0], p[i], p[i + 1]]);
        }
    }
    let mut primitives = Vec::new();
    for (mat, idx) in &groups {
        let at = bin.len();
        for v in idx {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        views.push(
            json!({"buffer": 0, "byteOffset": at, "byteLength": bin.len() - at, "target": 34963}),
        );
        accessors.push(json!({"bufferView": views.len() - 1, "componentType": 5125,
            "count": idx.len(), "type": "SCALAR"}));
        let mut p = json!({"attributes": attributes, "indices": accessors.len() - 1, "mode": 4});
        if let Some(k) = mat {
            p["material"] = json!(k);
        }
        primitives.push(p);
    }
    if primitives.is_empty() {
        views.push(json!({"buffer": 0, "byteOffset": bin.len(), "byteLength": 0, "target": 34963}));
        accessors.push(json!({"bufferView": views.len() - 1, "componentType": 5125,
            "count": 0, "type": "SCALAR"}));
        primitives
            .push(json!({"attributes": attributes, "indices": accessors.len() - 1, "mode": 4}));
    }
    // Materials, and their images in the binary chunk.
    let (mut images, mut textures, mut materials) = (Vec::new(), Vec::new(), Vec::new());
    for mm in &m.materials {
        let mut pbr = json!({
            "baseColorFactor": mm.base,
            "metallicFactor": mm.metalness,
            "roughnessFactor": mm.roughness,
        });
        if let Some(t) = &mm.texture {
            pad(&mut bin, 0);
            let at = bin.len();
            bin.extend_from_slice(&t.bytes);
            views.push(json!({"buffer": 0, "byteOffset": at, "byteLength": t.bytes.len()}));
            let mime = match t.kind {
                ImageKind::Png => "image/png",
                ImageKind::Jpeg => "image/jpeg",
            };
            images.push(json!({"bufferView": views.len() - 1, "mimeType": mime}));
            textures.push(json!({"source": images.len() - 1, "sampler": 0}));
            pbr["baseColorTexture"] = json!({"index": textures.len() - 1});
            // The image shows as it is; the material keeps its opacity.
            pbr["baseColorFactor"] = json!([1.0, 1.0, 1.0, mm.base[3]]);
        }
        let mut x = json!({"name": mm.name, "pbrMetallicRoughness": pbr, "doubleSided": true});
        if mm.base[3] < 1.0 {
            x["alphaMode"] = json!("BLEND");
        }
        if let Some(size) = mm.texture.as_ref().and_then(|t| t.size) {
            // Prism's own: one copy's size in metres, so a re-import keeps it.
            x["extras"] = json!({"prism_texture_size_m": size});
        }
        materials.push(x);
    }
    pad(&mut bin, 0);
    let mut doc = json!({
        "asset": {"version": "2.0", "generator": "Prism"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": "Prism"}],
        "meshes": [{"primitives": primitives}],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": views,
        "accessors": accessors,
    });
    if !materials.is_empty() {
        doc["materials"] = json!(materials);
    }
    if !images.is_empty() {
        doc["images"] = json!(images);
        doc["textures"] = json!(textures);
        doc["samplers"] = json!([{"wrapS": 10497, "wrapT": 10497}]);
    }
    let mut js = serde_json::to_vec(&doc).unwrap_or_default();
    pad(&mut js, b' ');
    let total = 12 + 8 + js.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    for w in [GLB, 2, total as u32, js.len() as u32, JSON] {
        out.extend_from_slice(&w.to_le_bytes());
    }
    out.extend_from_slice(&js);
    for w in [bin.len() as u32, BIN] {
        out.extend_from_slice(&w.to_le_bytes());
    }
    out.extend_from_slice(&bin);
    out
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// The JSON and the binary chunk of a GLB, or the JSON of a `.gltf`.
fn split(b: &[u8]) -> Result<(Value, Option<&[u8]>), FileError> {
    if u32_at(b, 0) == Some(GLB) {
        let len = u32_at(b, 12).ok_or_else(|| damaged("a short GLB"))? as usize;
        if u32_at(b, 16) != Some(JSON) {
            return Err(damaged("a GLB without JSON"));
        }
        let js = b.get(20..20 + len).ok_or_else(|| damaged("a short GLB"))?;
        let doc = serde_json::from_slice(js).map_err(|e| damaged(format!("glTF: {e}")))?;
        let at = 20 + len;
        let bin = match (u32_at(b, at), u32_at(b, at + 4)) {
            (Some(n), Some(BIN)) => b.get(at + 8..at + 8 + n as usize),
            _ => None,
        };
        return Ok((doc, bin));
    }
    let doc = serde_json::from_slice(b).map_err(|_| damaged("not a glTF file"))?;
    Ok((doc, None))
}

/// Base64, for `data:` buffers.
fn base64(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        })
    };
    let bytes: Vec<u8> = s
        .bytes()
        .filter(|c| !c.is_ascii_whitespace() && *c != b'=')
        .collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (k, c) in chunk.iter().enumerate() {
            n |= val(*c)? << (18 - 6 * k);
        }
        let take = chunk.len().saturating_sub(1);
        for k in 0..take {
            out.push((n >> (16 - 8 * k)) as u8);
        }
    }
    Some(out)
}

fn node_matrix(n: &Value) -> DMat4 {
    if let Some(m) = n.get("matrix").and_then(Value::as_array) {
        let v: Vec<f64> = m.iter().filter_map(Value::as_f64).collect();
        if v.len() == 16 {
            let mut a = [0.0; 16];
            a.copy_from_slice(&v);
            return DMat4::from_cols_array(&a);
        }
    }
    let vec = |k: &str, n_: usize, d: f64| -> Vec<f64> {
        n.get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
            .filter(|a| a.len() == n_)
            .unwrap_or_else(|| vec![d; n_])
    };
    let t = vec("translation", 3, 0.0);
    let s = vec("scale", 3, 1.0);
    let r = n
        .get("rotation")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
        .filter(|a| a.len() == 4)
        .map_or(DQuat::IDENTITY, |q| {
            DQuat::from_xyzw(q[0], q[1], q[2], q[3]).normalize()
        });
    DMat4::from_scale_rotation_translation(
        DVec3::new(s[0], s[1], s[2]),
        r,
        DVec3::new(t[0], t[1], t[2]),
    )
}

struct Doc<'a> {
    doc: &'a Value,
    buffers: Vec<Vec<u8>>,
}

impl Doc<'_> {
    fn get(&self, kind: &str, i: u64) -> Result<&Value, FileError> {
        self.doc
            .get(kind)
            .and_then(|a| a.get(i as usize))
            .ok_or_else(|| damaged(format!("glTF: missing {kind} {i}")))
    }

    /// An accessor's values as f64, `width` per element.
    fn accessor(&self, i: u64) -> Result<(Vec<f64>, usize), FileError> {
        let a = self.get("accessors", i)?;
        let count = a.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
        let width = match a.get("type").and_then(Value::as_str) {
            Some("SCALAR") => 1,
            Some("VEC2") => 2,
            Some("VEC3") => 3,
            Some(t) => return Err(damaged(format!("glTF: accessor type {t}"))),
            None => return Err(damaged("glTF: an accessor without a type")),
        };
        let ct = a.get("componentType").and_then(Value::as_u64).unwrap_or(0);
        let size = match ct {
            5121 => 1,
            5123 => 2,
            5125 | 5126 => 4,
            _ => return Err(damaged(format!("glTF: component type {ct}"))),
        };
        let Some(bv) = a.get("bufferView").and_then(Value::as_u64) else {
            return Ok((vec![0.0; count * width], width));
        };
        let v = self.get("bufferViews", bv)?;
        let buf = v.get("buffer").and_then(Value::as_u64).unwrap_or(0) as usize;
        let data = self
            .buffers
            .get(buf)
            .ok_or_else(|| damaged("glTF: a missing buffer"))?;
        let start = v.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
            + a.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let stride = v
            .get("byteStride")
            .and_then(Value::as_u64)
            .map_or(size * width, |s| s as usize);
        if count > data.len() {
            return Err(damaged("glTF: an accessor larger than its buffer"));
        }
        let mut out = Vec::with_capacity(count * width);
        for e in 0..count {
            for k in 0..width {
                let o = start + e * stride + k * size;
                let b = data
                    .get(o..o + size)
                    .ok_or_else(|| damaged("glTF: an accessor past its buffer"))?;
                out.push(match ct {
                    5121 => f64::from(b[0]),
                    5123 => f64::from(u16::from_le_bytes([b[0], b[1]])),
                    5125 => f64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                    _ => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                });
            }
        }
        Ok((out, width))
    }

    fn node(&self, i: u64, parent: DMat4, depth: usize, m: &mut Mesh) -> Result<(), FileError> {
        if depth > 64 {
            return Err(damaged("glTF: nodes nested too deep"));
        }
        let n = self.get("nodes", i)?;
        let world = parent * node_matrix(n);
        if let Some(mi) = n.get("mesh").and_then(Value::as_u64) {
            let mesh = self.get("meshes", mi)?;
            for p in mesh
                .get("primitives")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if p.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
                    continue;
                }
                let Some(pa) = p
                    .get("attributes")
                    .and_then(|a| a.get("POSITION"))
                    .and_then(Value::as_u64)
                else {
                    continue;
                };
                let (pos, _) = self.accessor(pa)?;
                let first = m.positions.len() as u32;
                for c in pos.chunks(3) {
                    m.positions
                        .push(world.transform_point3(DVec3::new(c[0], c[1], c[2])));
                }
                let n_pos = (pos.len() / 3) as u32;
                // Texture coordinates, one per position (zero where none).
                let uv = match p
                    .get("attributes")
                    .and_then(|a| a.get("TEXCOORD_0"))
                    .and_then(Value::as_u64)
                {
                    Some(ui) => self.accessor(ui)?.0,
                    None => Vec::new(),
                };
                for k in 0..n_pos as usize {
                    m.uvs.push(match uv.get(2 * k..2 * k + 2) {
                        Some(q) => prism_math::DVec2::new(q[0], q[1]),
                        None => prism_math::DVec2::ZERO,
                    });
                }
                let mat = p
                    .get("material")
                    .and_then(Value::as_u64)
                    .map(|k| k as u32)
                    .filter(|k| (*k as usize) < m.materials.len());
                let idx: Vec<u32> = match p.get("indices").and_then(Value::as_u64) {
                    Some(ii) => self.accessor(ii)?.0.iter().map(|x| *x as u32).collect(),
                    None => (0..n_pos).collect(),
                };
                for t in idx.chunks(3) {
                    if t.len() == 3 && t.iter().all(|i| *i < n_pos) {
                        m.polygons.push(t.iter().map(|i| first + i).collect());
                        m.polygon_materials.push(mat);
                    }
                }
            }
        }
        if let Some(children) = n.get("children").and_then(Value::as_array) {
            for c in children.iter().filter_map(Value::as_u64) {
                self.node(c, world, depth + 1, m)?;
            }
        }
        Ok(())
    }
}

pub fn read(b: &[u8]) -> Result<Mesh, FileError> {
    let (doc, bin) = split(b)?;
    let mut buffers = Vec::new();
    for (k, buf) in doc
        .get("buffers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        match buf.get("uri").and_then(Value::as_str) {
            None if k == 0 => buffers.push(bin.unwrap_or_default().to_vec()),
            Some(uri) if uri.starts_with("data:") => {
                let data = uri.split_once(',').map(|x| x.1).unwrap_or("");
                buffers.push(base64(data).ok_or_else(|| damaged("glTF: a bad data URI"))?);
            }
            _ => {
                return Err(damaged(
                    "glTF: buffers in separate files are not supported; use GLB",
                ));
            }
        }
    }
    let d = Doc { doc: &doc, buffers };
    let mut m = Mesh {
        materials: d.materials(),
        ..Mesh::default()
    };
    let scene = doc.get("scene").and_then(Value::as_u64).unwrap_or(0);
    let roots: Vec<u64> = match doc
        .get("scenes")
        .and_then(|s| s.get(scene as usize))
        .and_then(|s| s.get("nodes"))
        .and_then(Value::as_array)
    {
        Some(n) => n.iter().filter_map(Value::as_u64).collect(),
        None => (0..doc
            .get("nodes")
            .and_then(Value::as_array)
            .map_or(0, Vec::len) as u64)
            .collect(),
    };
    for r in roots {
        d.node(r, DMat4::IDENTITY, 0, &mut m)?;
    }
    // Nothing used a material, or nothing had coordinates: leave them out.
    if m.polygon_materials.iter().all(Option::is_none) {
        m.polygon_materials.clear();
        m.materials.clear();
    }
    if m.uvs.iter().all(|q| *q == prism_math::DVec2::ZERO) {
        m.uvs.clear();
    }
    Ok(m)
}

impl Doc<'_> {
    /// The bytes of a buffer view.
    fn view_bytes(&self, i: u64) -> Option<&[u8]> {
        let v = self.get("bufferViews", i).ok()?;
        let buf = self.buffers.get(v.get("buffer")?.as_u64()? as usize)?;
        let at = v.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let len = v.get("byteLength")?.as_u64()? as usize;
        buf.get(at..at.checked_add(len)?)
    }

    /// An image's bytes, if it is a PNG or JPEG inside the file.
    fn image(&self, i: u64) -> Option<MeshImage> {
        let img = self.get("images", i).ok()?;
        let bytes = match (
            img.get("bufferView").and_then(Value::as_u64),
            img.get("uri"),
        ) {
            (Some(v), _) => self.view_bytes(v)?.to_vec(),
            (None, Some(Value::String(uri))) if uri.starts_with("data:") => {
                base64(uri.split_once(',')?.1)?
            }
            _ => return None,
        };
        let kind = prism_doc::Image::read(bytes.clone()).ok()?.kind;
        Some(MeshImage {
            kind,
            bytes: std::sync::Arc::new(bytes),
            size: None,
        })
    }

    /// The file's materials, in order (so a primitive's index names one).
    fn materials(&self) -> Vec<MeshMaterial> {
        let Some(list) = self.doc.get("materials").and_then(Value::as_array) else {
            return Vec::new();
        };
        list.iter()
            .enumerate()
            .map(|(k, x)| {
                let pbr = x.get("pbrMetallicRoughness");
                let num = |key: &str, d: f64| {
                    pbr.and_then(|p| p.get(key))
                        .and_then(Value::as_f64)
                        .unwrap_or(d) as f32
                };
                let mut base = [1.0f32; 4];
                if let Some(c) = pbr
                    .and_then(|p| p.get("baseColorFactor"))
                    .and_then(Value::as_array)
                {
                    for (b, v) in base.iter_mut().zip(c) {
                        *b = v.as_f64().unwrap_or(1.0).clamp(0.0, 1.0) as f32;
                    }
                }
                let texture = pbr
                    .and_then(|p| p.get("baseColorTexture"))
                    .and_then(|t| t.get("index"))
                    .and_then(Value::as_u64)
                    .and_then(|t| self.get("textures", t).ok())
                    .and_then(|t| t.get("source"))
                    .and_then(Value::as_u64)
                    .and_then(|i| self.image(i))
                    .map(|mut i| {
                        i.size = x
                            .get("extras")
                            .and_then(|e| e.get("prism_texture_size_m"))
                            .and_then(Value::as_array)
                            .and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]))
                            .filter(|s| s[0] > 0.0 && s[1] > 0.0);
                        i
                    });
                MeshMaterial {
                    name: x
                        .get("name")
                        .and_then(Value::as_str)
                        .map_or_else(|| format!("Material {}", k + 1), str::to_owned),
                    base,
                    // glTF's defaults are fully rough and metal.
                    roughness: num("roughnessFactor", 1.0).clamp(0.0, 1.0),
                    metalness: num("metallicFactor", 1.0).clamp(0.0, 1.0),
                    texture,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_reads() {
        assert_eq!(super::base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(super::base64("AAEC").unwrap(), [0, 1, 2]);
    }
}
