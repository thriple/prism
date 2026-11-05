//! Curves and surfaces. Lines and planes are built; the enums already exist
//! so exact arcs and NURBS can be added later without changing edges and
//! faces (PLAN 1.1).

use prism_math::{DVec3, Plane};
use serde::{Deserialize, Serialize};

/// The curve an edge follows between its two vertices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Curve {
    /// A straight line.
    #[default]
    Line,
}

impl Curve {
    /// A point at `t` (0 to 1) between the edge's ends.
    pub fn point(&self, a: DVec3, b: DVec3, t: f64) -> DVec3 {
        match self {
            Curve::Line => a.lerp(b, t),
        }
    }
}

/// The surface a face lies on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Surface {
    Plane(Plane),
}

impl Surface {
    pub fn plane(&self) -> &Plane {
        match self {
            Surface::Plane(p) => p,
        }
    }

    /// Which way the face points.
    pub fn normal(&self) -> DVec3 {
        self.plane().normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_points() {
        let p = Curve::Line.point(DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0), 0.25);
        assert_eq!(p, DVec3::new(1.0, 0.0, 0.0));
    }
}
