//! Effects (brief 47): a short line animation when an edit finishes.
//!
//! One rule for every tool: an edit run through `Session::run` compares
//! the drawn lines before and after it. The new lines animate; when only
//! lines went (Erase), or the edit moved things (Move, Rotate, Scale), the
//! old ones fade where they were. Undo, redo and opening a file do not
//! animate. Time comes from the events, so it replays the same way.

use prism_doc::Command;
use prism_view::effects::{self, Effect, Kind, Seg};
use prism_view::overlay::Overlay;

use super::Session;

/// What kind of effect a command gets, before the shape is known.
#[derive(Clone, Copy, Debug)]
pub(super) enum Want {
    Echo,
    Trace,
    Sweep(prism_math::DVec3),
    Ghost,
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

    /// The effects running now.
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    fn effect_length(&self) -> f64 {
        if self.reduce_motion {
            effects::REDUCED
        } else {
            effects::DURATION
        }
    }

    /// The drawn lines before an edit; None when effects are off or the
    /// scene is too big to compare.
    pub(super) fn effect_before(&self) -> Option<Vec<Seg>> {
        if !self.effects_on || self.scene.edges.len() > effects::MAX_SCENE {
            return None;
        }
        Some(effects::segments(&self.scene))
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
            Command::Translate { .. }
            | Command::Rotate { .. }
            | Command::Scale { .. }
            | Command::Place { .. }
            | Command::Erase { .. } => Want::Ghost,
            _ => Want::Echo,
        }
    }

    /// After an edit: start its effect.
    pub(super) fn effect_after(&mut self, before: Option<Vec<Seg>>, want: Want) {
        let Some(before) = before else {
            return;
        };
        let after = effects::segments(&self.scene);
        let (made, gone) = effects::diff(&before, &after, self.scene.tol.dist);
        let (kind, segs) = match want {
            Want::Ghost if !gone.is_empty() => (Kind::Ghost, gone),
            _ if made.is_empty() => (Kind::Ghost, gone),
            Want::Trace => (Kind::Trace, made),
            Want::Sweep(v) => (Kind::Sweep(v), made),
            Want::Echo | Want::Ghost => (Kind::Echo, made),
        };
        if segs.is_empty() || segs.len() > effects::MAX_SEGMENTS {
            return;
        }
        if self.effects.len() >= effects::MAX_RUNNING {
            self.effects.remove(0);
        }
        self.effects.push(Effect {
            kind,
            segs,
            start: self.time,
        });
        self.overlay_dirty = true;
        self.dirty = true;
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
