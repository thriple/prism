# Inspector & Features: the panel, then live features

**Goal.** Window > Inspector shows whatever is selected and lets you change it (PLAN 1.17). Later parts add editable features (fillet, chamfer, boolean, shell, array, mirror) that stay live until baked.

**Read first:** PLAN 1.17, `docs/feature-map.md`, NOTES "Inspector" (this brief's notes), the Entity Info card in `prism-app/src/chrome/panels.rs`.

## Size
L. Owner chose (2026-11-18): panel first, then the rest.

## Parts
1. **The panel (built).** Schema-driven: the session returns sections of fields (`Session::inspector`), the window draws them, and `Session::inspector_set(key, text)` takes the typed text back. MCP `get_inspector` and `set_inspector` use the same two calls.
2. **The modifier stack (not started).** Modifiers stored per body, re-evaluated after direct edits, stable-ID references with a geometric fallback, "broken" and Bake.
3. **Editable features (not started).** Fillet and chamfer first (a distance changed after the fact), then booleans, shell, hole, array and mirror.
4. **Pick references, previews, named parameters (not started).**

## Part 1: the panel
- **Window > Inspector** (`win.inspector`), in the side column with Entity Info.
- **Shown for one or more selected groups:**
  - General: name (one group only), tag, material (shown), hidden, locked.
  - Position: X, Y, Z of the low corner of the selection's box in the world.
  - Size: width X, depth Y, height Z; faces, edges and volume (shown). Volume is worked out once for each selection and model, and shows "—" when a group is not a closed solid.
  - The breadcrumb is the open groups the selection sits in.
- **Shown for selected geometry:** the counts, the area of the faces, front and back materials. Nothing to edit yet.
- **Several groups:** a field they disagree on shows "—"; an edit applies to all of them. Moving moves the selection as one; sizing scales it about the low corner.
- **Typing:** lengths take units and simple expressions (`2*25.4mm`, `(1m+20cm)/2`; two lengths cannot be multiplied). A bare number is metres, as in the measurement box. Each committed change is one undo step. A refused change (bad text, a size of zero, a locked group) shows its reason in the window and changes nothing.
- **Not here yet:** rotation, scale as a factor, the pivot, local terms, scrubbing by dragging, preview ghosts.

## Tests
- Fields for a group, an edit and its undo, for position and size.
- Name, tag, hide and lock; refusals change nothing.
- Several groups: mixed values, one edit moves both.
- Selected faces: area; no fields to set.
- Expressions: units, brackets, bad input.
- MCP: read, set, error.

## Acceptance
Select a group, open Window > Inspector, type `1.5m` in X: the group moves, and Undo puts it back. Select two groups with different tags: the tag shows "—".
