//! Solid > Validate and Heal (Solids & CAD part 1, brief 37).
//!
//! - [`validate`]: each connected piece of faces, and whether it is a valid
//!   solid (closed, facing out, nothing crossing or of no size, and passed
//!   by the kernel's checker), with its volume or the reasons it is not.
//! - [`heal`]: Model Health's safe fixes in a fixed order, plus welding
//!   near misses and filling flat holes.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Curve, Surface};
use prism_math::{DVec3, Plane};
use prism_topo::{EdgeId, FaceId, Model, VertexId};

use crate::health::{self, Found};
use crate::regions::{Fill, rebuild_plane};

/// Said for a solid the bridge cannot send yet: a face with a sphere's
/// pole or a cone's tip inside it (not at a corner), or a curved face
/// that goes all the way round (other than a cylinder's).
pub const KERNEL_NOT_YET: &str = "A face round a pole or a tip, or one that goes all the way round, cannot go to the kernel yet: checked by Prism's rules only.";

/// Said for a solid whose freeform faces meet each other along many curve
/// pieces: the kernel's check of faces against faces takes too long there
/// yet (Solids & CAD part 3c).
pub const KERNEL_TOO_LONG: &str = "Freeform faces that meet along many curve pieces take the kernel too long to check yet: checked by Prism's rules only.";

/// The most [`kernel_work`] Validate sends to the kernel: a few seconds.
const KERNEL_BUDGET: f64 = 24.0;

/// What Validate says about one piece of faces.
#[derive(Clone, Debug, PartialEq)]
pub struct SolidCheck {
    pub faces: BTreeSet<FaceId>,
    /// Closed, facing out, and passed by the kernel (where it can go).
    pub valid: bool,
    /// The volume enclosed, when it is closed.
    pub volume: Option<f64>,
    /// Why it is not a valid solid, in plain words with counts.
    pub reasons: Vec<String>,
    /// Why the kernel did not check it (freeform faces, approximate
    /// edges): checked by Prism's rules only.
    pub kernel_skipped: Option<&'static str>,
}

/// The pieces of faces joined across edges, each ordered by its lowest face.
pub fn pieces(model: &Model) -> Vec<BTreeSet<FaceId>> {
    let ids: Vec<FaceId> = model.faces().map(|(id, _)| id).collect();
    let index: BTreeMap<FaceId, usize> = ids.iter().enumerate().map(|(i, f)| (*f, i)).collect();
    let mut parent: Vec<usize> = (0..ids.len()).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for fs in model.edge_faces().values() {
        for w in fs.windows(2) {
            let (a, b) = (
                find(&mut parent, index[&w[0]]),
                find(&mut parent, index[&w[1]]),
            );
            parent[a.max(b)] = a.min(b);
        }
    }
    let mut out: BTreeMap<usize, BTreeSet<FaceId>> = BTreeMap::new();
    for (i, f) in ids.iter().enumerate() {
        out.entry(find(&mut parent, i)).or_default().insert(*f);
    }
    out.into_values().collect()
}

