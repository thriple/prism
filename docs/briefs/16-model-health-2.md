# Chunk 9: Model Health (part 2: in the viewport, and before export)

**Goal.** Problems show where they are, and an export says when the model will not print or render cleanly (PLAN 1.10, 1.3).

**Read first:** NOTES "Model Health (part 1)"; PLAN 1.10, 1.3; `crates/prism-app/src/transfer.rs`.

## Requirements
- **In the viewport, while Model Health is open:**
  - open edges are drawn red;
  - edges with three or more faces are drawn orange;
  - stray edges are drawn grey;
  - reversed faces are tinted;
  - interior and crossing faces are outlined.
  The drawing follows each edit, as the panel's counts do.
- **A switch in the panel:** "Show in the model" turns the drawing off, for a heavy model.
- **Before export (mesh formats):**
  - the shown geometry is checked;
  - if it has open edges, reversed faces or crossing faces, the export still happens, and the status says what was found ("Exported. 4 open edges: the model is not watertight; see Model Health").
  - STL and 3MF say "not watertight"; the others only list.
- **Checks while the window is closed** are not built here. They would cost up to 180 ms per edit on big models, and the web build has no threads. The panel and the export check cover the need.

## Tests
- The marks a session gives for an open box (red edges) and a flipped face (tint), and none for a clean box.
- The export note for an open box, and none for a closed one (pure function).

## Acceptance
- [ ] Problems show in the model while the panel is open, and update with edits.
- [ ] Exporting an open model says so.
- [ ] PLAN 6.0 and NOTES are updated.
