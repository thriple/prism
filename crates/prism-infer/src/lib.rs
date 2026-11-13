//! The inference engine (PLAN 5), as a pure function.
//!
//! [`infer`] takes the scene, the camera, the cursor, what the tool is doing,
//! the lock and the [`Memory`] of woken points, and returns where a click
//! would land and why ([`Inference`]). It reads no clock and keeps no state:
//! the caller keeps the memory and passes the time in, so a recorded session
//! replays exactly.
//!
//! The behaviour and the starting numbers are the feel test's (`infer(`,
//! `guideLines(`, `trackDwell(` in `docs/prototype/Main.dc.html`). Its
//! absolute tolerances are replaced by pixel distances and the model's
//! relative tolerance.

use std::collections::BTreeSet;

use prism_geom::{Curve, circle_line};
use prism_math::{DVec2, DVec3, Plane};
use prism_topo::{EdgeId, FaceId, Guide as StoredGuide, GuideId, VertexId};
use prism_view::{Scene, View};
use serde::{Deserialize, Serialize};

/// The tunable feel (the prototype's Feel Panel).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Scales every capture radius (0.5 to 2.5).
    pub aperture: f64,
    /// The current snap holds out to this many times its radius.
    pub release: f64,
    /// Hovering a point this long (seconds) wakes it.
    pub wake: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            aperture: 1.0,
            release: 1.6,
            wake: 0.22,
        }
    }
}

/// How much better another candidate must score to take over from the
/// current snap.
pub const SWITCH_MARGIN: f64 = 0.5;
/// At most this many woken points are kept.
pub const MAX_WOKEN: usize = 5;

/// What kind of snap it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Endpoint,
    Midpoint,
    OnEdge,
    /// On an arc, at the closest point.
    OnArc,
    /// An arc's centre (offered once the arc is hovered).
    Centre,
    /// A circle's quarter points (where they lie on the arc).
    Quadrant,
    /// Along an arc's tangent at its end.
    Tangent,
    OnFace,
    OnGround,
    Intersection,
    GuideCross,
    Guide,
    /// A guide point in the model (Tape Measure).
    GuidePoint,
    Free,
}

impl Kind {
    /// Priority and capture radius in pixels at aperture 1 (feel test).
    pub fn priority(self) -> (f64, f64) {
        match self {
            Kind::Endpoint => (6.0, 14.0),
            Kind::GuidePoint => (5.8, 14.0),
            Kind::GuideCross => (5.5, 12.0),
            Kind::Centre => (5.2, 12.0),
            Kind::Midpoint => (5.0, 12.0),
            Kind::Quadrant => (4.8, 12.0),
            Kind::Intersection => (4.6, 12.0),
            Kind::Tangent => (3.4, 8.0),
            Kind::Guide => (3.3, 8.0),
            Kind::OnEdge | Kind::OnArc => (3.0, 9.0),
            Kind::OnFace => (1.0, 0.0),
            Kind::OnGround => (0.8, 0.0),
            Kind::Free => (0.0, 0.0),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Endpoint => "Endpoint",
            Kind::Midpoint => "Midpoint",
            Kind::OnEdge => "On Edge",
            Kind::OnArc => "On Arc",
            Kind::Centre => "Centre",
            Kind::Quadrant => "Quadrant",
            Kind::Tangent => "Tangent",
            Kind::OnFace => "On Face",
            Kind::OnGround => "On Ground",
            Kind::Intersection => "Intersection",
            Kind::GuideCross => "Guide Cross",
            Kind::Guide => "Guide",
            Kind::GuidePoint => "Guide Point",
            Kind::Free => "Free",
        }
    }

    /// Kinds that are a definite point (projected onto a locked axis).
    pub fn is_point(self) -> bool {
        matches!(
            self,
            Kind::Endpoint
                | Kind::Midpoint
                | Kind::Centre
                | Kind::Quadrant
                | Kind::Intersection
                | Kind::GuideCross
                | Kind::GuidePoint
        )
    }

    /// Kinds that wake when hovered long enough.
    pub fn wakes(self) -> bool {
        matches!(
            self,
            Kind::Endpoint
                | Kind::Midpoint
                | Kind::OnEdge
                | Kind::OnArc
                | Kind::Centre
                | Kind::Quadrant
                | Kind::GuidePoint
        )
    }
}