/// Validate every piece of faces in the model.
pub fn validate(model: &Model) -> Vec<SolidCheck> {
    let r = health::check(model);
    let tol = model.tol().dist;
    let checks: Vec<SolidCheck> = pieces(model)
        .into_iter()
        .map(|faces| {
            let edges: BTreeSet<EdgeId> = faces
                .iter()
                .filter_map(|f| model.face(*f))
                .flat_map(|f| f.edge_ids().collect::<Vec<_>>())
                .collect();
            let verts: BTreeSet<VertexId> = faces
                .iter()
                .filter_map(|f| model.face(*f))
                .flat_map(|f| f.loops().flatten().copied().collect::<Vec<_>>())
                .collect();
            let count = |f: &Found| {
                f.faces.intersection(&faces).count()
                    + f.edges.intersection(&edges).count()
                    + f.vertices.intersection(&verts).count()
            };
            let open = count(&r.open_edges);
            let fans = count(&r.non_manifold);
            let mut reasons = Vec::new();
            let mut say = |n: usize, one: &str, many: &str| {
                if n > 0 {
                    reasons.push(format!("{n} {}", if n == 1 { one } else { many }));
                }
            };
            say(open, "open edge (a hole)", "open edges (holes)");
            say(
                fans,
                "edge with 3 or more faces",
                "edges with 3 or more faces",
            );
            say(
                r.reversed.faces.intersection(&faces).count(),
                "face pointing inward",
                "faces pointing inward",
            );
            say(
                r.interior.faces.intersection(&faces).count(),
                "face inside the solid",
                "faces inside the solid",
            );
            say(
                r.crossing.faces.intersection(&faces).count(),
                "face crossing another with no edge",
                "faces crossing others with no edge",
            );
            say(
                count(&r.degenerate),
                "piece of no size",
                "pieces of no size",
            );
            say(
                count(&r.off_circle),
                "point off its curve",
                "points off their curves",
            );
            say(
                count(&r.near_misses),
                "point almost touching another",
                "points almost touching others",
            );
            let closed = open == 0 && fans == 0;
            let volume = closed.then(|| {
                faces
                    .iter()
                    .map(|f| health::face_volume(model, *f))
                    .sum::<f64>()
            });
            let mut kernel_skipped = None;
            // A hollow (a shell facing into itself, which Model Health
            // found inside another solid) is checked with the solid around
            // it, below; the kernel takes one shell at a time.
            let hollow = volume.is_some_and(|v| v < 0.0);
            if hollow {
            } else if reasons.is_empty() && !faces.iter().all(|f| kernel_takes(model, *f)) {
                kernel_skipped = Some(KERNEL_NOT_YET);
            } else if reasons.is_empty() && kernel_work(model, &faces) > KERNEL_BUDGET {
                kernel_skipped = Some(KERNEL_TOO_LONG);
            } else if reasons.is_empty() {
                let found = crate::bridge::region(model, &faces)
                    .and_then(|region| prism_kernel::solid::Solid::faults(&region, tol));
                match found {
                    Ok(words) => reasons.extend(words),
                    Err(why)
                        if why == crate::bridge::APPROXIMATE
                            || why == prism_kernel::solid::SURFACE_NOT_YET =>
                    {
                        kernel_skipped = Some(why);
                    }
                    Err(why) => reasons.push(why.trim_start_matches("Refused: ").to_owned()),
                }
            }
            SolidCheck {
                faces,
                valid: reasons.is_empty(),
                volume,
                reasons,
                kernel_skipped,
            }
        })
        .collect();
    with_hollows(model, checks)
}

/// Each hollow joined to the solid around it: the smallest one whose box
/// holds it.
fn with_hollows(model: &Model, checks: Vec<SolidCheck>) -> Vec<SolidCheck> {
    let bounds = |c: &SolidCheck| {
        let pts: Vec<DVec3> = c
            .faces
            .iter()
            .filter_map(|f| model.face(*f))
            .flat_map(|f| model.points(&f.outer))
            .collect();
        let first = pts.first().copied().unwrap_or(DVec3::ZERO);
        pts.iter()
            .fold((first, first), |(l, h), p| (l.min(*p), h.max(*p)))
    };
    let is_hollow = |c: &SolidCheck| c.volume.is_some_and(|v| v < 0.0);
    let (hollows, mut out): (Vec<SolidCheck>, Vec<SolidCheck>) =
        checks.into_iter().partition(|c| is_hollow(c));
    for h in hollows {
        let (lo, hi) = bounds(&h);
        let around = out
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                let (l, u) = bounds(c);
                l.cmple(lo).all() && hi.cmple(u).all()
            })
            .min_by(|a, b| {
                a.1.volume
                    .unwrap_or(0.0)
                    .total_cmp(&b.1.volume.unwrap_or(0.0))
            })
            .map(|(i, _)| i);
        match around {
            Some(i) => {
                let c = &mut out[i];
                c.faces.extend(h.faces);
                c.volume = c.volume.zip(h.volume).map(|(a, b)| a + b);
                c.reasons.extend(h.reasons);
                c.valid = c.valid && h.valid;
            }
            None => out.push(SolidCheck {
                valid: false,
                reasons: vec!["the whole solid faces inward".to_owned()],
                ..h
            }),
        }
    }
    out.sort_by_key(|c| c.faces.first().copied());
    out
}

/// Whether the bridge can send this face to the kernel's checker yet.
fn kernel_takes(model: &Model, f: FaceId) -> bool {
    let Some(face) = model.face(f) else {
        return false;
    };
    let tol = model.tol().dist;
    let laid = crate::crossing::Laid::new(model, f);
    // A point where the surface's parameters pinch (a pole, a tip) inside
    // the face, not at a corner of it: the bridge cannot send it yet.
    let pinched = |p: DVec3| {
        !face
            .loops()
            .flatten()
            .any(|v| model.vertex(*v).distance(p) <= tol)
            && laid
                .as_ref()
                .is_none_or(|l| l.classify(p, tol) != Some(false))
    };
    let wraps = laid.as_ref().is_none_or(|l| l.wraps());
    match &face.surface {
        Surface::Plane(_) | Surface::Cylinder(_) => true,
        Surface::Cone(c) => !pinched(c.apex),
        Surface::Sphere(x) => {
            !wraps
                && !pinched(x.centre + x.axis * x.radius)
                && !pinched(x.centre - x.axis * x.radius)
        }
        Surface::Torus(_) | Surface::Nurbs(_) => !wraps,
    }
}

