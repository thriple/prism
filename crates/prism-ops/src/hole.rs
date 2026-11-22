//! Holes (Shapes & Placement part 6): the cutters for a plain, counterbored
//! or countersunk hole, as exact solids of revolution about the z axis,
//! the surface at z = 0 and the hole going down. The Hole tool subtracts one
//! from a solid (a live boolean, so it stays editable).

use prism_geom::Surface;
use prism_math::{DVec3, Plane};
use prism_topo::{EdgeId, Face, Model, VertexId};

/// What a hole is.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HoleKind {
    Plain,
    /// A wider step at the top: its diameter and how deep it goes.
    Counterbore {
        diameter: f64,
        depth: f64,
    },
    /// A cone at the top: its diameter at the surface and the angle
    /// between its sides (degrees; 90 is usual).
    Countersink {
        diameter: f64,
        angle: f64,
    },
}

/// A hole: its diameter and depth (None goes right through) and its kind.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HoleSpec {
    pub diameter: f64,
    pub depth: Option<f64>,
    pub kind: HoleKind,
}

impl Default for HoleSpec {
    fn default() -> Self {
        HoleSpec {
            diameter: 0.01,
            depth: Some(0.02),
            kind: HoleKind::Plain,
        }
    }
}

pub const TOO_SMALL: &str = "A hole needs a size above 0.01 mm";
pub const BORE_SMALLER: &str = "A counterbore must be wider than the hole";
pub const BORE_DEEPER: &str = "A counterbore must be shallower than the hole";
pub const SINK_SMALLER: &str = "A countersink must be wider than the hole";
pub const SINK_ANGLE: &str = "A countersink's angle is between 10 and 170 degrees";
pub const SINK_DEEPER: &str = "A countersink must be shallower than the hole";

impl HoleSpec {
    /// Why the hole is not a hole, if it is not. `through` is the depth
    /// used when the hole goes right through (the body's size).
    pub fn check(&self, through: f64) -> Result<(), &'static str> {
        let depth = self.depth.unwrap_or(through);
        let small = |v: f64| v.is_nan() || v < 1e-5;
        if small(self.diameter) || small(depth) {
            return Err(TOO_SMALL);
        }
        match self.kind {
            HoleKind::Plain => {}
            HoleKind::Counterbore {
                diameter,
                depth: bore,
            } => {
                if small(diameter) || small(bore) {
                    return Err(TOO_SMALL);
                }
                if diameter <= self.diameter {
                    return Err(BORE_SMALLER);
                }
                if bore >= depth {
                    return Err(BORE_DEEPER);
                }
            }
            HoleKind::Countersink { diameter, angle } => {
                if small(diameter) {
                    return Err(TOO_SMALL);
                }
                if diameter <= self.diameter {
                    return Err(SINK_SMALLER);
                }
                if !(10.0..=170.0).contains(&angle) {
                    return Err(SINK_ANGLE);
                }
                if sink_depth(self.diameter, diameter, angle) >= depth {
                    return Err(SINK_DEEPER);
                }
            }
        }
        Ok(())
    }

    /// The solid to take out: about the z axis, the surface at z = 0, going
    /// down `depth` (or `through` if the hole goes through), and a little
    /// above the surface so the cut is clean.
    pub fn cutter(&self, through: f64) -> Model {
        let depth = self.depth.unwrap_or(through);
        let r = self.diameter / 2.0;
        let above = (self.diameter * 0.25).max(1e-4);
        let rings: Vec<(f64, f64)> = match self.kind {
            HoleKind::Plain => vec![(-depth, r), (above, r)],
            HoleKind::Counterbore {
                diameter,
                depth: bore,
            } => vec![
                (-depth, r),
                (-bore, r),
                (-bore, diameter / 2.0),
                (above, diameter / 2.0),
            ],
            HoleKind::Countersink { diameter, angle } => {
                let d = sink_depth(self.diameter, diameter, angle);
                vec![
                    (-depth, r),
                    (-d, r),
                    (0.0, diameter / 2.0),
                    (above, diameter / 2.0),
                ]
            }
        };
        let mut m = lathe(&rings);
        crate::health::orient(&mut m, None);
        m
    }
}

/// How far down a countersink goes to narrow from `wide` to `narrow`.
fn sink_depth(narrow: f64, wide: f64, angle: f64) -> f64 {
    (wide - narrow) / 2.0 / (angle.to_radians() / 2.0).tan()
}

