# Inspector & Features: the panel, then live features

**Goal.** Window > Inspector shows whatever is selected and lets you change it (PLAN 1.17). Later parts add editable features (fillet, chamfer, boolean, shell, array, mirror) that stay live until baked.

**Read first:** PLAN 1.17, `docs/feature-map.md`, NOTES "Inspector" (this brief's notes), the Entity Info card in `prism-app/src/chrome/panels.rs`.

## Size
L. Owner chose (2026-11-18): panel first, then the rest.

## Parts
1. **The panel (built).** Schema-driven: the session returns sections of fields (`Session::inspector`), the window draws them, and `Session::inspector_set(key, text)` takes the typed text back. MCP `get_inspector` and `set_inspector` use the same two calls.
2. **The modifier stack (built, first with the live round).** Features stored on a body's definition over its base geometry, re-run after direct edits, stable-ID references with a geometric fallback, "broken" and Bake. See Part 2 below.
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

## Part 2: the modifier stack (owner chose, 2026-11-18: direct edits re-run the stack)
- **Where:** `Definition.stack: Option<Stack>`. `Stack { base, features }`; the definition's `model` stays the result, so everything that reads a model is unchanged. Components share it, so every instance updates.
- **First feature:** `Feature::Round` (fillet or chamfer on edges). A round made inside a group (Fillet window or tool, `RoundEdges`) stays live. Loose geometry at the root is rounded outright.
- **Edges are found again** by id first, then by place (the ends within tolerance). The first feature's ids are base ids and are always followed. If the edges are gone or the kernel refuses, the feature is marked **broken**, skipped (the plain base shows) and never silently dropped.
- **A direct edit** (any geometry command run in the body) is applied to the base and the features run again. An edit that names an element made by a feature (a face of the fillet) finds nothing in the base and changes nothing.
- **Anything else that writes the body's geometry** (Make Group inside it, Explode, Change Axes, paste) **bakes first**, so nothing is lost or put out of step. Instance changes (move, copy, rename) leave the stack alone. Materials are applied to the base too.
- **Inspector:** a **Features** section for one selected body: the size of each feature (a length), a Remove button for each, and Bake. Keys `feature.N.size`, `feature.N.remove`, `bake` (MCP `set_inspector`, `get_inspector` shows them as `action` fields). A size the kernel refuses is refused with a reason.
- **Menu:** Edit > **Bake Features** (`edit.bake`) for the selected groups.
- **Commands:** `SetFeatureSize`, `RemoveFeature`, `BakeFeatures`; each is one undo step.
- **File:** schema 15. `groups.json` lists a body's `features`, and the base is `geometry/defs/<id>.base.bin`.
- **Left for later:** re-picking a broken feature's edges; other feature kinds (part 3); pick-in-viewport (part 4); a ghost of the change; features on loose geometry.

## Tests
- Fields for a group, an edit and its undo, for position and size.
- Name, tag, hide and lock; refusals change nothing.
- Several groups: mixed values, one edit moves both.
- Selected faces: area; no fields to set.
- Expressions: units, brackets, bad input.
- MCP: read, set, error.

## Acceptance
Select a group, open Window > Inspector, type `1.5m` in X: the group moves, and Undo puts it back. Select two groups with different tags: the tag shows "—".
