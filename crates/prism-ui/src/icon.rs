//! Stroked line icons, read from the small SVG snippets the prototype uses
//! (`<path>`, `<circle>`, `<rect>`, `<ellipse>`, with `rotate()` transforms and
//! dashes). Each icon is turned into polylines once, in a 24 by 24 box, and
//! drawn at any size.

use egui::{Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

use crate::theme::size;

/// One stroked line of an icon, in icon units (0 to 24).
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub points: Vec<Pos2>,
    pub closed: bool,
    /// Dash and gap lengths, if dashed.
    pub dash: Option<(f32, f32)>,
}

/// An icon: its lines.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Icon {
    pub lines: Vec<Line>,
}

const CURVE_STEPS: usize = 12;
const CIRCLE_STEPS: usize = 40;

impl Icon {
    /// Read an icon from SVG elements.
    pub fn parse(svg: &str) -> Result<Icon, String> {
        let mut icon = Icon::default();
        let mut rest = svg;
        while let Some(start) = rest.find('<') {
            let end = rest[start..].find('>').ok_or("unclosed element")? + start;
            let element = &rest[start + 1..end];
            rest = &rest[end + 1..];
            let element = element.trim_end_matches('/').trim();
            let (name, attrs) = element
                .split_once(char::is_whitespace)
                .unwrap_or((element, ""));
            let attr = |key: &str| attribute(attrs, key);
            let num = |key: &str| attr(key).and_then(|v| v.trim().parse::<f32>().ok());
            let dash = attr("stroke-dasharray").and_then(|d| {
                let v: Vec<f32> = numbers(&d);
                match v.as_slice() {
                    [a] => Some((*a, *a)),
                    [a, b, ..] => Some((*a, *b)),
                    _ => None,
                }
            });
            let mut lines: Vec<Line> = match name {
                "path" => path(&attr("d").ok_or("path without d")?)?,
                "circle" => {
                    let (cx, cy, r) = (
                        num("cx").unwrap_or(0.0),
                        num("cy").unwrap_or(0.0),
                        num("r").ok_or("circle without r")?,
                    );
                    vec![ellipse(pos2(cx, cy), r, r)]
                }
                "ellipse" => {
                    let c = pos2(num("cx").unwrap_or(0.0), num("cy").unwrap_or(0.0));
                    vec![ellipse(
                        c,
                        num("rx").ok_or("no rx")?,
                        num("ry").ok_or("no ry")?,
                    )]
                }
                "rect" => {
                    let r = Rect::from_min_size(
                        pos2(num("x").unwrap_or(0.0), num("y").unwrap_or(0.0)),
                        vec2(
                            num("width").ok_or("no width")?,
                            num("height").ok_or("no height")?,
                        ),
                    );
                    vec![rounded_rect(r, num("rx").unwrap_or(0.0))]
                }
                other => return Err(format!("unsupported element <{other}>")),
            };
            if let Some(t) = attr("transform") {
                let rotate = rotation(&t)?;
                for line in &mut lines {
                    for p in &mut line.points {
                        *p = rotate(*p);
                    }
                }
            }
            for mut line in lines {
                line.dash = dash;
                icon.lines.push(line);
            }
        }
        Ok(icon)
    }

    /// Draw the icon centred in `rect` (its larger side is the icon's size).
    pub fn paint(&self, painter: &Painter, rect: Rect, color: egui::Color32) {
        let scale = rect.width().min(rect.height()) / size::ICON_BOX;
        let origin = rect.center() - Vec2::splat(size::ICON_BOX * 0.5 * scale);
        let stroke = Stroke::new(size::ICON_STROKE * scale, color);
        for line in &self.lines {
            let mut pts: Vec<Pos2> = line
                .points
                .iter()
                .map(|p| origin + p.to_vec2() * scale)
                .collect();
            if line.closed && pts.len() > 2 {
                pts.push(pts[0]);
            }
            match line.dash {
                Some((dash, gap)) => {
                    painter.extend(Shape::dashed_line(&pts, stroke, dash * scale, gap * scale));
                }
                None => {
                    painter.add(Shape::line(pts, stroke));
                }
            }
        }
    }
}

