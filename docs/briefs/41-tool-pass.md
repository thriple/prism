# Tool pass: Solid, Fabricate and Shapes tools obvious to use (2026-11-16)

**Goal.** Every built tool in the Solid, Fabricate and Shapes groups says how to use it: what to pick, what happens, and how to finish. The owner asked for this (NOTES "Owner feedback for the next session (2026-11-16)") after Session 10 (a recorded session) went wrong in the Loft tool.

**Read first:** NOTES "Owner feedback for the next session (2026-11-16)", `prism-tools/src/session/lofting.rs`, `prism-ops/src/sides.rs`, `prism-app/src/chrome/loft.rs`, `prism-tools/src/session/booleans.rs`, `prism-tools/src/session/cage.rs` (`surface_from_selection`).

**What is built in these groups:**
- **Shapes:** the Loft tool; Loft, Revolve and Sweep, which work on the curves already selected.
- **Solid:** Union, Subtract, Intersect, Trim, Split, Outer Shell and Heal.
- **Fabricate:** nothing yet. Shell / Thicken and Primitives are not built either.

## Size
L: three parts, each pushed and tested on its own.

## Requirements
- **Part 1. The Loft tool, from Session 10.**
  - Sides that close a ring can be picked in any order. Out-of-order sides were refused with "neither close a ring nor stand apart".
  - A click on an edge that joins the side being picked adds that edge to the side. Before, each click replaced the side.
  - Enter makes the surface as soon as the ring closes, even while its last side is still being picked. Before, it took two Enters.
  - Smooth / Straight is greyed out where it changes nothing: a ring, or two sections. Before, toggling it remade the same loft, so the next undo removed the whole loft.
  - Outside the Loft tool: the Eraser showed a stale refusal from Move ("a face with arcs would fold") while it erased.
  - Part 1b (owner's screenshot): a line drawn across an arc parts it into two curves, so hover, click and the Loft tool take one piece. Alt+click in the Loft tool takes one edge, whatever the threshold.
- **Part 2. Select-first tools and the Solid menu.**
  - Loft, Revolve and Sweep picked with nothing right selected say what to select, then take clicks like the Solid tools, instead of only refusing.
  - Every Solid tool's hint names what to click next. Heal says what it fixed.
  - Outer Shell is removed: Union does it (owner, 2026-11-16).
  - Both Lofts stay (owner, 2026-11-16): the menu Loft gets the visual feedback above.
  - Edit tools on a closed group say "double-click the group to open it" (from the Fillet window's leftovers).
- **Part 3. Shell / Thicken.** Build the tool as designed in NOTES ("Shell / Thicken design"). It has its own brief: `42-shell-thicken.md`.

**MCP:** nothing new. `use_tool`, `run_action` and `replay` drive these tools, and `replay` plays the owner's recorded sessions.

## Out of scope
- Fabricate tools (none built), Primitives, Smooth / Subdivide.

## Tests
- Part 1: a ring picked out of order fills; clicks along a side add to it; one Enter makes a closed ring; Smooth / Straight is greyed and leaves a ring alone; the Eraser shows its own hint.
- Part 2: each select-first tool with no selection prompts, then makes its surface from clicks.

## Acceptance
- [ ] Session 10's lofts work the way the owner tried them.
- [ ] PLAN, NOTES and HANDOFF are updated after each part.
