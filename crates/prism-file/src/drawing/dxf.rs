//! DXF out (brief 45, part 6a): AutoCAD 2000 ASCII, the version CAM
//! programs read. Lines, arcs, circles, ellipses and splines go out as
//! themselves; a chain of lines and arcs as one LWPOLYLINE with bulges
//! (CAM programs cut a closed polyline as one path). Each part is a layer.
//! No clock: the same drawing writes the same bytes.

use std::f64::consts::{PI, TAU};
use std::fmt::Write as _;

use prism_geom::Nurbs;
use prism_math::{DAffine2, DVec2, DVec3, LengthUnit};

use super::{Drawing, Path, Seg, num};

/// The text of `d` as a DXF file.
pub fn write(d: &Drawing) -> String {
    let mut w = Writer {
        out: String::new(),
        next: 0x20,
    };
    let layers: Vec<String> = {
        let mut v: Vec<String> = Vec::new();
        for p in &d.parts {
            let n = layer_name(&p.name);
            if !v.contains(&n) {
                v.push(n);
            }
        }
        v
    };
    // The handles of what entities point back to, fixed first.
    let model_space = 0x1F;
    let mut entities = Writer {
        out: String::new(),
        next: 0x1000,
    };
    for p in &d.parts {
        let layer = layer_name(&p.name);
        for path in &p.paths {
            entities.path(path, &layer, model_space, d.eps);
        }
    }

    w.pair(0, "SECTION");
    w.pair(2, "HEADER");
    w.pair(9, "$ACADVER");
    w.pair(1, "AC1015");
    w.pair(9, "$HANDSEED");
    w.pair(5, &format!("{:X}", entities.next));
    w.pair(9, "$INSUNITS");
    w.pair(70, insunits(d.unit));
    w.pair(9, "$MEASUREMENT");
    w.pair(
        70,
        if d.unit == LengthUnit::Inches {
            "0"
        } else {
            "1"
        },
    );
    w.pair(0, "ENDSEC");

    w.pair(0, "SECTION");
    w.pair(2, "CLASSES");
    w.pair(0, "ENDSEC");

    w.pair(0, "SECTION");
    w.pair(2, "TABLES");
    let t = w.table("VPORT", 1);
    w.record("VPORT", &t, "AcDbViewportTableRecord");
    w.pair(2, "*Active");
    w.pair(70, "0");
    for (code, v) in [
        (10, "0"),
        (20, "0"),
        (11, "1"),
        (21, "1"),
        (12, "0"),
        (22, "0"),
        (13, "0"),
        (23, "0"),
        (14, "1"),
        (24, "1"),
        (15, "0"),
        (25, "0"),
        (16, "0"),
        (26, "0"),
        (36, "1"),
        (17, "0"),
        (27, "0"),
        (37, "0"),
        (40, "1"),
        (41, "1"),
        (42, "50"),
        (43, "0"),
        (44, "0"),
        (50, "0"),
        (51, "0"),
        (71, "0"),
        (72, "100"),
        (73, "1"),
        (74, "3"),
        (75, "0"),
        (76, "0"),
        (77, "0"),
        (78, "0"),
    ] {
        w.pair(code, v);
    }
    w.end_table();
    let t = w.table("LTYPE", 3);
    for (name, about) in [
        ("ByBlock", ""),
        ("ByLayer", ""),
        ("Continuous", "Solid line"),
    ] {
        w.record("LTYPE", &t, "AcDbLinetypeTableRecord");
        w.pair(2, name);
        w.pair(70, "0");
        w.pair(3, about);
        w.pair(72, "65");
        w.pair(73, "0");
        w.pair(40, "0");
    }
    w.end_table();
    let t = w.table("LAYER", layers.len() + 1);
    for name in std::iter::once("0").chain(layers.iter().map(String::as_str)) {
        w.record("LAYER", &t, "AcDbLayerTableRecord");
        w.pair(2, name);
        w.pair(70, "0");
        w.pair(62, "7");
        w.pair(6, "Continuous");
        w.pair(370, "-3");
    }
    w.end_table();
    let t = w.table("STYLE", 1);
    w.record("STYLE", &t, "AcDbTextStyleTableRecord");
    for (code, v) in [
        (2, "Standard"),
        (70, "0"),
        (40, "0"),
        (41, "1"),
        (50, "0"),
        (71, "0"),
        (42, "2.5"),
        (3, "txt"),
        (4, ""),
    ] {
        w.pair(code, v);
    }
    w.end_table();
    for name in ["VIEW", "UCS"] {
        w.table(name, 0);
        w.end_table();
    }
    let t = w.table("APPID", 1);
    w.record("APPID", &t, "AcDbRegAppTableRecord");
    w.pair(2, "ACAD");
    w.pair(70, "0");
    w.end_table();
    let t = w.table("DIMSTYLE", 1);
    w.pair(100, "AcDbDimStyleTable");
    w.pair(0, "DIMSTYLE");
    let h = w.handle();
    w.pair(105, &h);
    w.pair(330, &t);
    w.pair(100, "AcDbSymbolTableRecord");
    w.pair(100, "AcDbDimStyleTableRecord");
    w.pair(2, "Standard");
    w.pair(70, "0");
    w.end_table();
    let t = w.table("BLOCK_RECORD", 2);
    let spaces = ["*Model_Space", "*Paper_Space"];
    let mut records = Vec::new();
    for name in spaces {
        w.pair(0, "BLOCK_RECORD");
        let h = if name == "*Model_Space" {
            format!("{model_space:X}")
        } else {
            w.handle()
        };
        w.pair(5, &h);
        w.pair(330, &t);
        w.pair(100, "AcDbSymbolTableRecord");
        w.pair(100, "AcDbBlockTableRecord");
        w.pair(2, name);
        records.push(h);
    }
    w.end_table();
    w.pair(0, "ENDSEC");

    w.pair(0, "SECTION");
    w.pair(2, "BLOCKS");
    for (name, owner) in spaces.iter().zip(&records) {
        w.pair(0, "BLOCK");
        let h = w.handle();
        w.pair(5, &h);
        w.pair(330, owner);
        w.pair(100, "AcDbEntity");
        if *name == "*Paper_Space" {
            w.pair(67, "1");
        }
        w.pair(8, "0");
        w.pair(100, "AcDbBlockBegin");
        w.pair(2, name);
        w.pair(70, "0");
        w.pair(10, "0");
        w.pair(20, "0");
        w.pair(30, "0");
        w.pair(3, name);
        w.pair(1, "");
        w.pair(0, "ENDBLK");
        let h = w.handle();
        w.pair(5, &h);
        w.pair(330, owner);
        w.pair(100, "AcDbEntity");
        if *name == "*Paper_Space" {
            w.pair(67, "1");
        }
        w.pair(8, "0");
        w.pair(100, "AcDbBlockEnd");
    }
    w.pair(0, "ENDSEC");

    w.pair(0, "SECTION");
    w.pair(2, "ENTITIES");
    w.out.push_str(&entities.out);
    w.pair(0, "ENDSEC");

    w.pair(0, "SECTION");
    w.pair(2, "OBJECTS");
    let root = w.handle();
    let groups = w.handle();
    w.pair(0, "DICTIONARY");
    w.pair(5, &root);
    w.pair(330, "0");
    w.pair(100, "AcDbDictionary");
    w.pair(281, "1");
    w.pair(3, "ACAD_GROUP");
    w.pair(350, &groups);
    w.pair(0, "DICTIONARY");
    w.pair(5, &groups);
    w.pair(330, &root);
    w.pair(100, "AcDbDictionary");
    w.pair(281, "1");
    w.pair(0, "ENDSEC");
    w.pair(0, "EOF");
    w.out
}

