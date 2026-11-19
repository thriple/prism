# Inspector & Features: the panel, then live features

**Goal.** Window > Inspector shows whatever is selected and lets you change it (PLAN 1.17). Later parts add editable features (fillet, chamfer, boolean, shell, array, mirror) that stay live until baked.

**Read first:** PLAN 1.17, `docs/feature-map.md`, NOTES "Inspector" (this brief's notes), the Entity Info card in `prism-app/src/chrome/panels.rs`.

## Size
L. Owner chose (2026-11-18): panel first, then the rest.

## Parts
1. **The panel (built).** Schema-driven: the session returns sections of fields (`Session::inspector`), the window draws them, and `Session::inspector_set(key, text)` takes the typed text back. MCP `get_inspector` and `set_inspector` use the same two calls.
2. **The modifier stack (built, first with the live round).** Features stored on a body's definition over its base geometry, re-run after direct edits, stable-ID references with a geometric fallback, "broken" and Bake. See Part 2 below.
3. **Editable features.** Fillet and chamfer first (a distance changed after the fact; built, parts 2 to 2c), then booleans (owner chose, 2026-11-19: first, with the cutter kept inside the feature; 3a built, see Part 3a), then shell (owner chose it next, 2026-11-19; part 5a built, see Part 5a), hole, array and mirror.
4. **Previews (4a built), pick references (4b built), named parameters.** Part 4a: typing in a feature's field shows the result on the body before Enter. Part 4b: a round can be pointed at edges picked in the viewport. Named parameters are not started.

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
- **First feature:** `Feature::Round` (fillet or chamfer on edges). A round (Fillet window or tool, `RoundEdges`) stays live wherever it is made: in a group, in a component, or on loose geometry (the root definition has a stack too).
- **Edges are found again** by id first, then by place (the ends within tolerance). The first feature's ids are base ids and are always followed. If the edges are gone or the kernel refuses, the feature is marked **broken**, skipped (the plain base shows) and never silently dropped.
- **A direct edit** (any geometry command run in the body) goes to the base and the features run again. If the edit names elements (a face to push, corners to move, edges to erase, `Command::targets`) and the base has them all, that is all. If it names something a feature made (a face of the fillet, a corner of it), the fewest features that must be baked first are baked, in the same undo step, and a **red notice** says so ("A fillet or chamfer was baked to edit its faces. Undo puts it back."). A feature that could not follow an edit (the body is open, so the round cannot run) is baked the same way instead of being lost. Edits that work by place (a line drawn across a face) are compared by the shape they make: if running them under the features does not give the same shape, the features are baked first.
- **Solids move with their features.** Make Group and Make Component take the live features of the solids they select (a solid is carried whole, found by its connected faces); a solid only partly selected has its features baked first. Explode gives a group's features back to the body it was exploded into; Paste and Cut go through the same edit rule. The features a body keeps are not touched when something else is grouped.
- **Anything else that writes the body's geometry** (Change Axes, boolean results) **bakes first**, so nothing is lost or put out of step. Instance changes (move, copy, rename) leave the stack alone. Materials are applied to the base too.
- **Inspector:** a **Features** section for the one selected body, or, with no group selected, for the open group (the top level for loose geometry): the size of each feature (a length), a Remove button for each, and Bake. Keys `feature.N.size`, `feature.N.remove`, `bake` (MCP `set_inspector`, `get_inspector` shows them as `action` fields). A size the kernel refuses is refused with a reason.
- **Menu:** Edit > **Bake Features** (`edit.bake`) for the selected groups, or for the open group / top level when none is selected.
- **Commands:** `SetFeatureSize`, `RemoveFeature`, `BakeFeatures` (each names the definition, the root for loose geometry); each is one undo step.
- **File:** schema 15. `groups.json` lists a body's `features`, and the base is `geometry/defs/<id>.base.bin`.
- **Left for later:** re-picking a broken feature's edges; other feature kinds (part 3); pick-in-viewport (part 4); a ghost of the change; features on loose geometry.

## Part 2b: features you can select
- **The selection of exactly the faces a feature made is the feature selected** (`Session::selected_feature`). So the highlight, the Inspector and the menus work from the ordinary selection.
- **Select:** a click on a face a feature made picks the feature (all its faces). A round is one smooth surface, so Alt + click on it picks the same faces; on other faces it picks the face as before. Alt + click still picks a plane or dimension first.
- **Right-click** on a feature offers Explode Fillet / Chamfer, Remove Fillet / Chamfer and Bake Features (Edit menu too: `edit.explode_feature`, `edit.remove_feature`). Explode bakes that feature and the ones before it (they made what it works on).
- **Inspector:** a selected feature shows its size, Explode and Remove.
- **Outliner:** a "Features (n)" row under the top level and under each group with features; it opens to list them ("Fillet 0.300 m"); clicking one selects it, opening its group first.
- **File:** loose geometry's features are saved too (the root's base is `geometry/defs/0.base.bin`).