/// The world axes: 0 red (x), 1 green (y), 2 blue (z).
pub const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];
pub const AXIS_NAMES: [&str; 3] = ["Red", "Green", "Blue"];

/// Where a guide comes from, which identifies it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GuideSource {
    /// An axis through the tool's start point.
    Start(u8),
    /// An axis through woken point number `n` (in wake order).
    Woken(u32, u8),
    /// Parallel to woken edge number `n`, through the start point.
    Parallel(u32),
    /// A guide line stored in the model (by its id).
    Stored(u32),
    /// Where the hovered face crosses another (faces, and which piece):
    /// DECISIONS #29. Only snaps along its own length.
    Crossing(u32, u32, u8),
    /// The tangent of arc `edge` at its end, through the start point (`n`
    /// 0) or woken point number `n`.
    Tangent(u32, u32),
}

/// A guide line: infinite, through `origin` along unit `dir`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    pub source: GuideSource,
    pub origin: DVec3,
    pub dir: DVec3,
    /// The axis it runs along, if it is an axis guide (its colour).
    pub axis: Option<u8>,
}

impl Guide {
    pub fn label(&self) -> String {
        match self.source {
            GuideSource::Start(a) => format!("On {} Axis", AXIS_NAMES[a as usize]),
            GuideSource::Woken(_, a) => format!("{} from Point", AXIS_NAMES[a as usize]),
            GuideSource::Parallel(_) => "Parallel to Edge".to_owned(),
            GuideSource::Stored(_) => "On Guide".to_owned(),
            GuideSource::Crossing(..) => "Face Intersection".to_owned(),
            GuideSource::Tangent(..) => "Tangent".to_owned(),
        }
    }

    /// The woken item it comes from (guides from the same item never make a
    /// guide cross with each other).
    fn owner(&self) -> Option<u32> {
        match self.source {
            GuideSource::Start(_)
            | GuideSource::Parallel(_)
            | GuideSource::Stored(_)
            | GuideSource::Crossing(..)
            | GuideSource::Tangent(..) => None,
            GuideSource::Woken(n, _) => Some(n),
        }
    }
}

/// What identifies a candidate from one moment to the next (for hysteresis).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Key {
    Vertex(VertexId),
    GuidePoint(GuideId),
    Mid(EdgeId),
    Edge(EdgeId),
    /// An arc's centre, and a quadrant (0 to 3) on it.
    Centre(EdgeId),
    Quadrant(EdgeId, u8),
    Face(FaceId),
    Ground,
    Guide(GuideSource),
    Cross(GuideSource, GuideSource),
    Intersection(GuideSource, EdgeId),
    Lock,
    Free,
}

/// A woken point or edge.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Woken {
    /// Counts wakes, so guides keep their identity as older ones drop out.
    pub serial: u32,
    pub key: Key,
    pub point: DVec3,
    /// For a woken edge: its direction.
    pub edge_dir: Option<DVec3>,
}

/// What the inference engine remembers between calls: the woken points and
/// the hover that may wake one. The caller keeps it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub woken: Vec<Woken>,
    /// The snap being hovered, since when, and what it would wake.
    pub dwell: Option<(Key, f64, Woken)>,
    /// The last snap's key (hysteresis).
    pub prev: Option<Key>,
    serial: u32,
}

impl Memory {
    /// Note the latest inference at time `now` (seconds). Returns true if a
    /// point woke.
    pub fn observe(&mut self, inf: &Inference, now: f64, settings: &Settings) -> bool {
        self.prev = inf.key;
        let Some(key) = inf.key else {
            self.dwell = None;
            return false;
        };
        if !inf.kind.wakes() || inf.locked {
            self.dwell = None;
            return false;
        }
        match &self.dwell {
            Some((k, _, _)) if *k == key => {}
            _ => {
                let w = Woken {
                    serial: 0,
                    key,
                    point: inf.point,
                    edge_dir: if inf.kind == Kind::OnEdge {
                        inf.edge_dir
                    } else {
                        None
                    },
                };
                self.dwell = Some((key, now, w));
            }
        }
        self.tick(now, settings)
    }

