//! OBJ: vertices and faces (polygons), no materials.

use std::fmt::Write;

use prism_math::DVec3;

use super::{Mesh, damaged};
use crate::FileError;

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
                m.positions.push(DVec3::from_array(c));
            }
            Some("f") => {
                let count = m.positions.len() as i64;
                let mut poly = Vec::new();
                for t in w {
                    let i: i64 = t
                        .split('/')
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
                    poly.push(i as u32);
                }
                if poly.len() >= 3 {
                    m.polygons.push(poly);
                }
            }
            _ => {}
        }
    }
    Ok(m)
}
