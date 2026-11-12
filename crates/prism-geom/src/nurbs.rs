//! NURBS curves (Freeform NURBS part 1). Prism owns the type, its
//! evaluation and its file layout; the hard constructions (skinning,
//! fitting) go through the kernel (PLAN 2.3).
//!
//! A curve is its degree, knots, control points and weights. A closed
//! smooth curve is periodic: its last `degree` control points repeat its
//! first, and its knots are evenly spaced, so it has no corner at the seam.

use std::sync::Arc;

use prism_math::{DAffine3, DVec3, Plane, newell};
use serde::{Deserialize, Serialize};

use crate::SEGMENTS_PER_TURN;

/// The highest degree a curve or surface may have.
pub const MAX_DEGREE: usize = 7;

/// A weighted point: `p` is the point times its weight.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct H {
    pub p: DVec3,
    pub w: f64,
}

impl H {
    pub fn of(p: DVec3, w: f64) -> H {
        H { p: p * w, w }
    }
    fn lerp(self, o: H, t: f64) -> H {
        H {
            p: self.p.lerp(o.p, t),
            w: self.w + (o.w - self.w) * t,
        }
    }
    fn diff(self, o: H, k: f64) -> H {
        H {
            p: (self.p - o.p) * k,
            w: (self.w - o.w) * k,
        }
    }
    /// The point, and the derivative given this value's derivative `d`.
    pub fn split(self, d: H) -> (DVec3, DVec3) {
        let c = self.p / self.w;
        (c, (d.p - c * d.w) / self.w)
    }
}

/// The knot span holding `t`: `k` with `knots[k] <= t < knots[k + 1]`,
/// kept inside the curve (`n` control points).
pub(crate) fn span(p: usize, knots: &[f64], n: usize, t: f64) -> usize {
    let mut k = p;
    while k + 1 < n && t >= knots[k + 1] {
        k += 1;
    }
    k
}

/// de Boor's algorithm in span `k`: the value and the derivative, both
/// weighted. `ctrl(i)` is control point `i`.
pub(crate) fn de_boor(
    p: usize,
    knots: &[f64],
    k: usize,
    t: f64,
    ctrl: impl Fn(usize) -> H,
) -> (H, H) {
    let mut d = [H::default(); MAX_DEGREE + 1];
    for (j, x) in d.iter_mut().enumerate().take(p + 1) {
        *x = ctrl(k - p + j);
    }
    let mut der = H::default();
    for r in 1..=p {
        if r == p {
            der = d[p].diff(d[p - 1], p as f64 / (knots[k + 1] - knots[k]));
        }
        for j in (r..=p).rev() {
            let i = k - p + j;
            let den = knots[i + p - r + 1] - knots[i];
            let a = if den > 0.0 { (t - knots[i]) / den } else { 0.0 };
            d[j] = d[j - 1].lerp(d[j], a);
        }
    }
    (d[p], der)
}

