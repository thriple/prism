//! Effects (brief 47): a short line animation when an edit finishes.
//!
//! An edit's shape is found by one rule, for every tool: the drawn lines
//! that are new after it (`made`), and the ones that went (`gone`). This
//! module turns that shape into overlay lines for any moment `t` from 0
//! to 1. It reads no clock: the caller passes the time in, so tests can
//! ask for any frame.

use std::collections::BTreeMap;

use prism_math::{DAffine3, DQuat, DVec3};

use crate::overlay::Line;
use crate::scene::Scene;
use crate::style::Rgba;

/// How long an effect lasts, seconds.
pub const DURATION: f64 = 1.0;
/// How long it lasts when motion is reduced (a fade, nothing moves).
pub const REDUCED: f64 = 0.4;
/// Above this many lines an edit does not animate.
pub const MAX_SEGMENTS: usize = 4000;
/// A tiling's grid is many short pieces: it may have more.
pub const MAX_TILE: usize = 20_000;
/// Above this many lines in the scene, no edit animates (finding the
/// shape would cost too much).
pub const MAX_SCENE: usize = 200_000;
/// At most this many effects run at once; the oldest goes first.
pub const MAX_RUNNING: usize = 4;
/// How much larger the echo grows, as a fraction.
pub const GROW: f64 = 0.06;

/// A drawn line piece, in world coordinates.
pub type Seg = [DVec3; 2];

/// How a Travel moves: as the tool moved, turned or scaled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Turn {
    Move(DVec3),
    Rotate {
        centre: DVec3,
        axis: DVec3,
        angle: f64,
    },
    Scale {
        centre: DVec3,
        factor: DVec3,
    },
}

impl Turn {
    /// The change `k` of the way (0 to 1).
    pub fn at(&self, k: f64) -> DAffine3 {
        match *self {
            Turn::Move(by) => DAffine3::from_translation(by * k),
            Turn::Rotate {
                centre,
                axis,
                angle,
            } => {
                DAffine3::from_translation(centre)
                    * DAffine3::from_axis_angle(axis.normalize_or_zero(), angle * k)
                    * DAffine3::from_translation(-centre)
            }
            Turn::Scale { centre, factor } => {
                DAffine3::from_translation(centre)
                    * DAffine3::from_scale(DVec3::ONE.lerp(factor, k))
                    * DAffine3::from_translation(-centre)
            }
        }
    }
}

/// How an effect moves.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// The new lines flash, then a copy grows a little and fades.
    Echo,
    /// A bright tip runs along the new lines, then they fade.
    Trace,
    /// An outline travels by this much, ending where the new lines are
    /// (Push/Pull).
    Sweep(DVec3),
    /// The lines that went fade where they were (Erase).
    Ghost,
    /// New curved faces (Loft, Revolve, Fillet): their surface grid rolls
    /// across them along this direction, then fades.
    Tile(DVec3),
    /// The old outline moves, turns or scales to where it lands (Move,
    /// Rotate, Scale); its pinned ends stay.
    Travel(Turn),
    /// The profile outline travels along this path (Follow Me).
    Follow(Vec<DVec3>),
    /// Each line moves from its `from` place to where it is (Offset).
    Morph,
    /// Lines drawn in from their ends (a new group's box).
    Draw,
}

/// One running effect.
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub kind: Kind,
    /// The lines it draws: the new ones, or for a Ghost the ones that went.
    pub segs: Vec<Seg>,
    /// Morph: where each line starts from.
    pub from: Vec<Seg>,
    /// Travel: which ends stay put (empty: none).
    pub pins: Vec<[bool; 2]>,
    /// When it started, in the session's time.
    pub start: f64,
}

impl Effect {
    pub fn new(kind: Kind, segs: Vec<Seg>, start: f64) -> Effect {
        Effect {
            kind,
            segs,
            from: Vec::new(),
            pins: Vec::new(),
            start,
        }
    }
}

