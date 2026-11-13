//! The camera and SketchUp-style navigation (PLAN 1.5a, as changed in 1.8).
//!
//! Everything here is pure f64 maths: the app passes in the point under the
//! cursor (from picking) and the mouse movement, and gets a new camera back.
//! Screen positions are in logical pixels (points), so the same numbers hold
//! on a 1x and a 2x screen.

use prism_math::{DQuat, DVec2, DVec3};

/// Vertical field of view, as in the prototype.
pub const FOV_Y_DEG: f64 = 35.0;
/// One wheel notch moves this fraction of the way to the point under the cursor.
pub const ZOOM_PER_NOTCH: f64 = 0.12;
/// At most this many notches count from one wheel event.
pub const MAX_NOTCHES: f64 = 3.0;
/// Orbit speed, radians per pixel.
pub const ORBIT_PER_PX: f64 = 0.006;
/// The view never looks closer than this to straight up or down (|f.z|).
pub const MAX_LOOK_Z: f64 = 0.995;
/// Zoom Extents leaves this margin around the model.
pub const EXTENTS_MARGIN: f64 = 1.15;
/// Zooming in stops this many model tolerances from the surface.
pub const MIN_DISTANCE_TOLS: f64 = 100.0;
/// Zoom tool: fraction of the distance per pixel of vertical drag.
pub const ZOOM_PER_DRAG_PX: f64 = 0.005;
/// The most one zoom step may move toward the point (never reaches it).
pub const MAX_ZOOM_STEP: f64 = 0.5;

/// The narrowest and widest field of view that can be typed (degrees).
pub const FOV_MIN_DEG: f64 = 1.0;
pub const FOV_MAX_DEG: f64 = 120.0;
/// How far back a parallel view's eye stands, in camera distances: the
/// picture does not change, and rays from it, occlusion tests toward it and
/// depths from it then cover what is behind the camera too.
pub const PARALLEL_RAY_BACK: f64 = 1e4;

/// How the camera projects (Everyday Commands part 2): perspective with a
/// vertical field of view, or parallel. A parallel view shows what a
/// perspective one shows at the target's distance, at the same size.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Lens {
    #[serde(default)]
    pub parallel: bool,
    /// Vertical field of view, degrees.
    #[serde(default = "default_fov")]
    pub fov: f64,
}

fn default_fov() -> f64 {
    FOV_Y_DEG
}

impl Default for Lens {
    fn default() -> Self {
        Lens {
            parallel: false,
            fov: FOV_Y_DEG,
        }
    }
}

/// The standard views (Camera > Standard Views): the direction looked in.
pub fn standard_view(name: &str) -> Option<DVec3> {
    // Straight up and down are tipped a hair so the view has a right
    // direction (+X) and can still be orbited.
    let tip = 1e-9;
    Some(
        match name {
            "top" => DVec3::new(0.0, tip, -1.0),
            "bottom" => DVec3::new(0.0, tip, 1.0),
            "front" => DVec3::Y,
            "back" => -DVec3::Y,
            "left" => DVec3::X,
            "right" => -DVec3::X,
            "iso" => DVec3::new(-1.0, 1.0, -0.8),
            _ => return None,
        }
        .normalize(),
    )
}

/// Where the camera is and what it looks at. Up is always +Z (gravity).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub eye: DVec3,
    pub target: DVec3,
}

impl Camera {
    /// The prototype's starting view.
    pub fn home() -> Self {
        Self {
            eye: DVec3::new(7.5, -9.0, 5.4),
            target: DVec3::new(0.0, 0.0, 1.8),
        }
    }

    pub fn forward(&self) -> DVec3 {
        (self.target - self.eye).normalize()
    }

    pub fn distance(&self) -> f64 {
        self.eye.distance(self.target)
    }

    pub fn lerp(&self, other: &Camera, t: f64) -> Camera {
        Camera {
            eye: self.eye.lerp(other.eye, t),
            target: self.target.lerp(other.target, t),
        }
    }

    /// Move toward `q` by the fraction `k` of the distance (negative moves
    /// away). The point stays under the cursor because the eye moves along
    /// the line to it.
    pub fn zoom_toward(&self, q: DVec3, k: f64) -> Camera {
        let k = k.min(MAX_ZOOM_STEP);
        let mv = (q - self.eye) * k;
        Camera {
            eye: self.eye + mv,
            target: self.target + mv,
        }
    }

    /// Zoom a parallel view toward `q` by the fraction `k`: the view
    /// shrinks about `q`, which stays where it is on screen.
    pub fn zoom_parallel(&self, q: DVec3, k: f64) -> Camera {
        let k = k.min(MAX_ZOOM_STEP);
        let f = self.forward();
        let d = self.distance();
        let rel = q - self.eye;
        let across = rel - f * rel.dot(f);
        let eye = self.eye + across * k + f * (d * k);
        Camera {
            eye,
            target: eye + f * (d * (1.0 - k)),
        }
    }