/// About how long the kernel takes to check `faces`, in tenths of a
/// second. Where two freeform faces meet, it tests each one's edge
/// against the other piece by piece (an edge lying on a face is its
/// slowest case), so each edge between two freeform faces costs about
/// its own pieces for each face, more for a higher degree. Edges with
/// flat or simply curved faces cost next to nothing.
fn kernel_work(model: &Model, faces: &BTreeSet<FaceId>) -> f64 {
    let patch = |f: &FaceId| model.face(*f).and_then(|x| x.surface.patch());
    let mut work = 0.0;
    for (e, by) in model.edge_faces() {
        let [a, b] = by[..] else {
            continue;
        };
        let (Some(pa), Some(pb), Some(edge)) = (patch(&a), patch(&b), model.edge(e)) else {
            continue;
        };
        if !(faces.contains(&a) && faces.contains(&b)) {
            continue;
        }
        let pieces = match &edge.curve {
            Curve::Spline(sp) => sp.curve.spans() as f64,
            _ => 1.0,
        };
        for p in [pa, pb] {
            let d = p.surface.degree();
            let d = d[0].max(d[1]) as f64 + 1.0;
            work += 2.0 * pieces * d.powi(3) / 64.0;
        }
    }
    work
}

/// What Heal did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Healed {
    pub welded: usize,
    pub merged: usize,
    pub repaired: usize,
    pub filled: usize,
    pub turned: usize,
    pub removed: usize,
    pub stray: usize,
}

impl Healed {
    pub fn is_empty(&self) -> bool {
        *self == Healed::default()
    }

    /// In words: "Welded 4 points, filled 1 hole".
    pub fn words(&self) -> String {
        let parts: Vec<String> = [
            (self.welded, "Welded", "point", "points"),
            (self.merged, "merged", "repeated face", "repeated faces"),
            (self.repaired, "put back", "curve", "curves"),
            (self.filled, "filled", "hole", "holes"),
            (self.turned, "turned", "face", "faces"),
            (self.removed, "removed", "inside face", "inside faces"),
            (self.stray, "removed", "stray edge", "stray edges"),
        ]
        .into_iter()
        .filter(|p| p.0 > 0)
        .map(|(n, verb, one, many)| format!("{verb} {n} {}", if n == 1 { one } else { many }))
        .collect();
        if parts.is_empty() {
            return "Nothing needed healing".to_owned();
        }
        let mut s = parts.join(", ");
        if let Some(c) = s.get(0..1) {
            s.replace_range(0..1, &c.to_uppercase());
        }
        s
    }
}

/// Heal the pieces holding `only` (every piece if None), in this order:
/// weld near misses, merge repeated faces, put arcs and curved faces back
/// on their curves, fill flat holes, orient faces, remove interior faces,
/// and (for the whole model) stray edges.
pub fn heal(model: &mut Model, only: Option<&BTreeSet<FaceId>>) -> Healed {
    let tol = model.tol();
    let mut h = Healed::default();
    let scope = |model: &Model| -> Option<BTreeSet<FaceId>> {
        let seeds = only?;
        Some(
            pieces(model)
                .into_iter()
                .filter(|p| !p.is_disjoint(seeds))
                .flatten()
                .collect(),
        )
    };
    // 1. Weld points closer than ten tolerances (Model Health's near
    //    misses); the lower id stays.
    let faces = scope(model);
    let in_scope: Option<BTreeSet<VertexId>> = faces.as_ref().map(|fs| {
        fs.iter()
            .filter_map(|f| model.face(*f))
            .flat_map(|f| f.loops().flatten().copied().collect::<Vec<_>>())
            .collect()
    });
    let mut into: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for (a, b, _) in health::close_points(model, tol.dist * 10.0) {
        if in_scope
            .as_ref()
            .is_some_and(|s| !s.contains(&a) || !s.contains(&b))
        {
            continue;
        }
        let root = |mut v: VertexId, into: &BTreeMap<VertexId, VertexId>| {
            while let Some(w) = into.get(&v) {
                v = *w;
            }
            v
        };
        let (ra, rb) = (root(a, &into), root(b, &into));
        if ra != rb {
            into.insert(ra.max(rb), ra.min(rb));
        }
    }
    let flat: BTreeMap<VertexId, VertexId> = into
        .keys()
        .map(|v| {
            let mut w = *v;
            while let Some(x) = into.get(&w) {
                w = *x;
            }
            (*v, w)
        })
        .collect();
    h.welded = flat.len();
    crate::transform::apply_weld(model, &flat);
    // 2. Repeated faces.
    let before = model.faces().count();
    crate::settle::dedupe_faces(model);
    h.merged = before - model.faces().count();
    // 3. Curves.
    if !health::check(model).off_circle.is_empty() {
        h.repaired = health::repair_arcs(model);
    }
    // 4. Flat holes.
    let faces = scope(model);
    h.filled = fill_flat_holes(model, faces.as_ref());
    // 5 and 6. Orient, then interior faces.
    let faces = scope(model);
    h.turned = health::orient(model, faces.as_ref());
    h.removed = health::clean_envelope(model, faces.as_ref());
    if only.is_none() {
        h.stray = health::remove_stray(model);
    }
    h
}

