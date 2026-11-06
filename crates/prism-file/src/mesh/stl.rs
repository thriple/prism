//! STL: binary or ASCII to read, binary to write.

use prism_math::DVec3;

use super::{Mesh, damaged};
use crate::FileError;

pub fn write(m: &Mesh) -> Vec<u8> {
    let tris: Vec<[u32; 3]> = m
        .polygons
        .iter()
        .flat_map(|p| (1..p.len().saturating_sub(1)).map(move |i| [p[0], p[i], p[i + 1]]))
        .collect();
    let mut out = Vec::with_capacity(84 + 50 * tris.len());
    let mut header = [b' '; 80];
    header[..17].copy_from_slice(b"Prism binary STL ");
    out.extend_from_slice(&header);
    out.extend_from_slice(&(tris.len() as u32).to_le_bytes());
    for t in tris {
        let p = t.map(|i| m.positions[i as usize]);
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
        for v in [n, p[0], p[1], p[2]] {
            for c in v.to_array() {
                out.extend_from_slice(&(c as f32).to_le_bytes());
            }
        }
        out.extend_from_slice(&[0, 0]);
    }
    out
}

pub fn read(b: &[u8]) -> Result<Mesh, FileError> {
    if b.len() >= 84 {
        let n = u32::from_le_bytes([b[80], b[81], b[82], b[83]]) as usize;
        if n.checked_mul(50).and_then(|x| x.checked_add(84)) == Some(b.len()) {
            return Ok(binary(&b[84..], n));
        }
    }
    let text = std::str::from_utf8(b).map_err(|_| damaged("not an STL file"))?;
    if !text.trim_start().starts_with("solid") {
        return Err(damaged("not an STL file"));
    }
    ascii(text)
}

fn binary(b: &[u8], n: usize) -> Mesh {
    let mut m = Mesh::default();
    let f = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as f64;
    for t in 0..n {
        let base = t * 50 + 12;
        let first = m.positions.len() as u32;
        for k in 0..3 {
            let o = base + k * 12;
            m.positions.push(DVec3::new(f(o), f(o + 4), f(o + 8)));
        }
        m.polygons.push(vec![first, first + 1, first + 2]);
    }
    m
}

fn ascii(text: &str) -> Result<Mesh, FileError> {
    let mut m = Mesh::default();
    let mut corners = Vec::new();
    for line in text.lines() {
        let mut w = line.split_whitespace();
        match w.next() {
            Some("vertex") => {
                let mut c = [0.0; 3];
                for x in &mut c {
                    *x = w
                        .next()
                        .and_then(|s| s.parse::<f64>().ok())
                        .ok_or_else(|| damaged("a vertex is not three numbers"))?;
                }
                m.positions.push(DVec3::from_array(c));
                corners.push((m.positions.len() - 1) as u32);
            }
            Some("endloop") => {
                if corners.len() >= 3 {
                    m.polygons.push(std::mem::take(&mut corners));
                } else {
                    corners.clear();
                }
            }
            _ => {}
        }
    }
    Ok(m)
}
