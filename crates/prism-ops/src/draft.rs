//! Draft Angle (Shapes & Placement part 6): lean the chosen flat faces of a
//! solid by an angle about the line where each meets a neutral plane, as a
//! mould or a casting needs so the part comes out. The corners follow: each
//! is where its three faces now meet.

use std::collections::{BTreeMap, BTreeSet};

use prism_math::{DVec3, Plane};
use prism_topo::{FaceId, Model, VertexId};

pub const NOT_FLAT: &str = "Draft Angle works on flat faces";
pub const ANGLE_RANGE: &str = "A draft angle is between -45 and 45 degrees, and not zero";
pub const PARALLEL: &str = "A face parallel to the neutral plane has nothing to lean about";
pub const CORNER: &str =
    "A corner of these faces meets other than three faces: Draft Angle needs three at each";
pub const CANNOT: &str = "These faces cannot be leaned by that much (their corners no longer meet)";
pub const NOTHING: &str = "Select the faces to lean first";

/// The most a face may lean either way.
const MAX_DEGREES: f64 = 45.0;

/// The new plane of a face leaned `angle` about where it meets the
/// neutral plane (`pull` is its normal, pointing the way the part comes
/// out): positive angles make the part narrower the further along `pull`.
fn leaned(face: &Plane, neutral: &Plane, angle: f64, centre: DVec3) -> Result<Plane, &'static str> {
    let pull = neutral.normal;
    let n = face.normal;
    let hinge = n.cross(pull);
    if hinge.length() < 1e-9 {
        return Err(PARALLEL);
    }
    // The point on both planes nearest the face's middle.
    let (a, b) = (n.dot(pull), 1.0);
    let rhs = [
        n.dot(face.origin - centre),
        pull.dot(neutral.origin - centre),
    ];
    // centre + x n + y pull, with n.(x n + y pull) = rhs0 and pull.(...) = rhs1.
    let det = b - a * a;
    if det.abs() < 1e-12 {
        return Err(PARALLEL);
    }
    let x = (rhs[0] - a * rhs[1]) / det;
    let y = (rhs[1] - a * rhs[0]) / det;
    let q = centre + n * x + pull * y;
    // Turned about the hinge so its normal gains a share of `pull`.
    let lean = n * angle.cos() + (pull - n * pull.dot(n)).normalize_or_zero() * angle.sin();
    Ok(Plane::new(q, lean.normalize()))
}

fn same(a: &Plane, b: &Plane, tol: f64) -> bool {
    a.normal.dot(b.normal) > 1.0 - 1e-9 && (a.normal.dot(b.origin - a.origin)).abs() <= tol
}

/// The point where three planes meet, if they do at one point.
fn meet3(p: &[Plane; 3]) -> Option<DVec3> {
    let (n0, n1, n2) = (p[0].normal, p[1].normal, p[2].normal);
    let det = n0.dot(n1.cross(n2));
    if det.abs() < 1e-9 {
        return None;
    }
    let d = [
        n0.dot(p[0].origin),
        n1.dot(p[1].origin),
        n2.dot(p[2].origin),
    ];
    Some((n1.cross(n2) * d[0] + n2.cross(n0) * d[1] + n0.cross(n1) * d[2]) / det)
}

/// Lean `faces` by `angle` (radians) about the neutral plane (through
/// `origin`, `pull` along which the part comes out). Returns how many
/// faces leaned.
pub fn draft(
    model: &mut Model,
    faces: &BTreeSet<FaceId>,
    origin: DVec3,
    pull: DVec3,
    angle: f64,
) -> Result<usize, &'static str> {
    if faces.is_empty() {
        return Err(NOTHING);
    }
    if angle == 0.0 || angle.is_nan() || angle.abs() > MAX_DEGREES.to_radians() {
        return Err(ANGLE_RANGE);
    }
    let pull = pull.normalize_or_zero();
    if pull == DVec3::ZERO {
        return Err(PARALLEL);
    }
    let neutral = Plane::new(origin, pull);
    let tol = model.tol().dist;
    // The planes of every flat face now, and the leaned ones.
    let mut planes: BTreeMap<FaceId, Plane> = BTreeMap::new();
    for (id, f) in model.faces() {
        if let Some(p) = f.plane() {
            planes.insert(id, *p);
        } else if faces.contains(&id) {
            return Err(NOT_FLAT);
        }
    }
    let mut now = planes.clone();
    for id in faces {
        let f = model.face(*id).ok_or(NOT_FLAT)?;
        let old = *planes.get(id).ok_or(NOT_FLAT)?;
        let pts = model.loop_points(f, 0);
        let centre = pts.iter().copied().sum::<DVec3>() / pts.len().max(1) as f64;
        now.insert(*id, leaned(&old, &neutral, angle, centre)?);
    }
    // Every corner of a leaned face is where its faces now meet.
    let mut corners: BTreeMap<VertexId, Vec<FaceId>> = BTreeMap::new();
    for (id, f) in model.faces() {
        for v in f.loops().flatten() {
            corners.entry(*v).or_default().push(id);
        }
    }
    let mut moved: BTreeMap<VertexId, DVec3> = BTreeMap::new();
    for (v, fs) in &corners {
        if !fs.iter().any(|f| faces.contains(f)) {
            continue;
        }
        let mut distinct: Vec<Plane> = Vec::new();
        for f in fs {
            let p = now.get(f).ok_or(NOT_FLAT)?;
            if !distinct.iter().any(|d| same(d, p, tol)) {
                distinct.push(*p);
            }
        }
        let [a, b, c] = distinct.as_slice() else {
            return Err(CORNER);
        };
        let at = meet3(&[*a, *b, *c]).ok_or(CANNOT)?;
        // A corner may not fly off: no more than a few times the part's size.
        if at.distance(model.vertex(*v)) > 1e3 {
            return Err(CANNOT);
        }
        moved.insert(*v, at);
    }
    for (v, p) in &moved {
        model.set_vertex(*v, *p);
    }
    // Every corner is on each of its faces' planes now, so only the leaned
    // faces take a new plane.
    for id in faces {
        if let Some(p) = now.get(id) {
            let plane = prism_geom::Surface::Plane(*p);
            model.edit_face(*id, |f| f.surface = plane);
        }
    }
    Ok(faces.len())
}

/// Why a draft is refused, trying it on a copy.
pub fn refusal(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    origin: DVec3,
    pull: DVec3,
    angle: f64,
) -> Option<&'static str> {
    draft(&mut model.clone(), faces, origin, pull, angle).err()
}