    /// Wake the hovered point if it has been hovered long enough. Returns
    /// true if one woke (the caller then infers again).
    pub fn tick(&mut self, now: f64, settings: &Settings) -> bool {
        let Some((key, since, w)) = self.dwell else {
            return false;
        };
        if now - since < settings.wake || self.woken.iter().any(|a| a.key == key) {
            return false;
        }
        self.serial += 1;
        self.woken.push(Woken {
            serial: self.serial,
            ..w
        });
        if self.woken.len() > MAX_WOKEN {
            self.woken.remove(0);
        }
        true
    }

    /// When the hovered point will wake, if one is waiting.
    pub fn wakes_at(&self, settings: &Settings) -> Option<f64> {
        let (key, since, _) = self.dwell?;
        (!self.woken.iter().any(|a| a.key == key)).then_some(since + settings.wake)
    }

    pub fn clear(&mut self) {
        self.woken.clear();
        self.dwell = None;
    }
}

/// A lock on the inference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Lock {
    #[default]
    None,
    /// Along an axis through the start point.
    Axis(u8),
}

/// What the tool is doing.
#[derive(Clone, Debug, Default)]
pub struct Context {
    /// The point the current step started from (a line's last point).
    pub start: Option<DVec3>,
    /// Vertices that are moving and must not be snapped to.
    pub exclude: BTreeSet<VertexId>,
    /// The tool ignores locks (Rectangle: the arrows pick its plane).
    pub no_lock: bool,
}

/// The answer: where a click lands and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Inference {
    pub point: DVec3,
    pub kind: Kind,
    pub key: Option<Key>,
    pub label: String,
    /// The axis colour to show it in, if it is on an axis.
    pub axis: Option<u8>,
    /// Guides the point lies on (drawn dashed).
    pub guides: Vec<Guide>,
    /// What it snapped to.
    pub vertex: Option<VertexId>,
    pub edge: Option<EdgeId>,
    pub edge_dir: Option<DVec3>,
    pub face: Option<FaceId>,
    pub locked: bool,
    /// Pixels from the cursor to the snap.
    pub distance: f64,
}

#[derive(Clone, Debug)]
struct Candidate {
    kind: Kind,
    point: DVec3,
    key: Key,
    score: f64,
    distance: f64,
    vertex: Option<VertexId>,
    edge: Option<EdgeId>,
    edge_dir: Option<DVec3>,
    face: Option<FaceId>,
    guides: Vec<Guide>,
}

/// The closest points of the line `o + d s` and the line `a + u t`: (s, t,
/// distance). None if they are parallel.
fn closest(o: DVec3, d: DVec3, a: DVec3, u: DVec3) -> Option<(f64, f64, f64)> {
    let w = o - a;
    let (dd, du, uu) = (d.dot(d), d.dot(u), u.dot(u));
    let den = dd * uu - du * du;
    if den.abs() <= 1e-14 * dd * uu {
        return None;
    }
    let (dw, uw) = (d.dot(w), u.dot(w));
    let s = (du * uw - uu * dw) / den;
    let t = (dd * uw - du * dw) / den;
    let dist = (o + d * s).distance(a + u * t);
    Some((s, t, dist))
}

/// The guides for this context: axes through the start point and through
/// each woken point, and parallels to woken edges through the start point.
pub fn guides(ctx: &Context, mem: &Memory) -> Vec<Guide> {
    let mut out = Vec::new();
    if let Some(s) = ctx.start {
        for a in 0..3u8 {
            out.push(Guide {
                source: GuideSource::Start(a),
                origin: s,
                dir: AXES[a as usize],
                axis: Some(a),
            });
        }
    }
    for w in &mem.woken {
        // A woken arc only offers its centre (no guides).
        if w.edge_dir.is_none() && !matches!(w.key, Key::Edge(_)) {
            for a in 0..3u8 {
                out.push(Guide {
                    source: GuideSource::Woken(w.serial, a),
                    origin: w.point,
                    dir: AXES[a as usize],
                    axis: Some(a),
                });
            }
        }
    }
    if let Some(s) = ctx.start {
        for w in &mem.woken {
            if let Some(dir) = w.edge_dir {
                out.push(Guide {
                    source: GuideSource::Parallel(w.serial),
                    origin: s,
                    dir,
                    axis: None,
                });
            }
        }
    }
    out
}

