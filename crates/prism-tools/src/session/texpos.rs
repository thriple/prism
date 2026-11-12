//! Texture > Position (Basic Materials part 3, brief 28): move, turn and
//! size a texture on a flat face with two pins, as in SketchUp.
//!
//! Pick the tool with a textured face selected, or click a textured face
//! with it. Two pins sit on a corner of one copy of the image: drag the
//! **red pin** (or anywhere else on the face) to move the texture; drag the
//! **green pin** to turn it and change its size about the red pin. Pins
//! snap with the inference engine to points on the face's plane. Enter, or
//! a click off the face, keeps the placement as one command
//! (`Command::PlaceTexture`); Esc puts it back. Curved faces keep their
//! exact coordinates, so the tool takes flat faces only.

use prism_doc::Command;
use prism_math::{DAffine3, DQuat, DVec2, DVec3, Plane};
use prism_topo::{FaceId, UvMap};
use prism_view::overlay::{Line, Mark, Overlay, Shape};
use prism_view::{Scene, View};

use super::Session;

pub const TEXTURE: &str = "tool.texture";

/// Said when the tool is given something it cannot position.
pub const NOT_TEXTURED: &str =
    "Click a flat face painted with a textured material (curved faces keep their own placement)";

/// The pin being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pin {
    /// Moves the texture.
    Move,
    /// Turns and sizes it about the red pin.
    Turn,
}

/// A texture being positioned.
#[derive(Clone, Debug)]
pub(super) struct TexPos {
    face: FaceId,
    plane: Plane,
    /// The stored placement before (None: the default).
    before: Option<UvMap>,
    /// The placement as it is now.
    map: UvMap,
    /// One copy of the image, in metres.
    size: [f64; 2],
    grab: Option<Grab>,
}

#[derive(Clone, Copy, Debug)]
struct Grab {
    pin: Pin,
    /// Where the drag started, and the placement then.
    from: DVec3,
    map0: UvMap,
    /// The red and green pins when the drag started.
    red: DVec3,
    green: DVec3,
}

/// The point on `plane` with texture coordinates `uv` under `map`.
fn world(map: &UvMap, plane: &Plane, uv: DVec2) -> DVec3 {
    let n = plane.normal;
    let (u, v) = (map.u - n * map.u.dot(n), map.v - n * map.v.dot(n));
    let o = plane.project(map.origin);
    let (a, b, c) = (u.dot(u), u.dot(v), v.dot(v));
    let det = a * c - b * b;
    if det.abs() <= f64::MIN_POSITIVE {
        return o;
    }
    // (p - o)·u = uv.x and (p - o)·v = uv.y, with p - o = s u + t v.
    let s = (uv.x * c - uv.y * b) / det;
    let t = (uv.y * a - uv.x * b) / det;
    o + u * s + v * t
}

impl TexPos {
    pub(super) fn face(&self) -> FaceId {
        self.face
    }

    /// The red pin: the corner of the copy of the image nearest the
    /// middle of the face (so the pins sit on the face), and the green
    /// pin one image-width along.
    fn pins(&self, middle: DVec3) -> (DVec3, DVec3) {
        if let Some(g) = &self.grab {
            // While dragging, the pins follow the placement from where
            // they were.
            let t = |p: DVec3| {
                let uv = g.map0.at(p);
                world(&self.map, &self.plane, uv)
            };
            return (t(g.red), t(g.green));
        }
        let [w, h] = self.size;
        let q = self.map.at(middle);
        let corner = DVec2::new((q.x / w).floor() * w, (q.y / h).floor() * h);
        (
            world(&self.map, &self.plane, corner),
            world(&self.map, &self.plane, corner + DVec2::new(w, 0.0)),
        )
    }
}

