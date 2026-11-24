//! Duplicate Again (Ctrl+D): repeat the last copy. After a Move or Rotate
//! that made a copy, each press makes the next copy one more step along:
//! the same distance in the same direction, or the same turn about the same
//! centre. Hit it several times and you get a chain (a row of fence posts,
//! a ring of bolts). `x5` typed after a copy counts too: Ctrl+D then makes
//! the sixth.
//!
//! The copies are always made from what was copied (not from the last
//! copy), `k` steps away, so the chain stays exact and never drifts. If the
//! selection is something else, or the model has changed since, the same
//! step is made from what is selected now, as the first of a new chain.

use std::collections::BTreeSet;

use prism_doc::InstanceId;
use prism_ops::select::Selection;

use super::Session;
use super::edit::EditOp;

pub const DUP_NOTHING: &str =
    "Copy something with Move or Rotate (tap Ctrl) first, then Ctrl+D repeats it";
pub const DUP_SELECT: &str = "Select something to duplicate first";

/// What Ctrl+D repeats.
#[derive(Clone, Debug)]
pub(super) struct DupMemo {
    /// The copy step: a Move or Rotate with `copy` on.
    op: EditOp,
    /// What was copied, and where we were.
    selection: Selection,
    groups: BTreeSet<InstanceId>,
    ctx: Vec<InstanceId>,
    /// Copies made so far in this chain.
    k: u32,
    /// The document as the last copy left it.
    rev: u64,
}

impl Session {
    /// A copy by Move or Rotate was just made (`k` copies in the chain).
    pub(super) fn note_dup(&mut self, op: &EditOp, k: u32) {
        let copy = matches!(
            op,
            EditOp::Move { copy: true, .. } | EditOp::Rotate { copy: true, .. }
        );
        if !copy {
            return;
        }
        self.dup = Some(DupMemo {
            op: op.clone(),
            selection: self.selection.clone(),
            groups: self.groups.clone(),
            ctx: self.ctx.clone(),
            k,
            rev: self.doc.revision(),
        });
    }

    /// Edit > Duplicate Again.
    pub(super) fn duplicate_again(&mut self) {
        let Some(memo) = self.dup.clone() else {
            return self.dup_notice(DUP_NOTHING);
        };
        if self.selection.is_empty() && self.groups.is_empty() {
            return self.dup_notice(DUP_SELECT);
        }
        if self.locked_selected() {
            return;
        }
        // The next in the chain, or the first of a new one.
        let chained = memo.rev == self.doc.revision()
            && memo.selection == self.selection
            && memo.groups == self.groups
            && memo.ctx == self.ctx;
        let k = if chained { memo.k + 1 } else { 1 };
        let kf = f64::from(k);
        self.cancel();
        let verts = self.selection.all_vertices(self.work_model());
        let step = match memo.op.clone() {
            EditOp::Move { from, delta, .. } => EditOp::Move {
                from,
                selection: self.selection.clone(),
                verts,
                delta: delta * kf,
                copy: true,
                // A copy is not the joint's own part.
                joint: None,
            },
            EditOp::Rotate {
                centre,
                normal,
                u0,
                u1,
                angle,
                ..
            } => EditOp::Rotate {
                centre,
                normal,
                u0,
                u1,
                angle: angle * kf,
                selection: self.selection.clone(),
                verts,
                copy: true,
                pressed: None,
                joint: None,
            },
            _ => return,
        };
        let Some(command) = self.transform_command(&step) else {
            return;
        };
        if self.run(command) {
            // The memo keeps the one-step op, with this chain's `k`.
            let one = memo.op;
            self.note_dup(&one, k);
        }
    }

    fn dup_notice(&mut self, text: &'static str) {
        self.notice = Some(text);
        self.overlay_dirty = true;
        self.dirty = true;
    }
}