/// Where a click at `cursor` (logical pixels) lands.
pub fn infer(
    scene: &Scene,
    view: &View,
    cursor: DVec2,
    ctx: &Context,
    lock: Lock,
    mem: &Memory,
    settings: &Settings,
) -> Inference {
    let (o, dir) = view.ray(cursor);
    let tol = scene.tol.dist;
    // How close in 3D two guides, or a guide and an edge, must come to meet.
    let meet = tol * 10.0;
    let extent = scene
        .bounds
        .map_or(1.0, |(lo, hi)| lo.distance(hi))
        .max(view.eye.length())
        .max(tol);
    // Guides reach this far from their origin.
    let reach = extent * 100.0;
    let prev = mem.prev;
    let mut cands: Vec<Candidate> = Vec::new();
    let mut push = |kind: Kind, p: DVec3, key: Key, visible_only: bool, extra: Candidate| {
        let Some((s, _)) = view.to_screen(p) else {
            return;
        };
        let (prio, radius) = kind.priority();
        let r = radius * settings.aperture;
        let d = s.distance(cursor);
        if d > r * settings.release || (d > r && Some(key) != prev) {
            return;
        }
        if visible_only && scene.occluded(view.eye, p) {
            return;
        }
        cands.push(Candidate {
            kind,
            point: p,
            key,
            score: prio - d / r,
            distance: d,
            ..extra
        });
    };
    let blank = |kind: Kind, key: Key| Candidate {
        kind,
        point: DVec3::ZERO,
        key,
        score: 0.0,
        distance: 0.0,
        vertex: None,
        edge: None,
        edge_dir: None,
        face: None,
        guides: Vec::new(),
    };

    for &(id, p) in &scene.vertices {
        if ctx.exclude.contains(&id) {
            continue;
        }
        let mut c = blank(Kind::Endpoint, Key::Vertex(id));
        c.vertex = Some(id);
        push(Kind::Endpoint, p, Key::Vertex(id), true, c);
    }
    for e in &scene.edges {
        if ctx.exclude.contains(&e.va) || ctx.exclude.contains(&e.vb) {
            continue;
        }
        if let Curve::Arc {
            centre,
            axis,
            radius,
        } = e.curve
        {
            let mut c = blank(Kind::Midpoint, Key::Mid(e.id));
            c.edge = Some(e.id);
            push(
                Kind::Midpoint,
                e.curve.point(e.a, e.b, 0.5),
                Key::Mid(e.id),
                true,
                c.clone(),
            );
            // On the arc: the closest drawn piece, then exactly onto it.
            let near = e
                .segments()
                .filter_map(|(a, b)| {
                    let (s, t, _) = closest(o, dir, a, b - a)?;
                    let p = a.lerp(b, t.clamp(0.0, 1.0));
                    let (q, _) = view.to_screen(p)?;
                    (s > 0.0).then(|| (q.distance(cursor), p))
                })
                .min_by(|x, y| x.0.total_cmp(&y.0));
            if let Some((_, p)) = near {
                let mut c = c.clone();
                c.kind = Kind::OnArc;
                c.key = Key::Edge(e.id);
                push(
                    Kind::OnArc,
                    e.curve.closest_point(e.a, e.b, p),
                    Key::Edge(e.id),
                    true,
                    c.clone(),
                );
            }
            let f = Plane::new(centre, axis).frame();
            for (k, q) in [f.u, f.v, -f.u, -f.v].into_iter().enumerate() {
                let q = centre + q * radius;
                if e.curve.closest_point(e.a, e.b, q).distance(q) <= tol {
                    let key = Key::Quadrant(e.id, k as u8);
                    let mut c = c.clone();
                    c.kind = Kind::Quadrant;
                    c.key = key;
                    push(Kind::Quadrant, q, key, true, c);
                }
            }
            // The centre, while the arc is hovered or woken.
            let hovered = |k: Key| k == Key::Edge(e.id) || k == Key::Mid(e.id);
            if prev.is_some_and(hovered) || mem.woken.iter().any(|w| hovered(w.key)) {
                let key = Key::Centre(e.id);
                let mut c = blank(Kind::Centre, key);
                c.edge = Some(e.id);
                push(Kind::Centre, centre, key, false, c);
            }
            continue;
        }
        if e.curve.is_spline() {
            // On the curve: the closest drawn piece, then exactly onto it.
            let near = e
                .segments()
                .filter_map(|(a, b)| {
                    let (s, t, _) = closest(o, dir, a, b - a)?;
                    let p = a.lerp(b, t.clamp(0.0, 1.0));
                    let (q, _) = view.to_screen(p)?;
                    (s > 0.0).then(|| (q.distance(cursor), p))
                })
                .min_by(|x, y| x.0.total_cmp(&y.0));
            if let Some((_, p)) = near {
                let mut c = blank(Kind::OnArc, Key::Edge(e.id));
                c.edge = Some(e.id);
                push(
                    Kind::OnArc,
                    e.curve.closest_point(e.a, e.b, p),
                    Key::Edge(e.id),
                    true,
                    c,
                );
            }
            continue;
        }
        let u = e.b - e.a;
        let mut c = blank(Kind::Midpoint, Key::Mid(e.id));
        c.edge = Some(e.id);
        c.edge_dir = Some(u.normalize());
        push(
            Kind::Midpoint,
            e.a.lerp(e.b, 0.5),
            Key::Mid(e.id),
            true,
            c.clone(),
        );
        if let Some((s, t, _)) = closest(o, dir, e.a, u)
            && s > 0.0
        {
            let p = e.a + u * t.clamp(0.0, 1.0);
            let mut c = c;
            c.kind = Kind::OnEdge;
            c.key = Key::Edge(e.id);
            push(Kind::OnEdge, p, Key::Edge(e.id), true, c);
        }
    }
    let mut gl = guides(ctx, mem);
    // Tangents of the arcs that end at the start point or a woken point.
    let ends = ctx.start.map(|p| (0, p)).into_iter().chain(
        mem.woken
            .iter()
            .filter(|w| matches!(w.key, Key::Vertex(_)))
            .map(|w| (w.serial, w.point)),
    );
    for (n, p) in ends {
        for e in scene.edges.iter().filter(|e| e.curve.is_curved()) {
            let d = if e.a.distance(p) <= tol {
                e.curve.start_tangent(e.a, e.b)
            } else if e.b.distance(p) <= tol {
                e.curve.end_tangent(e.a, e.b)
            } else {
                continue;
            };
            // A circle's seam has the same tangent from both halves.
            if d != DVec3::ZERO
                && !gl
                    .iter()
                    .any(|g| g.origin == p && g.dir.cross(d).length() < 1e-9)
            {
                gl.push(Guide {
                    source: GuideSource::Tangent(n, e.id.0),
                    origin: p,
                    dir: d,
                    axis: None,
                });
            }
        }
    }
    for (id, g) in &scene.guides {
        match *g {
            StoredGuide::Line { origin, dir } => gl.push(Guide {
                source: GuideSource::Stored(id.0),
                origin,
                dir,
                axis: None,
            }),
            StoredGuide::Point { at } => {
                let key = Key::GuidePoint(*id);
                push(
                    Kind::GuidePoint,
                    at,
                    key,
                    false,
                    blank(Kind::GuidePoint, key),
                );
            }
        }
    }
    // Where the face under the cursor crosses other faces (DECISIONS #29).
    let mut spans: Vec<(GuideSource, f64)> = Vec::new();
    // Not for curved faces yet: their crossings need ellipses.
    if let Some((hid, _)) = scene.ray_face(o, dir)
        && let Some(hf) = scene.face(hid).filter(|f| f.curved.is_none())
    {
        let pad = DVec3::splat(tol);
        let hflat = hf.flat();
        for of in &scene.faces {
            if of.id == hid
                || of.curved.is_some()
                || (hf.hi + pad).cmplt(of.lo).any()
                || (of.hi + pad).cmplt(hf.lo).any()
            {
                continue;
            }
            for (k, (a, b)) in prism_ops::intersect::crossings(&hflat, &of.flat(), &scene.tol)
                .into_iter()
                .enumerate()
                .take(255)
            {
                let source = GuideSource::Crossing(hid.0, of.id.0, k as u8);
                gl.push(Guide {
                    source,
                    origin: a,
                    dir: (b - a).normalize(),
                    axis: None,
                });
                spans.push((source, a.distance(b)));
            }
        }
    }
    // A crossing guide only reaches along its own length.
    let within = |g: &Guide, t: f64| {
        spans
            .iter()
            .find(|(s, _)| *s == g.source)
            .is_none_or(|(_, len)| t >= -tol && t <= len + tol)
    };
    for g in &gl {
        if let Some((s, t, _)) = closest(o, dir, g.origin, g.dir)
            && s > 0.0
            && (t.abs() > tol
                || matches!(g.source, GuideSource::Stored(_) | GuideSource::Crossing(..)))
            && t.abs() < reach
            && within(g, t)
        {
            let kind = if matches!(g.source, GuideSource::Tangent(..)) {
                Kind::Tangent
            } else {
                Kind::Guide
            };
            let mut c = blank(kind, Key::Guide(g.source));
            c.guides = vec![*g];
            push(kind, g.origin + g.dir * t, Key::Guide(g.source), false, c);
        }
    }
    for i in 0..gl.len() {
        for j in i + 1..gl.len() {
            let (a, b) = (&gl[i], &gl[j]);
            if a.owner().is_some() && a.owner() == b.owner() {
                continue;
            }
            if a.origin == b.origin {
                continue;
            }
            if let Some((s, t, d)) = closest(a.origin, a.dir, b.origin, b.dir)
                && d < meet
                && within(a, s)
                && within(b, t)
            {
                let p = (a.origin + a.dir * s).lerp(b.origin + b.dir * t, 0.5);
                let key = Key::Cross(a.source, b.source);
                let mut c = blank(Kind::GuideCross, key);
                c.guides = vec![*a, *b];
                push(Kind::GuideCross, p, key, false, c);
            }
        }
    }
    for g in &gl {
        for e in &scene.edges {
            if ctx.exclude.contains(&e.va) || ctx.exclude.contains(&e.vb) {
                continue;
            }
            if let Curve::Arc {
                centre,
                axis,
                radius,
            } = e.curve
            {
                // Where the guide meets the arc, exactly.
                for p in circle_line(centre, axis, radius, g.origin, g.origin + g.dir, tol) {
                    let s = (p - g.origin).dot(g.dir);
                    if e.curve.closest_point(e.a, e.b, p).distance(p) <= tol
                        && p.distance(e.a) > tol
                        && p.distance(e.b) > tol
                        && s.abs() < reach
                        && within(g, s)
                    {
                        let key = Key::Intersection(g.source, e.id);
                        let mut c = blank(Kind::Intersection, key);
                        c.guides = vec![*g];
                        c.edge = Some(e.id);
                        push(Kind::Intersection, p, key, true, c);
                    }
                }
                continue;
            }
            let u = e.b - e.a;
            if let Some((s, t, d)) = closest(g.origin, g.dir, e.a, u)
                && d < meet
                && t > 0.0
                && t < 1.0
                && s.abs() < reach
                && within(g, s)
            {
                let key = Key::Intersection(g.source, e.id);
                let mut c = blank(Kind::Intersection, key);
                c.guides = vec![*g];
                c.edge = Some(e.id);
                push(Kind::Intersection, e.a + u * t, key, true, c);
            }
        }
    }
    // A face or the ground under the cursor, always available.
    let face_hit = scene.ray_face(o, dir);
    let ground = (dir.z < -1e-12 && o.z > 0.0)
        .then(|| -o.z / dir.z)
        .filter(|t| *t < reach);
    match (face_hit, ground) {
        (Some((f, tf)), g) if g.is_none_or(|tg| tf <= tg) => {
            let mut c = blank(Kind::OnFace, Key::Face(f));
            c.point = o + dir * tf;
            c.face = Some(f);
            c.score = Kind::OnFace.priority().0;
            cands.push(c);
        }
        (_, Some(tg)) => {
            let mut c = blank(Kind::OnGround, Key::Ground);
            c.point = o + dir * tg;
            c.score = Kind::OnGround.priority().0;
            cands.push(c);
        }
        _ => {}
    }

    // The winner, with hysteresis.
    let mut best: Option<&Candidate> = None;
    for c in &cands {
        if best.is_none_or(|b| c.score > b.score) {
            best = Some(c);
        }
    }
    if let (Some(pk), Some(b)) = (prev, best)
        && b.key != pk
        && let Some(pc) = cands.iter().find(|c| c.key == pk)
        && b.score < pc.score + SWITCH_MARGIN
    {
        best = Some(pc);
    }

    let mut inf = match (ctx.start, lock) {
        (Some(start), Lock::Axis(a)) if !ctx.no_lock => {
            let ad = AXES[a as usize];
            let p = match best {
                Some(b) if b.kind.is_point() => start + ad * (b.point - start).dot(ad),
                _ => closest(o, dir, start, ad).map_or(start, |(_, t, _)| start + ad * t),
            };
            Inference {
                point: p,
                kind: Kind::Guide,
                key: Some(Key::Lock),
                label: format!("On {} Axis (locked)", AXIS_NAMES[a as usize]),
                axis: Some(a),
                guides: vec![Guide {
                    source: GuideSource::Start(a),
                    origin: start,
                    dir: ad,
                    axis: Some(a),
                }],
                vertex: None,
                edge: None,
                edge_dir: None,
                face: None,
                locked: true,
                distance: 0.0,
            }
        }
        _ => match best {
            Some(b) => {
                let axis = b
                    .guides
                    .first()
                    .and_then(|g| g.axis)
                    .filter(|_| matches!(b.kind, Kind::Guide) || b.guides.len() == 1);
                let label = match b.kind {
                    Kind::Guide | Kind::Tangent => b.guides[0].label(),
                    // A spline is not an arc.
                    Kind::OnArc
                        if b.edge
                            .and_then(|e| scene.edge(e))
                            .is_some_and(|e| e.curve.is_spline()) =>
                    {
                        "On Curve".to_owned()
                    }
                    k => k.label().to_owned(),
                };
                Inference {
                    point: b.point,
                    kind: b.kind,
                    key: Some(b.key),
                    label,
                    axis,
                    guides: b.guides.clone(),
                    vertex: b.vertex,
                    edge: b.edge,
                    edge_dir: b.edge_dir,
                    face: b.face,
                    locked: false,
                    distance: b.distance,
                }
            }
            None => {
                let p = match ctx.start {
                    // On the plane through the start that faces the view.
                    Some(start) => {
                        let f = view.f;
                        let ax = (0..3)
                            .max_by(|&i, &j| f[i].abs().total_cmp(&f[j].abs()))
                            .unwrap_or(2);
                        if dir[ax].abs() > 1e-12 {
                            o + dir * ((start[ax] - o[ax]) / dir[ax])
                        } else {
                            o + dir * view.eye.distance(start)
                        }
                    }
                    // A parallel view: at the depth of the model's middle.
                    None if view.parallel.is_some() => {
                        let c = scene
                            .bounds
                            .map_or(DVec3::ZERO, |(lo, hi)| lo.lerp(hi, 0.5));
                        o + dir * (c - o).dot(dir)
                    }
                    None => o + dir * extent.min(view.eye.length().max(tol)),
                };
                Inference {
                    point: p,
                    kind: Kind::Free,
                    key: None,
                    label: Kind::Free.label().to_owned(),
                    axis: None,
                    guides: Vec::new(),
                    vertex: None,
                    edge: None,
                    edge_dir: None,
                    face: None,
                    locked: false,
                    distance: 0.0,
                }
            }
        },
    };
    // Also show every other guide the point happens to lie on.
    for g in &gl {
        let r = inf.point - g.origin;
        let off = r - g.dir * r.dot(g.dir);
        if off.length() < meet && r.length() > tol && !inf.guides.contains(g) {
            inf.guides.push(*g);
        }
    }
    inf
}