/// Give every loop of open edges that lies flat a face (in `only`'s
/// pieces, or everywhere). Returns how many faces were made.
fn fill_flat_holes(model: &mut Model, only: Option<&BTreeSet<FaceId>>) -> usize {
    let tol = model.tol();
    let edge_faces = model.edge_faces();
    let open: Vec<EdgeId> = edge_faces
        .iter()
        .filter(|(_, fs)| fs.len() == 1 && only.is_none_or(|s| s.contains(&fs[0])))
        .map(|(e, _)| *e)
        .collect();
    // Each point's open edges.
    let mut at: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for e in &open {
        if let Some(ed) = model.edge(*e) {
            at.entry(ed.a).or_default().push(*e);
            at.entry(ed.b).or_default().push(*e);
        }
    }
    // Loops, walked where each point has exactly two open edges.
    let mut used: BTreeSet<EdgeId> = BTreeSet::new();
    let mut planes: Vec<(Plane, BTreeSet<EdgeId>)> = Vec::new();
    for start in &open {
        if used.contains(start) {
            continue;
        }
        let Some(first) = model.edge(*start) else {
            continue;
        };
        let (begin, mut v) = (first.a, first.b);
        let mut ring = vec![*start];
        let mut pts = vec![model.vertex(first.a)];
        let mut e = *start;
        let closed = loop {
            pts.extend(
                model
                    .edge(e)
                    .map(|x| {
                        let (p, q) = if x.a == v {
                            (model.vertex(x.b), model.vertex(x.a))
                        } else {
                            (model.vertex(x.a), model.vertex(x.b))
                        };
                        x.curve
                            .inner_points(p, q)
                            .into_iter()
                            .chain([q])
                            .collect::<Vec<DVec3>>()
                    })
                    .unwrap_or_default(),
            );
            if v == begin {
                break true;
            }
            let next: Vec<EdgeId> = at
                .get(&v)
                .map(|es| es.iter().copied().filter(|x| *x != e).collect())
                .unwrap_or_default();
            let [n] = next.as_slice() else { break false };
            if ring.contains(n) {
                break false;
            }
            ring.push(*n);
            e = *n;
            v = model
                .edge(e)
                .map_or(v, |x| if x.a == v { x.b } else { x.a });
        };
        used.extend(ring.iter().copied());
        if !closed || ring.len() < 2 {
            continue;
        }
        let normal = prism_math::newell(&pts);
        let Some(n) = normal.try_normalize() else {
            continue;
        };
        let centre = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
        let plane = Plane::new(centre, n);
        let flat = pts.iter().all(|p| plane.contains(*p, &tol))
            && ring.iter().all(|e| match model.edge(*e).map(|x| &x.curve) {
                Some(Curve::Line) => true,
                Some(Curve::Arc { axis, .. }) => tol.parallel(*axis, n),
                _ => false,
            });
        if !flat {
            continue;
        }
        match planes
            .iter_mut()
            .find(|(p, _)| p.same_surface(&plane, &tol))
        {
            Some((_, es)) => es.extend(ring),
            None => planes.push((plane, ring.into_iter().collect())),
        }
    }
    planes
        .iter()
        .map(|(plane, edges)| {
            rebuild_plane(model, plane, edges, &tol, Fill::Closing(None), 0.0).len()
        })
        .sum()
}