## Part 2c: the Fillet tool's handle
- **Only with the Fillet tool active and a feature selected** a guide is drawn: the sharp edge the feature rounds (dashed), and a square handle across the flat face that most faces the view, as far from the edge as the feature's size. No guides show under any other tool.
- **Drag the handle** and the body shows the new size live; let go and it is set (`SetFeatureSize`, one undo step) with the feature still selected. Esc while dragging leaves the size as it was. A size the kernel refuses is not taken (the preview stays at the last size that worked). Typing a size + Enter sets it too.
- **The guide** comes from `Stack::size_guide(index)` (pure, in `prism-doc`): the first edge of the feature as it was before the feature ran, and the flat faces beside it. None for a broken feature, a curved edge, or an edge with no flat face beside it; the Inspector still sets the size then.
- **With a feature selected** the tool does not treat its faces as edges to round; a click on an edge starts a new set.
- **MCP:** the size is `set_inspector` `feature.N.size`; the handle itself is a pointer drag (press, move, release) like the tool's other drags.

## Part 3a: live booleans (owner chose, 2026-11-19)
- **Union, Subtract and Intersect of two groups stay live.** The result is the second solid (its own features kept) with a `Feature::Boolean { op, tool }` after them; the first solid is the tool, kept inside the feature in the body's own coordinates (so it works in components and undo, and the Inspector later edits it). Trim and Split still make plain groups.
- **It runs like the fillet:** direct edits go under it (an edit that names a face the boolean made, such as one it cut, bakes it first with the red notice); a result the kernel refuses marks it broken; Bake and Remove work (`edit.bake`, `RemoveFeature`); it is selectable (the faces it made), listed in the Outliner and moves with Make Group, Make Component and Explode.
- **Inspector:** a Features row shows the kind ("Subtract") and a note; no size. Part 3b-1 adds the tool's fields; the ghost is still to come.
- **File:** schema 16. The tool is saved as `geometry/defs/<id>.tool<k>.bin` beside the base.
- **MCP:** the Solid tools and `set_inspector` (`feature.N.*`) as for the fillet.

## Part 3b-1: the cutter's fields
- **Six rows under a live boolean:** Cutter X, Y, Z (the corner of the box round the second solid) and Cutter width, depth, height, in the body's own coordinates (so a rotated or scaled group shows its own axes, not the world's). Typing one moves or stretches the cutter and runs the features again: `SetCutter` (one undo step, "Change Feature").
- **Refused, nothing changes:** a size of zero or less, text that is not a length, the same value, or a change that breaks the shape (the result must stay a solid).

## Part 3b-2: the ghost and the kind
- **Ghost:** with a live boolean selected (open its body, then select the cut from the Outliner or its faces), a dashed outline of the cutter is drawn where the cutter fields put it, under any tool. It is not selectable and not part of the model.
- **Kind:** the feature's Inspector row is a choice, Union, Subtract or Intersect. Changing it runs the features again (`SetBoolKind`, one undo step, "Change Feature"). A kind that would leave nothing is refused and nothing changes.
- **MCP:** `set_inspector` with `feature.N.cutter.x`, `.y`, `.z`, `.w`, `.d` or `.h`, and `feature.N.kind` with `Union`, `Subtract` or `Intersect`; read the rows with `get_inspector`.