/// `$INSUNITS` for a unit.
fn insunits(u: LengthUnit) -> &'static str {
    match u {
        LengthUnit::Millimetres => "4",
        LengthUnit::Centimetres => "5",
        LengthUnit::Metres => "6",
        LengthUnit::Inches | LengthUnit::FeetInches => "1",
    }
}

/// A part's name as a layer name: the characters DXF does not allow in
/// one become "_".
fn layer_name(name: &str) -> String {
    let n: String = name
        .chars()
        .map(|c| {
            if "<>/\\\":;?*|=`,".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let n = n.trim().to_owned();
    if n.is_empty() { "Part".to_owned() } else { n }
}

struct Writer {
    out: String,
    next: u32,
}

impl Writer {
    fn pair(&mut self, code: i32, value: &str) {
        let _ = write!(self.out, "{code:>3}\n{value}\n");
    }

    fn handle(&mut self) -> String {
        let h = format!("{:X}", self.next);
        self.next += 1;
        h
    }

    /// A table's start; its handle.
    fn table(&mut self, name: &str, count: usize) -> String {
        self.pair(0, "TABLE");
        self.pair(2, name);
        let h = self.handle();
        self.pair(5, &h);
        self.pair(330, "0");
        self.pair(100, "AcDbSymbolTable");
        self.pair(70, &count.to_string());
        h
    }

    fn end_table(&mut self) {
        self.pair(0, "ENDTAB");
    }

    fn record(&mut self, kind: &str, table: &str, class: &str) {
        self.pair(0, kind);
        let h = self.handle();
        self.pair(5, &h);
        self.pair(330, table);
        self.pair(100, "AcDbSymbolTableRecord");
        self.pair(100, class);
    }

    fn entity(&mut self, kind: &str, layer: &str, owner: u32, class: &str) {
        self.pair(0, kind);
        let h = self.handle();
        self.pair(5, &h);
        self.pair(330, &format!("{owner:X}"));
        self.pair(100, "AcDbEntity");
        self.pair(8, layer);
        self.pair(100, class);
    }

    fn point(&mut self, code: i32, p: DVec2) {
        self.pair(code, &num(p.x));
        self.pair(code + 10, &num(p.y));
        self.pair(code + 20, "0");
    }

    fn path(&mut self, p: &Path, layer: &str, owner: u32, eps: f64) {
        // A closed run of arcs of one circle, all the way round: a circle.
        if p.closed
            && let Some((c, r)) = whole_circle(p, eps)
        {
            self.entity("CIRCLE", layer, owner, "AcDbCircle");
            self.point(10, c);
            self.pair(40, &num(r));
            return;
        }
        let bulges: Option<Vec<f64>> = p
            .segs
            .iter()
            .map(|s| match s {
                Seg::Line(..) => Some(0.0),
                s => s.circle(eps).map(|(_, _, _, turn)| (turn / 4.0).tan()),
            })
            .collect();
        match bulges {
            Some(b) if p.segs.len() > 1 => {
                let mut points: Vec<DVec2> = p.segs.iter().map(Seg::start).collect();
                if !p.closed
                    && let Some(s) = p.segs.last()
                {
                    points.push(s.end());
                }
                self.entity("LWPOLYLINE", layer, owner, "AcDbPolyline");
                self.pair(90, &points.len().to_string());
                self.pair(70, if p.closed { "1" } else { "0" });
                for (i, q) in points.iter().enumerate() {
                    self.pair(10, &num(q.x));
                    self.pair(20, &num(q.y));
                    if let Some(b) = b.get(i).filter(|b| **b != 0.0) {
                        self.pair(42, &num(*b));
                    }
                }
            }
            _ => {
                for s in &p.segs {
                    self.seg(s, layer, owner, eps);
                }
            }
        }
    }

    fn seg(&mut self, s: &Seg, layer: &str, owner: u32, eps: f64) {
        match s {
            Seg::Line(a, b) => {
                self.entity("LINE", layer, owner, "AcDbLine");
                self.point(10, *a);
                self.point(11, *b);
            }
            Seg::Conic { .. } => {
                if let Some((c, r, start, turn)) = s.circle(eps) {
                    if (turn.abs() - TAU).abs() <= 1e-9 {
                        self.entity("CIRCLE", layer, owner, "AcDbCircle");
                        self.point(10, c);
                        self.pair(40, &num(r));
                        return;
                    }
                    // Arcs run counter-clockwise from their start angle.
                    let (a0, a1) = if turn > 0.0 {
                        (start, start + turn)
                    } else {
                        (start + turn, start)
                    };
                    self.entity("ARC", layer, owner, "AcDbCircle");
                    self.point(10, c);
                    self.pair(40, &num(r));
                    self.pair(100, "AcDbArc");
                    self.pair(50, &num(a0.to_degrees().rem_euclid(360.0)));
                    self.pair(51, &num(a1.to_degrees().rem_euclid(360.0)));
                } else if let Some((c, major, ratio, t0, t1)) = s.ellipse() {
                    self.entity("ELLIPSE", layer, owner, "AcDbEllipse");
                    self.point(10, c);
                    self.point(11, major);
                    self.pair(210, "0");
                    self.pair(220, "0");
                    self.pair(230, "1");
                    self.pair(40, &num(ratio));
                    let full = (t1 - t0 - TAU).abs() <= 1e-9;
                    self.pair(41, &num(if full { 0.0 } else { t0 }));
                    self.pair(42, &num(if full { TAU } else { t1 }));
                }
            }
            Seg::Spline(n) => {
                let rational = n.weights().iter().any(|w| (*w - 1.0).abs() > 1e-12);
                self.entity("SPLINE", layer, owner, "AcDbSpline");
                self.pair(210, "0");
                self.pair(220, "0");
                self.pair(230, "1");
                self.pair(70, if rational { "12" } else { "8" });
                self.pair(71, &n.degree().to_string());
                self.pair(72, &n.knots().len().to_string());
                self.pair(73, &n.points().len().to_string());
                self.pair(74, "0");
                for k in n.knots() {
                    self.pair(40, &k.to_string());
                }
                if rational {
                    for w in n.weights() {
                        self.pair(41, &w.to_string());
                    }
                }
                for p in n.points() {
                    self.point(10, p.truncate());
                }
            }
        }
    }
}

/// A closed path of arcs of one circle going all the way round: its
/// centre and radius.
fn whole_circle(p: &Path, eps: f64) -> Option<(DVec2, f64)> {
    let mut first: Option<(DVec2, f64)> = None;
    let mut total = 0.0;
    for s in &p.segs {
        let (c, r, _, turn) = s.circle(eps)?;
        if let Some((c0, r0)) = first
            && (c.distance(c0) > eps || (r - r0).abs() > eps)
        {
            return None;
        }
        first.get_or_insert((c, r));
        total += turn;
    }
    first.filter(|_| (total.abs() - TAU).abs() <= 1e-6)
}

// ---------- Reading (part 6b) ----------

/// One entity's group codes and values, in order.
type Entity = Vec<(i32, String)>;

/// A DXF file's drawing: its lines, arcs, circles, ellipses, polylines,
/// splines and points, blocks placed where they stand; what was left out.
pub fn read(text: &str) -> Result<super::Read, String> {
    let mut pairs = Vec::new();
    let mut lines = text.lines();
    while let (Some(c), Some(v)) = (lines.next(), lines.next()) {
        let code: i32 = c
            .trim()
            .parse()
            .map_err(|_| "Not a DXF file: a group code is not a number.".to_owned())?;
        pairs.push((code, v.trim().to_owned()));
    }
    if !pairs.iter().any(|(c, v)| *c == 0 && v == "SECTION") {
        return Err("Not a DXF file: it has no sections.".to_owned());
    }
    let mut stated = None;
    let mut blocks: std::collections::BTreeMap<String, (DVec2, Vec<Entity>)> =
        std::collections::BTreeMap::new();
    let mut entities: Vec<Entity> = Vec::new();
    let mut i = 0;
    while i < pairs.len() {
        if pairs[i] != (0, "SECTION".to_owned()) {
            i += 1;
            continue;
        }
        let name = pairs.get(i + 1).map(|p| p.1.clone()).unwrap_or_default();
        i += 2;
        let start = i;
        while i < pairs.len() && pairs[i] != (0, "ENDSEC".to_owned()) {
            i += 1;
        }
        let body = &pairs[start..i];
        match name.as_str() {
            "HEADER" => {
                if let Some(k) = body.iter().position(|p| p == &(9, "$INSUNITS".to_owned())) {
                    stated = body
                        .get(k + 1)
                        .and_then(|p| p.1.parse::<i32>().ok())
                        .and_then(units);
                }
            }
            "BLOCKS" => {
                let mut current: Option<(String, DVec2, Vec<Entity>)> = None;
                for e in split(body) {
                    match e.first().map(|p| p.1.as_str()) {
                        Some("BLOCK") => {
                            current = Some((text_of(&e, 2), point(&e, 10), Vec::new()));
                        }
                        Some("ENDBLK") => {
                            if let Some((n, base, es)) = current.take() {
                                blocks.insert(n, (base, es));
                            }
                        }
                        _ => {
                            if let Some(c) = current.as_mut() {
                                c.2.push(e);
                            }
                        }
                    }
                }
            }
            "ENTITIES" => entities = split(body),
            _ => {}
        }
        i += 1;
    }
    let mut r = Reading {
        blocks: &blocks,
        paths: Vec::new(),
        points: Vec::new(),
        skipped: Vec::new(),
    };
    r.entities(&entities, &DAffine2::IDENTITY, 0);
    Ok(super::Read {
        paths: r.paths,
        points: r.points,
        stated,
        skipped: r.skipped,
    })
}

/// Metres in a `$INSUNITS` unit; None for "no unit" and the rare ones.
fn units(code: i32) -> Option<f64> {
    match code {
        1 => Some(0.0254),
        2 => Some(0.3048),
        4 => Some(1e-3),
        5 => Some(1e-2),
        6 => Some(1.0),
        10 => Some(0.9144),
        _ => None,
    }
}

/// Pairs cut into entities, each starting at a code 0.
fn split(pairs: &[(i32, String)]) -> Vec<Entity> {
    let mut out: Vec<Entity> = Vec::new();
    for p in pairs {
        if p.0 == 0 || out.is_empty() {
            out.push(Vec::new());
        }
        if let Some(e) = out.last_mut() {
            e.push(p.clone());
        }
    }
    out
}

fn num_of(e: &Entity, code: i32) -> Option<f64> {
    e.iter()
        .find(|p| p.0 == code)
        .and_then(|p| p.1.parse().ok())
}

fn nums(e: &Entity, code: i32) -> Vec<f64> {
    e.iter()
        .filter(|p| p.0 == code)
        .filter_map(|p| p.1.parse().ok())
        .collect()
}

fn text_of(e: &Entity, code: i32) -> String {
    e.iter()
        .find(|p| p.0 == code)
        .map(|p| p.1.clone())
        .unwrap_or_default()
}

/// The point at codes `code` and `code + 10`.
fn point(e: &Entity, code: i32) -> DVec2 {
    DVec2::new(
        num_of(e, code).unwrap_or(0.0),
        num_of(e, code + 10).unwrap_or(0.0),
    )
}

/// Why an entity of this kind is left out.
fn why_not(kind: &str) -> String {
    match kind {
        "TEXT" | "MTEXT" | "ATTRIB" | "ATTDEF" => "text".to_owned(),
        "DIMENSION" | "LEADER" | "MLEADER" | "TOLERANCE" => "dimensions".to_owned(),
        "HATCH" | "SOLID" | "TRACE" | "WIPEOUT" => "hatches and fills".to_owned(),
        "IMAGE" | "OLE2FRAME" => "images".to_owned(),
        "3DFACE" | "MESH" | "3DSOLID" | "BODY" | "REGION" | "SURFACE" => "3D objects".to_owned(),
        k => k.to_lowercase(),
    }
}

struct Reading<'a> {
    blocks: &'a std::collections::BTreeMap<String, (DVec2, Vec<Entity>)>,
    paths: Vec<Path>,
    points: Vec<DVec2>,
    skipped: Vec<String>,
}

impl Reading<'_> {
    fn entities(&mut self, es: &[Entity], at: &DAffine2, depth: usize) {
        let mut k = 0;
        while k < es.len() {
            let e = &es[k];
            let kind = e.first().map(|p| p.1.clone()).unwrap_or_default();
            k += 1;
            // Paper space is a sheet layout, not the model.
            if num_of(e, 67) == Some(1.0) {
                continue;
            }
            if kind == "POLYLINE" {
                let mut verts = Vec::new();
                while k < es.len() {
                    let v = &es[k];
                    k += 1;
                    match v.first().map(|p| p.1.as_str()) {
                        Some("VERTEX") => verts.push(v.clone()),
                        _ => break,
                    }
                }
                let flags = num_of(e, 70).unwrap_or(0.0) as i32;
                if flags & (16 | 64) != 0 {
                    self.skipped.push("3D objects".to_owned());
                    continue;
                }
                let pts: Vec<(DVec2, f64)> = verts
                    .iter()
                    .map(|v| (point(v, 10), num_of(v, 42).unwrap_or(0.0)))
                    .collect();
                let ocs = if flags & 8 != 0 {
                    DAffine2::IDENTITY
                } else {
                    match self.ocs(e) {
                        Some(m) => m,
                        None => continue,
                    }
                };
                self.polyline(&pts, flags & 1 != 0, &(*at * ocs));
                continue;
            }
            match kind.as_str() {
                "LINE" => self.add(vec![Seg::Line(point(e, 10), point(e, 11))], false, at),
                "POINT" => self.points.push(at.transform_point2(point(e, 10))),
                "CIRCLE" | "ARC" => {
                    let Some(ocs) = self.ocs(e) else { continue };
                    let r = num_of(e, 40).unwrap_or(0.0);
                    let (a0, a1) = if kind == "ARC" {
                        (
                            num_of(e, 50).unwrap_or(0.0).to_radians(),
                            num_of(e, 51).unwrap_or(0.0).to_radians(),
                        )
                    } else {
                        (0.0, TAU)
                    };
                    let mut sweep = (a1 - a0).rem_euclid(TAU);
                    if sweep <= 1e-12 {
                        sweep = TAU;
                    }
                    let u = DVec2::from_angle(a0) * r;
                    let seg = Seg::Conic {
                        c: point(e, 10),
                        u,
                        v: u.perp(),
                        sweep,
                    };
                    self.add(vec![seg], kind == "CIRCLE", &(*at * ocs));
                }
                "ELLIPSE" => {
                    let n = DVec3::new(
                        num_of(e, 210).unwrap_or(0.0),
                        num_of(e, 220).unwrap_or(0.0),
                        num_of(e, 230).unwrap_or(1.0),
                    );
                    if n.x.abs() > 1e-9 || n.y.abs() > 1e-9 {
                        self.skipped
                            .push("objects not flat on the drawing".to_owned());
                        continue;
                    }
                    let major = point(e, 11);
                    let ratio = num_of(e, 40).unwrap_or(1.0);
                    let minor = major.perp() * ratio * n.z.signum();
                    let s = num_of(e, 41).unwrap_or(0.0);
                    let t = num_of(e, 42).unwrap_or(TAU);
                    let mut sweep = (t - s).rem_euclid(TAU);
                    if sweep <= 1e-12 {
                        sweep = TAU;
                    }
                    let seg = Seg::Conic {
                        c: point(e, 10),
                        u: major * s.cos() + minor * s.sin(),
                        v: -major * s.sin() + minor * s.cos(),
                        sweep,
                    };
                    self.add(vec![seg], sweep >= TAU, at);
                }
                "LWPOLYLINE" => {
                    let Some(ocs) = self.ocs(e) else { continue };
                    let mut pts: Vec<(DVec2, f64)> = Vec::new();
                    for (c, v) in e {
                        let x: f64 = v.parse().unwrap_or(0.0);
                        match c {
                            10 => pts.push((DVec2::new(x, 0.0), 0.0)),
                            20 => {
                                if let Some(p) = pts.last_mut() {
                                    p.0.y = x;
                                }
                            }
                            42 => {
                                if let Some(p) = pts.last_mut() {
                                    p.1 = x;
                                }
                            }
                            _ => {}
                        }
                    }
                    let closed = num_of(e, 70).unwrap_or(0.0) as i32 & 1 != 0;
                    self.polyline(&pts, closed, &(*at * ocs));
                }
                "SPLINE" => match spline(e) {
                    Some(n) => self.add(vec![Seg::Spline(n)], false, at),
                    None => self
                        .skipped
                        .push("splines that do not fit together".to_owned()),
                },
                "INSERT" => {
                    let name = text_of(e, 2);
                    let Some((base, inner)) = self.blocks.get(&name) else {
                        continue;
                    };
                    if depth > 16 {
                        continue;
                    }
                    let Some(ocs) = self.ocs(e) else { continue };
                    let s = DVec2::new(num_of(e, 41).unwrap_or(1.0), num_of(e, 42).unwrap_or(1.0));
                    let place = DAffine2::from_translation(point(e, 10))
                        * DAffine2::from_angle(num_of(e, 50).unwrap_or(0.0).to_radians())
                        * DAffine2::from_scale(s)
                        * DAffine2::from_translation(-*base);
                    self.entities(inner, &(*at * ocs * place), depth + 1);
                }
                "SEQEND" | "VIEWPORT" | "" => {}
                k => self.skipped.push(why_not(k)),
            }
        }
    }

    /// The entity's own flat coordinates (OCS) on the drawing: the same
    /// for the usual +z, mirrored in x for -z. None (counted) for any
    /// other: the entity is not flat on the drawing.
    fn ocs(&mut self, e: &Entity) -> Option<DAffine2> {
        let (x, y, z) = (
            num_of(e, 210).unwrap_or(0.0),
            num_of(e, 220).unwrap_or(0.0),
            num_of(e, 230).unwrap_or(1.0),
        );
        if x.abs() > 1e-9 || y.abs() > 1e-9 {
            self.skipped
                .push("objects not flat on the drawing".to_owned());
            return None;
        }
        Some(if z < 0.0 {
            DAffine2::from_scale(DVec2::new(-1.0, 1.0))
        } else {
            DAffine2::IDENTITY
        })
    }

    /// Points with bulges (the arc to the next point: the tangent of a
    /// quarter of its turn, counter-clockwise above 0).
    fn polyline(&mut self, pts: &[(DVec2, f64)], closed: bool, at: &DAffine2) {
        let n = pts.len();
        let mut segs = Vec::new();
        let last = if closed { n } else { n.saturating_sub(1) };
        for i in 0..last {
            let (a, bulge) = pts[i];
            let b = pts[(i + 1) % n].0;
            if a.distance(b) == 0.0 {
                continue;
            }
            if bulge.abs() < 1e-12 {
                segs.push(Seg::Line(a, b));
                continue;
            }
            let turn = 4.0 * bulge.atan();
            // The centre: on the chord's bisector, the chord seen from it
            // at the turn.
            let mid = (a + b) * 0.5;
            let half = a.distance(b) * 0.5;
            let r = half / (turn * 0.5).sin().abs();
            let side = (r * r - half * half).max(0.0).sqrt();
            let dir = (b - a).normalize().perp();
            let along = if turn.abs() < PI { side } else { -side };
            let c = mid + dir * along * turn.signum();
            let u = a - c;
            let v = if turn > 0.0 { u.perp() } else { -u.perp() };
            segs.push(Seg::Conic {
                c,
                u,
                v,
                sweep: turn.abs(),
            });
        }
        self.add(segs, closed, at);
    }

    fn add(&mut self, segs: Vec<Seg>, closed: bool, at: &DAffine2) {
        if segs.is_empty() {
            return;
        }
        let segs = segs.into_iter().map(|s| s.mapped(at)).collect();
        self.paths.push(Path { segs, closed });
    }
}

/// A SPLINE entity as a curve: by its control points, or through its fit
/// points when it has none.
fn spline(e: &Entity) -> Option<Nurbs> {
    let degree = num_of(e, 71)? as usize;
    let knots = nums(e, 40);
    let xs = nums(e, 10);
    let ys = nums(e, 20);
    let pts: Vec<DVec3> = xs
        .iter()
        .zip(&ys)
        .map(|(x, y)| DVec3::new(*x, *y, 0.0))
        .collect();
    if pts.is_empty() {
        let fit: Vec<DVec3> = nums(e, 11)
            .iter()
            .zip(&nums(e, 21))
            .map(|(x, y)| DVec3::new(*x, *y, 0.0))
            .collect();
        return Nurbs::through(&fit, false);
    }
    let mut weights = nums(e, 41);
    if weights.len() != pts.len() {
        weights = vec![1.0; pts.len()];
    }
    Nurbs::new(degree, knots, pts, weights, false)
}