    /// Zoom by `k` toward `q`, as the lens does it.
    pub fn zoom_lens(&self, q: DVec3, k: f64, lens: Lens) -> Camera {
        if lens.parallel {
            self.zoom_parallel(q, k)
        } else {
            self.zoom_toward(q, k)
        }
    }

    /// The wheel, as the lens zooms: a parallel view stops at the same
    /// smallest size as a perspective one stops at its distance.
    pub fn wheel_lens(&self, q: DVec3, notches: f64, min_distance: f64, lens: Lens) -> Camera {
        if !lens.parallel {
            return self.wheel(q, notches, min_distance);
        }
        let k = notches.clamp(-MAX_NOTCHES, MAX_NOTCHES) * ZOOM_PER_NOTCH;
        if k > 0.0 && self.distance() * (1.0 - k) < min_distance {
            return *self;
        }
        self.zoom_parallel(q, k)
    }

    /// A pinch, as the lens zooms.
    pub fn pinch_lens(&self, q: DVec3, factor: f64, min_distance: f64, lens: Lens) -> Camera {
        if !lens.parallel {
            return self.pinch(q, factor, min_distance);
        }
        if factor <= 0.0 || (factor > 1.0 && self.distance() / factor < min_distance) {
            return *self;
        }
        self.zoom_parallel(q, 1.0 - 1.0 / factor)
    }

    /// Look in direction `dir` at the box `lo`..`hi`, fitted to the view.
    pub fn looking(dir: DVec3, lo: DVec3, hi: DVec3, aspect: f64) -> Camera {
        let c = lo.lerp(hi, 0.5);
        Camera {
            eye: c - dir.normalize(),
            target: c,
        }
        .extents(lo, hi, aspect)
    }

    /// Look Around: turn the view about the eye (`yaw` about the vertical,
    /// `pitch` up and down), never straight up or down.
    pub fn look_around(&self, yaw: f64, pitch: f64) -> Camera {
        let d = self.distance();
        let f = DQuat::from_axis_angle(DVec3::Z, yaw) * self.forward();
        let r = right_of(f);
        let f2 = DQuat::from_axis_angle(r, pitch) * f;
        let f = if f2.z.abs() < MAX_LOOK_Z { f2 } else { f };
        Camera {
            eye: self.eye,
            target: self.eye + f * d,
        }
    }

    /// Walk: `ahead` forward along the ground (the eye's height kept), and
    /// a turn of `yaw` about the vertical.
    pub fn walk(&self, ahead: f64, yaw: f64) -> Camera {
        let turned = self.look_around(yaw, 0.0);
        let f = turned.forward();
        let flat = DVec3::new(f.x, f.y, 0.0).normalize_or(DVec3::Y);
        let mv = flat * ahead;
        Camera {
            eye: turned.eye + mv,
            target: turned.target + mv,
        }
    }

    /// Zoom from wheel notches (positive zooms in). Zooming in stops once
    /// the eye would come closer than `min_distance` to `q`.
    pub fn wheel(&self, q: DVec3, notches: f64, min_distance: f64) -> Camera {
        let k = notches.clamp(-MAX_NOTCHES, MAX_NOTCHES) * ZOOM_PER_NOTCH;
        if k > 0.0 && self.eye.distance(q) * (1.0 - k) < min_distance {
            return *self;
        }
        self.zoom_toward(q, k)
    }

    /// Zoom by a pinch factor (above 1 zooms in): the distance to `q` is
    /// divided by the factor.
    pub fn pinch(&self, q: DVec3, factor: f64, min_distance: f64) -> Camera {
        if factor <= 0.0 || (factor > 1.0 && self.eye.distance(q) / factor < min_distance) {
            return *self;
        }
        self.zoom_toward(q, 1.0 - 1.0 / factor)
    }

    /// Orbit about `pivot`: `yaw` about the vertical, `pitch` about the
    /// camera's right axis. The horizon stays level; the view never turns
    /// straight up or down, and never dips further below the ground.
    pub fn orbit(&self, pivot: DVec3, yaw: f64, pitch: f64) -> Camera {
        let rot =
            |p: DVec3, axis: DVec3, a: f64| pivot + DQuat::from_axis_angle(axis, a) * (p - pivot);
        let eye = rot(self.eye, DVec3::Z, yaw);
        let target = rot(self.target, DVec3::Z, yaw);
        let f = (target - eye).normalize();
        let r = right_of(f);
        let e2 = rot(eye, r, -pitch);
        let t2 = rot(target, r, -pitch);
        let f2 = (t2 - e2).normalize();
        let above = e2.z > 0.0 || e2.z >= eye.z;
        // Not tipped over the top or the bottom (which would turn the view upside down).
        let same_way = f2.x * f.x + f2.y * f.y > 0.0;
        // From a view straight down or up (Top, Bottom), moving toward the
        // horizon is allowed.
        let leaving_pole = f.z.abs() >= MAX_LOOK_Z && f2.z.abs() < f.z.abs();
        let same_way = same_way || leaving_pole;
        if (f2.z.abs() < MAX_LOOK_Z || leaving_pole) && above && same_way {
            Camera {
                eye: e2,
                target: t2,
            }
        } else {
            Camera { eye, target }
        }
    }