/// The `p + 1` basis functions that are not zero in span `k`, at `t`:
/// entry `r` belongs to control point `k - p + r`.
pub(crate) fn basis(p: usize, knots: &[f64], k: usize, t: f64) -> [f64; MAX_DEGREE + 1] {
    let mut n = [0.0; MAX_DEGREE + 1];
    let mut left = [0.0; MAX_DEGREE + 1];
    let mut right = [0.0; MAX_DEGREE + 1];
    n[0] = 1.0;
    for j in 1..=p {
        left[j] = t - knots[k + 1 - j];
        right[j] = knots[k + j] - t;
        let mut saved = 0.0;
        for r in 0..j {
            let den = right[r + 1] + left[j - r];
            let temp = if den > 0.0 { n[r] / den } else { 0.0 };
            n[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        n[j] = saved;
    }
    n
}

/// Whether `knots` suit `n` control points of degree `p`.
pub(crate) fn knots_ok(p: usize, knots: &[f64], n: usize) -> bool {
    (1..=MAX_DEGREE).contains(&p)
        && n > p
        && knots.len() == n + p + 1
        && knots.iter().all(|k| k.is_finite())
        && knots.windows(2).all(|w| w[0] <= w[1])
        && knots[p] < knots[n]
        && knots.windows(p + 2).all(|w| w[0] < w[p + 1])
}

/// Clamped, evenly spaced knots over 0 to 1.
pub(crate) fn clamped_knots(p: usize, n: usize) -> Vec<f64> {
    let spans = n - p;
    (0..n + p + 1)
        .map(|i| {
            if i <= p {
                0.0
            } else if i >= n {
                1.0
            } else {
                (i - p) as f64 / spans as f64
            }
        })
        .collect()
}

/// Solve `a x = b` (rows of `a` are `n` long) by elimination with
/// pivoting. None if the system has no single answer.
pub(crate) fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<DVec3>) -> Option<Vec<DVec3>> {
    let n = b.len();
    for c in 0..n {
        let piv = (c..n).max_by(|x, y| a[*x][c].abs().total_cmp(&a[*y][c].abs()))?;
        if a[piv][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, piv);
        b.swap(c, piv);
        for r in c + 1..n {
            let f = a[r][c] / a[c][c];
            if f != 0.0 {
                let (top, rest) = a.split_at_mut(r);
                for (x, y) in rest[0][c..].iter_mut().zip(&top[c][c..]) {
                    *x -= f * y;
                }
                b[r] = b[r] - b[c] * f;
            }
        }
    }
    for c in (0..n).rev() {
        let mut s = b[c];
        for k in c + 1..n {
            s -= b[k] * a[c][k];
        }
        b[c] = s / a[c][c];
    }
    Some(b)
}

/// A NURBS curve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Nurbs {
    degree: usize,
    knots: Vec<f64>,
    points: Vec<DVec3>,
    weights: Vec<f64>,
    /// Closed and smooth at the seam: the last `degree` control points
    /// repeat the first.
    #[serde(default)]
    periodic: bool,
}

impl Nurbs {
    /// A checked curve. None if the parts do not fit together.
    pub fn new(
        degree: usize,
        knots: Vec<f64>,
        points: Vec<DVec3>,
        weights: Vec<f64>,
        periodic: bool,
    ) -> Option<Nurbs> {
        let n = points.len();
        let ok = knots_ok(degree, &knots, n)
            && weights.len() == n
            && points.iter().all(|p| p.is_finite())
            && weights.iter().all(|w| w.is_finite() && *w > 0.0)
            && (!periodic
                || n > 2 * degree
                    && (0..degree).all(|i| {
                        points[i] == points[n - degree + i] && weights[i] == weights[n - degree + i]
                    }));
        ok.then_some(Nurbs {
            degree,
            knots,
            points,
            weights,
            periodic,
        })
    }

    /// An open curve shaped by `points` (its control points): it starts at
    /// the first and ends at the last. The degree is lowered for few points.
    pub fn open(points: &[DVec3], degree: usize) -> Option<Nurbs> {
        let n = points.len();
        if n < 2 {
            return None;
        }
        let p = degree.clamp(1, MAX_DEGREE).min(n - 1);
        Nurbs::new(p, clamped_knots(p, n), points.to_vec(), vec![1.0; n], false)
    }

    /// A closed smooth curve shaped by `points` (three or more).
    pub fn closed(points: &[DVec3], degree: usize) -> Option<Nurbs> {
        let n = points.len();
        if n < 3 {
            return None;
        }
        let p = degree.clamp(1, MAX_DEGREE).min(n - 1);
        let all: Vec<DVec3> = points.iter().chain(&points[..p]).copied().collect();
        let knots = (0..n + 2 * p + 1)
            .map(|i| (i as f64 - p as f64) / n as f64)
            .collect();
        Nurbs::new(p, knots, all, vec![1.0; n + p], true)
    }