/// The value of `key="…"` in an element's attributes.
fn attribute(attrs: &str, key: &str) -> Option<String> {
    let mut search = attrs;
    while let Some(i) = search.find(key) {
        let before_ok = i == 0 || search[..i].ends_with(char::is_whitespace);
        let after = &search[i + key.len()..];
        if before_ok && let Some(v) = after.strip_prefix("=\"") {
            return v.split('"').next().map(str::to_owned);
        }
        search = &search[i + key.len()..];
    }
    None
}

/// All numbers in a string, SVG style ("1.5.5" is 1.5 and .5; "2-3" is 2 and -3).
fn numbers(s: &str) -> Vec<f32> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<f32>| {
        if let Ok(v) = cur.parse() {
            out.push(v);
        }
        cur.clear();
    };
    for c in s.chars() {
        match c {
            '0'..='9' => cur.push(c),
            '.' => {
                if cur.contains('.') {
                    flush(&mut cur, &mut out);
                }
                cur.push(c);
            }
            '-' => {
                flush(&mut cur, &mut out);
                cur.push(c);
            }
            _ => flush(&mut cur, &mut out),
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// A `rotate(a cx cy)` transform as a function.
fn rotation(t: &str) -> Result<impl Fn(Pos2) -> Pos2, String> {
    let inner = t
        .trim()
        .strip_prefix("rotate(")
        .and_then(|r| r.strip_suffix(')'))
        .ok_or_else(|| format!("unsupported transform {t:?}"))?;
    let v = numbers(inner);
    let angle = v.first().copied().unwrap_or(0.0).to_radians();
    let c = pos2(
        v.get(1).copied().unwrap_or(0.0),
        v.get(2).copied().unwrap_or(0.0),
    );
    let (s, co) = angle.sin_cos();
    Ok(move |p: Pos2| {
        let d = p - c;
        c + vec2(d.x * co - d.y * s, d.x * s + d.y * co)
    })
}

fn ellipse(c: Pos2, rx: f32, ry: f32) -> Line {
    let points = (0..CIRCLE_STEPS)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / CIRCLE_STEPS as f32;
            c + vec2(rx * a.cos(), ry * a.sin())
        })
        .collect();
    Line {
        points,
        closed: true,
        dash: None,
    }
}

fn rounded_rect(r: Rect, rx: f32) -> Line {
    let rx = rx.min(r.width() * 0.5).min(r.height() * 0.5);
    if rx <= 0.0 {
        return Line {
            points: vec![
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
            ],
            closed: true,
            dash: None,
        };
    }
    let mut points = Vec::new();
    let corners = [
        (pos2(r.right() - rx, r.top() + rx), -90.0f32),
        (pos2(r.right() - rx, r.bottom() - rx), 0.0),
        (pos2(r.left() + rx, r.bottom() - rx), 90.0),
        (pos2(r.left() + rx, r.top() + rx), 180.0),
    ];
    for (c, start) in corners {
        for i in 0..=4 {
            let a = (start + 90.0 * i as f32 / 4.0).to_radians();
            points.push(c + vec2(a.cos(), a.sin()) * rx);
        }
    }
    Line {
        points,
        closed: true,
        dash: None,
    }
}

