# Brief 33: Everyday Commands

PLAN 6.0 row "Everyday Commands" (added 2026-11-13, owner request to
regroup the plan). It covers the greyed commands that no chunk owned, and
Smart Guides (PLAN 5.1). Outcome: a part copied and pasted in place, seen
from the front in parallel projection.

**Read first:** PLAN 1.6 (the Edit and Camera menus), 1.5a (camera
navigation), 5.1 (Smart Guides), Appendix A. The prototype's Smart Guides
(`smartInit(`, `smartSnap(`, `smartDraw(`) are the reference for part 4.

## Parts (each tested, documented and pushed on its own)
1. **Clipboard and Lock.**
   - **Copy** (Ctrl+C) keeps the selection: loose geometry and whole
     groups and components, in world coordinates, with the materials,
     images and tags they use. **Cut** (Ctrl+X) copies, then deletes, in
     one step. The clipboard lives in the app, so it is shared by every
     open document (tab).
   - **Paste** (Ctrl+V): the copy follows the cursor with the inference
     engine, held at the low corner of its box, and a click places it.
     Esc cancels. Loose geometry joins the open group's geometry by the
     sticky rules (as a Move copy does); groups stay groups.
   - **Paste in Place** (Ctrl+Shift+V) puts it back where it was copied
     from, in whichever group is open.
   - Pasting into another document brings the materials, images and tags
     along. A material or tag with the same name and value is reused. A
     component whose definition is the same is reused; a different one
     with the same name gets a new name.
   - **Lock / Unlock** (Edit > Hide & Lock, the context menu): a locked
     group or component can be selected, copied, hidden and tagged, but
     not moved, rotated, scaled, erased, exploded, painted, flipped or
     opened. Its box is drawn in the locked colour. A refused edit says
     why in the status bar. Locks are saved in the file and undone like
     any edit.
   - **Intersect Faces > With Context**: with a group open, where its
     faces cross the geometry outside it, edges are added inside the
     group (nothing outside changes).
2. **Camera.** Standard views (Top, Bottom, Front, Back, Left, Right,
   Iso), Perspective and Parallel Projection, Field of View (typed in the
   measurement box), Zoom Selection, Zoom Window, Walk and Look Around.
   Saved with the file (the camera already is).
3. **Drawing and windows.** The Rotated Rectangle tool (three clicks:
   one side, then its width, any plane, with inference); the Freehand
   tool (a drag fitted to a NURBS curve, flat when drawn on a face); the
   Preferences window (units, tolerance shown, inference settings, the
   keymap); About; the tool hint panel.
4. **Smart Guides** (PLAN 5.1): while moving, copying or pasting a group
   or component, its box lines up with its neighbours and snaps to equal
   gaps, with guides, arrows and a `d:` label; a time budget; Ctrl+U and
   View > Smart Guides.

Center / Align moves to Shapes & Placement, with Align and Distribute
(it is the same family). Weld moves to Solids & CAD, with Convert.

## Rules
- Every edit is a command; refusals come through `Command::refusal` or
  the status bar.
- Every new action has a menu path; tools use the inference engine.
- Paste and pasted placement are replayable: the clipboard is set by
  events, never read from the system in a pure crate.

## MCP
Actions reach MCP by themselves (`run_action`). The model read gains each
group's `locked`. Camera views and projection are read and set through
the existing camera tools (part 2 extends them if needed).

## Out of scope
The system clipboard (copying between Prism and other programs);
dragging between windows; locking loose geometry (SketchUp locks groups
only).

## Tests
Doc: copy and paste round trips (geometry, groups, components shared,
materials and tags remapped into another document), paste in place in an
open group, cut is one undo step, lock refuses each edit and undoes.
Session: copy, paste by clicks with inference, Esc, paste in place, lock
and the refused move, With Context. Files: locks survive a save.

## Acceptance
- [x] Every part: tests green, NOTES updated, pushed (part 4 on 2026-11-13).
- [ ] A part copied and pasted in place, seen from the front in parallel
  projection.
- [ ] PLAN 6.0 updated.
