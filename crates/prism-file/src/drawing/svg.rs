//! SVG out (brief 45, part 6a): one `<path>` per joined chain, lines as
//! lines, arcs as arcs, splines as quadratic or cubic curves. The page is
//! sized in mm, cm or inches with a viewBox, so laser programs read the
//! true size. SVG's y runs down the page, so y is turned over.

use std::f64::consts::{FRAC_PI_2, TAU};
use std::fmt::Write as _;

use prism_geom::Nurbs;
use prism_math::{DAffine2, DVec2, DVec3, LengthUnit};

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

// ---------- Reading (part 6b) ----------

/// An SVG file's drawing: its paths, lines, polylines, polygons,
/// rectangles, circles and ellipses, through their groups' transforms, y
/// turned up. Its unit is from the page's size and viewBox (96 px to the
/// inch otherwise); text and images are left out.
pub fn read(text: &str) -> Result<super::Read, String> {
    use quick_xml::events::Event;
    let mut r = quick_xml::Reader::from_reader(text.as_bytes());
    let mut buf = Vec::new();
    // Each open element's transform, and whether it is skipped.
    let mut stack: Vec<(DAffine2, bool)> = Vec::new();
    let mut paths = Vec::new();
    let mut skipped = Vec::new();
    let mut stated = None;
    loop {
        let (e, empty) = match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => (e.into_owned(), false),
            Ok(Event::Empty(e)) => (e.into_owned(), true),
            Ok(Event::End(_)) => {
                stack.pop();
                continue;
            }
            Ok(Event::Eof) => break,
            Ok(_) => continue,
            Err(err) => return Err(format!("Not an SVG file: {err}")),
        };
        let name = e.local_name().as_ref().to_owned();
        let get = |k: &str| -> Option<String> {
            e.attributes()
                .flatten()
                .find(|a| a.key.local_name().as_ref() == k)
                .map(|a| a.value.to_string())
        };
        let num = |k: &str| get(k).and_then(|v| length(&v)).map_or(0.0, |(x, _)| x);
        let (parent, skip) = stack.last().copied().unwrap_or((DAffine2::IDENTITY, false));
        let mut at = parent;
        let mut skip_inside = skip;
        if stack.is_empty() && name == "svg" {
            // The page: y turned up, and its unit.
            let mm = |k: &str| get(k).and_then(|v| length(&v)).map(|(x, u)| x * u);
            let vb: Vec<f64> = get("viewBox").map(|v| numbers(&v)).unwrap_or_default();
            let px = 25.4 / 96.0;
            let per_unit = match (mm("width"), vb.get(2)) {
                (Some(w), Some(vw)) if *vw > 0.0 => w / vw,
                _ => px,
            };
            stated = Some(per_unit / 1000.0);
            at = DAffine2::from_scale(DVec2::new(1.0, -1.0));
        } else if !skip {
            if let Some(t) = get("transform") {
                at = parent * transform(&t);
            }
            match name.as_str() {
                "defs" | "symbol" | "clipPath" | "mask" | "pattern" | "marker" | "style"
                | "title" | "desc" | "metadata" => skip_inside = true,
                "text" => {
                    skipped.push("text".to_owned());
                    skip_inside = true;
                }
                "image" => skipped.push("images".to_owned()),
                "use" => skipped.push("copies (use)".to_owned()),
                "svg" => {
                    at *= DAffine2::from_translation(DVec2::new(num("x"), num("y")));
                }
                "path" => {
                    for p in path_data(&get("d").unwrap_or_default()) {
                        paths.push(p.mapped(&at));
                    }
                }
                "line" => paths.push(
                    Path {
                        segs: vec![Seg::Line(
                            DVec2::new(num("x1"), num("y1")),
                            DVec2::new(num("x2"), num("y2")),
                        )],
                        closed: false,
                    }
                    .mapped(&at),
                ),
                "polyline" | "polygon" => {
                    let v = numbers(&get("points").unwrap_or_default());
                    let pts: Vec<DVec2> = v
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|c| DVec2::new(c[0], c[1]))
                        .collect();
                    let closed = name == "polygon";
                    let mut segs: Vec<Seg> =
                        pts.windows(2).map(|w| Seg::Line(w[0], w[1])).collect();
                    if closed && pts.len() > 2 {
                        segs.push(Seg::Line(pts[pts.len() - 1], pts[0]));
                    }
                    if !segs.is_empty() {
                        paths.push(Path { segs, closed }.mapped(&at));
                    }
                }
                "rect" => {
                    let (x, y, w, h) = (num("x"), num("y"), num("width"), num("height"));
                    let (mut rx, mut ry) = (get("rx"), get("ry"));
                    if rx.is_none() {
                        rx.clone_from(&ry);
                    }
                    if ry.is_none() {
                        ry.clone_from(&rx);
                    }
                    let rx = rx
                        .and_then(|v| length(&v))
                        .map_or(0.0, |v| v.0)
                        .min(w / 2.0);
                    let ry = ry
                        .and_then(|v| length(&v))
                        .map_or(0.0, |v| v.0)
                        .min(h / 2.0);
                    if w > 0.0 && h > 0.0 {
                        paths.push(rect(x, y, w, h, rx, ry).mapped(&at));
                    }
                }
                "circle" | "ellipse" => {
                    let c = DVec2::new(num("cx"), num("cy"));
                    let (rx, ry) = if name == "circle" {
                        (num("r"), num("r"))
                    } else {
                        (num("rx"), num("ry"))
                    };
                    if rx > 0.0 && ry > 0.0 {
                        paths.push(
                            Path {
                                segs: vec![Seg::Conic {
                                    c,
                                    u: DVec2::new(rx, 0.0),
                                    v: DVec2::new(0.0, ry),
                                    sweep: TAU,
                                }],
                                closed: true,
                            }
                            .mapped(&at),
                        );
                    }
                }
                _ => {}
            }
        }
        if !empty {
            stack.push((at, skip_inside));
        }
    }
    if stated.is_none() {
        return Err("Not an SVG file: it has no <svg> page.".to_owned());
    }
    Ok(super::Read {
        paths,
        points: Vec::new(),
        stated,
        skipped,
    })
}