impl Session {
    /// Start positioning the texture on `face`, if it is flat and textured.
    pub(super) fn texpos_start(&mut self, face: FaceId) -> bool {
        let m = self.work_model();
        let Some(f) = m.face(face) else { return false };
        let Some(plane) = f.plane().copied() else {
            return false;
        };
        let asm = self.doc.assembly();
        let size = [f.paint.front, f.paint.back]
            .into_iter()
            .flatten()
            .find_map(|id| asm.material(id).and_then(|x| x.texture.as_ref()))
            .map(|t| t.size);
        let Some(size) = size.filter(|s| s[0] > 0.0 && s[1] > 0.0) else {
            return false;
        };
        let (before, map) = (f.paint.uv, f.texture_map().expect("flat"));
        self.texpos = Some(TexPos {
            face,
            plane,
            before,
            map,
            size,
            grab: None,
        });
        self.texpos_notice = None;
        self.texpos_preview();
        true
    }

    /// The tool was picked: a textured face already selected starts it.
    pub(super) fn texpos_picked(&mut self) {
        let faces: Vec<FaceId> = self.selection.faces.iter().copied().collect();
        if let [f] = faces[..] {
            self.texpos_start(f);
        }
    }

    /// Whether a texture is being positioned.
    pub(super) fn texpos_active(&self) -> bool {
        self.texpos.is_some()
    }

    /// The middle of the face being positioned.
    fn texpos_middle(&self, t: &TexPos) -> DVec3 {
        let p = self
            .scene
            .face(t.face)
            .map_or(t.plane.origin, |f| f.lo.lerp(f.hi, 0.5));
        t.plane.project(p)
    }

    /// The pins, for tests and the overlay.
    pub fn texture_pins(&self) -> Option<(DVec3, DVec3)> {
        let t = self.texpos.as_ref()?;
        Some(t.pins(self.texpos_middle(t)))
    }

    /// The placement being shown (for tests).
    pub fn texture_map_now(&self) -> Option<UvMap> {
        self.texpos.as_ref().map(|t| t.map)
    }

    /// Where the cursor is on the face's plane: the inference's point if
    /// it lies on the plane, else the ray through the cursor.
    fn texpos_point(&self, t: &TexPos, s: DVec2, view: &View) -> Option<DVec3> {
        if let Some(i) = &self.inference
            && t.plane.distance(i.point).abs() <= self.scene.tol.dist
        {
            return Some(i.point);
        }
        let (o, d) = view.ray(s);
        let k = d.dot(t.plane.normal);
        if k.abs() <= 1e-12 {
            return None;
        }
        let x = (t.plane.origin - o).dot(t.plane.normal) / k;
        (x > 0.0).then(|| o + d * x)
    }

    pub(super) fn texpos_press(&mut self, s: DVec2) {
        let view = self.view();
        let Some(t) = self.texpos.clone() else {
            // Not started: a click on a textured face starts it.
            let (o, d) = view.ray(s);
            let hit = self
                .scene
                .ray_face(o, d)
                .filter(|(f, _)| !self.scene.face_owner.contains_key(f));
            match hit {
                Some((f, _)) if self.texpos_start(f) => {}
                _ => self.texpos_notice = Some(NOT_TEXTURED),
            }
            return;
        };
        let (red, green) = t.pins(self.texpos_middle(&t));
        let near = |p: DVec3| {
            view.to_screen(p)
                .is_some_and(|(q, _)| q.distance(s) <= self.settings.aperture.max(8.0))
        };
        let on_face = {
            let (o, d) = view.ray(s);
            self.scene.ray_face(o, d).is_some_and(|(f, _)| f == t.face)
        };
        let pin = if near(green) {
            Pin::Turn
        } else if near(red) || on_face {
            Pin::Move
        } else {
            // A click off the face keeps the placement.
            self.texpos_commit();
            return;
        };
        let from = match pin {
            Pin::Turn => green,
            Pin::Move if near(red) => red,
            Pin::Move => self.texpos_point(&t, s, &view).unwrap_or(red),
        };
        if let Some(t) = &mut self.texpos {
            t.grab = Some(Grab {
                pin,
                from,
                map0: t.map,
                red,
                green,
            });
        }
        self.overlay_dirty = true;
    }