/// Every line piece the scene draws: edges (arcs as their drawn points)
/// that are not hidden ghosts and not cut away by a section plane.
pub fn segments(scene: &Scene) -> Vec<Seg> {
    let mut out = Vec::new();
    for e in &scene.edges {
        if e.ghost {
            continue;
        }
        let mut prev = e.a;
        for p in e.mid.iter().copied().chain(std::iter::once(e.b)) {
            let m = (prev + p) * 0.5;
            if scene.kept(m) {
                out.push([prev, p]);
            }
            prev = p;
        }
    }
    out
}

type Key = [i64; 3];

fn key(p: DVec3, h: f64) -> Key {
    let q = p / h;
    [q.x.round() as i64, q.y.round() as i64, q.z.round() as i64]
}

fn seg_key(s: &Seg, h: f64) -> (Key, Key) {
    let (a, b) = (key(s[0], h), key(s[1], h));
    if a <= b { (a, b) } else { (b, a) }
}

/// A straight line's key: its direction (sign fixed) and its point
/// nearest the origin, so pieces of one line share it.
fn line_key(s: &Seg, h: f64) -> Option<(Key, Key, DVec3)> {
    let mut d = (s[1] - s[0]).try_normalize()?;
    let first = if d.x.abs() > 1e-9 {
        d.x
    } else if d.y.abs() > 1e-9 {
        d.y
    } else {
        d.z
    };
    if first < 0.0 {
        d = -d;
    }
    let foot = s[0] - d * s[0].dot(d);
    Some((key(d, 1e-6), key(foot, h * 4.0), d))
}

/// The pieces in `after` that are new, and those in `before` that went.
/// A piece of an old line (an edge split in two) is not new.
pub fn diff(before: &[Seg], after: &[Seg], tol: f64) -> (Vec<Seg>, Vec<Seg>) {
    let h = tol.max(f64::MIN_POSITIVE);
    let old: std::collections::BTreeSet<_> = before.iter().map(|s| seg_key(s, h)).collect();
    let now: std::collections::BTreeSet<_> = after.iter().map(|s| seg_key(s, h)).collect();
    let made: Vec<Seg> = after
        .iter()
        .filter(|s| !old.contains(&seg_key(s, h)))
        .copied()
        .collect();
    let gone: Vec<Seg> = before
        .iter()
        .filter(|s| !now.contains(&seg_key(s, h)))
        .copied()
        .collect();
    (within(&made, before, h), within(&gone, after, h))
}

/// Drop the pieces of `segs` that lie along a line piece of `other`.
fn within(segs: &[Seg], other: &[Seg], h: f64) -> Vec<Seg> {
    if segs.is_empty() {
        return Vec::new();
    }
    let mut lines: BTreeMap<(Key, Key), Vec<(f64, f64)>> = BTreeMap::new();
    for s in other {
        if let Some((k, f, d)) = line_key(s, h) {
            let (a, b) = (s[0].dot(d), s[1].dot(d));
            lines.entry((k, f)).or_default().push((a.min(b), a.max(b)));
        }
    }
    // Join pieces that touch, so a line split in two still covers itself.
    for v in lines.values_mut() {
        v.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut joined: Vec<(f64, f64)> = Vec::with_capacity(v.len());
        for &(p, q) in v.iter() {
            match joined.last_mut() {
                Some(last) if p <= last.1 + h => last.1 = last.1.max(q),
                _ => joined.push((p, q)),
            }
        }
        *v = joined;
    }
    segs.iter()
        .filter(|s| {
            let Some((k, f, d)) = line_key(s, h) else {
                return false;
            };
            let (a, b) = (s[0].dot(d), s[1].dot(d));
            let (lo, hi) = (a.min(b), a.max(b));
            !lines
                .get(&(k, f))
                .is_some_and(|v| v.iter().any(|(p, q)| *p <= lo + h && hi <= *q + h))
        })
        .copied()
        .collect()
}

/// About this many grid lines each way across a face.
pub const GRID_LINES: f64 = 10.0;