    /// Orbit from a mouse movement in pixels.
    pub fn orbit_drag(&self, pivot: DVec3, dx: f64, dy: f64) -> Camera {
        self.orbit(pivot, -dx * ORBIT_PER_PX, dy * ORBIT_PER_PX)
    }

    /// Pan so that the grabbed point `p` follows the mouse by `d` pixels.
    pub fn pan(&self, view: &View, p: DVec3, d: DVec2) -> Camera {
        let depth = (p - view.eye).dot(view.f).max(f64::MIN_POSITIVE);
        let k = view.units_per_px(depth);
        let mv = view.r * (-d.x * k) + view.u * (d.y * k);
        Camera {
            eye: self.eye + mv,
            target: self.target + mv,
        }
    }

    /// Move sideways so that `q` is in the centre of the view (double
    /// middle-click), keeping the direction and the distance along it.
    pub fn recentre(&self, q: DVec3) -> Camera {
        let f = self.forward();
        let along = (q - self.eye).dot(f);
        let eye = q - f * along;
        Camera { eye, target: q }
    }

    /// Fit the box `lo`..`hi` in a view of the given aspect (width / height),
    /// keeping the view direction.
    pub fn extents(&self, lo: DVec3, hi: DVec3, aspect: f64) -> Camera {
        let c = lo.lerp(hi, 0.5);
        let diag = lo.distance(hi);
        let rad = (diag * 0.5)
            .max(diag.max(c.length()) * 1e-9)
            .max(f64::MIN_POSITIVE);
        let half = (FOV_Y_DEG.to_radians() * 0.5).tan();
        let hf = (half * aspect.min(1.0)).atan();
        let f = self.forward();
        Camera {
            eye: c - f * (rad / hf.sin() * EXTENTS_MARGIN),
            target: c,
        }
    }
}

fn right_of(f: DVec3) -> DVec3 {
    let r = f.cross(DVec3::Z);
    if r.length() < 1e-12 {
        DVec3::X
    } else {
        r.normalize()
    }
}

/// A camera seen through a viewport of `w` by `h` logical pixels.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub eye: DVec3,
    pub f: DVec3,
    pub r: DVec3,
    pub u: DVec3,
    /// Pixels per unit at depth 1.
    pub foc: f64,
    pub w: f64,
    pub h: f64,
    /// A parallel projection's world units per pixel (None: perspective).
    pub parallel: Option<f64>,
}

impl View {
    pub fn new(cam: &Camera, w: f64, h: f64) -> View {
        View::with_lens(cam, w, h, Lens::default())
    }

    /// A camera seen through a lens.
    pub fn with_lens(cam: &Camera, w: f64, h: f64, lens: Lens) -> View {
        let f = cam.forward();
        let r = right_of(f);
        let fov = lens.fov.clamp(FOV_MIN_DEG, FOV_MAX_DEG);
        let foc = (h * 0.5) / (fov.to_radians() * 0.5).tan();
        let parallel = lens.parallel.then(|| cam.distance() / foc);
        View {
            eye: match parallel {
                Some(_) => cam.target - f * (cam.distance() * PARALLEL_RAY_BACK),
                None => cam.eye,
            },
            f,
            r,
            u: r.cross(f),
            foc,
            w,
            h,
            parallel,
        }
    }

    pub fn aspect(&self) -> f64 {
        self.w / self.h.max(1e-9)
    }

    /// Screen position (x right, y down, in pixels) and depth along the view
    /// direction, or `None` if the point is not in front of the eye.
    pub fn to_screen(&self, p: DVec3) -> Option<(DVec2, f64)> {
        let d = p - self.eye;
        let z = d.dot(self.f);
        // A parallel view shows what is behind the eye too.
        if let Some(k) = self.parallel {
            return Some((
                DVec2::new(
                    self.w * 0.5 + d.dot(self.r) / k,
                    self.h * 0.5 - d.dot(self.u) / k,
                ),
                z,
            ));
        }
        if z <= 0.0 {
            return None;
        }
        Some((
            DVec2::new(
                self.w * 0.5 + d.dot(self.r) / z * self.foc,
                self.h * 0.5 - d.dot(self.u) / z * self.foc,
            ),
            z,
        ))
    }

