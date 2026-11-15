# Solids & CAD part 1: Validate and Heal (2026-11-15)

**Goal.** Solid > Validate and Heal says, for each solid in the open group (or the model), whether it is a valid solid that booleans and STEP can trust, and why not, in plain words. A Heal button fixes what can be fixed safely, in one undo step. This is part 1 of Solids & CAD (PLAN 6.0a): later parts (booleans, STEP) check their input the same way.

**Read first:** `prism-ops/src/health.rs` (Model Health's checks and fixes), `prism-ops/src/bridge.rs` (a solid's faces to the kernel), `prism-kernel/src/solid.rs` (`Solid::build`, which runs the kernel's checker), `prism-tools/src/session/health.rs`, `prism-app/src/chrome/panels.rs`.

## Size
M: two steps. A: the core (validate, heal, the command). B: the window, the action, MCP.

## Requirements
- **A solid** is a connected piece of faces (joined across edges) in the open context. Groups and components inside the context are listed by name and validated from their own geometry; healing them needs opening them (as Model Health works on the open group).
- **Validate** (`prism_ops::solid_check::validate`), per piece:
  - *Valid solid*: closed (every edge has exactly two of its faces), no reversed, interior, crossing, degenerate or off-curve faces, and the kernel's checker passes. Shows its volume.
  - *Not a solid*: the reasons, each with a count ("3 open edges (holes)", "2 faces pointing inward", "1 edge with 3 or more faces", "faces crossing with no edge").
  - The kernel's findings in plain words ("an edge does not lie on its face", "faces cross", "a face of no size"), grouped with counts. Freeform (NURBS) faces cannot go to the kernel yet: those solids are checked by Prism's rules only, and the row says so.
- **Heal** (`prism_ops::solid_check::heal`, `Command::Heal`), on the open context's loose geometry (the selected faces' pieces if faces are selected), in this order, then validate again:
  1. Weld near misses (points closer than ten tolerances, Model Health's "Near misses").
  2. Merge repeated faces (`settle::dedupe_faces`).
  3. Put arcs and curved faces back on their curves (`repair_arcs`).
  4. Fill a hole whose edge loop lies flat with a face.
  5. Orient faces, so each piece faces out.
  6. Remove interior faces (Clean Envelope) and stray edges.
  It says what it did ("Welded 4 points, filled 1 hole, turned 2 faces"). Nothing changes if nothing needs healing (no undo step).
- **The Solids window** (Solid > Validate and Heal opens it, in the right column): one row per solid (name, Valid / Not a solid, volume or reasons); a click selects and zooms to it; a Heal button and the last heal's words. It rechecks when the model changes while open, like Model Health.
- **MCP:** `run_action("solid.heal")` opens the window (it reaches no data). A new tool `validate_solids` returns each solid's faces, verdict, volume and reasons; `heal` runs the command and returns what it did.

## Out of scope
- Healing inside closed groups (open them first).
- Gaps wider than ten tolerances, holes whose edges are not flat (a patch is part 4's cutting of freeform faces).
- Kernel calls off the interface thread (DECISIONS #39: with the first slow operation).

## Tests
- A box: valid, volume exact. The same box with one face erased: "1 hole"; Heal fills it and it is valid again, same volume.
- A box with a face reversed: Heal turns it; with a vertex nudged by five tolerances: Heal welds it.
- A sphere and a cylinder (curved faces): valid through the kernel, exact volume.
- A box with an interior wall: Heal removes it.
- A valid solid: Heal changes nothing and adds no undo step.
- The session: the action opens the window; Heal is one undo step.
- MCP: `validate_solids` on a box and on an open box.

## Acceptance
- [ ] A leaky imported mesh is healed to a valid solid, and a valid box says Valid with its volume (owner).
- [ ] PLAN, NOTES and HANDOFF are updated.