/// A length and the millimetres in its unit (px for none).
fn length(s: &str) -> Option<(f64, f64)> {
    let s = s.trim();
    let end = s
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(s.len());
    let x: f64 = s[..end].trim().parse().ok()?;
    let mm = match &s[end..] {
        "" | "px" => 25.4 / 96.0,
        "mm" => 1.0,
        "cm" => 10.0,
        "in" => 25.4,
        "pt" => 25.4 / 72.0,
        "pc" => 25.4 / 6.0,
        _ => return None,
    };
    Some((x, mm))
}

/// The numbers in a list ("1,2 3-4.5.5e1").
fn numbers(s: &str) -> Vec<f64> {
    let mut t = Tokens::new(s);
    let mut out = Vec::new();
    while let Some(x) = t.number() {
        out.push(x);
    }
    out
}

/// A `transform` attribute as one map.
fn transform(s: &str) -> DAffine2 {
    let mut m = DAffine2::IDENTITY;
    for part in s.split(')') {
        let Some((name, args)) = part.split_once('(') else {
            continue;
        };
        let a = numbers(args);
        let g = |i: usize, d: f64| a.get(i).copied().unwrap_or(d);
        let t = match name.trim().trim_start_matches(',').trim() {
            "matrix" if a.len() == 6 => {
                DAffine2::from_cols_array(&[a[0], a[1], a[2], a[3], a[4], a[5]])
            }
            "translate" => DAffine2::from_translation(DVec2::new(g(0, 0.0), g(1, 0.0))),
            "scale" => DAffine2::from_scale(DVec2::new(g(0, 1.0), g(1, g(0, 1.0)))),
            "rotate" => {
                let c = DVec2::new(g(1, 0.0), g(2, 0.0));
                DAffine2::from_translation(c)
                    * DAffine2::from_angle(g(0, 0.0).to_radians())
                    * DAffine2::from_translation(-c)
            }
            "skewX" => {
                DAffine2::from_cols_array(&[1.0, 0.0, g(0, 0.0).to_radians().tan(), 1.0, 0.0, 0.0])
            }
            "skewY" => {
                DAffine2::from_cols_array(&[1.0, g(0, 0.0).to_radians().tan(), 0.0, 1.0, 0.0, 0.0])
            }
            _ => DAffine2::IDENTITY,
        };
        m *= t;
    }
    m
}

/// A rectangle, its corners rounded by `rx` and `ry`.
fn rect(x: f64, y: f64, w: f64, h: f64, rx: f64, ry: f64) -> Path {
    let p = |a: f64, b: f64| DVec2::new(a, b);
    if rx <= 0.0 || ry <= 0.0 {
        let c = [p(x, y), p(x + w, y), p(x + w, y + h), p(x, y + h)];
        return Path {
            segs: (0..4).map(|i| Seg::Line(c[i], c[(i + 1) % 4])).collect(),
            closed: true,
        };
    }
    // Clockwise on the page (y down): along the top, round each corner.
    let corner = |c: DVec2, from: DVec2| Seg::Conic {
        c,
        u: from,
        v: from.perp() * if from.x != 0.0 { ry / rx } else { rx / ry },
        sweep: FRAC_PI_2,
    };
    let mut segs = Vec::new();
    let line = |a: DVec2, b: DVec2, segs: &mut Vec<Seg>| {
        if a.distance(b) > 0.0 {
            segs.push(Seg::Line(a, b));
        }
    };
    line(p(x + rx, y), p(x + w - rx, y), &mut segs);
    segs.push(corner(p(x + w - rx, y + ry), p(0.0, -ry)));
    line(p(x + w, y + ry), p(x + w, y + h - ry), &mut segs);
    segs.push(corner(p(x + w - rx, y + h - ry), p(rx, 0.0)));
    line(p(x + w - rx, y + h), p(x + rx, y + h), &mut segs);
    segs.push(corner(p(x + rx, y + h - ry), p(0.0, ry)));
    line(p(x, y + h - ry), p(x, y + ry), &mut segs);
    segs.push(corner(p(x + rx, y + ry), p(-rx, 0.0)));
    Path { segs, closed: true }
}