## Part 4a: previews while typing
- **What:** while a feature's size, cutter or similar field has the keys and its text is not yet committed, the body is drawn with that value (a preview, like the Fillet handle's). Enter commits it as before (one undo step); leaving the field takes the preview away. Text that is not valid, or a value the kernel refuses (the shape would break), shows the body as it is. Nothing is added to the document until Enter.
- **Where:** the body that is open (a group seen from outside is not previewed; open it first). `Session::inspector_preview(key, text)` and `inspector_preview_end()`; the window sends them as the text changes and when the field is left.
- **MCP:** nothing new; `set_inspector` sets the value directly, so there is nothing to preview there.

## Part 4b: pick the edges of a round in the viewport
- **How:** open the body (or work on loose geometry), select edges with the Select tool; the Inspector's Features list shows, under each fillet or chamfer, a button "Round the N selected edges". It points that round at those edges (`SetFeatureEdges`, one undo step, "Change Feature"), which also mends a broken round whose edges were gone.
- **Where the edges are looked up:** the picked edges are matched by place in the model as it is just before that round runs, so edges the round (or a later feature) made cannot be picked for it. An edge that is not there, no edges, or a result the kernel refuses changes nothing.
- **Not here:** picking edges for a round while it is selected as a feature; adding to its edges (it replaces them); pick references for other kinds of feature.
- **MCP:** `set_inspector` with `feature.N.edges` (any text) uses the current selection; select the edges first with the selection tools.

## Part 5a: live shell
- **The Shell / Thicken tool stays live.** `Command::Shell` now adds a `Feature::Shell { faces, open, thickness }` (like a round): the shelled body, the faces left open and the thickness are kept, and direct edits run under it. The faces are kept by id and by place (the middle of the outline and the normal), so they are found again after other edits. A shell the kernel refuses is not made, as before.
- **Inspector:** a Features row "Thickness (negative goes in)" sets the thickness (`feature.N.size`, one undo step; a shell may be negative, zero is refused), Explode, Remove and Bake as for the others; the Outliner lists it as "Shell 0.1 m". The typed value previews on the open body (part 4a).
- **Moves with its body:** placing, exploding and Make Component carry it (its faces move with the body). Make Group on loose geometry bakes it (a shell has no edges to say which solid it is on).
- **File:** schema 17 (a new kind of feature; no new files).
- **Not here:** picking the open faces again in the viewport, a drag handle for the thickness, and a ghost; Shell on a curved face stays as the tool had it.
- **MCP:** `set_inspector` with `feature.N.size` and a signed thickness, as for the fillet.

## Part 5b: the live shell's thickness handle (owner, 2026-11-19)
- **Why:** a number typed in a field is not how this app should feel; changing a thing should be physical. The Fillet tool already has a drag handle (part 2c); the live shell gets one the same way.
- **What:** with the Shell tool active and a live shell selected (click a face it made with Select, or pick it in the Outliner, then choose Shell), a square handle sits on the wall: the middle of the first face shelled, moved out along that face's normal by the thickness. Drag it along that line: the body shows the new thickness live, through the face turns it the other way (inside or outside). Let go: it is set (`SetFeatureSize`, one undo step) with the feature still selected. Esc while dragging leaves it as it was. A thickness the kernel refuses is not taken (the preview stays at the last one that worked). The hint line says so.
- **Not yet:** snapping the thickness to nearby geometry or guides (comes with the rim handles, brief 49 part 3), and a handle for a shell whose first face is curved.
- **MCP:** the handle is a pointer drag (press, move, release); `set_inspector` `feature.N.size` still sets a number.

## Tests
- Fields for a group, an edit and its undo, for position and size.
- Name, tag, hide and lock; refusals change nothing.
- Several groups: mixed values, one edit moves both.
- Selected faces: area; no fields to set.
- Expressions: units, brackets, bad input.
- MCP: read, set, error.

## Acceptance
Select a group, open Window > Inspector, type `1.5m` in X: the group moves, and Undo puts it back. Select two groups with different tags: the tag shows "—".
