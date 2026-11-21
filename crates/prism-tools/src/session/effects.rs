//! Effects (brief 47): a short line animation when an edit finishes.
//!
//! One rule for every tool: an edit run through `Session::run` compares
//! the drawn lines before and after it. The new lines animate; when only
//! lines went (Erase), or the edit moved things (Move, Rotate, Scale), the
//! old ones fade where they were. Undo, redo and opening a file do not
//! animate. Time comes from the events, so it replays the same way.

use std::collections::BTreeSet;

use prism_doc::{Change, Command, InstanceId, Motion};
use prism_math::DVec3;
use prism_topo::{EdgeId, FaceId};
use prism_view::effects::{self, Effect, Kind, Seg, Turn};
use prism_view::overlay::Overlay;
use prism_view::scene::FOREIGN;

use super::Session;

/// What kind of effect a command gets, before the shape is known.
#[derive(Clone, Debug)]
pub(super) enum Want {
    Echo,
    Trace,
    Sweep(DVec3),
    /// Erase: the lines that went fade.
    Ghost,
    /// Move, Rotate, Scale in place: the old outline travels.
    Travel(Turn),
    /// A copy: it travels from the original.
    Copy(Turn),
    /// Follow Me: the profile (its outline) and the path.
    Follow(Vec<Seg>, Vec<DVec3>),
    /// Offset: the new loop starts on the face's old outline.
    Morph(Vec<Seg>),
    /// Make Group / Component: the new group's box draws in. The groups
    /// there were before.
    Group(BTreeSet<InstanceId>),
}

/// What the scene held before an edit.
pub(super) struct Before {
    segs: Vec<Seg>,
    /// The curved faces drawn (a new one tiles).
    curved: BTreeSet<FaceId>,
}

fn turn_of(c: &Change) -> Turn {
    match *c {
        Change::Translate { by } => Turn::Move(by),
        Change::Rotate {
            centre,
            axis,
            angle,
        } => Turn::Rotate {
            centre,
            axis,
            angle,
        },
        Change::Scale { centre, factor } => Turn::Scale { centre, factor },
        // No turn to show: the copy appears across the plane.
        Change::Mirror { origin, .. } => Turn::Scale {
            centre: origin,
            factor: DVec3::ONE,
        },
    }
}

impl Session {
    /// Turn effects on or off (View > Effects). Off stops any running.
    pub fn set_effects(&mut self, on: bool) {
        self.effects_on = on;
        if !on && !self.effects.is_empty() {
            self.effects.clear();
            self.overlay_dirty = true;
            self.dirty = true;
        }
    }

    pub fn effects_on(&self) -> bool {
        self.effects_on
    }

    /// How long an effect lasts, seconds (Preferences > Effects).
    pub fn set_effects_length(&mut self, secs: f64) {
        self.effects_length = secs.max(0.05);
    }

    /// The effects running now.
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    fn effect_length(&self) -> f64 {
        if self.reduce_motion {
            effects::REDUCED.min(self.effects_length)
        } else {
            self.effects_length
        }
    }

    /// The drawn lines before an edit; None when effects are off or the
    /// scene is too big to compare.
    pub(super) fn effect_before(&self) -> Option<Before> {
        if !self.effects_on || self.scene.edges.len() > effects::MAX_SCENE {
            return None;
        }
        Some(Before {
            segs: effects::segments(&self.scene),
            curved: self.own_curved().collect(),
        })
    }

