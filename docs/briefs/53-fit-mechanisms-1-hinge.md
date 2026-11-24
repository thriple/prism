# Fit & Mechanisms part 1: the Hinge joint (owner request, 2026-11-24, pushed ahead of schedule)

**Why now, out of order.** Fit & Mechanisms is next in PLAN.md's build order, but Shapes & Placement (the chunk before it) still has two parts left (Bend/Twist/Taper, Unfold/Lay Flat). The owner is working on vehicles for Beverly (their Bevy game editor) and needs doors and wheels to turn, so this one part is pushed ahead; Shapes & Placement's own last two parts are paused, to finish once this is done (PLAN.md 6.0, "Order").

**Goal.** A Hinge joint between two parts: pick them, pick an axis, optionally limit the swing, then Rotate one part about that axis like anything else, typed angle and all. Covers a door on its frame directly. A wheel just leaves the limit off, so it spins freely — Fit & Mechanisms' other three joint kinds (Slide, Ball, Fixed), the tolerance modifier, Fit Helper/Check Fit and Sweep Motion Range are later parts of this same chunk, not this one.

**Read first:**
- PLAN.md section 1.15 (the whole "Joints and mechanisms" block this part narrows down to just Hinge) and the Tools row in section 1.4 (Hinge is listed as its own tool, beside Section Plane and Axes).
- `docs/briefs/36-group-axes.md` and `crates/prism-tools/src/session/axes.rs` — the closest existing tool: three clicks with the inference engine to set an axis/pivot, a gizmo, stored as document state a group carries. Hinge's own axis-picking (edge, a hole or cylinder's axis, a sphere's centre (not applicable here), a face normal) reuses the same inference, between two parts instead of inside one group.
- `crates/prism-tools/src/session/edit.rs` (Move and Rotate) — where Rotate's axis and the measurement box's typed angle live; this part makes Rotate pick up a joint's axis and clamp to its limit when the selection is one side of one.
- `crates/prism-doc/src/assembly.rs` and `crates/prism-file/src/lib.rs` (`SCHEMA`, currently 18) — how a new document entity (Section Plane, Construction Plane, Dimension) gets added, undoably, with its own schema bump; a Hinge is the same shape of thing, between two instances instead of on one face.
- `docs/briefs/48-inspector.md` — the schema-driven panel, for the joint's own Inspector fields (limits, a Locked toggle).

## Design decisions (none of this is in docs/DECISIONS.md yet; these are this brief's defaults — say so if any should be different)

- **A joint is a document object**, like a Section Plane or Construction Plane: its own id, undoable, saved in the file (schema 19), drawn as a small gizmo (a pin through the axis, with a short arc swept between the limits if set) that follows Hide and can be hidden on its own.
- **The two parts are two instances** (groups or components) in the same open context — siblings, the way the booleans in Solids & CAD take two solid groups. A loose-geometry side is out of scope for this part (a hinge wants something to carry on both sides); if the owner needs that, it can follow.
- **The axis** is a line (a point and a direction), picked with the normal inference against either part: an edge, a hole or cylinder's axis, or a face normal, the same candidates Change Axes and the Array tool already offer. Stored in the world, like a Section Plane's, not relative to either part, since either part can move independently once the joint exists (within its limit).
- **The limit is optional**, a min and max angle in degrees, set in the Inspector or by dragging a handle on the gizmo's arc to the limit (the same "drag to the limit" feel the Fillet handle and Angle Guides already have) measured from the pose the joint was made at (its own angle zero). No limit means it turns freely — the wheel case.
- **Moving:** with a joint in place, Rotate on one of its two parts (the other left alone, or both selected and locked together — out of scope here, a `Fixed` joint's job later) finds the joint, uses its axis instead of the usual plane-normal guess, and clamps the drag or the typed angle in the measurement box to the limit if there is one. Move is unaffected (Slide is the later part for sliding joints). A joint beyond its limit (another edit moved a part) is left alone, not snapped back; Model Health already has the shape for "flag it, don't block" if that is ever wanted.
- **Saved poses (Open, Closed) are out of scope for this part.** PLAN 1.15 says they are "stored as scenes," but Modes & Scenes is not built yet (it is a Presentation-tier chunk, after Rendering in the order) — there is nothing to store them in. Left for whichever part needs it once Modes & Scenes exists.
- **Sweep Motion Range, collision and clearance are out of scope here too** — they need the other joint kinds and the tolerance modifier to mean anything, and are later parts.

## How it works

- **The Hinge tool:** click the first part, click the second, then pick the axis with the inference engine against either one (an edge, a cylinder or hole's axis, a face normal), as a third click; Enter or a click on the axis finishes it. Esc at any point cancels. One undo step.
- **The gizmo:** a short pin along the axis, through both parts' combined box at the axis; a swept arc between the limits if set, or a full circle if not. Selectable and hideable like a Section Plane.
- **The Inspector:** the two parts' names (read-only, pick-in-viewport to change either — the same pattern as the boolean cutter's Inspector field), the axis (read-only; redo it with the tool to change it), a limit toggle with min/max angle fields, and a Locked toggle (for a door you want to stop swinging without deleting the joint).
- **Rotating:** select one of the joint's two parts and use Rotate as usual; its axis comes from the joint, and a limit clamps the drag and any typed angle. Rotating the *other* part of the pair does the same, about the same axis. Rotating a selection that includes both, or geometry with no joint, is unaffected.
- **Right-click / Edit menu:** Remove Joint on a selected one; Hide same as any other hideable entity.

## Out of scope (this part; later parts of Fit & Mechanisms unless noted)

- Slide, Ball and Fixed joints.
- The tolerance modifier and machine/material profiles.
- Fit Helper and Check Fit.
- Sweep Motion Range.
- Saved poses (needs Modes & Scenes, not built).
- Closed loops of joints (a four-bar linkage) — PLAN 1.15 already defers this to "later," past even the other joint kinds.
- Collision between the two parts while rotating (that's Sweep Motion Range's job).

## Tests

- Making a joint: two boxes, an axis from a picked edge; undo removes it, redo brings it back exactly (position, axis, limit).
- A joint with no limit rotates its part all the way round, repeatedly, never refusing (the wheel case).
- A joint with a limit clamps both a dragged and a typed Rotate angle to it, and the measurement box shows the clamped value, not the requested one.
- Rotating the *other* side of the same joint turns about the same axis.
- Removing a joint leaves Rotate working as it did before the joint existed (plane-normal guess, no clamp).
- The gizmo is picked, hidden, and excluded when hidden (Hidden is free, section 1.14).
- A file round-trip (schema 19) keeps every joint's parts, axis and limit exactly.
- MCP: list joints, read one's fields, create and remove one, matching the pattern `get_inspector`/`set_inspector` already use for other entities.

## Acceptance

- [ ] A door (one group hinged to another) swings open and closed with a limit, by dragging or typing an angle.
- [ ] A wheel (one group hinged to a chassis, no limit) spins freely through Rotate.
- [ ] PLAN, NOTES and this brief's checkboxes are updated when the part is done.