    /// A cubic curve through `points`, open or closed.
    pub fn through(points: &[DVec3], closed: bool) -> Option<Nurbs> {
        let n = points.len();
        if closed {
            let mut c = Nurbs::closed(points, 3)?;
            let p = c.degree;
            let rows: Vec<Vec<f64>> = (0..n)
                .map(|i| {
                    let t = i as f64 / n as f64;
                    let k = span(p, &c.knots, n + p, t);
                    let b = basis(p, &c.knots, k, t);
                    let mut row = vec![0.0; n];
                    for (r, v) in b.iter().enumerate().take(p + 1) {
                        row[(k - p + r) % n] += v;
                    }
                    row
                })
                .collect();
            let q = solve(rows, points.to_vec())?;
            for (i, x) in q.iter().enumerate() {
                c.set_control(i, *x);
            }
            return Some(c);
        }
        if n < 3 {
            return Nurbs::open(points, 1);
        }
        let p = 3.min(n - 1);
        // Chord-length parameters, and knots from their averages.
        let total: f64 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
        if total <= 0.0 {
            return None;
        }
        let mut ts = vec![0.0];
        for w in points.windows(2) {
            let last = ts[ts.len() - 1];
            ts.push(last + w[0].distance(w[1]) / total);
        }
        ts[n - 1] = 1.0;
        let mut knots = vec![0.0; n + p + 1];
        for j in 1..n - p {
            knots[j + p] = ts[j..j + p].iter().sum::<f64>() / p as f64;
        }
        for k in knots.iter_mut().skip(n) {
            *k = 1.0;
        }
        if !knots_ok(p, &knots, n) {
            return None;
        }
        let rows: Vec<Vec<f64>> = ts
            .iter()
            .map(|&t| {
                let k = span(p, &knots, n, t);
                let b = basis(p, &knots, k, t);
                let mut row = vec![0.0; n];
                row[k - p..=k].copy_from_slice(&b[..=p]);
                row
            })
            .collect();
        let q = solve(rows, points.to_vec())?;
        Nurbs::new(p, knots, q, vec![1.0; n], false)
    }

    pub fn degree(&self) -> usize {
        self.degree
    }

    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Every control point, a periodic curve's repeats included.
    pub fn points(&self) -> &[DVec3] {
        &self.points
    }

    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    pub fn is_periodic(&self) -> bool {
        self.periodic
    }

    /// The control points that can be moved on their own (a periodic
    /// curve's repeats left out).
    pub fn cage(&self) -> &[DVec3] {
        let n = self.points.len();
        &self.points[..if self.periodic { n - self.degree } else { n }]
    }

    /// Move control point `i` of the cage (and its repeat).
    pub fn set_control(&mut self, i: usize, p: DVec3) {
        let n = self.cage().len();
        if i >= n {
            return;
        }
        self.points[i] = p;
        if self.periodic && i < self.degree {
            self.points[n + i] = p;
        }
    }

    /// The first and last parameter.
    pub fn domain(&self) -> (f64, f64) {
        (self.knots[self.degree], self.knots[self.points.len()])
    }

    /// Whether the curve ends where it starts.
    pub fn is_closed(&self) -> bool {
        let n = self.points.len();
        self.periodic
            || (self.knots[0] == self.knots[self.degree]
                && self.knots[n] == self.knots[n + self.degree]
                && self.points[0] == self.points[n - 1])
    }

    /// `t` brought into the domain: wrapped round a closed curve, held at
    /// the ends of an open one.
    pub fn wrap(&self, t: f64) -> f64 {
        let (lo, hi) = self.domain();
        if (lo..=hi).contains(&t) {
            t
        } else if self.is_closed() {
            lo + (t - lo).rem_euclid(hi - lo)
        } else {
            t.clamp(lo, hi)
        }
    }

    fn weighted(&self, t: f64) -> (H, H) {
        let t = self.wrap(t);
        let k = span(self.degree, &self.knots, self.points.len(), t);
        de_boor(self.degree, &self.knots, k, t, |i| {
            H::of(self.points[i], self.weights[i])
        })
    }

    /// The point at `t`.
    pub fn eval(&self, t: f64) -> DVec3 {
        let (c, _) = self.weighted(t);
        c.p / c.w
    }

    /// The point and the first derivative at `t`.
    pub fn eval_d(&self, t: f64) -> (DVec3, DVec3) {
        let (c, d) = self.weighted(t);
        c.split(d)
    }