/// Numbers and commands in path data.
struct Tokens<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Tokens<'a> {
    fn new(s: &'a str) -> Self {
        Tokens {
            s: s.as_bytes(),
            i: 0,
        }
    }

    fn space(&mut self) {
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_whitespace() || self.s[self.i] == b',')
        {
            self.i += 1;
        }
    }

    /// The next command letter, if one is next.
    fn command(&mut self) -> Option<u8> {
        self.space();
        let c = *self.s.get(self.i)?;
        (c.is_ascii_alphabetic() && c != b'e' && c != b'E').then(|| {
            self.i += 1;
            c
        })
    }

    fn number(&mut self) -> Option<f64> {
        self.space();
        let start = self.i;
        let s = self.s;
        let mut i = self.i;
        if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
            i += 1;
        }
        let mut dot = false;
        while i < s.len() && (s[i].is_ascii_digit() || (s[i] == b'.' && !dot)) {
            dot |= s[i] == b'.';
            i += 1;
        }
        if i < s.len() && (s[i] == b'e' || s[i] == b'E') {
            let mut j = i + 1;
            if j < s.len() && (s[j] == b'+' || s[j] == b'-') {
                j += 1;
            }
            if j < s.len() && s[j].is_ascii_digit() {
                i = j;
                while i < s.len() && s[i].is_ascii_digit() {
                    i += 1;
                }
            }
        }
        let x = std::str::from_utf8(&s[start..i]).ok()?.parse().ok()?;
        self.i = i;
        Some(x)
    }

    /// An arc's flag: one "0" or "1", which may touch what follows.
    fn flag(&mut self) -> Option<bool> {
        self.space();
        let c = *self.s.get(self.i)?;
        if c == b'0' || c == b'1' {
            self.i += 1;
            Some(c == b'1')
        } else {
            None
        }
    }
}

/// Path data as paths: each subpath (from an M) is one.
fn path_data(d: &str) -> Vec<Path> {
    let mut t = Tokens::new(d);
    let mut out: Vec<Path> = Vec::new();
    let mut segs: Vec<Seg> = Vec::new();
    let (mut at, mut start) = (DVec2::ZERO, DVec2::ZERO);
    // The last control point, for S and T.
    let mut last_c: Option<DVec2> = None;
    let mut last_q: Option<DVec2> = None;
    let mut cmd = b'M';
    let flush = |segs: &mut Vec<Seg>, out: &mut Vec<Path>, closed: bool| {
        if !segs.is_empty() {
            out.push(Path {
                segs: std::mem::take(segs),
                closed,
            });
        }
    };
    loop {
        if let Some(c) = t.command() {
            cmd = c;
        } else if t.i >= t.s.len() {
            break;
        }
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { at } else { DVec2::ZERO };
        let up = cmd.to_ascii_uppercase();
        if up == b'Z' {
            if at.distance(start) > 0.0 {
                segs.push(Seg::Line(at, start));
            }
            flush(&mut segs, &mut out, true);
            at = start;
            last_c = None;
            last_q = None;
            // Numbers after Z (no command) start a new subpath.
            cmd = if rel { b'm' } else { b'M' };
            continue;
        }
        let pt = |t: &mut Tokens| -> Option<DVec2> {
            let x = t.number()?;
            let y = t.number()?;
            Some(base + DVec2::new(x, y))
        };
        let before = t.i;
        match up {
            b'M' => {
                let Some(p) = pt(&mut t) else { break };
                flush(&mut segs, &mut out, false);
                at = p;
                start = p;
                // Pairs after a moveto are lines.
                cmd = if rel { b'l' } else { b'L' };
                last_c = None;
                last_q = None;
            }
            b'L' => {
                let Some(p) = pt(&mut t) else { break };
                segs.push(Seg::Line(at, p));
                at = p;
                last_c = None;
                last_q = None;
            }
            b'H' | b'V' => {
                let Some(x) = t.number() else { break };
                let p = if up == b'H' {
                    DVec2::new(if rel { at.x + x } else { x }, at.y)
                } else {
                    DVec2::new(at.x, if rel { at.y + x } else { x })
                };
                segs.push(Seg::Line(at, p));
                at = p;
                last_c = None;
                last_q = None;
            }
            b'C' | b'S' => {
                let c1 = if up == b'C' {
                    let Some(c) = pt(&mut t) else { break };
                    c
                } else {
                    last_c.map_or(at, |c| at * 2.0 - c)
                };
                let (Some(c2), Some(p)) = (pt(&mut t), pt(&mut t)) else {
                    break;
                };
                segs.push(bezier_seg(&[at, c1, c2, p]));
                at = p;
                last_c = Some(c2);
                last_q = None;
            }
            b'Q' | b'T' => {
                let c = if up == b'Q' {
                    let Some(c) = pt(&mut t) else { break };
                    c
                } else {
                    last_q.map_or(at, |c| at * 2.0 - c)
                };
                let Some(p) = pt(&mut t) else { break };
                segs.push(bezier_seg(&[at, c, p]));
                at = p;
                last_q = Some(c);
                last_c = None;
            }
            b'A' => {
                let (Some(rx), Some(ry), Some(rot)) = (t.number(), t.number(), t.number()) else {
                    break;
                };
                let (Some(large), Some(sweep)) = (t.flag(), t.flag()) else {
                    break;
                };
                let Some(p) = pt(&mut t) else { break };
                segs.extend(arc(at, p, rx, ry, rot, large, sweep));
                at = p;
                last_c = None;
                last_q = None;
            }
            _ => break,
        }
        if t.i == before {
            break;
        }
    }
    flush(&mut segs, &mut out, false);
    out
}

