//! SVG out (brief 45, part 6a): one `<path>` per joined chain, lines as
//! lines, arcs as arcs, splines as quadratic or cubic curves. The page is
//! sized in mm, cm or inches with a viewBox, so laser programs read the
//! true size. SVG's y runs down the page, so y is turned over.

use std::f64::consts::FRAC_PI_2;
use std::fmt::Write as _;

use prism_geom::Nurbs;
use prism_math::{DVec2, DVec3, LengthUnit};

use super::{Drawing, Path, Seg, bounds, num};

/// Said for a spline that SVG's curves can only follow closely.
pub const CLOSE: &str = "a spline written as close cubic curves (SVG has no NURBS)";

/// The text of `d` as an SVG file, and what was only followed closely.
pub fn write(d: &Drawing) -> (String, Vec<&'static str>) {
    let mut notes = Vec::new();
    let all: Vec<Path> = d.parts.iter().flat_map(|p| p.paths.clone()).collect();
    let (lo, hi) = bounds(&all).unwrap_or((DVec2::ZERO, DVec2::ONE));
    let size = (hi - lo).max(DVec2::splat(d.eps.max(1e-9)));
    // The page's own unit: CSS has no metres, so metres are shown in mm.
    let (css, per) = match d.unit {
        LengthUnit::Millimetres => ("mm", 1.0),
        LengthUnit::Centimetres => ("cm", 1.0),
        LengthUnit::Metres => ("mm", 1000.0),
        LengthUnit::Inches | LengthUnit::FeetInches => ("in", 1.0),
    };
    // A hairline, a tenth of a millimetre.
    let stroke = 0.1e-3 / super::metres(d.unit);
    let mut out = String::new();
    let _ = writeln!(out, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}{css}" height="{}{css}" viewBox="{} {} {} {}">"#,
        num(size.x * per),
        num(size.y * per),
        num(lo.x),
        num(-hi.y),
        num(size.x),
        num(size.y),
    );
    let mut ids: Vec<String> = Vec::new();
    for part in &d.parts {
        let mut id = ident(&part.name);
        let base = id.clone();
        let mut n = 1;
        while ids.contains(&id) {
            n += 1;
            id = format!("{base}-{n}");
        }
        ids.push(id.clone());
        let _ = writeln!(
            out,
            r##"  <g id="{id}" fill="none" stroke="#000" stroke-width="{}">"##,
            num(stroke)
        );
        for p in &part.paths {
            let _ = writeln!(out, r#"    <path d="{}"/>"#, path(p, d.eps, &mut notes));
        }
        let _ = writeln!(out, "  </g>");
    }
    let _ = writeln!(out, "</svg>");
    (out, notes)
}

/// A name as an XML id: letters, digits, "-" and "_", starting with a
/// letter.
fn ident(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.starts_with(|c: char| c.is_ascii_alphabetic()) {
        s
    } else {
        format!("p_{s}")
    }
}

/// A point on the page: y turned over.
fn pt(p: DVec2) -> String {
    format!("{} {}", num(p.x), num(-p.y))
}

fn path(p: &Path, eps: f64, notes: &mut Vec<&'static str>) -> String {
    let mut d = String::new();
    let mut at: Option<DVec2> = None;
    for s in &p.segs {
        let start = s.start();
        if at.is_none_or(|a| a.distance(start) > eps) {
            let _ = write!(d, "M{} ", pt(start));
        }
        seg(s, &mut d, eps, notes);
        at = Some(s.end());
    }
    if p.closed {
        d.push('Z');
    }
    d.trim_end().to_owned()
}