/// The grid lines of these faces' surfaces, from their drawn triangles:
/// each triangle's sides that run along the surface's own directions (its
/// unrolled coordinates), thinned to about [`GRID_LINES`] each way.
pub fn surface_grid(scene: &Scene, faces: &[usize]) -> Vec<Seg> {
    let m = &scene.mesh;
    let mut out = Vec::new();
    for &f in faces {
        let Some(face) = scene.faces.get(f) else {
            continue;
        };
        let r = face.tris.start as usize..face.tris.end as usize;
        let tris = m.indices.get(r).unwrap_or(&[]);
        // Each side once, with which way it runs (0: along u, at one v;
        // 1: along v, at one u) and where.
        let mut seen = std::collections::BTreeSet::new();
        let mut sides: Vec<(usize, f64, f64, Seg)> = Vec::new();
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for tri in tris.as_chunks::<3>().0 {
            for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                if !seen.insert((a.min(b), a.max(b))) {
                    continue;
                }
                let (Some(ua), Some(ub)) = (m.uvs.get(a as usize), m.uvs.get(b as usize)) else {
                    continue;
                };
                let d = *ub - *ua;
                // A cell's diagonal runs both ways at once: not a grid line.
                let (way, step) = if d.x.abs() > 4.0 * d.y.abs() {
                    (0, d.x.abs())
                } else if d.y.abs() > 4.0 * d.x.abs() {
                    (1, d.y.abs())
                } else {
                    continue;
                };
                let at = if way == 0 {
                    (ua.y + ub.y) * 0.5
                } else {
                    (ua.x + ub.x) * 0.5
                };
                for (k, c) in [ua.x, ua.y].into_iter().enumerate() {
                    lo[k] = lo[k].min(c);
                    hi[k] = hi[k].max(c);
                }
                sides.push((
                    way,
                    at,
                    step,
                    [m.positions[a as usize], m.positions[b as usize]],
                ));
            }
        }
        // A side along u sits at one v: keep the rows near every `gap`.
        for way in 0..2 {
            let across = 1 - way;
            let mut steps: Vec<f64> = sides
                .iter()
                .filter(|s| s.0 == across)
                .map(|s| s.2)
                .collect();
            steps.sort_by(f64::total_cmp);
            let Some(&cell) = steps.get(steps.len() / 2) else {
                continue;
            };
            let span = hi[across] - lo[across];
            let gap = (span / GRID_LINES).max(cell);
            for s in sides.iter().filter(|s| s.0 == way) {
                let x = s.1 - lo[across];
                if (x - gap * (x / gap).round()).abs() <= cell * 0.5 {
                    out.push(s.3);
                }
            }
        }
    }
    out
}

