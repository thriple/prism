//! PLY: ASCII and binary little-endian to read, binary little-endian to write.

use std::fmt::Write;

use prism_math::DVec3;

use super::{Mesh, damaged};
use crate::FileError;

pub fn write(m: &Mesh) -> Vec<u8> {
    let mut h = String::from("ply\nformat binary_little_endian 1.0\ncomment Prism\n");
    let _ = writeln!(h, "element vertex {}", m.positions.len());
    h.push_str("property double x\nproperty double y\nproperty double z\n");
    let _ = writeln!(h, "element face {}", m.polygons.len());
    h.push_str("property list uchar int vertex_indices\nend_header\n");
    let mut out = h.into_bytes();
    for p in &m.positions {
        for c in p.to_array() {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    for f in &m.polygons {
        // A face with more than 255 corners is written as a fan.
        if f.len() <= 255 {
            out.push(f.len() as u8);
            for i in f {
                out.extend_from_slice(&(*i as i32).to_le_bytes());
            }
        } else {
            out.push(3);
            for i in [f[0], f[1], f[2]] {
                out.extend_from_slice(&(i as i32).to_le_bytes());
            }
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Ty {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Ty {
    fn parse(s: &str) -> Option<Ty> {
        Some(match s {
            "char" | "int8" => Ty::I8,
            "uchar" | "uint8" => Ty::U8,
            "short" | "int16" => Ty::I16,
            "ushort" | "uint16" => Ty::U16,
            "int" | "int32" => Ty::I32,
            "uint" | "uint32" => Ty::U32,
            "float" | "float32" => Ty::F32,
            "double" | "float64" => Ty::F64,
            _ => return None,
        })
    }

    fn size(self) -> usize {
        match self {
            Ty::I8 | Ty::U8 => 1,
            Ty::I16 | Ty::U16 => 2,
            Ty::I32 | Ty::U32 | Ty::F32 => 4,
            Ty::F64 => 8,
        }
    }
}

enum Prop {
    One(String, Ty),
    List(String, Ty, Ty),
}

struct Element {
    name: String,
    count: usize,
    props: Vec<Prop>,
}

/// Values from the body, in order, whichever encoding it uses.
struct Values<'a> {
    binary: bool,
    bytes: &'a [u8],
    at: usize,
    words: std::str::SplitAsciiWhitespace<'a>,
}

impl Values<'_> {
    fn next(&mut self, t: Ty) -> Result<f64, FileError> {
        if !self.binary {
            return self
                .words
                .next()
                .and_then(|w| w.parse::<f64>().ok())
                .ok_or_else(|| damaged("the PLY data ends early"));
        }
        let n = t.size();
        let b = self
            .bytes
            .get(self.at..self.at + n)
            .ok_or_else(|| damaged("the PLY data ends early"))?;
        self.at += n;
        Ok(match t {
            Ty::I8 => f64::from(b[0] as i8),
            Ty::U8 => f64::from(b[0]),
            Ty::I16 => f64::from(i16::from_le_bytes([b[0], b[1]])),
            Ty::U16 => f64::from(u16::from_le_bytes([b[0], b[1]])),
            Ty::I32 => f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            Ty::U32 => f64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            Ty::F32 => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            Ty::F64 => f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
        })
    }
}

pub fn read(b: &[u8]) -> Result<Mesh, FileError> {
    let end = b
        .windows(10)
        .position(|w| w == b"end_header")
        .ok_or_else(|| damaged("not a PLY file"))?;
    let header = std::str::from_utf8(&b[..end]).map_err(|_| damaged("not a PLY file"))?;
    let mut body_at = end + 10;
    while b.get(body_at).is_some_and(|c| *c == b'\r' || *c == b'\n') {
        body_at += 1;
        if b.get(body_at - 1) == Some(&b'\n') {
            break;
        }
    }
    let mut lines = header.lines();
    if lines.next().map(str::trim) != Some("ply") {
        return Err(damaged("not a PLY file"));
    }
    let mut binary = false;
    let mut elements: Vec<Element> = Vec::new();
    for line in lines {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.as_slice() {
            ["format", "ascii", _] => binary = false,
            ["format", "binary_little_endian", _] => binary = true,
            ["format", f, _] => return Err(damaged(format!("PLY format {f} is not supported"))),
            ["element", name, n] => elements.push(Element {
                name: (*name).to_owned(),
                count: n.parse().map_err(|_| damaged("a bad element count"))?,
                props: Vec::new(),
            }),
            ["property", "list", c, t, name] => {
                let (c, t) = Ty::parse(c)
                    .zip(Ty::parse(t))
                    .ok_or_else(|| damaged("a bad PLY property type"))?;
                elements
                    .last_mut()
                    .ok_or_else(|| damaged("a property outside an element"))?
                    .props
                    .push(Prop::List((*name).to_owned(), c, t));
            }
            ["property", t, name] => {
                let t = Ty::parse(t).ok_or_else(|| damaged("a bad PLY property type"))?;
                elements
                    .last_mut()
                    .ok_or_else(|| damaged("a property outside an element"))?
                    .props
                    .push(Prop::One((*name).to_owned(), t));
            }
            _ => {}
        }
    }
    let body = &b[body_at.min(b.len())..];
    let text = if binary {
        ""
    } else {
        std::str::from_utf8(body).map_err(|_| damaged("the PLY data is not text"))?
    };
    let mut v = Values {
        binary,
        bytes: body,
        at: 0,
        words: text.split_ascii_whitespace(),
    };
    let mut m = Mesh::default();
    for e in &elements {
        // A count that cannot fit in the file is a damaged header.
        let min_row = if binary { e.props.len().max(1) } else { 1 };
        if e.count.saturating_mul(min_row) > body.len() {
            return Err(damaged("the PLY data ends early"));
        }
        for _ in 0..e.count {
            let mut p = [0.0; 3];
            let mut poly = Vec::new();
            for prop in &e.props {
                match prop {
                    Prop::One(name, t) => {
                        let x = v.next(*t)?;
                        match (e.name.as_str(), name.as_str()) {
                            ("vertex", "x") => p[0] = x,
                            ("vertex", "y") => p[1] = x,
                            ("vertex", "z") => p[2] = x,
                            _ => {}
                        }
                    }
                    Prop::List(name, c, t) => {
                        let n = v.next(*c)?;
                        if !(0.0..=65_536.0).contains(&n) {
                            return Err(damaged("a bad PLY list length"));
                        }
                        for _ in 0..n as usize {
                            let i = v.next(*t)?;
                            if e.name == "face" && name.starts_with("vertex_ind") {
                                if i < 0.0 {
                                    return Err(damaged("a negative vertex index"));
                                }
                                poly.push(i as u32);
                            }
                        }
                    }
                }
            }
            match e.name.as_str() {
                "vertex" => m.positions.push(DVec3::from_array(p)),
                "face" if poly.len() >= 3 => m.polygons.push(poly),
                _ => {}
            }
        }
    }
    Ok(m)
}
