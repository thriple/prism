# Research: Groups & Components (after Files & Formats)

Written 2026-11-06 (Guam), before the brief. PLAN 1.2 and 1.9 are the spec. This file records design questions found by reading our code, so the brief can settle them.

## What SketchUp does (the behaviour to match, PLAN 1.2)

- **Group:** geometry wrapped so nothing inside sticks to, splits or merges with anything outside. In SketchUp a group is a component definition with one instance that is not listed in the library; copying a group makes the copies independent (SketchUp makes them unique when one is edited).
- **Component:** a named definition and any number of instances, each with its own transform (move, rotate, scale, mirror). Editing one instance edits the definition, so all instances change.
- **Open for editing:** double-click (or Enter on the selection). The rest of the model fades and is locked. A dashed box shows the context. Esc or a click outside closes it, one level at a time. Inference still snaps to geometry outside the context.
- **Make Unique:** the instance gets its own copy of the definition.
- **Explode:** the instance's geometry, transformed, joins its parent. This is where sticky geometry and the overlap split run (PLAN 1.9, the icosahedron case).
- **Glue to and Cut opening:** a component can stick to a face and slide on it, and cut a hole through it (windows, doors). The brief can leave these to a part 2.
- **Outliner and Entity Info:** a tree of groups and components, and a panel for names, tags and the definition.

## What our code means for it

1. **Where the geometry lives.**
   - Today the document has one `prism_topo::Model`.
   - Suggest: one `Model` per definition, in local coordinates, plus a root model. Instances are `{definition, transform, name, tag}` in the parent.
   - PLAN 1.9's "local coordinates" says the same: each definition keeps positions near its own origin, in f64.
   - Every existing operation (sticky geometry, push/pull, offset) then works unchanged inside a definition, because it only ever sees one `Model`. That is what isolation means.
2. **Undo memory.**
   - `prism-doc` keeps a full copy of the model before and after every step.
   - With definitions, put each `Model` behind an `Arc` and copy on write. An undo step then shares every definition it did not change.
   - The Large Scene already makes each step copy a large model, so this helps today too.
3. **Rendering and "hidden must be free".**
   - The scene is built once per definition and drawn once per instance with its transform (GPU instancing).
   - Hidden instances are left out of the instance list, so they cost nothing.
   - Lines and the face-id pass need the instance id too, so picking can tell instances apart. Today the id texture stores a face id; it would become an (instance, face) pair, or a combined id.
4. **Inference across instances.**
   - `prism_infer::infer` takes one `Scene`.
   - With instances it needs the candidates of every visible instance, in world space.
   - Options:
     - transform each instance's candidates on the fly (cheap for a few instances);
     - a world-space cache rebuilt when instances move.
   - This is also where the spatial index noted in NOTES (Edit Tools part 1) starts to matter.
5. **Selection.** `Selection` holds vertices, edges, faces and guides of one model. It needs instances (outside a context) and a path to the open context (inside). PLAN 1.2's blue bounding box for a selected group is new drawing.
6. **Smart Guides.** NOTES says they wait for groups. They come with this chunk or straight after it (PLAN 5.1).
7. **Files.** Do Files & Formats first, as the plan says. Then design the `.prism` layout so each definition is its own geometry chunk from the start, and groups add no format change.

## Decided (2026-11-06)
- Copies of a group are independent at once; only components are linked (DECISIONS #22).
- Glue to and Cut opening come later. When they are built, they get a small visual sign when gluing is possible and a global toggle in the right-click menu (DECISIONS #23).

## Cost: undo states are never drawn or snapped
- Only the current model is drawn and snapped to.
- Undo steps are kept snapshots, and they cost memory only. Sharing unchanged definitions between snapshots (point 2) keeps that small.
- Snapping cost grows with the geometry near the cursor in the visible instances: many copies of a big component all on screen. A spatial index over instances handles that. Hidden instances are skipped (PLAN 1.14).