    /// Dragging a pin (after inference ran).
    pub(super) fn texpos_hover(&mut self, s: DVec2, view: &View) {
        let Some(t) = &self.texpos else { return };
        let Some(g) = t.grab else {
            self.overlay_dirty = true;
            return;
        };
        let Some(q) = self.texpos_point(t, s, view) else {
            return;
        };
        let n = t.plane.normal;
        let m = match g.pin {
            Pin::Move => DAffine3::from_translation(q - g.from),
            Pin::Turn => {
                let (a, b) = (g.green - g.red, q - g.red);
                let k = b.length() / a.length().max(f64::MIN_POSITIVE);
                if k <= 1e-6 || !k.is_finite() {
                    return;
                }
                let angle = a.cross(b).dot(n).atan2(a.dot(b));
                DAffine3::from_translation(g.red)
                    * DAffine3::from_quat(DQuat::from_axis_angle(n, angle))
                    * DAffine3::from_scale(DVec3::splat(k))
                    * DAffine3::from_translation(-g.red)
            }
        };
        if let Some(t) = &mut self.texpos {
            t.map = g.map0.transformed(&m);
        }
        self.texpos_preview();
    }

    pub(super) fn texpos_release(&mut self) {
        if let Some(t) = &mut self.texpos {
            t.grab = None;
        }
        self.overlay_dirty = true;
    }

    /// Enter or a click off the face: keep the placement (one command).
    pub(super) fn texpos_commit(&mut self) {
        let Some(t) = self.texpos.take() else { return };
        self.set_preview_scene(None);
        let changed = t.before != Some(t.map)
            && !(t.before.is_none() && t.map == UvMap::default_for(&t.plane));
        if changed {
            self.run(Command::PlaceTexture {
                faces: [t.face].into(),
                map: Some(t.map),
            });
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Esc: put it back.
    pub(super) fn texpos_cancel(&mut self) {
        if self.texpos.take().is_some() {
            self.set_preview_scene(None);
        }
    }

    /// The scene with the face's texture where it is now.
    fn texpos_preview(&mut self) {
        let Some(t) = &self.texpos else { return };
        let mut scene: Scene = self.scene.clone();
        if let Some(&k) = scene.face_index.get(&t.face) {
            for i in 0..scene.mesh.positions.len() {
                if scene.mesh.faces[i] as usize == k {
                    scene.mesh.uvs[i] = t.map.at(scene.mesh.positions[i]);
                }
            }
        }
        self.set_preview_scene(Some(scene));
        self.overlay_dirty = true;
    }

    /// Texture > Reset Position: the selected flat faces go back to the
    /// default placement.
    pub(super) fn texture_reset(&mut self) {
        let m = self.work_model();
        let faces: std::collections::BTreeSet<FaceId> = self
            .selection
            .faces
            .iter()
            .copied()
            .filter(|f| m.face(*f).is_some_and(|x| x.paint.uv.is_some()))
            .collect();
        if faces.is_empty() {
            return;
        }
        self.cancel();
        self.run(Command::PlaceTexture { faces, map: None });
        self.mark_all_dirty();
    }

    pub(super) fn texpos_hint(&self) -> Option<&'static str> {
        if self.tool != TEXTURE {
            return None;
        }
        Some(match &self.texpos {
            None => self
                .texpos_notice
                .unwrap_or("Click a textured face to position its texture (flat faces only)"),
            Some(t) if t.grab.is_some() => "Drag to place · release to drop the pin",
            Some(_) => {
                "Drag the red pin (or the face) to move · the green pin turns and sizes · Enter or click off the face to finish · Esc cancels"
            }
        })
    }

    /// The pins and one copy of the image outlined.
    pub(super) fn texpos_overlay(&self, o: &mut Overlay) {
        let Some(t) = &self.texpos else { return };
        let fb = &self.style.feedback;
        let (red, green) = t.pins(self.texpos_middle(t));
        let corner = t.map.at(red);
        let [w, h] = t.size;
        let c = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]
            .map(|(x, y)| world(&t.map, &t.plane, corner + DVec2::new(x, y)));
        for k in 0..4 {
            o.lines.push(Line {
                a: c[k],
                b: c[(k + 1) % 4],
                color: fb.rubber_band,
                width: 1.2,
                dash: [4.0, 3.0],
            });
        }
        for (at, color) in [(red, self.style.axes[0]), (green, self.style.axes[1])] {
            o.marks.push(Mark {
                at,
                shape: Shape::Ring,
                color,
                size: fb.woken_size * 1.6,
            });
        }
    }
}