    /// The ray through a screen position: origin and unit direction.
    pub fn ray(&self, s: DVec2) -> (DVec3, DVec3) {
        if let Some(k) = self.parallel {
            // Parallel rays, starting well behind the view.
            let across = self.r * ((s.x - self.w * 0.5) * k) - self.u * ((s.y - self.h * 0.5) * k);
            return (self.eye + across, self.f);
        }
        let d = self.f
            + self.r * ((s.x - self.w * 0.5) / self.foc)
            + self.u * (-(s.y - self.h * 0.5) / self.foc);
        (self.eye, d.normalize())
    }

    /// Where the horizon is on screen (pixels from the top); can be off screen.
    pub fn horizon_y(&self) -> f64 {
        if self.parallel.is_some() {
            // The horizon is at infinity: off the top looking down, off the
            // bottom looking up.
            if self.f.z.abs() < 1e-12 {
                return self.h * 0.5;
            }
            return self.h * 0.5 + self.f.z.signum() * self.h * 1e3;
        }
        let pitch = self.f.z.clamp(-1.0, 1.0).asin();
        self.h * 0.5 + self.foc * pitch.tan()
    }

    /// World units per pixel at a given depth.
    pub fn units_per_px(&self, depth: f64) -> f64 {
        if let Some(k) = self.parallel {
            return k;
        }
        depth / self.foc
    }
}

/// Where the cursor "is" for zoom, orbit and pan: the model under it, else
/// the ground, else a point at `fallback` distance along the ray.
pub fn surface_point(view: &View, s: DVec2, model_hit: Option<DVec3>, fallback: f64) -> DVec3 {
    if let Some(p) = model_hit {
        return p;
    }
    let (o, d) = view.ray(s);
    if d.z < -1e-9 && o.z > 0.0 {
        let t = -o.z / d.z;
        // Very far ground hits near the horizon would make giant steps.
        if t < fallback * 50.0 {
            return o + d * t;
        }
    }
    o + d * fallback
}

/// A short ease between two cameras, for wheel zoom and Zoom Extents.
/// Pure: the caller passes the time step.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    from: Camera,
    to: Camera,
    t: f64,
    duration: f64,
}

/// How long a wheel zoom eases, in seconds.
pub const EASE_WHEEL: f64 = 0.12;
/// How long Zoom Extents eases, in seconds.
pub const EASE_EXTENTS: f64 = 0.25;

impl Motion {
    pub fn new(from: Camera, to: Camera, duration: f64) -> Self {
        Self {
            from,
            to,
            t: 0.0,
            duration,
        }
    }

    /// Where the camera is now.
    pub fn current(&self) -> Camera {
        if self.duration <= 0.0 || self.t >= self.duration {
            return self.to;
        }
        let s = self.t / self.duration;
        self.from.lerp(&self.to, 1.0 - (1.0 - s).powi(3))
    }

    pub fn goal(&self) -> Camera {
        self.to
    }

    /// Advance by `dt` seconds. Returns true while still moving.
    pub fn step(&mut self, dt: f64) -> bool {
        self.t += dt.max(0.0);
        self.t < self.duration
    }

    pub fn done(&self) -> bool {
        self.t >= self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(c: &Camera) -> View {
        View::new(c, 800.0, 600.0)
    }

    #[test]
    fn screen_and_ray_round_trip() {
        let c = Camera::home();
        let v = view(&c);
        let s = DVec2::new(123.0, 456.0);
        let (o, d) = v.ray(s);
        let (back, _) = v.to_screen(o + d * 7.0).expect("in front");
        assert!(back.distance(s) < 1e-9);
        let (centre, _) = v.to_screen(c.target).expect("in front");
        assert!(centre.distance(DVec2::new(400.0, 300.0)) < 1e-9);
    }

    #[test]
    fn recentre_puts_the_point_in_the_middle() {
        let c = Camera::home();
        let q = DVec3::new(1.0, 1.0, 1.0);
        let c2 = c.recentre(q);
        let (s, _) = view(&c2).to_screen(q).expect("in front");
        assert!(s.distance(DVec2::new(400.0, 300.0)) < 1e-9);
        assert!((c2.forward() - c.forward()).length() < 1e-12);
    }

    #[test]
    fn motion_ends_at_the_goal() {
        let a = Camera::home();
        let b = a.zoom_toward(DVec3::ZERO, 0.3);
        let mut m = Motion::new(a, b, EASE_WHEEL);
        assert_eq!(m.current(), a);
        assert!(m.step(0.05));
        let mid = m.current();
        assert!(mid != a && mid != b);
        assert!(!m.step(0.1));
        assert_eq!(m.current(), b);
    }
}