    /// The curvature vector at `t`: towards the centre of the bend, as long
    /// as one over the bend's radius.
    pub fn curvature(&self, t: f64) -> DVec3 {
        let (lo, hi) = self.domain();
        let h = (hi - lo) * 1e-4;
        let (t0, t1) = if self.is_closed() {
            (t - h, t + h)
        } else {
            ((t - h).max(lo), (t + h).min(hi))
        };
        let (p0, d0) = self.eval_d(t0);
        let (p1, d1) = self.eval_d(t1);
        let ds = p0.distance(p1);
        if ds <= 0.0 {
            return DVec3::ZERO;
        }
        (d1.normalize_or_zero() - d0.normalize_or_zero()) / ds
    }

    /// The curve after `m`. Any affine map keeps a NURBS curve exact.
    pub fn transformed(&self, m: &DAffine3) -> Nurbs {
        let mut c = self.clone();
        for p in &mut c.points {
            *p = m.transform_point3(*p);
        }
        c
    }

    /// The same curve with one more knot at `t` (so one more control
    /// point). None for a periodic curve or a `t` outside it.
    pub fn insert_knot(&self, t: f64) -> Option<Nurbs> {
        let (lo, hi) = self.domain();
        if self.periodic || t <= lo || t >= hi {
            return None;
        }
        let (p, n) = (self.degree, self.points.len());
        let k = span(p, &self.knots, n, t);
        let old = |i: usize| H::of(self.points[i], self.weights[i]);
        let mut q = Vec::with_capacity(n + 1);
        for i in 0..=n {
            q.push(if i + p <= k {
                old(i)
            } else if i <= k {
                let a = (t - self.knots[i]) / (self.knots[i + p] - self.knots[i]);
                old(i - 1).lerp(old(i), a)
            } else {
                old(i - 1)
            });
        }
        let mut knots = self.knots.clone();
        knots.insert(k + 1, t);
        Nurbs::new(
            p,
            knots,
            q.iter().map(|h| h.p / h.w).collect(),
            q.iter().map(|h| h.w).collect(),
            false,
        )
    }

    /// The number of knot spans of non-zero length in the domain.
    pub fn spans(&self) -> usize {
        let (p, n) = (self.degree, self.points.len());
        (p..n)
            .filter(|&k| self.knots[k] < self.knots[k + 1])
            .count()
    }

    /// The parameter in `lo..=hi` whose point is closest to `p`. The range
    /// may run past the end of a closed curve.
    pub fn closest(&self, p: DVec3, lo: f64, hi: f64) -> f64 {
        let (d0, d1) = self.domain();
        let n = ((self.spans() * 8) as f64 * ((hi - lo) / (d1 - d0)).abs().max(0.05)).ceil()
            as usize
            + 8;
        let at = |i: usize| lo + (hi - lo) * i as f64 / n as f64;
        let mut best = (f64::INFINITY, 0);
        for i in 0..=n {
            let d = self.eval(at(i)).distance_squared(p);
            if d < best.0 {
                best = (d, i);
            }
        }
        let (a, b) = (at(best.1.saturating_sub(1)), at((best.1 + 1).min(n)));
        let mut t = at(best.1);
        for _ in 0..24 {
            let (c, d) = self.eval_d(t);
            let dd = d.length_squared();
            if dd == 0.0 {
                break;
            }
            let step = (p - c).dot(d) / dd;
            let next = (t + step).clamp(a.min(b), a.max(b));
            if (next - t).abs() <= 1e-15 * (d1 - d0) {
                t = next;
                break;
            }
            t = next;
        }
        t
    }

    /// How far the direction of travel turns between `t0` and `t1`, in
    /// radians (measured at a few points per span).
    pub fn turning(&self, t0: f64, t1: f64) -> f64 {
        let (d0, d1) = self.domain();
        let n = (((t1 - t0).abs() / (d1 - d0) * self.spans() as f64).ceil() as usize).max(1)
            * (self.degree + 1);
        let mut last = self.eval_d(t0).1.normalize_or_zero();
        let mut sum = 0.0;
        for i in 1..=n {
            let d = self
                .eval_d(t0 + (t1 - t0) * i as f64 / n as f64)
                .1
                .normalize_or_zero();
            sum += last.dot(d).clamp(-1.0, 1.0).acos();
            last = d;
        }
        sum
    }

