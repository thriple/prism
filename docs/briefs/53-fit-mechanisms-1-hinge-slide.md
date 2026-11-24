# Fit & Mechanisms part 1: Hinge and Slide joints (owner request, 2026-11-24, pushed ahead of schedule)

**Why now, out of order.** Fit & Mechanisms is next in PLAN.md's build order, but Shapes & Placement (the chunk before it) still has two parts left (Bend/Twist/Taper, Unfold/Lay Flat). The owner is working on vehicles for Beverly (their Bevy game editor) and needs doors, wheels and piston-style shock absorbers to work, so this one part is pushed ahead; Shapes & Placement's own last two parts are paused, to finish once this is done (PLAN.md 6.0, "Order").

**Goal.** A Hinge joint (rotate about an axis, limit optional) and a Slide joint (move along an axis, range optional) between two parts: pick them, pick an axis, optionally bound the travel, then Rotate or Move one part as usual, typed value and all. Hinge covers a door directly; left with no limit, it covers a wheel too. Slide with a range covers a shock absorber's piston sliding in its housing. Fit & Mechanisms' other two joint kinds (Ball, Fixed), the tolerance modifier, Fit Helper/Check Fit and Sweep Motion Range are later parts of this same chunk, not this one.

**Read first:**
- PLAN.md section 1.15 (the whole "Joints and mechanisms" block this part narrows down to Hinge and Slide) and the Tools row in section 1.4 (Hinge and Slide are listed as their own tools, beside Section Plane and Axes).
- `docs/briefs/36-group-axes.md` and `crates/prism-tools/src/session/axes.rs` — the closest existing tool: clicks with the inference engine to set an axis/pivot, a gizmo, stored as document state. Hinge and Slide's own axis-picking (edge, a hole or cylinder's axis, a face normal) reuses the same inference, between two parts instead of inside one group.
- `crates/prism-tools/src/session/edit.rs` (Move and Rotate) — where their axes and the measurement box's typed value live; this part makes Rotate pick up a Hinge's axis and Move pick up a Slide's, each clamping to its limit, when the selection is one side of a joint.
- `crates/prism-doc/src/assembly.rs` and `crates/prism-file/src/lib.rs` (`SCHEMA`, currently 18) — how a new document entity (Section Plane, Construction Plane, Dimension) gets added, undoably, with its own schema bump; a joint is the same shape of thing, between two instances instead of on one face.
- `docs/UI-STYLE.md` (the `DynamicIsland` row) and `docs/ui-kit/` — the owner asked (2026-11-24) that a joint's settings be handled physically in the viewport wherever possible, not as Inspector fields, with only what truly can't be physicalized moved to a small island at the bottom centre of the screen, in the kit's `DynamicIsland` style (used today for status toasts; this is a second, contextual use of the same component).

## Design decisions (none of this is in docs/DECISIONS.md yet; these are this brief's defaults — say so if any should be different)

