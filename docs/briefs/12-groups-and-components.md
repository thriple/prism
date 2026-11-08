# Chunk 8: Groups & Components (part 1: groups)

**Goal.** Geometry can be wrapped in a group that nothing outside sticks to. A group can be moved, rotated, scaled, copied and erased as one object, opened for editing, and exploded back into its parent, where the overlap rules run (PLAN 1.2, 1.9).

**Read first:**
- PLAN 1.2 and 1.9;
- `docs/research/groups-and-components.md`;
- DECISIONS #22 and #23.

## Parts of this chunk (PLAN 6.0a)
1. **Groups** (this brief): the data model, Make Group, whole-group selection and transforms, open and close for editing, Explode, the file format.
2. **Components:** Make Component, linked instances, Make Unique, mirror (Flip Along on an instance), a definition's axes, drawing each definition once per instance on the GPU, the Components window.
3. **Panels:** Outliner, Entity Info, tags, and hiding groups and instances ("hidden must be free", PLAN 1.14).

Smart Guides (PLAN 5.1) wait for groups, so they come after this chunk.

## Requirements
- **Where the geometry lives:**
  - The document holds definitions. Each definition has its own `Model` in local coordinates, plus the instances placed in it. The root is a definition too.
  - An instance is `{definition, transform}`. The transform is an affine map from local to parent coordinates, in f64.
  - Each definition sits behind an `Arc` and is copied on write, so an undo step shares every definition it did not change.
  - A group is a definition with one instance. Copying a group makes a new definition (DECISIONS #22).
  - Groups can be nested.
- **Make Group (G, Edit menu, right-click menu):**
  - The selected geometry moves out of the open context into a new group, in one undo step.
  - The new group's origin is the corner of the selection's box, with the parent's axes, so its local coordinates stay near zero.
  - Edges and faces that are shared with geometry left outside are copied, so the outside stays whole. This is what SketchUp does.
  - Selected groups go inside the new group too.
  - The new group is selected.
- **Selecting a group:**
  - Outside a group, a click on any of its faces or edges selects the whole group.
  - Hover shows the group, and a selected group shows a blue box (PLAN 1.2).
  - Box select takes groups wholly inside the box (window) or touching it (crossing), as it does for geometry.
  - Select All takes every group in the context.
- **Edit tools on groups:**
  - Move, Rotate and Scale on a selected group change only its transform. Move copies (Ctrl) and arrays work too.
  - Erase (and Delete) remove the group.
  - Push/Pull, Offset and the Eraser on a single face or edge need the group to be open.
- **Opening and closing a group:**
  - Double-click a group, or press Enter with one group selected, to open it.
  - Inside, the rest of the model is drawn faded and cannot be picked or edited, and a dashed box shows the context.
  - All tools work inside as they do at the root. Their commands are mapped into the group's local coordinates.
  - Inference still snaps to visible geometry outside the context, in world space.
  - Esc (when no tool step is in progress, after closing menus, as in `actions/esc.rs`) or a click outside closes the group, one level at a time.
  - Open and close are not undo steps. Undo of an edit inside a group opens that group again.
- **Isolation:**
  - Drawing at the root never sticks to, splits or merges with geometry inside a group, and the other way round.
  - Two groups can overlap exactly without either changing.
- **Explode (Edit menu, right-click menu):**
  - The group's geometry, transformed, joins its parent's model. Then the overlap and duplicate rules run (PLAN 1.9), so coincident vertices, edges and faces collapse into one.
  - Groups nested inside become children of the parent.
  - The exploded geometry is selected.
- **Files:**
  - `.prism` schema 2 stores each definition as its own geometry entry, and the instance tree in the manifest or document entry.
  - Schema 1 files open through `migrate`, as one root definition.
  - Recovery journals and recorded sessions keep working.
  - Mesh export flattens the instances. Import stays as now: geometry beside the model, at the root.
- **Menus:** Make Group and Explode become active (DECISIONS #20). Make Component and Make Unique stay greyed until part 2. Planned Features ticks "Groups and components, open and exit" only when part 2 is done.

## Design notes from the code
- Every existing operation already works on one `Model`. Running it on the open context's definition is what isolation means, so `prism-ops` should barely change.
- The session's scene for the open context stays as today, built in world space through the context's transform. Everything else becomes a list of instances, which are drawn, picked as a whole (a new `Hit::Instance`), and offered to inference as world-space candidates.
- The simplest drawing for this part is to flatten instances into the scene. Drawing a definition once per instance on the GPU (instancing) comes with components in part 2, where it pays off.
- Scale inside a rotated group works along the group's own axes, as in SketchUp, so its local scale stays axis-aligned.
- `Document::run` takes the context to run in. The context is a path of instances from the root.

## Out of scope
- Components, Make Unique, mirroring an instance (part 2).
- Outliner, Entity Info, tags, hiding (part 3).
- Glue to and Cut opening (DECISIONS #23, later).
- Smart Guides (after this chunk).
- Copy and paste between groups or tabs: the clipboard is not built yet.

## Tests
- **Data model:**
  - Make Group, Explode, and transforms of groups, with exact undo and redo.
  - Undo steps share the definitions they did not change (`Arc::ptr_eq`).
- **Isolation:** a rectangle drawn at the root across a group's face changes nothing inside it, and the other way round.
- **Golden case, the icosahedron (PLAN 1.9):**
  - a grouped icosahedron is copied and moved aside;
  - the copy is sliced in half;
  - the original is moved to line up exactly with the copy's half;
  - both are exploded into one valid mesh, passing `check`.
  It runs at scales 1e-4, 1 and 1e3, with a rotated group too.
- **Property tests:**
  - Exploding a group that was moved, rotated or scaled gives the same model as doing the transform on loose geometry, within the tolerance weld.
  - Undo and redo of any sequence with groups is exact.
- **Session:**
  - Clicking a group selects it.
  - Double-click opens it, and a line drawn inside lands in the group.
  - Inference snaps to a vertex outside the context.
  - Esc closes the group.
  - This is recorded as a session that replays in CI.
- **Files:**
  - Schema 2 round trip, with the same bytes each time.
  - A schema 1 golden file opens.
- **Screenshot:** an open group with the rest of the model faded and the dashed box.

## Acceptance
- [ ] A box made into a group moves, rotates and copies as one object, and a line drawn across it does not split it.
- [ ] Double-click opens the group, edits inside work, and Esc closes it.
- [ ] The icosahedron case works.
- [ ] A file with groups saves and opens again exactly.
- [ ] PLAN 6.0 is updated, and NOTES lists what is left.