/// A Bézier piece as a spline (2 to 4 control points).
fn bezier_seg(p: &[DVec2]) -> Seg {
    let pts: Vec<DVec3> = p.iter().map(|q| q.extend(0.0)).collect();
    let n = pts.len();
    let mut knots = vec![0.0; n];
    knots.extend(vec![1.0; n]);
    match Nurbs::new(n - 1, knots, pts, vec![1.0; n], false) {
        Some(c) => Seg::Spline(c),
        None => Seg::Line(p[0], p[n - 1]),
    }
}

/// An SVG arc (from its ends, radii, turn and flags) as a conic (SVG
/// 1.1, F.6.5 and F.6.6).
fn arc(a: DVec2, b: DVec2, rx: f64, ry: f64, rot: f64, large: bool, sweep: bool) -> Vec<Seg> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if a.distance(b) == 0.0 {
        return Vec::new();
    }
    if rx == 0.0 || ry == 0.0 {
        return vec![Seg::Line(a, b)];
    }
    let phi = rot.to_radians();
    let (s, c) = phi.sin_cos();
    let h = (a - b) * 0.5;
    let p = DVec2::new(c * h.x + s * h.y, -s * h.x + c * h.y);
    let l = (p.x * p.x) / (rx * rx) + (p.y * p.y) / (ry * ry);
    if l > 1.0 {
        rx *= l.sqrt();
        ry *= l.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * p.y * p.y - ry * ry * p.x * p.x).max(0.0);
    let den = rx * rx * p.y * p.y + ry * ry * p.x * p.x;
    let mut k = (num / den).sqrt();
    if large == sweep {
        k = -k;
    }
    let cp = DVec2::new(k * rx * p.y / ry, -k * ry * p.x / rx);
    let centre = DVec2::new(c * cp.x - s * cp.y, s * cp.x + c * cp.y) + (a + b) * 0.5;
    let angle = |u: DVec2, v: DVec2| u.angle_to(v);
    let u0 = DVec2::new((p.x - cp.x) / rx, (p.y - cp.y) / ry);
    let u1 = DVec2::new((-p.x - cp.x) / rx, (-p.y - cp.y) / ry);
    let th1 = angle(DVec2::X, u0);
    let mut dth = angle(u0, u1);
    if !sweep && dth > 0.0 {
        dth -= TAU;
    } else if sweep && dth < 0.0 {
        dth += TAU;
    }
    // The ellipse's axes, then started at th1 and walked its way.
    let ax = DVec2::new(c, s) * rx;
    let ay = DVec2::new(-s, c) * ry;
    let sign = dth.signum();
    vec![Seg::Conic {
        c: centre,
        u: ax * th1.cos() + ay * th1.sin(),
        v: (-ax * th1.sin() + ay * th1.cos()) * sign,
        sweep: dth.abs(),
    }]
}

impl Path {
    fn mapped(&self, m: &DAffine2) -> Path {
        Path {
            segs: self.segs.iter().map(|s| s.mapped(m)).collect(),
            closed: self.closed,
        }
    }
}
