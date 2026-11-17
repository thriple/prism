//! DXF out (brief 45, part 6a): AutoCAD 2000 ASCII, the version CAM
//! programs read. Lines, arcs, circles, ellipses and splines go out as
//! themselves; a chain of lines and arcs as one LWPOLYLINE with bulges
//! (CAM programs cut a closed polyline as one path). Each part is a layer.
//! No clock: the same drawing writes the same bytes.

use std::f64::consts::TAU;
use std::fmt::Write as _;

use prism_math::{DVec2, LengthUnit};

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