    /// How many straight steps draw the part from `t0` to `t1`: as fine as
    /// an arc that turns as much.
    pub fn steps(&self, t0: f64, t1: f64) -> usize {
        let (d0, d1) = self.domain();
        let spans = ((t1 - t0).abs() / (d1 - d0) * self.spans() as f64).ceil() as usize;
        if self.degree == 1 {
            return spans.max(1);
        }
        let by_turn = (self.turning(t0, t1) / std::f64::consts::TAU * SEGMENTS_PER_TURN as f64)
            .ceil() as usize;
        by_turn.clamp((spans * 2).max(2), 1024)
    }

    /// The plane the curve lies in, if it lies in one.
    pub fn plane(&self, eps: f64) -> Option<Plane> {
        let n = newell(&self.points).normalize_or_zero();
        if n == DVec3::ZERO {
            return None;
        }
        let pl = Plane::new(self.points[0], n);
        self.points
            .iter()
            .all(|p| pl.distance(*p).abs() <= eps)
            .then_some(pl)
    }
}

/// The part of a NURBS curve an edge runs along: from `t0` to `t1`, which
/// is below `t0` for an edge walked against the curve, and may pass the end
/// of a closed curve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spline {
    pub curve: Arc<Nurbs>,
    pub t0: f64,
    pub t1: f64,
}

impl Spline {
    /// All of an open curve.
    pub fn whole(curve: Nurbs) -> Spline {
        let (t0, t1) = curve.domain();
        Spline {
            curve: Arc::new(curve),
            t0,
            t1,
        }
    }

    /// The parameter at fraction `s` of the way.
    pub fn at(&self, s: f64) -> f64 {
        self.t0 + (self.t1 - self.t0) * s
    }

    pub fn point(&self, s: f64) -> DVec3 {
        self.curve.eval(self.at(s))
    }

    /// The unit direction of travel at fraction `s`.
    pub fn tangent(&self, s: f64) -> DVec3 {
        let d = self.curve.eval_d(self.at(s)).1.normalize_or_zero();
        if self.t1 < self.t0 { -d } else { d }
    }

    pub fn reversed(&self) -> Spline {
        Spline {
            curve: self.curve.clone(),
            t0: self.t1,
            t1: self.t0,
        }
    }

    /// The fraction of the way whose point is closest to `p`.
    pub fn closest(&self, p: DVec3) -> f64 {
        let (lo, hi) = (self.t0.min(self.t1), self.t0.max(self.t1));
        let t = self.curve.closest(p, lo, hi);
        if self.t1 == self.t0 {
            0.0
        } else {
            (t - self.t0) / (self.t1 - self.t0)
        }
    }

    /// The two parts either side of the point closest to `p`.
    pub fn split(&self, p: DVec3) -> (Spline, Spline) {
        let t = self.at(self.closest(p));
        (
            Spline {
                curve: self.curve.clone(),
                t0: self.t0,
                t1: t,
            },
            Spline {
                curve: self.curve.clone(),
                t0: t,
                t1: self.t1,
            },
        )
    }

    /// The points strictly between the ends that it is drawn through.
    pub fn inner_points(&self) -> Vec<DVec3> {
        let n = self.curve.steps(self.t0, self.t1);
        (1..n).map(|i| self.point(i as f64 / n as f64)).collect()
    }

    pub fn length(&self) -> f64 {
        let n = self.curve.steps(self.t0, self.t1) * 4;
        let mut last = self.point(0.0);
        let mut sum = 0.0;
        for i in 1..=n {
            let p = self.point(i as f64 / n as f64);
            sum += last.distance(p);
            last = p;
        }
        sum
    }

    pub fn transformed(&self, m: &DAffine3) -> Spline {
        Spline {
            curve: Arc::new(self.curve.transformed(m)),
            t0: self.t0,
            t1: self.t1,
        }
    }

