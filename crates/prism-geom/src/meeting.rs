//! Where two curves meet, for any pair of curves (Freeform NURBS part 1).

use prism_math::DVec3;

use crate::Curve;

/// The closest points of segments `p0`–`p1` and `q0`–`q1`.
fn segment_closest(p0: DVec3, p1: DVec3, q0: DVec3, q1: DVec3) -> (DVec3, DVec3) {
    let (d1, d2, r) = (p1 - p0, q1 - q0, p0 - q0);
    let (a, e, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    let (s, t);
    if a == 0.0 && e == 0.0 {
        return (p0, q0);
    }
    if a == 0.0 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e == 0.0 {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let den = a * e - b * b;
            let s0 = if den > 0.0 {
                ((b * f - c * e) / den).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            } else {
                t = t0;
                s = s0;
            }
        }
    }
    (p0 + d1 * s, q0 + d2 * t)
}

/// Where two curves (each with its ends) meet, within `eps`: found by
/// stepping along both, each meeting then put on both curves exactly. For
/// any pair of curves; the pairs of lines and arcs have exact answers
/// ([`circle_line`], [`circle_circle`]) that are used first.
pub fn curves_meet(
    (c1, a1, b1): (&Curve, DVec3, DVec3),
    (c2, a2, b2): (&Curve, DVec3, DVec3),
    eps: f64,
) -> Vec<DVec3> {
    let poly = |c: &Curve, a: DVec3, b: DVec3| {
        let mut v = vec![a];
        v.extend(c.inner_points(a, b));
        v.push(b);
        v
    };
    let (p, q) = (poly(c1, a1, b1), poly(c2, a2, b2));
    let bounds = |v: &[DVec3]| {
        (
            v.iter().fold(v[0], |m, x| m.min(*x)),
            v.iter().fold(v[0], |m, x| m.max(*x)),
        )
    };
    let ((lo1, hi1), (lo2, hi2)) = (bounds(&p), bounds(&q));
    let pad = DVec3::splat(eps);
    if (lo1 - pad).cmpgt(hi2).any() || (lo2 - pad).cmpgt(hi1).any() {
        return Vec::new();
    }
    let mut out: Vec<DVec3> = Vec::new();
    for w in p.windows(2) {
        let (wlo, whi) = (w[0].min(w[1]), w[0].max(w[1]));
        if (wlo - pad).cmpgt(hi2).any() || (lo2 - pad).cmpgt(whi).any() {
            continue;
        }
        for z in q.windows(2) {
            let (x, y) = segment_closest(w[0], w[1], z[0], z[1]);
            let near = eps + 0.05 * (w[0].distance(w[1]) + z[0].distance(z[1]));
            if x.distance(y) > near {
                continue;
            }
            // Where the two curves' tangent lines come closest, put back
            // on the curves, until the two points agree (Newton's method
            // in effect: it closes fast even on a shallow crossing).
            let mut x = c1.closest_point(a1, b1, x.midpoint(y));
            let mut y = c2.closest_point(a2, b2, x);
            for _ in 0..60 {
                if x.distance(y) <= eps * 1e-6 {
                    break;
                }
                let (t1, t2) = (c1.tangent_at(a1, b1, x), c2.tangent_at(a2, b2, y));
                let (r, b) = (x - y, t1.dot(t2));
                let den = 1.0 - b * b;
                let (px, py) = if den > 1e-12 {
                    let (d, e) = (t1.dot(r), t2.dot(r));
                    (x + t1 * ((b * e - d) / den), y + t2 * ((e - b * d) / den))
                } else {
                    (y, x)
                };
                let (nx, ny) = (c1.closest_point(a1, b1, px), c2.closest_point(a2, b2, py));
                // No better: fall back to plain steps onto each in turn.
                if nx.distance(ny) < x.distance(y) {
                    (x, y) = (nx, ny);
                } else {
                    let ox = c1.closest_point(a1, b1, y);
                    let oy = c2.closest_point(a2, b2, ox);
                    if ox.distance(oy) >= x.distance(y) {
                        break;
                    }
                    (x, y) = (ox, oy);
                }
            }
            // One meeting found from two neighbouring steps is one meeting.
            if x.distance(y) <= eps && !out.iter().any(|o| o.distance(x) <= eps * 100.0) {
                out.push(x);
            }
        }
    }
    out
}