/// The lines of an SVG path's `d` attribute.
fn path(d: &str) -> Result<Vec<Line>, String> {
    // Split into commands with their numbers.
    let mut cmds: Vec<(char, Vec<f32>)> = Vec::new();
    let mut cur: Option<(char, String)> = None;
    for c in d.chars() {
        if c.is_ascii_alphabetic() && c != 'e' {
            if let Some((cmd, args)) = cur.take() {
                cmds.push((cmd, numbers(&args)));
            }
            cur = Some((c, String::new()));
        } else if let Some((_, args)) = &mut cur {
            args.push(c);
        }
    }
    if let Some((cmd, args)) = cur {
        cmds.push((cmd, numbers(&args)));
    }

    let mut lines = Vec::new();
    let mut pts: Vec<Pos2> = Vec::new();
    let mut pos = Pos2::ZERO;
    let mut start = Pos2::ZERO;
    let mut last_ctrl: Option<Pos2> = None;
    let finish = |pts: &mut Vec<Pos2>, lines: &mut Vec<Line>, closed: bool| {
        if pts.len() > 1 {
            lines.push(Line {
                points: std::mem::take(pts),
                closed,
                dash: None,
            });
        } else {
            pts.clear();
        }
    };

    for (cmd, args) in cmds {
        let rel = cmd.is_ascii_lowercase();
        let at = |pos: Pos2, x: f32, y: f32| if rel { pos + vec2(x, y) } else { pos2(x, y) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                finish(&mut pts, &mut lines, false);
                for (i, xy) in args.as_chunks::<2>().0.iter().enumerate() {
                    pos = at(pos, xy[0], xy[1]);
                    if i == 0 {
                        start = pos;
                    }
                    pts.push(pos);
                }
                last_ctrl = None;
            }
            'L' => {
                for xy in args.as_chunks::<2>().0 {
                    pos = at(pos, xy[0], xy[1]);
                    pts.push(pos);
                }
                last_ctrl = None;
            }
            'H' => {
                for &x in &args {
                    pos = pos2(if rel { pos.x + x } else { x }, pos.y);
                    pts.push(pos);
                }
                last_ctrl = None;
            }
            'V' => {
                for &y in &args {
                    pos = pos2(pos.x, if rel { pos.y + y } else { y });
                    pts.push(pos);
                }
                last_ctrl = None;
            }
            'C' | 'S' => {
                let n = if cmd.eq_ignore_ascii_case(&'C') { 6 } else { 4 };
                for a in args.chunks_exact(n) {
                    let (c1, c2, end) = if n == 6 {
                        (
                            at(pos, a[0], a[1]),
                            at(pos, a[2], a[3]),
                            at(pos, a[4], a[5]),
                        )
                    } else {
                        let c1 = last_ctrl.map_or(pos, |c| pos + (pos - c));
                        (c1, at(pos, a[0], a[1]), at(pos, a[2], a[3]))
                    };
                    if pts.is_empty() {
                        pts.push(pos);
                    }
                    for i in 1..=CURVE_STEPS {
                        let t = i as f32 / CURVE_STEPS as f32;
                        let u = 1.0 - t;
                        let p = pos.to_vec2() * u * u * u
                            + c1.to_vec2() * 3.0 * u * u * t
                            + c2.to_vec2() * 3.0 * u * t * t
                            + end.to_vec2() * t * t * t;
                        pts.push(p.to_pos2());
                    }
                    last_ctrl = Some(c2);
                    pos = end;
                }
            }
            'Q' => {
                for a in args.as_chunks::<4>().0 {
                    let (c, end) = (at(pos, a[0], a[1]), at(pos, a[2], a[3]));
                    if pts.is_empty() {
                        pts.push(pos);
                    }
                    for i in 1..=CURVE_STEPS {
                        let t = i as f32 / CURVE_STEPS as f32;
                        let u = 1.0 - t;
                        let p = pos.to_vec2() * u * u
                            + c.to_vec2() * 2.0 * u * t
                            + end.to_vec2() * t * t;
                        pts.push(p.to_pos2());
                    }
                    pos = end;
                }
                last_ctrl = None;
            }
            'A' => {
                for a in args.as_chunks::<7>().0 {
                    let end = at(pos, a[5], a[6]);
                    if pts.is_empty() {
                        pts.push(pos);
                    }
                    pts.extend(arc(pos, end, a[0], a[1], a[2], a[3] != 0.0, a[4] != 0.0));
                    pos = end;
                }
                last_ctrl = None;
            }
            'Z' => {
                pos = start;
                finish(&mut pts, &mut lines, true);
                last_ctrl = None;
            }
            other => return Err(format!("unsupported path command {other}")),
        }
    }
    finish(&mut pts, &mut lines, false);
    Ok(lines)
}