/// The longest way across a set of lines: the world axis its box is
/// widest along.
pub fn long_axis(segs: &[Seg]) -> DVec3 {
    let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
    for p in segs.iter().flatten() {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let w = hi - lo;
    if w.x >= w.y && w.x >= w.z {
        DVec3::X
    } else if w.y >= w.z {
        DVec3::Y
    } else {
        DVec3::Z
    }
}

/// Which ends of `segs` are still there after an edit (a Travel leaves
/// them where they are: the far end of an edge stretched by a Move).
pub fn pins(segs: &[Seg], after: &[Seg], tol: f64) -> Vec<[bool; 2]> {
    let h = tol.max(f64::MIN_POSITIVE);
    let here: std::collections::BTreeSet<Key> =
        after.iter().flatten().map(|p| key(*p, h)).collect();
    segs.iter()
        .map(|s| [here.contains(&key(s[0], h)), here.contains(&key(s[1], h))])
        .collect()
}

/// The nearest point to `p` on any of `segs`.
pub fn nearest_on(segs: &[Seg], p: DVec3) -> Option<DVec3> {
    segs.iter()
        .map(|s| {
            let d = s[1] - s[0];
            let l2 = d.length_squared();
            let t = if l2 > 0.0 {
                ((p - s[0]).dot(d) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            s[0] + d * t
        })
        .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
}

/// Ease out: fast at first, settling at the end.
fn ease(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn alpha(c: Rgba, a: f64) -> Rgba {
    c.with_alpha((a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

const WIDTH: f32 = 2.0;
const TIP_WIDTH: f32 = 3.5;

fn line(a: DVec3, b: DVec3, color: Rgba, width: f32) -> Line {
    Line {
        a,
        b,
        color,
        width,
        dash: [0.0, 0.0],
    }
}

/// The overlay lines for an effect at `t` (0 to 1 of its length).
/// Nothing at `t >= 1`. With `reduced`, the lines only fade in place.
pub fn lines(e: &Effect, t: f64, reduced: bool, color: Rgba) -> Vec<Line> {
    if !(0.0..1.0).contains(&t) {
        return Vec::new();
    }
    let solid = |a: f64| {
        e.segs
            .iter()
            .map(|s| line(s[0], s[1], alpha(color, a), WIDTH))
            .collect::<Vec<_>>()
    };
    if reduced {
        return solid(0.8 * (1.0 - t));
    }
    let (k, fade) = phase(t);
    let c = alpha(color, 0.8 * fade);
    match &e.kind {
        Kind::Echo => echo(&e.segs, t, color),
        Kind::Ghost => solid(0.7 * (1.0 - ease(t))),
        Kind::Trace => trace(&e.segs, t, color),
        Kind::Sweep(v) => sweep(&e.segs, *v, t, color),
        Kind::Tile(dir) => tile(&e.segs, *dir, t, color),
        Kind::Travel(turn) => {
            let a = turn.at(k);
            e.segs
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let pin = e.pins.get(i).copied().unwrap_or([false; 2]);
                    let p = |j: usize| {
                        if pin[j] {
                            s[j]
                        } else {
                            a.transform_point3(s[j])
                        }
                    };
                    line(p(0), p(1), c, WIDTH)
                })
                .collect()
        }
        Kind::Follow(path) => follow(&e.segs, path, k, c),
        Kind::Morph => e
            .segs
            .iter()
            .zip(e.from.iter().chain(std::iter::repeat(&[DVec3::ZERO; 2])))
            .map(|(s, f)| line(f[0].lerp(s[0], k), f[1].lerp(s[1], k), c, WIDTH))
            .collect(),
        Kind::Draw => e
            .segs
            .iter()
            .flat_map(|s| {
                let m = (s[0] + s[1]) * 0.5;
                [
                    line(s[0], s[0].lerp(m, k), c, WIDTH),
                    line(s[1], s[1].lerp(m, k), c, WIDTH),
                ]
            })
            .collect(),
    }
}

/// The part of an effect that moves, then the part that fades: how far
/// it has moved (eased, 0 to 1) and how bright it still is.
fn phase(t: f64) -> (f64, f64) {
    let k = ease(t / TRAVEL);
    let fade = if t < TRAVEL {
        1.0
    } else {
        1.0 - (t - TRAVEL) / (1.0 - TRAVEL)
    };
    (k, fade)
}

/// Lines a fraction `front` of the way across light up; those behind dim
/// to a trace, those ahead are not drawn yet.
fn tile(segs: &[Seg], dir: DVec3, t: f64, color: Rgba) -> Vec<Line> {
    let d = dir.normalize_or_zero();
    let at: Vec<f64> = segs.iter().map(|s| ((s[0] + s[1]) * 0.5).dot(d)).collect();
    let lo = at.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = at.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (hi - lo).max(f64::MIN_POSITIVE);
    let (_, fade) = phase(t);
    // An even roll (eased at both ends), not the quick start the others
    // have. The front runs a little past both ends, so the first and last
    // rows light up fully.
    let x = (t / TRAVEL).clamp(0.0, 1.0);
    let k = x * x * (3.0 - 2.0 * x);
    let front = k * (1.0 + 2.0 * TILE_BAND) - TILE_BAND;
    segs.iter()
        .zip(at)
        .filter_map(|(s, x)| {
            let behind = front - (x - lo) / span;
            if behind < 0.0 {
                return None;
            }
            let glow = (1.0 - behind / TILE_BAND).max(0.0);
            let a = (0.3 + 0.6 * glow) * fade;
            Some(line(s[0], s[1], alpha(color, a), WIDTH))
        })
        .collect()
}

/// How wide the bright band of a tiling is, as a part of the face.
const TILE_BAND: f64 = 0.25;

/// The profile carried `k` of the way along the path: moved to the path's
/// point there and turned as the path turns.
fn follow(segs: &[Seg], path: &[DVec3], k: f64, c: Rgba) -> Vec<Line> {
    let (Some(&p0), true) = (path.first(), path.len() >= 2) else {
        return Vec::new();
    };
    let len: f64 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = k * len;
    let n = path.len();
    let mut at = (path[n - 1], (path[n - 1] - path[n - 2]).normalize_or_zero());
    for w in path.windows(2) {
        let l = w[0].distance(w[1]);
        if l <= 0.0 {
            continue;
        }
        let dir = (w[1] - w[0]) / l;
        if left <= l {
            at = (w[0] + dir * left, dir);
            break;
        }
        left -= l;
    }
    let t0 = (path[1] - path[0]).normalize_or_zero();
    let turn = DQuat::from_rotation_arc(t0, at.1);
    let place = |q: DVec3| at.0 + turn * (q - p0);
    segs.iter()
        .map(|s| line(place(s[0]), place(s[1]), c, WIDTH))
        .collect()
}

fn echo(segs: &[Seg], t: f64, color: Rgba) -> Vec<Line> {
    let e = ease(t);
    let flash = alpha(color, 0.9 * (1.0 - t).powi(2));
    let ring = alpha(color, 0.6 * (1.0 - e));
    let mut out: Vec<Line> = segs
        .iter()
        .map(|s| line(s[0], s[1], flash, WIDTH))
        .collect();
    // Each connected piece grows about its own centre.
    let k = 1.0 + GROW * e;
    for (piece, c) in pieces(segs) {
        for i in piece {
            let s = segs[i];
            out.push(line(c + (s[0] - c) * k, c + (s[1] - c) * k, ring, WIDTH));
        }
    }
    out
}

/// The time a trace takes to run; the rest it fades.
const RUN: f64 = 0.6;

fn trace(segs: &[Seg], t: f64, color: Rgba) -> Vec<Line> {
    let run = ease(t / RUN);
    let fade = if t < RUN {
        1.0
    } else {
        1.0 - (t - RUN) / (1.0 - RUN)
    };
    let body = alpha(color, 0.85 * fade);
    let tip = alpha(color, fade);
    let mut out = Vec::new();
    for chain in chains(segs) {
        let len: f64 = chain.windows(2).map(|w| w[0].distance(w[1])).sum();
        let upto = run * len;
        let tip_from = upto - 0.12 * len;
        let mut at = 0.0;
        for w in chain.windows(2) {
            let l = w[0].distance(w[1]);
            if at >= upto || l <= 0.0 {
                break;
            }
            let end = (upto - at).min(l);
            let p = |s: f64| w[0] + (w[1] - w[0]) * (s / l);
            out.push(line(w[0], p(end), body, WIDTH));
            if t < RUN && at + end > tip_from {
                let from = (tip_from - at).max(0.0);
                out.push(line(p(from), p(end), tip, TIP_WIDTH));
            }
            at += l;
        }
    }
    out
}

/// The time a sweep takes to travel; the rest it fades.
const TRAVEL: f64 = 0.7;

fn sweep(segs: &[Seg], v: DVec3, t: f64, color: Rgba) -> Vec<Line> {
    let back = v * (1.0 - ease(t / TRAVEL));
    let fade = if t < TRAVEL {
        1.0
    } else {
        1.0 - (t - TRAVEL) / (1.0 - TRAVEL)
    };
    let c = alpha(color, 0.8 * fade);
    let Some(dir) = v.try_normalize() else {
        return echo(segs, t, color);
    };
    segs.iter()
        .map(|s| {
            let along = (s[1] - s[0]).normalize_or_zero().dot(dir).abs();
            if along > 0.999 {
                // A side: from its base to the travelling outline.
                let (lo, hi) =
                    if s[0].dot(dir) * v.dot(dir).signum() <= s[1].dot(dir) * v.dot(dir).signum() {
                        (s[0], s[1])
                    } else {
                        (s[1], s[0])
                    };
                line(lo, hi - back, c, WIDTH)
            } else {
                line(s[0] - back, s[1] - back, c, WIDTH)
            }
        })
        .collect()
}

/// Connected pieces of line: each with the indices of its segments and
/// its centre (the middle of its pieces, weighted by length).
fn pieces(segs: &[Seg]) -> Vec<(Vec<usize>, DVec3)> {
    let h = scale(segs) * 1e-9;
    let mut parent: Vec<usize> = (0..segs.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut at: BTreeMap<Key, usize> = BTreeMap::new();
    for (i, s) in segs.iter().enumerate() {
        for p in s {
            match at.get(&key(*p, h)) {
                Some(&j) => {
                    let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                    parent[a.max(b)] = a.min(b);
                }
                None => {
                    at.insert(key(*p, h), i);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..segs.len() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    groups
        .into_values()
        .map(|g| {
            let (mut sum, mut w) = (DVec3::ZERO, 0.0);
            for &i in &g {
                let l = segs[i][0].distance(segs[i][1]);
                sum += (segs[i][0] + segs[i][1]) * 0.5 * l;
                w += l;
            }
            let c = if w > 0.0 { sum / w } else { segs[g[0]][0] };
            (g, c)
        })
        .collect()
}

/// The size of a set of lines (for a key grid that only joins ends that
/// meet).
fn scale(segs: &[Seg]) -> f64 {
    segs.iter()
        .flat_map(|s| s.iter())
        .fold(1.0_f64, |m, p| m.max(p.abs().max_element()))
}

/// The lines walked end to end: open chains from their ends, then loops.
pub fn chains(segs: &[Seg]) -> Vec<Vec<DVec3>> {
    let h = scale(segs) * 1e-9;
    let mut ends: BTreeMap<Key, Vec<usize>> = BTreeMap::new();
    for (i, s) in segs.iter().enumerate() {
        ends.entry(key(s[0], h)).or_default().push(i);
        ends.entry(key(s[1], h)).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    let walk = |from: usize, start: DVec3, used: &mut Vec<bool>| {
        let mut pts = vec![start];
        let mut i = from;
        let mut here = start;
        loop {
            used[i] = true;
            let s = segs[i];
            let next = if key(s[0], h) == key(here, h) {
                s[1]
            } else {
                s[0]
            };
            pts.push(next);
            here = next;
            let Some(&j) = ends
                .get(&key(here, h))
                .and_then(|v| v.iter().find(|&&j| !used[j]))
            else {
                break;
            };
            i = j;
        }
        pts
    };
    // Open chains start at an end that is not in the middle of a line.
    let starts: Vec<(Key, Vec<usize>)> = ends
        .iter()
        .filter(|(_, v)| v.len() != 2)
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    for (_, v) in starts {
        for i in v {
            if !used[i] {
                let s = segs[i];
                let p = if ends.get(&key(s[0], h)).is_some_and(|e| e.len() != 2) {
                    s[0]
                } else {
                    s[1]
                };
                out.push(walk(i, p, &mut used));
            }
        }
    }
    for i in 0..segs.len() {
        if !used[i] {
            out.push(walk(i, segs[i][0], &mut used));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: Rgba = Rgba::rgb(2, 206, 255);

    fn square() -> Vec<Seg> {
        let p = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(2.0, 2.0, 0.0),
            DVec3::new(0.0, 2.0, 0.0),
        ];
        (0..4).map(|i| [p[i], p[(i + 1) % 4]]).collect()
    }

    fn fx(kind: Kind, segs: Vec<Seg>) -> Effect {
        Effect::new(kind, segs, 0.0)
    }

    #[test]
    fn an_echo_flashes_grows_and_is_gone_at_the_end() {
        let e = fx(Kind::Echo, square());
        let start = lines(&e, 0.0, false, C);
        assert_eq!(start.len(), 8, "the lines and their copy");
        assert!(start.iter().all(|l| l.color.0[3] > 140), "bright at first");
        let mid = lines(&e, 0.5, false, C);
        // The copy at t = 0.5 has grown by GROW * ease(0.5) about (1, 1).
        let k = 1.0 + GROW * (1.0 - 0.5f64.powi(3));
        let c = DVec3::new(1.0, 1.0, 0.0);
        let grown = c + (DVec3::ZERO - c) * k;
        assert!(mid.iter().any(|l| l.a.distance(grown) < 1e-12));
        assert!(lines(&e, 1.0, false, C).is_empty(), "gone at the end");
        assert_eq!(mid, lines(&e, 0.5, false, C), "the same every time");
    }

    #[test]
    fn a_trace_runs_along_the_lines() {
        let e = fx(Kind::Trace, square());
        let early = lines(&e, 0.05, false, C);
        let late = lines(&e, 0.59, false, C);
        let len = |v: &[Line]| {
            v.iter()
                .filter(|l| l.width == WIDTH)
                .map(|l| l.a.distance(l.b))
                .sum::<f64>()
        };
        assert!(len(&early) < len(&late), "it runs on");
        assert!(early.iter().any(|l| l.width == TIP_WIDTH), "with a tip");
        let after = lines(&e, 0.8, false, C);
        assert!((len(&after) - 8.0).abs() < 1e-9, "then all of it fades");
        assert!(after.iter().all(|l| l.width == WIDTH));
    }

    #[test]
    fn a_sweep_travels_from_where_the_face_was() {
        let v = DVec3::new(0.0, 0.0, 3.0);
        let top: Vec<Seg> = square()
            .iter()
            .map(|s| [s[0] + v, s[1] + v])
            .chain([[DVec3::ZERO, v]])
            .collect();
        let e = fx(Kind::Sweep(v), top);
        let start = lines(&e, 0.0, false, C);
        assert!(
            start[..4].iter().all(|l| l.a.z.abs() < 1e-12),
            "the outline starts at the base"
        );
        assert!(
            start[4].a.distance(start[4].b) < 1e-12,
            "the side starts with no length"
        );
        let end = lines(&e, TRAVEL, false, C);
        assert!(
            end[..4].iter().all(|l| (l.a.z - 3.0).abs() < 1e-12),
            "and ends on top"
        );
    }

    #[test]
    fn a_ghost_and_reduced_motion_only_fade() {
        let g = fx(Kind::Ghost, square());
        let a = lines(&g, 0.2, false, C);
        let b = lines(&g, 0.6, false, C);
        assert_eq!(a.len(), 4);
        assert!(
            a.iter()
                .zip(&b)
                .all(|(x, y)| x.a == y.a && x.color.0[3] > y.color.0[3])
        );
        let e = fx(Kind::Echo, square());
        let r = lines(&e, 0.5, true, C);
        assert_eq!(r.len(), 4, "no copy grows");
        assert!(
            r.iter()
                .zip(square())
                .all(|(l, s)| l.a == s[0] && l.b == s[1])
        );
    }

    #[test]
    fn diff_finds_new_lines_but_not_split_ones() {
        let before = square();
        // A line across the square splits two of its sides.
        let m0 = DVec3::new(1.0, 0.0, 0.0);
        let m1 = DVec3::new(1.0, 2.0, 0.0);
        let mut after = vec![before[1], before[3]];
        after.extend([
            [before[0][0], m0],
            [m0, before[0][1]],
            [before[2][0], m1],
            [m1, before[2][1]],
            [m0, m1],
        ]);
        let (made, gone) = diff(&before, &after, 1e-6);
        assert_eq!(made, vec![[m0, m1]], "only the new line");
        assert!(gone.is_empty(), "the split sides did not go");
        let (made, gone) = diff(&after, &before, 1e-6);
        assert!(made.is_empty());
        assert_eq!(gone, vec![[m0, m1]]);
    }

    #[test]
    fn chains_walk_a_loop_and_an_open_line() {
        let mut segs = square();
        segs.push([DVec3::new(5.0, 0.0, 0.0), DVec3::new(6.0, 0.0, 0.0)]);
        segs.push([DVec3::new(6.0, 0.0, 0.0), DVec3::new(6.0, 1.0, 0.0)]);
        let c = chains(&segs);
        assert_eq!(c.len(), 2);
        assert!(c.iter().any(|p| p.len() == 3), "the open one, end to end");
        assert!(
            c.iter().any(|p| p.len() == 5 && p[0] == p[4]),
            "the loop closes"
        );
    }

    #[test]
    fn a_travel_turns_on_its_arc_and_keeps_its_pins() {
        let turn = Turn::Rotate {
            centre: DVec3::ZERO,
            axis: DVec3::Z,
            angle: std::f64::consts::PI,
        };
        let mut e = fx(
            Kind::Travel(turn),
            vec![[DVec3::X, DVec3::new(2.0, 0.0, 0.0)]],
        );
        e.pins = vec![[false, true]];
        let half = lines(&e, TRAVEL * 0.2, false, C);
        // Partway round, the moving end is off the line it starts and ends
        // on (an arc, not a slide), at its distance from the centre.
        assert!(half[0].a.y.abs() > 0.1 && (half[0].a.length() - 1.0).abs() < 1e-9);
        assert_eq!(half[0].b, DVec3::new(2.0, 0.0, 0.0), "the pinned end stays");
        let end = lines(&e, TRAVEL, false, C);
        assert!(
            (end[0].a + DVec3::X).length() < 1e-9,
            "it lands turned round"
        );
    }

    #[test]
    fn a_follow_carries_the_profile_round_the_corner() {
        let path = vec![
            DVec3::ZERO,
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(4.0, 3.0, 0.0),
        ];
        let up = [DVec3::ZERO, DVec3::Z];
        let e = fx(Kind::Follow(path), vec![up]);
        let start = lines(&e, 0.0, false, C);
        assert_eq!((start[0].a, start[0].b), (up[0], up[1]));
        let end = lines(&e, TRAVEL, false, C);
        assert!(
            (end[0].a - DVec3::new(4.0, 3.0, 0.0)).length() < 1e-9,
            "at the path's end"
        );
        assert!(
            (end[0].b - end[0].a - DVec3::Z).length() < 1e-9,
            "still upright"
        );
    }

    #[test]
    fn a_morph_and_a_draw_move_into_place() {
        let mut e = fx(Kind::Morph, square());
        e.from = square().iter().map(|s| [s[0] * 2.0, s[1] * 2.0]).collect();
        assert_eq!(
            lines(&e, 0.0, false, C)[1].a,
            e.from[1][0],
            "from the old place"
        );
        assert_eq!(
            lines(&e, TRAVEL, false, C)[1].a,
            e.segs[1][0],
            "to the new one"
        );
        let d = fx(Kind::Draw, vec![[DVec3::ZERO, DVec3::X]]);
        let at = |t| {
            lines(&d, t, false, C)
                .iter()
                .map(|l| l.a.distance(l.b))
                .sum::<f64>()
        };
        assert!(
            at(0.0) < 1e-12 && (at(TRAVEL) - 1.0).abs() < 1e-9,
            "drawn in from both ends"
        );
    }

    #[test]
    fn a_tiling_rolls_across() {
        let rows: Vec<Seg> = (0..10)
            .map(|i| {
                [
                    DVec3::new(f64::from(i), 0.0, 0.0),
                    DVec3::new(f64::from(i), 1.0, 0.0),
                ]
            })
            .collect();
        let e = fx(Kind::Tile(DVec3::X), rows);
        let n = |t| lines(&e, t, false, C).len();
        assert!(
            n(0.1) < n(0.4) && n(0.4) < n(0.69),
            "{} {} {}",
            n(0.1),
            n(0.4),
            n(0.69)
        );
        assert_eq!(n(0.9), 10, "all of it, fading");
    }

    #[test]
    fn pins_and_the_nearest_point() {
        let after = vec![[DVec3::ZERO, DVec3::Y]];
        let p = pins(&[[DVec3::ZERO, DVec3::X]], &after, 1e-9);
        assert_eq!(p, vec![[true, false]]);
        let n = nearest_on(&square(), DVec3::new(1.0, -3.0, 0.0));
        assert_eq!(n, Some(DVec3::new(1.0, 0.0, 0.0)));
    }
}