- **One `Joint` document object, two kinds.** Rather than two parallel data shapes, a joint is `{ a, b: InstanceId, axis: (point, direction), kind: Hinge | Slide, limit: Option<(f64, f64)>, stopped: bool }` — `limit` is degrees for a Hinge, model units for a Slide; `stopped` is this joint's own motion stop, not the same thing as the existing Lock/Unlock on a group (which refuses edits entirely) — named differently on purpose, to avoid the two being confused. Its own id, undoable, saved in the file (schema 19), drawn as a small gizmo that follows Hide and can be hidden on its own: a pin with a swept arc (or full circle, unlimited) for a Hinge; a short rail with end caps at the limits (or arrows, unlimited) for a Slide.
- **The two parts are two instances** (groups or components) in the same open context — siblings, the way the booleans in Solids & CAD take two solid groups. A loose-geometry side is out of scope for this part (a joint wants something to carry on both sides); if the owner needs that, it can follow.
- **The axis** is a line (a point and a direction), picked with the normal inference against either part: an edge, a hole or cylinder's axis, or a face normal — the same candidates Change Axes and the Array tool already offer. Stored in the world, not relative to either part, since either part can move independently once the joint exists (within its limit). For a Slide this is also the line the part is free to move along.
- **The limit is set by dragging, not by typing into a field.** Each bound (a Hinge's min and max angle; a Slide's min and max distance) has its own small handle on the gizmo (described under "the gizmo," below): drag one out from the gizmo's rest position to set that bound where you release it, the same "drag to the limit" feel the Fillet handle and Angle Guides already have, measured from the pose the joint was made at (its own zero). Drag a set handle back past its own start (or Delete while it is selected) to clear that bound again. No limit on either side means a Hinge turns freely (the wheel case) and a Slide moves without a stop either way.
- **Stopped is a click on the gizmo**, not a panel toggle: a small pin mark (not a padlock, to keep it visually apart from Lock/Unlock's own icon) at the joint's centre, the same "click an icon to flip a state" feel Hide already has in the Outliner. Stopped holds that part at its current pose — it still can't be dragged or typed past it — without deleting the joint: useful for a shock absorber you want posed at full compression while you model around it, or a door you want to stop swinging for a moment.
- **The island, for the one thing dragging can't do well: an exact number.** While the Hinge or Slide tool is active, or an existing joint is selected, a small island (`DynamicIsland`, bottom centre) shows each bound that is set, as a typed field (so "drag it to roughly 90°" can be corrected to exactly 90°), and a Remove Joint button. Nothing else lives there — no Stopped toggle, no part pickers, no axis fields; those are all on the gizmo itself or redone with the tool. The island closes when nothing Hinge/Slide-related is selected and no such tool is active.
- **Moving:** with a Hinge in place, Rotate on one of its two parts finds the joint, uses its axis instead of the usual plane-normal guess, and clamps the drag or typed angle to the limit if there is one. With a Slide in place, Move does the same along the joint's axis, clamped to its range — a piston dragged or typed past its stop stops there instead. Rotating/Moving a selection that includes both of a joint's parts, or geometry with no joint, is unaffected. A joint beyond its limit (another edit moved a part) is left alone, not snapped back; Model Health already has the shape for "flag it, don't block" if that is ever wanted.
- **Saved poses (Open, Closed) are out of scope for this part.** PLAN 1.15 says they are "stored as scenes," but Modes & Scenes is not built yet (a Presentation-tier chunk, after Rendering in the order) — there is nothing to store them in. Left for whichever part needs it once Modes & Scenes exists.
- **Sweep Motion Range, collision and clearance are out of scope here too** — they need Ball and Fixed too, and the tolerance modifier, to mean much, and are later parts.

## How it works

- **The Hinge tool:** click the first part, click the second, then pick the axis with the inference engine against either one, as a third click; Enter or a click on the axis finishes it. Esc at any point cancels. One undo step. **The Slide tool** is the same three clicks, making a Slide instead.
- **The gizmo:** for a Hinge, a short pin along the axis through both parts' combined box, with two small drag handles sitting on a full circle around it (one per bound); dragging one sweeps an arc from the joint's zero to where it is released, and the arc stays drawn once set. For a Slide, a short rail along the axis with a drag handle at each end sitting on open-ended guide lines; dragging one draws a stop at that distance, and the stop stays once set. A set bound's handle is drawn solid; an unset one is a faint outline so it is clear it can still be dragged out. The pin mark (Stopped) sits at the centre. All of it selectable and hideable like a Section Plane.
- **Rotating or moving:** select one of a joint's two parts and use Rotate (Hinge) or Move (Slide) as usual; the axis comes from the joint, and a set bound clamps the drag and any typed value, in the measurement box, to it.
- **Right-click / Edit menu:** Remove Joint on a selected one (also on the island); Hide same as any other hideable entity.

## Out of scope (this part; later parts of Fit & Mechanisms unless noted)

- Ball and Fixed joints.
- The tolerance modifier and machine/material profiles.
- Fit Helper and Check Fit.
- Sweep Motion Range.
- Saved poses (needs Modes & Scenes, not built).
- Closed loops of joints (a four-bar linkage) — PLAN 1.15 already defers this to "later," past even the other joint kinds.
- Collision between the two parts while moving or rotating (that's Sweep Motion Range's job).
- Cylindrical and planar joints ("can follow," per PLAN 1.15 — not asked for yet).

## Tests

- Making a Hinge and a Slide: two boxes each, an axis from a picked edge; undo removes either, redo brings it back exactly (parts, axis, kind, limit).
- A Hinge with no limit rotates its part all the way round, repeatedly, never refusing (the wheel case). A Slide with no range moves its part either way without a stop.
- Dragging a bound's handle on the gizmo sets that bound where released; dragging it back past its own start clears it again; the island's typed field shows and can correct the dragged value exactly.
- A Hinge with a limit clamps both a dragged and a typed Rotate angle to it; a Slide with a range clamps both a dragged and a typed Move distance to it — the measurement box shows the clamped value, not the requested one (the shock-absorber case: a piston pushed or typed past its stop stays at the stop).
- Rotating (or moving) the *other* side of the same joint acts about the same axis.
- Removing a joint leaves Rotate and Move working as they did before it existed (plane-normal guess, no clamp).
- Clicking the pin mark toggles Stopped; a Stopped joint's part does not move or rotate past its own current pose even within its own limit, and is unaffected by the existing group Lock/Unlock (the two are independent).
- The island appears while the Hinge or Slide tool is active, or an existing joint is selected, and closes otherwise; it never shows a part picker, an axis field or a Stopped toggle.
- The gizmo is picked, hidden, and excluded when hidden (Hidden is free, section 1.14).
- A file round-trip (schema 19) keeps every joint's parts, axis, kind and limit exactly.
- MCP: list joints, read one's fields, create and remove one of either kind, matching the pattern `get_inspector`/`set_inspector` already use for other entities (the island is a viewport overlay, not an Inspector panel, so MCP's read/write still goes through the joint's own fields, not an Inspector schema).

## Acceptance

- [x] A door (one group hinged to another) swings open and closed with a limit, by dragging or typing an angle.
- [x] A wheel (one group hinged to a chassis, no limit) spins freely through Rotate.
- [x] A shock absorber (a piston group slid in a housing group, with a range) compresses and extends by dragging or typing a distance, and stops at its travel limit.
- [x] A joint's limits are set by dragging its gizmo, not by opening a settings panel; the only floating control is the bottom-centre island, and only while it's relevant.
- [x] PLAN, NOTES and this brief's checkboxes are updated when the part is done.