    /// Whether this is all of its curve (so its ends are the curve's ends).
    pub fn is_whole(&self) -> bool {
        let (lo, hi) = self.curve.domain();
        (self.t0 == lo && self.t1 == hi) || (self.t0 == hi && self.t1 == lo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quarter() -> Nurbs {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        Nurbs::new(
            2,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y],
            vec![1.0, w, 1.0],
            false,
        )
        .unwrap()
    }

    #[test]
    fn a_rational_quadratic_is_a_circle() {
        let c = quarter();
        for i in 0..=20 {
            let t = i as f64 / 20.0;
            let (p, d) = c.eval_d(t);
            assert!((p.length() - 1.0).abs() < 1e-14);
            assert!(
                p.dot(d).abs() < 1e-13,
                "the tangent is square to the radius"
            );
            if i > 0 && i < 20 {
                assert!((c.curvature(t).length() - 1.0).abs() < 1e-5);
            }
        }
        assert_eq!(c.eval(0.0), DVec3::X);
        assert_eq!(c.eval(1.0), DVec3::Y);
    }

    #[test]
    fn inserting_a_knot_keeps_the_shape() {
        let pts = [
            DVec3::ZERO,
            DVec3::new(1.0, 2.0, 0.0),
            DVec3::new(3.0, -1.0, 1.0),
            DVec3::new(4.0, 0.5, 0.0),
            DVec3::new(6.0, 0.0, 2.0),
        ];
        for c in [Nurbs::open(&pts, 3).unwrap(), quarter()] {
            let d = c.insert_knot(0.37).unwrap();
            assert_eq!(d.points().len(), c.points().len() + 1);
            for i in 0..=50 {
                let t = i as f64 / 50.0;
                assert!(c.eval(t).distance(d.eval(t)) < 1e-13);
            }
        }
    }

    #[test]
    fn a_curve_through_points_passes_through_them() {
        let pts = [
            DVec3::ZERO,
            DVec3::new(1.0, 2.0, 0.0),
            DVec3::new(3.0, -1.0, 1.0),
            DVec3::new(4.0, 0.5, 0.0),
            DVec3::new(6.0, 0.0, 2.0),
        ];
        let c = Nurbs::through(&pts, false).unwrap();
        for p in pts {
            assert!(c.eval(c.closest(p, 0.0, 1.0)).distance(p) < 1e-10);
        }
        let c = Nurbs::through(&pts, true).unwrap();
        assert!(c.is_closed());
        for (i, p) in pts.iter().enumerate() {
            assert!(c.eval(i as f64 / 5.0).distance(*p) < 1e-10);
        }
    }

    #[test]
    fn a_closed_curve_is_smooth_at_the_seam() {
        let pts = [
            DVec3::ZERO,
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(2.0, 2.0, 0.0),
            DVec3::new(0.0, 2.0, 0.0),
        ];
        let c = Nurbs::closed(&pts, 3).unwrap();
        let (a, da) = c.eval_d(0.0);
        let (b, db) = c.eval_d(1.0 - 1e-12);
        assert!(a.distance(b) < 1e-9);
        assert!(da.distance(db) < 1e-6);
        assert_eq!(c.cage().len(), 4);
        assert_eq!(c.wrap(1.25), 0.25);
        assert!(c.plane(1e-9).is_some());
    }

    #[test]
    fn the_closest_point_and_the_split() {
        let c = Nurbs::open(
            &[
                DVec3::ZERO,
                DVec3::new(1.0, 2.0, 0.0),
                DVec3::new(3.0, 2.0, 0.0),
                DVec3::new(4.0, 0.0, 0.0),
            ],
            3,
        )
        .unwrap();
        let s = Spline::whole(c);
        let mid = s.point(0.4);
        assert!((s.closest(mid) - 0.4).abs() < 1e-9);
        let (a, b) = s.split(mid);
        assert!(a.point(1.0).distance(mid) < 1e-12 && b.point(0.0).distance(mid) < 1e-12);
        assert!((a.length() + b.length() - s.length()).abs() < 1e-3);
        let r = s.reversed();
        assert!(r.point(0.0).distance(s.point(1.0)) < 1e-15);
        assert!(r.tangent(0.0).distance(-s.tangent(1.0)) < 1e-12);
    }
}