fn seg(s: &Seg, d: &mut String, eps: f64, notes: &mut Vec<&'static str>) {
    match s {
        Seg::Line(_, b) => {
            let _ = write!(d, "L{} ", pt(*b));
        }
        Seg::Conic { c, u, v, sweep } => {
            let Some((_, major, ratio, _, _)) = s.ellipse() else {
                return;
            };
            let (rx, ry) = (major.length(), major.length() * ratio);
            let (rx, ry, rot) = match s.circle(eps) {
                Some((_, r, _, _)) => (r, r, 0.0),
                None => (rx, ry, -major.y.atan2(major.x).to_degrees()),
            };
            // Counter-clockwise on the sheet is clockwise on the page.
            let flag = if u.perp_dot(*v) > 0.0 { 0 } else { 1 };
            // In pieces of at most a quarter turn: no flag is ever large,
            // and the centre is well fixed by the ends (at half a turn it
            // swings on the last digit of the numbers).
            let n = (sweep / FRAC_PI_2 - 1e-9).ceil().max(1.0);
            for i in 1..=(n as usize) {
                let t = sweep * i as f64 / n;
                let q = *c + *u * t.cos() + *v * t.sin();
                let _ = write!(
                    d,
                    "A{} {} {} 0 {flag} {} ",
                    num(rx),
                    num(ry),
                    num(rot),
                    pt(q)
                );
            }
        }
        Seg::Spline(n) => spline(n, d, eps, notes),
    }
}

/// A spline as SVG curves: exactly as lines, quadratic or cubic pieces
/// when it is not rational and of degree 3 or less; else cubic pieces
/// within `eps`.
fn spline(n: &Nurbs, d: &mut String, eps: f64, notes: &mut Vec<&'static str>) {
    let w0 = n.weights().first().copied().unwrap_or(1.0);
    let rational = n
        .weights()
        .iter()
        .any(|w| (w - w0).abs() > 1e-12 * w0.abs());
    if !rational && let Some(pieces) = bezier(n) {
        for b in pieces {
            let _ = match b.len() {
                2 => write!(d, "L{} ", pt(b[1].truncate())),
                3 => write!(d, "Q{} {} ", pt(b[1].truncate()), pt(b[2].truncate())),
                _ => write!(
                    d,
                    "C{} {} {} ",
                    pt(b[1].truncate()),
                    pt(b[2].truncate()),
                    pt(b[3].truncate())
                ),
            };
        }
        return;
    }
    notes.push(CLOSE);
    let (t0, t1) = n.domain();
    let mut stack = vec![(t0, t1, 0)];
    // Depth first, left to right.
    while let Some((a, b, depth)) = stack.pop() {
        let (pa, da) = n.eval_d(a);
        let (pb, db) = n.eval_d(b);
        let h = (b - a) / 3.0;
        let (c1, c2) = (pa + da * h, pb - db * h);
        let cubic = |s: f64| {
            let r = 1.0 - s;
            pa * r * r * r + c1 * 3.0 * r * r * s + c2 * 3.0 * r * s * s + pb * s * s * s
        };
        let off = (1..8)
            .map(|i| {
                let s = f64::from(i) / 8.0;
                cubic(s).distance(n.eval(a + (b - a) * s))
            })
            .fold(0.0, f64::max);
        if off > eps && depth < 16 {
            let m = 0.5 * (a + b);
            stack.push((m, b, depth + 1));
            stack.push((a, m, depth + 1));
            continue;
        }
        let _ = write!(
            d,
            "C{} {} {} ",
            pt(c1.truncate()),
            pt(c2.truncate()),
            pt(pb.truncate())
        );
    }
}

/// An open clamped spline of degree 1 to 3 as Bézier pieces (each its
/// control points), by knot insertion. None for another kind.
fn bezier(n: &Nurbs) -> Option<Vec<Vec<DVec3>>> {
    let p = n.degree();
    if !(1..=3).contains(&p) || n.is_periodic() {
        return None;
    }
    let ks = n.knots();
    let (lo, hi) = (ks[0], ks[ks.len() - 1]);
    if ks[..=p].iter().any(|k| *k != lo) || ks[ks.len() - 1 - p..].iter().any(|k| *k != hi) {
        return None;
    }
    let mut inner: Vec<f64> = ks.iter().copied().filter(|k| *k > lo && *k < hi).collect();
    inner.dedup();
    let mut c = n.clone();
    for t in inner {
        while c.knots().iter().filter(|k| **k == t).count() < p {
            c = c.insert_knot(t)?;
        }
    }
    let pts = c.points();
    if !(pts.len() - 1).is_multiple_of(p) {
        return None;
    }
    Some(
        (0..(pts.len() - 1) / p)
            .map(|i| pts[i * p..=i * p + p].to_vec())
            .collect(),
    )
}