    /// The open group's own curved faces in the scene (not other groups').
    fn own_curved(&self) -> impl Iterator<Item = FaceId> + '_ {
        self.scene
            .faces
            .iter()
            .filter(|f| f.curved.is_some() && !f.cap && f.id.0 < FOREIGN)
            .map(|f| f.id)
    }

    /// A flat face's loops in the world, as lines.
    fn face_outline(&self, id: FaceId) -> Vec<Seg> {
        let Some(f) = self.scene.face(id).filter(|f| f.curved.is_none()) else {
            return Vec::new();
        };
        let at = |q: prism_math::DVec2| f.origin + f.u * q.x + f.v * q.y;
        std::iter::once(&f.outer)
            .chain(f.holes.iter())
            .flat_map(|l| (0..l.len()).map(move |i| [at(l[i]), at(l[(i + 1) % l.len()])]))
            .collect()
    }

    /// Follow Me's path in the world, from the end the profile is at (cut
    /// at `end` if it stops partway).
    fn follow_path(
        &self,
        path: &BTreeSet<EdgeId>,
        profile: &[Seg],
        end: Option<DVec3>,
    ) -> Vec<DVec3> {
        let segs: Vec<Seg> = path
            .iter()
            .filter_map(|e| self.scene.edge(*e))
            .flat_map(|e| {
                let pts: Vec<DVec3> = std::iter::once(e.a)
                    .chain(e.mid.iter().copied())
                    .chain(std::iter::once(e.b))
                    .collect();
                pts.windows(2).map(|w| [w[0], w[1]]).collect::<Vec<_>>()
            })
            .collect();
        let Some(mut pts) = effects::chains(&segs).into_iter().max_by_key(|c| c.len()) else {
            return Vec::new();
        };
        let n = profile.len().max(1) as f64;
        let centre = profile.iter().map(|s| s[0]).sum::<DVec3>() / n;
        let near = |p: &DVec3| p.distance(centre);
        let closed = pts.len() > 2 && pts[0].distance(pts[pts.len() - 1]) <= self.scene.tol.dist;
        if closed {
            pts.pop();
            let k = (0..pts.len())
                .min_by(|&a, &b| near(&pts[a]).total_cmp(&near(&pts[b])))
                .unwrap_or(0);
            pts.rotate_left(k);
            pts.push(pts[0]);
        } else if near(&pts[pts.len() - 1]) < near(&pts[0]) {
            pts.reverse();
        }
        if let Some(e) = end {
            // Stop at the piece nearest `end`.
            let k = pts
                .windows(2)
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let d = |w: &[DVec3]| {
                        effects::nearest_on(&[[w[0], w[1]]], e).map_or(f64::MAX, |p| p.distance(e))
                    };
                    d(a).total_cmp(&d(b))
                })
                .map_or(pts.len() - 1, |(k, _)| k + 1);
            pts.truncate(k);
            pts.push(e);
        }
        pts
    }

    /// Which effect a command gets.
    pub(super) fn effect_kind(&self, c: &Command) -> Want {
        match c {
            Command::In { command, .. } => self.effect_kind(command),
            Command::Batch { commands } => {
                commands.first().map_or(Want::Echo, |c| self.effect_kind(c))
            }
            Command::Segment { .. } | Command::Arc { .. } | Command::Spline { .. } => Want::Trace,
            Command::PushPull { face, distance, .. } => self
                .scene
                .face(*face)
                .filter(|f| f.curved.is_none())
                .map_or(Want::Echo, |f| Want::Sweep(f.normal * *distance)),
            Command::Translate { by, .. } => Want::Travel(Turn::Move(*by)),
            Command::Rotate {
                centre,
                axis,
                angle,
                ..
            } => Want::Travel(Turn::Rotate {
                centre: *centre,
                axis: *axis,
                angle: *angle,
            }),
            Command::Scale { centre, factor, .. } => Want::Travel(Turn::Scale {
                centre: *centre,
                factor: *factor,
            }),
            Command::Place { change, .. } => Want::Travel(turn_of(change)),
            Command::Copy { step, count: 1, .. } => Want::Copy(match *step {
                Motion::Translate { by } => Turn::Move(by),
                Motion::Rotate {
                    centre,
                    axis,
                    angle,
                } => Turn::Rotate {
                    centre,
                    axis,
                    angle,
                },
            }),
            Command::CopyInstances { step, count: 1, .. } => Want::Copy(turn_of(step)),
            Command::Erase { .. } | Command::EraseInstances { .. } => Want::Ghost,
            Command::FollowMe { path, profile, end } => {
                let outline = self.face_outline(*profile);
                let path = self.follow_path(path, &outline, *end);
                if outline.is_empty() || path.len() < 2 {
                    Want::Echo
                } else {
                    Want::Follow(outline, path)
                }
            }
            Command::Offset { face, .. } => Want::Morph(self.face_outline(*face)),
            Command::MakeGroup { .. } | Command::MakeComponent { .. } => {
                let asm = self.doc.assembly();
                let def = asm.context_def(&self.ctx).unwrap_or(prism_doc::ROOT);
                Want::Group(
                    asm.def(def)
                        .map(|d| d.instances.keys().copied().collect())
                        .unwrap_or_default(),
                )
            }
            _ => Want::Echo,
        }
    }

    /// After an edit: start its effect.
    pub(super) fn effect_after(&mut self, before: Option<Before>, want: Want) {
        let Some(before) = before else {
            return;
        };
        let tol = self.scene.tol.dist;
        let after = effects::segments(&self.scene);
        let (made, gone) = effects::diff(&before.segs, &after, tol);
        let mut e = match want {
            Want::Group(old) => {
                let asm = self.doc.assembly();
                let def = asm.context_def(&self.ctx).unwrap_or(prism_doc::ROOT);
                let new: Vec<InstanceId> = asm
                    .def(def)
                    .map(|d| {
                        d.instances
                            .keys()
                            .copied()
                            .filter(|i| !old.contains(i))
                            .collect()
                    })
                    .unwrap_or_default();
                let segs: Vec<Seg> = new
                    .iter()
                    .filter_map(|i| self.group_box(*i))
                    .flat_map(|c| {
                        (0..8usize).flat_map(move |i| {
                            [1usize, 2, 4]
                                .into_iter()
                                .filter(move |b| i & b == 0)
                                .map(move |b| [c[i], c[i | b]])
                        })
                    })
                    .collect();
                Effect::new(Kind::Draw, segs, self.time)
            }
            Want::Ghost | Want::Travel(_) if made.is_empty() && gone.is_empty() => return,
            Want::Ghost => Effect::new(Kind::Ghost, gone, self.time),
            Want::Travel(turn) if !gone.is_empty() => {
                let mut e = Effect::new(Kind::Travel(turn), gone, self.time);
                e.pins = effects::pins(&e.segs, &after, tol);
                e
            }
            _ if made.is_empty() => Effect::new(Kind::Ghost, gone, self.time),
            Want::Copy(turn) => {
                let back = turn.at(1.0).inverse();
                let from: Vec<Seg> = made
                    .iter()
                    .map(|s| [back.transform_point3(s[0]), back.transform_point3(s[1])])
                    .collect();
                Effect::new(Kind::Travel(turn), from, self.time)
            }
            Want::Trace => Effect::new(Kind::Trace, made, self.time),
            Want::Sweep(v) => Effect::new(Kind::Sweep(v), made, self.time),
            Want::Follow(profile, path) => Effect::new(Kind::Follow(path), profile, self.time),
            Want::Morph(old) => {
                let from = made
                    .iter()
                    .map(|s| s.map(|p| effects::nearest_on(&old, p).unwrap_or(p)))
                    .collect();
                let mut e = Effect::new(Kind::Morph, made, self.time);
                e.from = from;
                e
            }
            Want::Echo | Want::Travel(_) => self.tile_or_echo(&before.curved, made),
        };
        e.start = self.time;
        self.effect_push(e);
    }

    /// New curved faces tile; anything else echoes.
    fn tile_or_echo(&self, old: &BTreeSet<FaceId>, made: Vec<Seg>) -> Effect {
        let new: Vec<usize> = self
            .own_curved()
            .filter(|f| !old.contains(f))
            .filter_map(|f| self.scene.face_index.get(&f).copied())
            .collect();
        if !new.is_empty() {
            let mut segs = effects::surface_grid(&self.scene, &new);
            segs.extend(made.iter().copied());
            if segs.len() <= effects::MAX_TILE {
                let dir = effects::long_axis(&segs);
                return Effect::new(Kind::Tile(dir), segs, self.time);
            }
        }
        Effect::new(Kind::Echo, made, self.time)
    }

    /// Start an effect (when they are on and it is not too big).
    pub(super) fn effect_push(&mut self, e: Effect) {
        let most = match e.kind {
            Kind::Tile(_) => effects::MAX_TILE,
            _ => effects::MAX_SEGMENTS,
        };
        if !self.effects_on || e.segs.is_empty() || e.segs.len() > most {
            return;
        }
        if self.effects.len() >= effects::MAX_RUNNING {
            self.effects.remove(0);
        }
        self.effects.push(e);
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// A plane's outline echoes when it is placed.
    pub(super) fn effect_outline(&mut self, c: [DVec3; 4]) {
        let segs = (0..4).map(|i| [c[i], c[(i + 1) % 4]]).collect();
        self.effect_push(Effect::new(Kind::Echo, segs, self.time));
    }

    /// A new dimension traces from one end, along its line, to the other.
    pub(super) fn effect_dimension(&mut self, a: DVec3, b: DVec3, offset: DVec3) {
        let segs = vec![[a, a + offset], [a + offset, b + offset], [b + offset, b]];
        self.effect_push(Effect::new(Kind::Trace, segs, self.time));
    }

    /// Drop the effects that are over; keep drawing while any run.
    pub(super) fn effects_age(&mut self) {
        if self.effects.is_empty() {
            return;
        }
        let (now, len) = (self.time, self.effect_length());
        self.effects.retain(|e| now - e.start < len);
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Undo, redo or a new document: what was animating is not there.
    pub(super) fn effects_clear(&mut self) {
        if !self.effects.is_empty() {
            self.effects.clear();
            self.overlay_dirty = true;
            self.dirty = true;
        }
    }

    pub(super) fn effects_overlay(&self, o: &mut Overlay) {
        let len = self.effect_length();
        let color = self.style.feedback.effect;
        for e in &self.effects {
            let t = (self.time - e.start) / len;
            o.lines
                .extend(effects::lines(e, t, self.reduce_motion, color));
        }
    }
}