/// Points along an SVG elliptical arc from `p0` to `p1` (not including `p0`).
fn arc(p0: Pos2, p1: Pos2, rx: f32, ry: f32, angle: f32, large: bool, sweep: bool) -> Vec<Pos2> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || p0 == p1 {
        return vec![p1];
    }
    let phi = angle.to_radians();
    let (s, c) = phi.sin_cos();
    let d = (p0 - p1) * 0.5;
    let x1 = c * d.x + s * d.y;
    let y1 = -s * d.x + c * d.y;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let k = lambda.sqrt();
        rx *= k;
        ry *= k;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut f = (num / den).max(0.0).sqrt();
    if large == sweep {
        f = -f;
    }
    let cx1 = f * rx * y1 / ry;
    let cy1 = -f * ry * x1 / rx;
    let mid = (p0.to_vec2() + p1.to_vec2()) * 0.5;
    let centre = pos2(c * cx1 - s * cy1 + mid.x, s * cx1 + c * cy1 + mid.y);
    let angle_of = |ux: f32, uy: f32| uy.atan2(ux);
    let t1 = angle_of((x1 - cx1) / rx, (y1 - cy1) / ry);
    let t2 = angle_of((-x1 - cx1) / rx, (-y1 - cy1) / ry);
    let tau = std::f32::consts::TAU;
    let mut dt = t2 - t1;
    if sweep && dt < 0.0 {
        dt += tau;
    } else if !sweep && dt > 0.0 {
        dt -= tau;
    }
    let steps = ((dt.abs() / tau) * CIRCLE_STEPS as f32).ceil().max(2.0) as usize;
    (1..=steps)
        .map(|i| {
            let t = t1 + dt * i as f32 / steps as f32;
            let (x, y) = (rx * t.cos(), ry * t.sin());
            pos2(c * x - s * y + centre.x, s * x + c * y + centre.y)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_split_like_svg() {
        assert_eq!(numbers("1.5.5"), vec![1.5, 0.5]);
        assert_eq!(numbers("1.2-6.8"), vec![1.2, -6.8]);
        assert_eq!(numbers("M6 3l12 8"), vec![6.0, 3.0, 12.0, 8.0]);
    }

    #[test]
    fn paths_with_relative_moves_and_close() {
        let icon = Icon::parse(r#"<path d="M6 3l12 8-5.2 1.2L16 19z"/>"#).expect("parses");
        assert_eq!(icon.lines.len(), 1);
        let line = &icon.lines[0];
        assert!(line.closed);
        assert_eq!(line.points[1], pos2(18.0, 11.0));
        assert!((line.points[2].x - 12.8).abs() < 1e-4);
    }

    #[test]
    fn shapes_transforms_and_dashes() {
        let icon = Icon::parse(
            r#"<circle cx="12" cy="12" r="3"/><rect x="4" y="7" width="16" height="11" rx="1.5"/><ellipse cx="12" cy="12" rx="9" ry="4" transform="rotate(-25 12 12)"/><path d="M12 3v18" stroke-dasharray="2 2"/>"#,
        )
        .expect("parses");
        assert_eq!(icon.lines.len(), 4);
        assert!((icon.lines[0].points[0].distance(pos2(12.0, 12.0)) - 3.0).abs() < 1e-4);
        assert_eq!(icon.lines[3].dash, Some((2.0, 2.0)));
        // The rotated ellipse's first point is no longer level with its centre.
        assert!((icon.lines[2].points[0].y - 12.0).abs() > 1.0);
    }

    #[test]
    fn arcs_end_where_they_should() {
        let icon = Icon::parse(r#"<path d="M19 12a7 7 0 1 1-2.2-5.1"/>"#).expect("parses");
        let last = *icon.lines[0].points.last().expect("points");
        assert!(
            (last.x - 16.8).abs() < 1e-3 && (last.y - 6.9).abs() < 1e-3,
            "{last:?}"
        );
        // A large arc goes most of the way round, so it has many points.
        assert!(icon.lines[0].points.len() > 20);
        // Every point stays near the circle of radius 7.
        let pts = &icon.lines[0].points;
        let centre = pts.iter().fold(Vec2::ZERO, |a, p| a + p.to_vec2()) / pts.len() as f32;
        assert!(
            pts.iter()
                .all(|p| (p.distance(centre.to_pos2()) - 7.0).abs() < 1.5)
        );
    }

    #[test]
    fn unknown_elements_are_errors() {
        assert!(Icon::parse("<polygon points=\"1 2 3 4\"/>").is_err());
        assert!(Icon::parse("<path d=\"M1 1T4 4\"/>").is_err());
    }
}