/// An arc from `a` to `b`, counter-clockwise about +z.
fn ring_arc(m: &mut Model, a: VertexId, b: VertexId, z: f64, curve: prism_topo::CurveId) -> EdgeId {
    let centre = DVec3::Z * z;
    let radius = m.vertex(a).distance(centre);
    m.add_curve_edge(
        a,
        b,
        prism_geom::Curve::Arc {
            centre,
            axis: DVec3::Z,
            radius,
        },
        Some(curve),
    )
    .expect("an arc")
}

/// A solid of revolution about z from rings `(z, radius)` listed from the
/// bottom up: a disc at each end and between rings a cylinder (same
/// radius), a cone (radii differ) or a flat step (same height). Every
/// seam is at +x and -x, the rings in two halves.
pub fn lathe(rings: &[(f64, f64)]) -> Model {
    let mut m = Model::new();
    let n = rings.len();
    let mut v: Vec<[VertexId; 2]> = Vec::new();
    let mut arcs: Vec<[EdgeId; 2]> = Vec::new();
    for &(z, r) in rings {
        let p0 = m.add_vertex(DVec3::new(r, 0.0, z));
        let p1 = m.add_vertex(DVec3::new(-r, 0.0, z));
        let id = m.new_curve_id();
        let a = ring_arc(&mut m, p0, p1, z, id);
        let b = ring_arc(&mut m, p1, p0, z, id);
        v.push([p0, p1]);
        arcs.push([a, b]);
    }
    // The end discs.
    let disc = |m: &mut Model, i: usize, up: bool| {
        let (z, _) = rings[i];
        let [p0, p1] = v[i];
        let [a, b] = arcs[i];
        let normal = if up { DVec3::Z } else { -DVec3::Z };
        let edges = if up { vec![a, b] } else { vec![b, a] };
        m.add_face(Face::with_edges(
            vec![p0, p1],
            vec![],
            Surface::Plane(Plane::new(DVec3::Z * z, normal)),
            vec![edges],
        ));
    };
    disc(&mut m, 0, false);
    disc(&mut m, n - 1, true);
    for i in 0..n - 1 {
        let ((z0, r0), (z1, r1)) = (rings[i], rings[i + 1]);
        if (z0 - z1).abs() < 1e-12 {
            // A flat step: a ring of plane between two circles.
            let (big, small) = if r0 > r1 { (i, i + 1) } else { (i + 1, i) };
            let (pb, ps) = (v[big], v[small]);
            let (ab, as_) = (arcs[big], arcs[small]);
            m.add_face(Face::with_edges(
                vec![pb[0], pb[1]],
                vec![vec![ps[1], ps[0]]],
                Surface::Plane(Plane::new(DVec3::Z * z0, DVec3::Z)),
                vec![vec![ab[0], ab[1]], vec![as_[0], as_[1]]],
            ));
            continue;
        }
        let s0 = m.add_new_edge(v[i][0], v[i + 1][0]);
        let s1 = m.add_new_edge(v[i][1], v[i + 1][1]);
        let side = if (r0 - r1).abs() < 1e-12 {
            Surface::Cylinder(prism_geom::Cylinder {
                origin: DVec3::ZERO,
                axis: DVec3::Z,
                radius: r0,
                outward: true,
            })
        } else {
            // The apex is where the radius would reach zero; the axis runs
            // from it toward the rings.
            let apex_z = z0 - r0 * (z1 - z0) / (r1 - r0);
            let toward = if (z0 + z1) / 2.0 > apex_z {
                DVec3::Z
            } else {
                -DVec3::Z
            };
            Surface::Cone(prism_geom::Cone {
                apex: DVec3::Z * apex_z,
                axis: toward,
                half_angle: ((r1 - r0).abs() / (z1 - z0).abs()).atan(),
                outward: true,
            })
        };
        let (lo, hi) = (&arcs[i], &arcs[i + 1]);
        m.add_face(Face::with_edges(
            vec![v[i][0], v[i][1], v[i + 1][1], v[i + 1][0]],
            vec![],
            side.clone(),
            vec![vec![lo[0], s1, hi[0], s0]],
        ));
        m.add_face(Face::with_edges(
            vec![v[i][1], v[i][0], v[i + 1][0], v[i + 1][1]],
            vec![],
            side,
            vec![vec![lo[1], s0, hi[1], s1]],
        ));
    }
    m
}
