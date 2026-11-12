# Prism 3D: research and plan

The goal is a 3D modeling program built from three ideas:

- **SketchUp's modeling feel.** Tools are simple, feedback depends on what you are doing, the margins are generous, and the inference system guides you.
- **Fusion 360's precise curved geometry.** Exact arcs and NURBS curves and surfaces, plus solids with booleans and fillets.
- **Blender's rendering.** Physically based materials and a path tracer.

Later tiers add: (2) animation, (3) particle, water and fabric simulation, (4) texture nodes, (5) projection mapping and compositing.

**The priority over everything else is that the tools feel right.** Every architectural decision below is judged first by whether it lets us make modeling feel excellent, and only then by feature reach.

---

## 1. What "feels right" means

These requirements come from the brief and from studying how SketchUp, MoI 3D and Plasticity behave.

1. **Generous picking.** Clicking reasonably close to a line selects it. Picking is done in screen space with an aperture measured in pixels, not by casting a ray against thin geometry. Vertices win over edges, and edges win over faces.
2. **Show it before the click.** Whatever a click would hit or snap to is highlighted as you hover, with a glyph, a colour and a short label ("Endpoint", "Midpoint", "On Edge", "On Red Axis", "Parallel to Edge", "Tangent"). Nothing should surprise you.
3. **Inference from recent context.** Points, edges and curves you hovered recently become anchors. From each anchor the app projects guides: axis-parallel guides (red, green, blue, plus local axes), parallel or perpendicular guides to a recent edge, and tangent guides from a recent curve. Where two guides cross you get a strong snap point. This is the part the brief calls "monumental", so it gets the most design effort and the most tests.
4. **Stability, no flicker.** A snap holds (hysteresis) until the cursor clearly leaves it. The release radius is larger than the capture radius.
5. **Locking.** Shift locks the current inference. The arrow keys lock to the axes: up is blue, right is red, left is green, and down releases the lock (owner's decision, DECISIONS #17). A lock stays visible until released.
6. **Type while drawing.** After a click, typing goes straight into a measurement box (SketchUp calls it the VCB) without focusing a field first. Examples: `2.5m`, `300,200`, `x5` or `/5` after a copy to make an array, and angles in the rotate tool. Units and simple expressions are supported.
7. **Lines look good.** Edges have a constant pixel width, are anti-aliased, and never z-fight with faces. Profile and silhouette lines are thicker. Selected and hovered states are clearly distinct, and guides are dashed. Line quality is a first-class rendering feature, not a debug overlay.
8. **Few, simple tools.** Each tool does one obvious thing, shows its next step in a status hint, and recovers gracefully: Esc cancels an operation in progress, and the tool stays active.
9. **Sticky geometry, done better than SketchUp.** Edges that touch split each other, coplanar closed loops become faces, and a line across a face splits it. This should be robust where SketchUp's is known to be flaky (near-coplanar faces, a face that fails to split when the new boundary shares only one vertex, twisted faces).
10. **True arcs and circles.** SketchUp stores circles as polygons, so its tangent inference is approximate. The forums note that magenta "tangent" really means aligned to a segment. Prism stores exact arcs and NURBS, so tangent guides are real tangents.

### 1.1 Interaction model: it looks like a mesh, it is NURBS underneath

You see and handle **points, lines and faces**, as in a mesh or SketchUp model. Every one of them is exact underneath:

| What you see | What it is |
|---|---|
| A vertex | An exact 3D point |
| A line (edge) | An exact curve: a line, an arc or circle, or a NURBS curve |
| A face | An exact surface (a plane, a cylinder or cone, or a NURBS patch), trimmed by its edges |
| The shaded model | A tessellation made for display only. It is regenerated and never edited directly. |

Consequences for the design:
- **There is one model and one set of tools, with no "polygon mode" and no "NURBS mode".** A box drawn with the Line tool is planar faces. Push/Pull on a circle makes a true cylinder. Moving an arc's midpoint keeps it an exact arc. Inference works the same everywhere, because it snaps to exact curves and surfaces, not to tessellated triangles.
- **Freeform faces expose their control net as a mesh-like cage** of points and lines. You drag cage points and cage edges with the normal Move, Rotate and Scale tools and the normal inference (axis locks, typed distances), the way you would push mesh vertices. The surface follows smoothly. This is the T-splines/SubD feel from Fusion's Form workspace, but the result is NURBS directly, so there is no conversion step.
- **Mesh-style operations map to exact operations.** Push/Pull becomes extrude or offset of a face. Moving an edge reshapes the adjacent faces (direct modeling). Insert edge loop becomes knot insertion or a split curve. "Smooth" turns a planar cage region into a NURBS patch with the chosen continuity (G1 or G2) to its neighbours.
- **Display tessellation is adaptive to screen-space error,** so curves always look smooth. Edges are drawn from the exact curve, never as visible polygon facets. SketchUp's segmented circles are avoided entirely.
- This choice moves NURBS from a later add-on to the foundation. Exact curves and the unified surface type exist from M2. Planes are just the simplest surface. Freeform cage editing comes in M5.

### 1.2 Points, highlighting, groups and components (follow SketchUp)

**Points are as easy to click as lines.**
- Vertices, including the end and middle points of curves and NURBS cage points, are first-class selectable entities with the same generous screen-space aperture as edges.
- SketchUp cannot select a vertex on its own. In Prism you can click a point, prehighlight it, select it, drag it with Move, and box-select points.
- When a point and an edge are both within reach, the point wins, so the end of a line is always grabbable.

**Highlighting follows SketchUp's visual language:**
- Selected edges turn blue and slightly thicker.
- Selected faces get SketchUp's blue dot stipple, shown in screen space so it stays readable at any zoom.
- Selected groups and components show a blue bounding box.
- On top of that, a subtle hover prehighlight shows what a click will select.
- Colours and stipple are part of the style settings.

**Groups (SketchUp semantics):**
- Select geometry and choose Make Group.
- Copies of a group are independent at once: editing one never changes another. Only components have linked instances (DECISIONS #22). The group isolates its geometry: nothing inside sticks to, splits or merges with geometry outside.
- Double-click opens the group for editing. Everything else fades and can't be edited, and a dashed box shows the editing context.
- Esc (when no tool operation is in progress) or clicking outside closes it, one level at a time. Groups can be nested, and the outliner shows the hierarchy.

**Components:**
- A named definition with any number of instances. Editing one instance updates all of them live, and the others highlight while you edit.
- Each instance has its own transform and can be mirrored.
- "Make unique" splits off a copy, and a definition has its own axes and insertion point.
- "Glue to" lets a component snap to and slide on faces. "Cut opening" lets it cut through the face it is glued to, as windows and doors do. These come after the first Groups & Components chunk (DECISIONS #23). In SketchUp it is hard to tell when gluing will happen, so Prism adds:
  - a small visual sign when the component can glue to the face under it;
  - a global on/off toggle for gluing in the right-click menu.
- A components library browser comes later.

These build on the transactions and stable IDs in section 4. Groups and components live in M3, with selection highlighting in M1/M2.

### 1.3 Export: mesh or NURBS, depending on where the model goes

Because the model is exact underneath, Prism can export either an exact model or a mesh. Each target gets a preset:

| Use | Format | Notes |
|---|---|---|
| Machining or CAM | **STEP** (AP214/AP242) exact NURBS solids | The standard input for Fusion CAM and most CAM software. IGES for legacy tools. |
| Rhino and other NURBS tools | **3DM** via openNURBS (open source, MIT-style licence) | Keeps curves, surfaces, layers and names |
| Routing, laser and CNC 2D | **DXF / SVG** of flat faces, sections or projections | Arcs exported as true arcs, not segments, because CAM turns them into G2/G3 moves. Includes a lay-flat option for sheet parts. |
| 3D printing | **3MF** (preferred: units, colours, multiple bodies) and **STL** | Watertight check and repair before export; units are explicit |
| 3D media, games, web | **glTF/GLB**, **OBJ**, **USD/USDZ**, later FBX | Materials, UVs, smoothing; animation in tier 2 |

Mesh export lets you control chord tolerance, angle tolerance and maximum edge length, and shows a preview with the triangle count. Imports mirror these formats. SketchUp's .skp is out of scope for now: the SDK is proprietary.

### 1.4 Tool set: SketchUp's tools and what Prism does with them

SketchUp's toolbar is grouped as Principal, Drawing, Edit, Construction, Camera, Sections, Solid tools (Pro only) and Sandbox (terrain). Most models are made with about ten tools: Select, Line, Rectangle, Circle/Arc, Push/Pull, Move, Rotate, Offset, Follow Me and Tape Measure, plus Eraser and Paint Bucket. Prism keeps that small core and SketchUp's one-letter shortcuts.

| Tool | Prism plan |
|---|---|
| Select (Space) | Same, plus selectable points (section 1.2) and hover prehighlight |
| Line (L) | Same; creates exact line edges |
| Rectangle (R), Rotated Rectangle | Same |
| Circle (C), Polygon | Circles are **exact**, with no segment count |
| Arc (A): 2-point, 3-point, centre, pie | Exact arcs and real tangent inference; double-clicking a corner fillets it |
| Freehand | Fits a smooth NURBS curve instead of a polyline |
| Push/Pull (P) | Works on **any** face: planar faces extrude, curved faces offset |
| Move (M) | Same, including reshaping by moving points and edges; also moves NURBS cage points |
| Rotate (Q), Scale (S) | Same |
| Offset (F) | Same, and also works on curved faces and curve chains |
| Follow Me | Exact sweep along exact paths |
| Tape Measure (T), Protractor | Same; guides take part in inference |
| Eraser (E) | Erasing the edge between two faces on one plane merges them into one face; where faces meet at an angle, erasing their edge removes both (DECISIONS #15); erasing a hole's edge closes the hole |
| Paint Bucket (B) | Same, with physically based materials |
| Make Group / Make Component (G) | Same semantics (section 1.2) |
| Dimensions, Text, Axes, Section Plane | Same |
| Orbit (O), Pan (H), Zoom (Z), Walk | Same |
| Solid tools (Union, Subtract, Intersect, Trim, Split, Outer Shell) | **Core tools, not a paid add-on**, on exact solids |
| Sandbox (terrain) | Later; NURBS surfaces through contours or a point network |

**New tools SketchUp lacks.** Each one is kept as simple as SketchUp's own tools and follows the shared grammar (section 1.5). The SketchUp extension in brackets is what users install today to fill the gap.

*Edges and corners*
- **Fillet / round edges, chamfer / bevel** (RoundCorner): click an edge, drag or type a radius or distance, and the ghost shows the result. Works on solid edges and on 2D corners.
- **Trim / Extend** curves and edges to where they intersect, a CAD staple.

*Shapes and primitives*
- **Primitives**: box, cylinder, cone, **sphere**, torus, capsule, all exact. SketchUp has no sphere tool and relies on a Follow Me trick.
- **Revolve / lathe**: a profile around an axis, with an angle.
- **Loft / skin** (Curviloft): a surface through several selected edges or profiles. Lofts tend to bulge too far between profiles, so Prism's loft is built to be adjustable after it is made:
  1. **The loft stays live.** Editing or moving a profile with the normal tools updates the surface, until you explode it.
  2. **End conditions as viewport handles.** At each profile an arrow sets the condition: free, tangent (G1), curvature (G2), or tangent to an adjacent surface. Dragging the arrow's length sets the **weight**, which is how far the surface pushes out from that profile. This is Fusion's tangency weight, but done directly in the viewport.
  3. **Loft styles, from Rhino**, picked from a short list with a live ghost preview of each:

     | Style | Behaviour |
     |---|---|
     | Normal | Average stretching between profiles (chord-length parameterization). The default. |
     | Loose | Smoothest result. The surface may move away from the profiles, because its control points sit on the profiles' control points. |
     | Tight | Hugs the profiles closely (square-root chord-length parameterization). The best fix for bulging. |
     | Straight | Straight (ruled) sections between profiles. |
     | Uniform | Uniform knot vectors. Predictable control points for later cage editing. |
     | Developable | Can be unrolled flat without stretching, for sheet material and routing. Rhino moved this to a separate DevLoft command, which works between two curves. |

     Further options, also from Rhino's Loft:
     - **Refit tolerance**: simplifies the profiles to a tolerance first, for lighter surfaces.
     - **Closed loft**: wraps from the last profile back to the first.
     - **Match start/end tangent**: when a profile is the edge of another surface.

     On top of the styles, a **tension control** blends continuously between Straight and Loose, overall or per span, for fine tuning.
  4. **Dragging the surface itself.** Grab a point on the loft and move it, and a guide curve through that point is added automatically. You fix a bulge by pulling it back without drawing rails first.
  5. **Optional rails and spine (centreline).** Rails do not have to touch every profile exactly. Fusion users complain that they must.
  6. **The bulge is visible.** Cross-sections, curvature combs and zebra stripes show the shape. Profile start points can be dragged to fix twisting.
  7. **Escape hatch:** convert the loft to a control cage and edit it freely (section 1.1).
- **Sweep**: a profile along a path, with options for twist and scale.
- **Shell / thicken** (Joint Push Pull): a surface becomes a solid of a set thickness, and a solid becomes hollow.
- **Smooth / subdivide** (SubD, Artisan): cage to smooth NURBS (section 1.1).

*Copying and placement*
- **Mirror**, plus a **live symmetry mode** that mirrors edits as you model.
- **Arrays**: linear, polar and along a path. SketchUp only does the linear and rotate-copy arrays.
- **Align / distribute** selected objects.
- **Drop to face / place on surface**: drop an object until it rests on whatever is below it.

*Deformation*
- **Bend, twist, taper, stretch** (FredoScale), with live ghosts.
- **Draft angle** on faces, for moulds and machining.

*Precision helpers*
- **Construction planes**. SketchUp only has guide lines and points.
- **Snap to grid**, optional, with lower priority than object snaps. SketchUp has no native grid snap.
- **Project curves onto a surface.**

*Fabrication, given the routing and machining goals (section 1.3)*
- **Unfold / flatten** sheet parts to 2D (Unfold Tool, Flattery).
- **Lay flat**: puts a face down on the ground plane, for printing and cutting.
- **Hole** with counterbore or countersink, later.

*3D text, done properly.* SketchUp's 3D Text is a modal dialog with a tiny preview, and it only lists part of the system's fonts.
- **Text is a tool, not a dialog.**
  - Click a face or point, using inference, and type directly in the viewport.
  - The ghost updates live as you type and change settings.
  - Settings sit in a small panel beside the cursor or in Entity Info: font, size, depth, alignment, letter and line spacing.
- **Every font on the system.**
  - Natively, fonts come from the OS font database (`fontdb` / `font-kit`).
  - On the web, the Local Font Access API is used (Chromium only, and it asks permission).
  - You can always load .ttf/.otf files yourself, and they are embedded in the document so the file still opens on another machine.
- **Fonts are listed properly**: each family shown in its own typeface, searchable, with favourites and recent fonts. Weights and styles are grouped under the family, and variable-font axes such as weight and width become sliders.
- **Proper typesetting through a shaping engine** (`rustybuzz`): kerning, ligatures, any script, and right-to-left text.
- **The letter shapes are exact.** TrueType and CFF glyph outlines are already quadratic or cubic Béziers, so they become exact NURBS curves, not polygons. That gives:
  - clean STEP or DXF output for routing, laser cutting and V-carving;
  - overlapping contours unioned automatically;
  - holes in letters such as "o" handled correctly.
- **Output options**: outline curves only, flat faces, or a solid extruded to a depth. Round or bevel edges reuse the Fillet tool.
- **Text along a curve or path**, and wrapped onto a curved face.
- **Text stays editable.** It is a parametric object, so double-clicking it re-opens the text and its settings, until you explode it into plain geometry.

*Light editability without a full history tree*
- The last operation stays editable through the measurement box.
- Primitives keep their parameters (sphere radius, segment of arc) until edited in a way that breaks them. This gives some of Fusion's parametric convenience without a timeline. Section 1.17 sets the rule for how far live editing goes.

---|---|
| Push/Pull | The extrusion follows the cursor, and stacked faces show before you commit |
| Move or copy | The geometry moves live, and reshaped faces deform live |
| Rotate | A full-circle protractor on the inferred plane, a ghost of the result, and the angle readout. It snaps with the same inference as the other tools. **Up arrow locks the protractor to the plane of the face under it**; you can then move the locked protractor elsewhere and click and drag to start the rotation there, as in SketchUp (DECISIONS #18) |
| Offset | The offset outline |
| Follow Me | The swept profile |
| Fillet | The rounded edge at the current radius |
| Eraser | Highlights what will be removed |
| Paint Bucket | Material preview on the face under the cursor |

The status bar gives a one-line hint for the next step, and the cursor icon shows the tool and its active modifier.

**3. Standard modifier meanings.** The same key means the same kind of thing in every tool:

| Key | Meaning | Examples from SketchUp |
|---|---|---|
| **Shift** | Lock or constrain | Lock the current inference or plane (Line, Rectangle, Move); snap to 45° steps (Rotate, Protractor); toggle uniform scaling (Scale); toggle an item in or out of the selection (Select); hide instead of erase (Eraser); replace a material everywhere (Paint) |
| **Ctrl** (Option on Mac) | Make something new instead of changing the original | Copy (Move, Rotate); about-centre (Scale); from centre (Rectangle); add to selection (Select); soften (Eraser); fill connected faces (Paint) |
| **Alt** (Cmd on Mac) | Alternate mode | Allow auto-fold (Move); cycle inference modes (Line); **extrude from a new loop (Push/Pull, tap to toggle)**; follow a face's perimeter (Follow Me); sample a material (Paint) |
| **Arrow keys** | Axis lock | Up blue, right red, left green, down parallel or perpendicular |
| **Esc** | Cancel or back out (see below) | Cancels a tool operation midway, closes UI windows, and exits an opened group or component |
| **Enter / double-click** | Confirm or repeat | See the next point |

**Esc is reserved for cancelling and backing out**, and is never a tool modifier. One press does the first of these that applies:
1. A tool operation is in progress: cancel it. The partial result is discarded, and the tool stays active and ready.
2. A UI window, popup or menu is open: close it.
3. You are inside an opened group or component: exit one level.

Otherwise Esc does nothing.

These are SketchUp's own bindings as far as I can recall. Each should be checked against SketchUp before it's final. All bindings can be remapped.

**4. Tool memory (single-state history per tool)**
- Each tool remembers its last value: distance for Push/Pull, Move and Offset; angle for Rotate; radius for Fillet and Arc; count for arrays.
- **Double-click** applies the remembered value again to whatever is under the cursor. Push/Pull and Offset already do this in SketchUp; in Prism it's the rule for every tool where it makes sense.
- The value shows in the measurement box when the tool is picked. Typing a new value right after an operation edits that operation (section 1).

**5. Workspace basics**
- A colour-coded axis gizmo at the origin (red X, green Y, blue Z), solid on the positive side and dotted on the negative side, which can be toggled on or off.
- The ground plane, or grid, and the horizon can be toggled too.
- The Axes tool moves or realigns the drawing axes. Inference and the axis-lock keys follow whichever axes are active.

### 1.5a Camera navigation (SketchUp muscle memory)

Navigation copies SketchUp exactly by default, so existing muscle memory works on day one.

| Action | Mouse | Hotkey (tool) |
|---|---|---|
| Orbit | Middle-drag | **O** |
| Pan | Shift + middle-drag | **H** |
| Zoom | Scroll wheel | **Z** (drag to zoom, or type a field of view) |
| Zoom Extents | — | **Shift+Z** |
| Zoom Window | — | Ctrl+Shift+W |
| Re-centre on a point | Double middle-click | — |

Behaviour that makes it feel right:
- **Zoom targets the surface under the cursor.**
  - Each scroll step moves a fixed fraction of the distance to the point under the cursor. Big steps when far away, fine steps when close, and you can never zoom through the surface you're aiming at.
  - With nothing under the cursor, the step falls back to the distance to the model's bounds or the ground plane.
  - In parallel projection, zoom scales about the cursor instead.
- **Orbit pivots on what you start dragging over**, so the thing you're looking at stays put. The pivot is the object under the cursor first, then the ground plane under the cursor, and only if neither is hit the model centre (the prototype uses the distance to the target). Gravity keeps the horizon level; Ctrl while orbiting releases gravity and allows roll.
- **Pan keeps the grabbed point under the cursor**, at that point's depth, so the model moves exactly with the mouse.
- **Camera moves never interrupt a tool.** Middle-mouse navigation works in the middle of any operation, for example orbiting halfway through drawing a line or a push/pull. The tool, its ghost and its inference carry on.
  - O, H and Z are normal tools: they stay active after the mouse is released, and Esc returns to the tool you were using (owner's decision, DECISIONS #16). Middle-mouse navigation needs no tool change at all.
- **Smooth but fast.** A short ease on scroll zoom and Zoom Extents, never long enough to feel laggy, and it can be switched off.
- **Trackpads**, which SketchUp handles poorly: two-finger scroll pans, pinch zooms at the cursor, and orbit happens only with the Orbit tool (O) or middle-drag, never from scroll. All of this can be changed.
- **Keymap presets**: SketchUp (the default), plus Blender and Fusion so users from other tools can keep their habits. Every binding can be remapped (section 1.6).

### 1.6 Commands (not tools)

**Greyed out until built (DECISIONS #20):** menus, the dock, the context menu and search show only what is built, even partly, as active. Everything else is greyed and says "Not built yet". Each change that builds something makes it active.

Commands act on the current selection immediately, with no mouse steps. They live in the context menu and the main menu, and each can be given a shortcut.

**Every operation has a menu entry.**

- All tools, commands and settings live in **one action registry**. Each entry has an id, a name, a menu path, a default hotkey and an "enabled when" rule. The menu bar, context menus, toolbars, hotkey bindings and command search are all generated from that registry.
- So nothing can be reachable only by a hotkey or a hidden gesture. A unit test fails the build if any registered action has no menu path.
- Menus show each item's hotkey, and items that don't apply right now are greyed out rather than hidden.
- **Command search** (Shift+S, as in SketchUp) finds any action by name.

Proposed menu bar, close to SketchUp's layout:

| Menu | Contents |
|---|---|
| File | New, Open, Save, Save As, Import, Export (section 1.3 presets), recent files, **Document Settings** (units, and the separator used in the measurement box) |
| Edit | Undo/Redo, Cut/Copy/Paste, Paste in Place, Delete, Select All / Deselect All, Hide Selected / Unhide Selected / Unhide All / Isolate, Lock/Unlock, Make Group / Make Component, Explode, Make Unique, Flip Along X/Y/Z, Intersect Faces, Reverse/Orient Faces, Soften/Smooth, Weld |
| View | Toolbars and panels, show faces / lines / points, axes, ground and grid, guides, hidden geometry, X-ray, line and outline thickness, edge and face styles, lighting (default or sun), shadows, saved styles, section display (section 1.14) |
| Camera | Orbit, Pan, Zoom, Zoom Extents, Zoom Selection, standard views, perspective/parallel, field of view, Walk, Look Around |
| Draw | Line, Freehand, Rectangle, Rotated Rectangle, Circle, Polygon, Arc variants, Curve, Surface (loft, revolve, sweep, patch) |
| Tools | Select, Eraser, Paint Bucket, Move, Rotate, Scale, Push/Pull, Offset, Follow Me, Fillet/Chamfer, Shell, Mirror, Tape Measure, Protractor, Dimensions, Text, Axes, Section Plane, Tolerance Modifier, Fit Helper, Hinge, Slide (section 1.15) |
| Solid | Union, Subtract, Intersect, Trim, Split, Outer Shell, Convert (curve chain ↔ NURBS curve, faces ↔ smooth patch), validate and heal |
| Render | Materials, lights, environment, render view, render settings, engine, output and passes, render queue (from M7, section 1.16) |
| Window | Outliner, Inspector (section 1.17), View Settings, Model Health, Entity Info, Materials, Components, Tags, Preferences (including keymap and inference tuning) |
| Scripting | Greyed out for now. A placeholder for a future scripting and automation layer (section 8, decision 6) |
| Help | Shortcuts reference, the tool hint panel, **Pen and tablet input** (a toggle, greyed out until pen support exists), about |

The right-click context menu shows the subset of this registry that applies to what is under the cursor or selected, for example Flip Along, Make Group, Reverse Faces and Intersect.

**Standard app commands**, each in both the menu and a hotkey (Cmd instead of Ctrl on Mac):

| Command | Hotkey | Notes |
|---|---|---|
| Undo | Ctrl+Z | Covers every edit (section 4 transactions). Pressed mid-operation, it cancels the operation, like Esc. |
| Redo | Ctrl+Y and Ctrl+Shift+Z | |
| Select All | Ctrl+A | Selects everything in the current context: only the contents of an opened group or component |
| Deselect All | Ctrl+T (SketchUp's binding), Ctrl+Shift+A | Clicking empty space with Select does the same |
| Cut, Copy, Paste | Ctrl+X / C / V | Paste follows the cursor with inference until you click |
| Paste in Place | Ctrl+Shift+V | Keeps the original position |
| Delete | Delete | Deletes the selection |
| New, Open, Save, Save As | Ctrl+N / O / S / Shift+S | Plus autosave (section 4) |

**Modelling commands:**

- **Flip Along: X / Y / Z (red / green / blue).** Mirrors the selection across a plane through the centre of its own bounding box, in its local axes for a group or component. It does not flip across the world origin, so the selection stays where it is.
- **Make Group, Make Component, Explode, Make Unique.**
- **Intersect Faces** with the model, with the selection, or within the context.
- **Reverse Faces** and **Orient Faces**.
- **Soften/Smooth edges**, and **Weld edges** (joins chained edges into one curve).
- **Hide, Unhide, Isolate** (hide everything else), **Lock, Unlock**.
- **Booleans** on solids: Union, Subtract, Intersect, Trim, Split, Outer Shell.
- **Center/Align, Zoom Selection.**
- **Convert:** curve chain ↔ single NURBS curve; planar faces ↔ smooth NURBS patch.

### 1.7 Per-tool matrix: where memory, ghosts and constraints make sense

Rules of thumb:
- **Memory** is worth having when the tool's value is a scalar that people often repeat (a distance, radius, angle or count) and one click can name the new target.
  - Where the target is a face or edge, **double-click** reapplies the value.
  - Where the tool works on a selection (Move, Rotate, Scale), **Enter** reapplies it instead, because double-click is not free there.
- **A ghost preview** is for anything that changes geometry. Instant commands use undo instead, and camera tools are their own live preview.
- **Constraints** only make sense where there is a free direction or plane to lock. Push/Pull is already locked to the face normal, so its Shift means something else.
- Double-click keeps its SketchUp meaning in Select (pick a face with its edges, or everything connected) and in Line. It only means "repeat" in tools whose click target is a face or edge.

| Tool | Memory | Ghost | Shift | Ctrl | Alt | Arrows |
|---|---|---|---|---|---|---|
| Select | — | Prehighlight; box shows window or crossing | Toggle in or out of the selection | Add | Pick inside groups without opening them | — |
| Line | — (lengths rarely repeat) | Rubber-band line with length | Lock inference | — | Cycle inference modes | Axis lock |
| Rectangle | Last W×H; Enter reuses it | Yes | Lock plane | From centre | 3-point rotated rectangle | Plane axis |
| Circle / Polygon | Radius and side count; **double-click places the same size** (bolt holes) | Yes | Lock plane | — | 2-point (edge-to-edge) circle | Plane axis |
| Arc | Radius; **double-click a corner fillets it at that radius** | Yes | Lock plane | — | Cycle 2-point, 3-point and centre arcs | Plane axis |
| Freehand | — | The stroke as you draw | Lock plane | — | — | — |
| Push/Pull | Distance; **double-click repeats it** | Yes | Lock to the hovered height, so you can roam to match it | — | **Tap to toggle "new loop": the face boundary stays as a face and a new loop is extruded from it** | — |
| Move | Vector; Enter repeats it on a new selection; array count | Yes | Lock inference | Copy | Allow auto-fold | Axis lock |
| Rotate | Angle; Enter repeats it about the new selection's centre | Yes | **Snap the angle to 45° steps** while held | Copy | — | Up locks the protractor to the hovered face's plane (DECISIONS #18); Right and Left lock the axis to red or green; Down releases |
| Scale | Factor; **a box round the selection (5% off it, as the group box) with grips**: corners scale evenly, edges two ways, faces one way; no inference (owner, 2026-11-09) | Yes | A face grip scales evenly; an edge grip each way | — | **About the centre** (tap to toggle; owner, 2026-11-09) | — |
| Offset | Distance; **double-click repeats it** | Yes | — | — | — | — |
| Follow Me | — | Yes | — | — | Follow a face's perimeter | — |
| Fillet / Chamfer | Radius (or Distance) in its side window, which opens with the tool and previews the hovered edge; **double-click an edge repeats it** | Yes | Include tangent-connected edges | Chamfer instead of fillet | — | — |
| Tape Measure | Guide offset; **double-click an edge makes a guide at that offset** | Guide line | Lock inference | Toggle measuring only vs. making guides | — | Axis lock |
| Protractor | Angle | Guide line | Snap to 45° steps, as in Rotate | — | — | Lock the axis (and so the plane) |
| Eraser | — | Highlights what will go | Hide | Soften | — | — |
| Paint Bucket | Current material (inherent) | Material on the hovered face | Replace everywhere | Fill connected faces | Sample a material | — |
| Dimensions / Text | — | Yes | Lock inference | — | — | Axis lock |
| Section Plane | — | Yes | Lock plane | — | — | Choose axis |
| Axes | — | Yes | Lock inference | — | — | Axis lock |
| Orbit / Pan / Zoom | — | Live (no ghost) | Orbit: pan; Zoom: field of view | Orbit: no gravity | — | — |
| Commands (Flip, booleans, Intersect…) | — | — (undo instead) | — | — | — | — |

The tool memory values all show in the measurement box while the tool is active, so you can always see what a double-click or Enter will apply.

---

### 1.8 Feel-test prototype: what is built, and what it changed in this plan

A mesh-only, browser-based prototype (a Design artifact, "Prism Feel Test") exists to tune the feel of the primary tools before any real engine work. It uses a non-editable ground and a cube hovering above it. It has no NURBS, groups, files or rendering.

**Built (and so behaviour to keep):**
- **Tools:** Select, Line, Rectangle, Push/Pull, Move, Rotate, Scale and Eraser, plus Orbit, Pan and Zoom Extents.
- **Inference:** (planned with Edit Tools part 3: where an edge goes through a face, and the line where two faces cross, DECISIONS #29) Endpoint, Midpoint, On Edge, On Face and Ground, axis guides from the start point and from anchors woken by hovering, guide-cross and guide-on-edge intersections. Candidates are scored with hysteresis. Pick aperture, snap release and hover wake are live sliders.
- **Constraints and values:** Shift and the arrow keys lock axes. Typed values (`2.5`, `40cm`, `45`, `2,1.5`) go through the measurement box. Undo and redo cover every edit.
- **Push/Pull:** stretches closed prisms, extrudes otherwise, and **Alt toggles "new loop"** (section 1.4 and 1.7). Moving and rotating copy with Ctrl.
- **Sticky geometry:** edges split where they cross, closed loops become faces, and non-planar faces are triangulated.
- **Snap target highlight:** snapping to a point or an edge also lights up that vertex (a ring) or edge (a glow), as well as the snap marker.
- **No overlapping duplicates:** coplanar overlaps are split, so each patch of space has one face (section 1.9). A face wholly inside another becomes a hole in it.

**Navigation changes from section 1.5a, adopted from the prototype:**
- The **wheel only zooms**, toward the surface under the cursor. It never orbits.
- **Orbit and Pan are tools.** Left-drag orbits or pans only while the Orbit (O) or Pan (H) tool is active; Esc returns to the previous tool.
- **Middle-drag orbits** around the point under the cursor (object, then ground). **Shift + middle-drag pans.** This works mid-operation.

**UI shell (dark "Soft" look, IBM Plex fonts; the viewport itself is unchanged):**
- A **menu bar** with the nine menus of section 1.6, plus Help > Planned Features.
- A **tool dock** with every tool of section 1.4 in groups (Principal, Draw, Edit, Construct, Camera, Shapes, Copy & place, Solid, Fabricate).
- A **context menu** that shows the subset that applies to the right-clicked face, edge or empty space. Right-click selects the item under the cursor first.
- All three are generated from one action registry (id, name, hotkey, run, enabled-when), as section 1.6 requires. Entries with no implementation are **greyed out, never hidden**.
- **Planned Features** is a scrolling checklist generated from the same registry, with built features crossed out and a built/total count. This list is also the working progress tracker for the prototype.

**Open items the prototype raised:**
1. With Alt now meaning "new loop", SketchUp's Alt behaviour in Push/Pull (pull in a free direction) needs a new binding or is dropped. Ctrl is still unused in Push/Pull.
2. **Scale** currently scales about the selection's bounding-box centre; the plan says Ctrl = about centre. Decide which is the default.
3. The Draw tools that matter next for feel testing are Circle and Arc (exact), then Offset and Tape Measure.

### 1.8a UI style (Motion Kit, dark Soft glass)

The whole program uses the **Motion Kit** (`docs/ui-kit/`) in its **Soft glass** set, dark theme. The feel-test prototype already used its colours and its raised and pressed buttons, but only as a CSS approximation. The full program follows the kit itself: the same colour tokens, emboss shadows, spring motion, keycap hints, tooltips, context menu, command palette, toasts, docked panels, and the frosted "glass cutouts" onto a still aurora (drawn once and redrawn only when something changes, to save power; animating it is an off-by-default option) for slider rails, toggle tracks and dropdowns. The kit is written in React, so it is a specification and not code to reuse: Prism re-implements the look in Rust. `docs/UI-STYLE.md` lists the tokens, the motion numbers, and which kit component each Prism part copies. Every later panel (Inspector, View Settings, Model Health, hotkeys window, dialogs) is built from the same theme layer. The viewport stays outside this style and is drawn by our own renderer.

### 1.9 Overlaps, duplicates and precision

**The rule.** No two faces, edges or vertices may occupy the same space, unless they belong to different groups or to instances of a component. Overlapping coplanar faces are **split**, never merged: the shared patch becomes one face, and each leftover piece becomes its own face. A face drawn wholly inside another becomes a hole in it, and the inner face stays separately selectable and pullable.

Worked example: a group holding an icosahedron is duplicated and moved aside. The copy is sliced in half. The original is moved so that it lines up exactly with the copy's half. When both groups are exploded, the coincident faces, edges and vertices collapse to one, and the result is a single valid mesh.

**When the check runs.** After any edit that creates or moves geometry: Line, Rectangle, Move, Rotate, Scale, Push/Pull, Paste in Place, Import, and Explode of a group or an instance.

**Precision.**
- **Exact predicates first.** Planar and polyhedral work (sticky geometry, push/pull, planar booleans) uses exact predicates, which give exact answers about the numbers that are stored (which side of a plane a point is on, whether two segments cross). Points created by snapping reuse the existing vertex and are never compared by distance. After a rotation or a scale, points that should coincide can differ by rounding, so a weld step with the relative tolerance below still runs after transforms.
- **Tolerance only where numbers are approximate:** curve and surface intersections, imported data, transforms and rotations. It is **relative**, not fixed: a small fraction of the model's size (the prototype uses 1e-6 of the extent, clamped), so a sub-millimetre part and a kilometre-scale site both behave.
- **Local coordinates.** Each group or component keeps its own origin and transform and stores positions near that origin in f64. Geometry merges in a shared frame only when exploded.
- **Degenerate geometry** below the tolerance (slivers, zero-length edges) is cleaned up instead of kept. The tolerance is shown in Preferences and can be changed.

**With NURBS and the wider scope.**
- **Planar faces** merge by exact coincidence of their stored vertices, after the tolerance weld.
- **Curved faces** count as the same when their surface type, parameters and trimming curves match within the model tolerance. Exact arcs and circles keep their centre and radius, so they compare exactly.
- **Curved overlap splits** use the kernel's surface-surface intersection. That is the least proven operation in the pure-Rust kernels (section 2.3), so it is the one to test first. If it proves unreliable, use a heavier kernel for that one operation and keep the exact planar path.
- **Booleans** are the same split-and-merge applied to shared faces. Intersect Faces is the split half.
- **Instances** share their definition and cannot duplicate their own geometry. Duplicates only appear when an instance is exploded into its parent.

### 1.10 Model Health panel (debug and clean-up)

A panel, opened from Window, that lists things that probably should not be there. Each row has a count, and clicking it selects and zooms to the items. Checks are pure functions over the model, so they can be unit-tested, run live in the background and run before export (matching the watertight check in 1.3 and Solid > Validate and Heal).

| Check | Finds |
|---|---|
| Duplicates | Coincident vertices, edges and faces, plus near-misses just under the tolerance |
| Reversed faces | Faces whose normal disagrees with their neighbours or points into a solid |
| Holes and open edges | Boundary edges that should be shared, gaps in a shell, non-manifold edges, and whether each shell is watertight |
| Interior faces | Faces fully enclosed by other geometry. **Clean envelope** removes them and keeps the outer shell, with a preview first and one undo step. It works on the selection by default, to avoid deleting hidden detail by accident |
| Degenerate geometry | Zero-area faces, slivers, zero-length edges, stray loose edges |
| Crossing faces | Faces that pass through each other with no edge along the crossing, connected or not (DECISIONS #29). **Intersect Faces** fixes them |
| Nesting | Counts of groups and components, nesting depth, and instances that share a definition |
| Scale | Parts far from the origin or smaller than the tolerance |

The same checks can draw in the viewport (problem edges coloured, reversed faces tinted), using the debug overlay already planned for inference. Each check is a registered action with a Window menu path, so it also appears in the Planned Features list.

### 1.11 UI modes

The tiers become **modes**: Model (tier 1), Materials (node graphs, tier 4), Render, Animate (tier 2), Simulate (tier 3) and Project (tier 5). A split button at the right end of the menu bar shows the current mode's name and icon. Clicking it cycles to the next mode, Shift+click cycles backwards, and its dropdown arrow opens a list of modes with a hotkey and a one-line description each. A separate hotkey returns to the previous mode.

- **Shared across modes:** the viewport, camera, selection, undo history and inference. A mode swaps only the dock, the side panel, the mode-specific menus and the footer hints.
- **Menus:** File, Edit, View, Camera, Window and Help are in every mode. Draw, Tools and Solid belong to Model. Actions from other modes stay findable in Command Search, which switches the mode when one is chosen. This is a deliberate exception to "grey out, don't hide", and it applies across modes only.
- **Panels are not modes:** Model Health, Outliner, Entity Info, Components and Tags open in any mode from the Window menu.
- **Background work:** renders and simulations keep running when you switch back to Model, with a small progress indicator beside the switcher.
- **Tentative: Scenes.** A possible extra mode, or a panel, for named cameras and saved scene states (camera, visible tags, section planes, style), as in SketchUp's scenes. It is a natural base for Animate (camera paths and cuts) and Project (projectors are calibrated cameras, section 4). Cameras are already first-class scene objects, so this costs little extra data. Whether it is a mode or a panel is undecided.

### 1.12 Hotkeys preferences window

Window > Preferences > Keyboard. Every binding comes from the action registry (section 1.6), so anything with a menu entry can be bound.

- **Table:** all actions grouped like the menus, searchable by name or by key. Columns: action, primary key, alternate key, the mode it applies to (all modes, or one), and a dot where it differs from the preset.
- **Recording:** click a cell and press the new combination. Esc cancels, Backspace clears. Modifiers are allowed; keys are stored by physical position, so L, R and P stay put on AZERTY and other layouts.
- **Conflicts:** if the key is taken, the window names the other action and offers Replace, Keep both (only if they apply to different modes) or Cancel.
- **Reserved keys:** in the web build it warns about keys the browser takes (Ctrl+T, Ctrl+N, Ctrl+W, Ctrl+Tab, F-keys) and suggests another. The native build warns only about OS-reserved keys (Cmd+Q, Alt+F4).
- **Modifiers tab:** the Shift, Ctrl and Alt meanings from section 1.7 can be remapped. The window warns when a change breaks the rule that the same key means the same kind of thing in every tool.
- **Mouse and navigation tab:** orbit, pan and zoom buttons, invert zoom, sensitivity, trackpad gestures.
- **Presets:** SketchUp (default), Blender, Fusion, and "Custom". Reset one action or everything.
- **Share:** export and import as a JSON file, and print a one-page cheat sheet. The measurement box's typed characters are not remappable.
- **Everywhere else:** menus, tooltips and the hint panel show the current binding, Ctrl or Cmd per platform.

### 1.13 Saving, files and formats

**The native format: `.prism`.** One file per document, so it works in any synced folder (iCloud, OneDrive, Dropbox) and can be emailed.
- **Container:** a ZIP archive holding a versioned manifest, the scene graph and definitions, binary geometry chunks, node-graph materials, embedded textures and fonts, and a thumbnail. Embedding makes the file portable, and Model Health can purge what nothing uses.
- **Contents are exact:** planar faces with holes, exact arcs and circles, NURBS control points and knots, stable IDs, groups and components with their transforms, tags, scenes and cameras, section planes, guides, units and the model tolerance (section 1.9). Cached tessellation is optional and only speeds up opening.
- **Undo history is not saved** unless asked. Parametric leftovers (a primitive's radius, a live loft) are saved as parameters.
- **Versioning:** every file carries a schema version. New versions open older files through migration functions, and unknown fields are kept, not dropped. Golden-file tests (old files, opened, saved and compared) run in CI.
- **Large files:** geometry chunks are binary and load in the background with progress. They are not one giant JSON file.

**Saving safely.**
- **Atomic saves:** write to a temporary file, then rename. The previous version is kept as `name.prism.bak`.
- **Autosave and recovery:** the command log (section 4) is journaled to a recovery file every few seconds or edits. After a crash, Prism offers to restore it. On the web the journal goes in IndexedDB, which every browser supports. The browser's private file system (OPFS) only recently gained a main-thread writer in Safari, and we use no WASM threads.
- **Exact numbers in text:** wherever f64 values go through JSON, serde_json's `float_roundtrip` feature is on. Without it, about 11% of ordinary coordinates come back one bit off (tested 2026-11-06, `docs/research/files-and-formats.md`).
- **Synced folders:** atomic writes avoid half-written files. If a sync service makes a conflict copy, Prism opens either one normally.
- **Web build:** Chrome and Edge on desktop can open and save in place through the File System Access API. Firefox and Safari have no save picker, so Save downloads a copy there. Chrome on iPad cannot save in place either, because iOS gives it Apple's browser engine. There, Save downloads a copy and Open uses the Files picker, and a copy is also kept in the browser's private storage as a safety net.
- **Several documents:** tabs in one window (DECISIONS #27). Double-clicking a `.prism` file opens it in a new tab of the Prism that is already running, or switches to its tab if it is open (DECISIONS #24).
- **Native builds:** recent files appear in the File menu. A file named on the command line opens, which is how Windows passes a double-clicked file. Registering `.prism` as a file type comes in steps (DECISIONS #24):
  - **Files & Formats:** the Mac app's `Info.plist` declares the `.prism` type.
  - **Packaging step:** macOS sends a double-clicked file as an Apple event that winit does not pass on, so Prism adds a small handler of its own. The same step adds Windows file-type registration, which needs an installer.

**Import and export formats.** The formats below extend the presets in section 1.3.

| Group | Formats | Import | Export | Milestone |
|---|---|---|---|---|
| Native | `.prism` | yes | yes | Files & Formats |
| Mesh | STL (binary and ASCII), OBJ with MTL, 3MF, glTF / GLB, PLY (crates and licences checked in `docs/research/files-and-formats.md`; 3MF is written with our own code, DECISIONS #25) | yes | yes | Files & Formats |
| Mesh, later | USD / USDZ, FBX | USD later | yes | later |
| Exact CAD | STEP (AP214, AP242), IGES | yes | yes | Solids & CAD |
| Rhino | 3DM (openNURBS) | yes | yes | Solids & CAD |
| 2D | DXF, SVG | yes | yes | Solids & CAD |
| 2D print | PDF (vector) | — | yes | later |
| Images | PNG, JPG, WebP, HDR, EXR for textures and environments | yes | PNG, JPG, EXR for renders | Rendering |
| Video | MP4, image sequences | — | yes | Animation |
| Fonts | TTF, OTF | yes (embedded in the file) | — | later |
| SketchUp | `.skp` is out of scope (proprietary SDK). Use SketchUp's own export to OBJ, STL or Collada (DAE) | DAE | — | later |
| Not planned | DWG, Fusion `.f3d`, Parasolid and ACIS native files | — | — | export STEP from the other program |
| Architecture, later | IFC | maybe | maybe | undecided |

**Import rules.**
- **Units are asked or detected,** and a scale is offered, with the model size shown before it is accepted.
- **Faces are rebuilt:** CAD edge data does not always include faces, so Prism rebuilds them.
- **Meshes:** an option recovers planar faces by merging coplanar triangles, and another softens edges below an angle. Otherwise the mesh is kept as triangles.
- **Merging:** imported geometry goes through the duplicate and overlap rules (section 1.9) using the relative tolerance, or is placed in its own group.
- **Far-from-origin models** are moved near the origin and keep their original offset, so nothing jitters (section 4).
- **A Model Health report** (section 1.10) appears after every import.
- **Large imports** run in the background, with progress and a cancel button.
- **Drag and drop** onto the window imports. winit delivers dropped files natively but not on the web, so the web build listens for drops on the canvas itself.

**Export rules.**
- **Scope:** the whole model, the selection, visible only, by tag, or one component.
- **Settings:** units, up axis (Z up, or Y up for glTF and OBJ), and the chord and angle tolerances and maximum edge length from section 1.3, with a preview of the triangle count.
- **Checks:** a watertight check before printing formats, with the Model Health fixes offered.
- **Structure and materials:** objects per body or per component, with materials and textures carried where the format allows.
- **Reproducible:** the same model gives the same file.

**Fidelity is stated, not hidden.** Exporting exact geometry to a mesh format is lossy, and the export dialog says so. Importing a mesh does not recover exact curves. A round trip through STEP keeps NURBS surfaces and solids, and through 3DM keeps curves, surfaces, layers and names.

### 1.14 Modeling view options

How the model is drawn while you work. Everything here is a registered action (section 1.6), lives in the View menu and in a docked **View Settings** panel, and is saved per **style**. A style is a named preset (Default, Hidden Line, X-ray, Technical, Presentation) that scenes can remember (section 1.11).

**What to show.** Each is a toggle, with a one-click row of buttons in the toolbar.
- **Faces, Lines and Points** can each be switched off on their own. Points are the selectable vertices from section 1.2, with a size setting.
- **Guides, Axes, Ground, Grid, Section planes, Dimensions and Text** each have a toggle.
- **Hidden geometry** (below).

**Lines.**
- **Line thickness** and **outline (profile) thickness** are separate settings in screen pixels. The outline is the heavier line along silhouettes, and 0 turns it off. Neither changes with zoom unless "Scale with zoom" is on.
- **Profile lines (as SketchUp's Profiles).** Owner request, 2026-11-09. An edge is drawn as a profile when exactly one of its faces is turned toward the viewer and seen: silhouettes (one face toward you, one away), the border of an open face, and an edge whose other faces are hidden. Profiles are worked out per group or component, so a group's outline stays heavy where it touches other geometry. The rule depends on the view, not on a fixed angle, so profiles move as you orbit. Profiles also help find modeling problems: a face that did not merge into the face beside it, lines that are not aligned, or a gap all show as a stray heavy line. The renderer already draws them with this rule for edges with one or two faces, off by default and set from the debug panel. View Settings gives them a menu toggle and a thickness, and turns them on in the default style. Still to do: edges with three or more faces, and the "seen" part (occlusion), which needs the id buffer.
- **Colour lines by:** all one colour, by material, by axis, or by tag. Selected, hovered and snapped lines keep their own colours.
- **Occluded lines** can be hidden, dashed or faint, which is the Hidden Line style.
- **Anti-aliasing** and DPI scaling are set here, with a "fast mode while orbiting" option for heavy models.

**Faces.**
- **Styles:** Shaded, Shaded with textures, Monochrome, Hidden Line, Wireframe.
- **Front and back colours.** Back faces are tinted so reversed faces stand out (this is also what the Model Health reversed-face check highlights).
- **Transparency mode (X-ray).** All faces become see-through, with an opacity slider, so you can snap to things behind others. A material can also carry its own transparency. Sorting is "Fast" (sorted by depth) or "Accurate" (order-independent), because the fast mode can flicker on intersecting faces.

**Hidden geometry** is two different things, and the menu names them separately.
- **Hidden entities** are things you hid on purpose. *Hide Selected*, *Unhide Selected*, *Unhide All*, *Unhide Last* and *Isolate* are in Edit (section 1.6). "Show Hidden" draws them as a faint dotted ghost so you can select and unhide them.
- **Hidden lines** are lines blocked by faces (see Lines above), and soft or smoothed edges.
- Tags can be switched on and off as a group, and a tag's state is part of a scene.

**Hidden must be free.** Hiding groups, components, instances and tags is the main way to keep a heavy model responsive, so the plan makes it a rule: *a hidden object costs memory and nothing else.* Every system asks one shared "visible scene" and never looks at hidden objects.

| System | Rule when something is hidden |
|---|---|
| Drawing | Not drawn, not tessellated, not in the line or face passes. Its GPU buffers are freed after a short delay and rebuilt lazily when it is unhidden |
| Picking and inference | Not a candidate for a snap, a highlight or a click, and not in box select or Select All |
| Occlusion and hidden-line drawing | Does not hide anything behind it |
| Shadows and the sun | Casts no shadow |
| Final render and simulation | Excluded, unless the item is set to "render anyway" |
| Model Health, Fit Helper, Check Fit, motion sweep | Skipped by default, with an "include hidden" option |
| Export | Skipped by default (section 1.13 export scope "visible only") |
| Spatial index | Toggling visibility changes a flag on a top-level object, not the whole index, so hiding is instant and does not rebuild |

- **Instances:** hiding one instance excludes only that instance. The shared definition stays in memory while any instance is visible, and a hidden instance costs a transform.
- **Still paid for hidden objects:** memory, saving and loading the file, and undo history. Rows in the Outliner are virtualised, so a long tree costs only what is on screen.
- **Kept by design:** an operand of a live boolean stays hidden and editable inside its body (section 1.17), and a joint can refer to a hidden part. Those are evaluated when their owner is, but do not draw.
- **Loose hidden geometry** (hidden faces and edges that are not in a group) is not visible to inference or sticky geometry, so drawing over it could create overlaps. When it is unhidden, the overlap and duplicate rules (section 1.9) run, and Model Health lists any it fixes. Hiding groups, components and tags is the recommended way to manage a heavy model.
- **Display-only options that keep the object:** *Bounding box* and *low detail* display modes for objects you still want in the final render but not drawn in full while modeling, and an **Unload** command for components that frees their memory until needed (for example, a large referenced component).
- **Tested, not assumed:** a performance test hides 90% of a large model and checks that frame time, inference time and picking time are within a few percent of a model that only contains the visible 10%. The test runs in CI with the debug overlay's statistics (section 1.14, Performance).

**Lighting.**
- **Default lighting** is a soft headlight plus a sky-and-ground gradient. It is fast, uses no shadows, and is the standard working view.
- **Sun** places a real sun from a date, a time and a location, with a north angle. It drives shadows here and the sky and sun in a render (section 1.16). A time-of-day slider is in the toolbar.
- **Shadow settings:** shadows on faces, on the ground, and from edges, each switchable. Also a shadow softness and quality setting, and light and dark sliders for how strong the lit and shaded sides look.
- **Ground and sky:** ground colour and opacity, whether to show below ground, a sky gradient and horizon colour, and distance fog.

**Hierarchy tree (the Outliner).**
- **Shows** the model as a tree: groups, components, instances and tags, with a count and a nesting depth.
- **Each row** has an eye (visibility), a lock, a name you can edit, and an icon that says mesh or exact. You can drag a row to move it, and search or filter by name or tag.
- **Selection** is shared both ways with the viewport. Double-clicking a row enters it, and Esc leaves (section 1.5).
- **Definitions** are listed separately from their instances, so you can see what shares what. This feeds the nesting counts in Model Health (section 1.10).

**Performance.** Level-of-detail for tessellation, optional edge anti-aliasing, "fast mode while orbiting", and a stats overlay (faces, edges, frame time) that is shared with the inference debug overlay.

### 1.15 Machining tolerances, fit and mechanisms

Prism is meant to feed routers, laser cutters, CNC machines and printers (section 1.3). Parts that fit together need clearances, and mechanisms need to move without colliding. Two ideas cover this: **tolerances as a modifier**, and **joints with a motion check**.

**Design geometry stays nominal.** You model the exact intended shape. Tolerances are applied to a *fabrication copy* when you export, and a "Show compensated" toggle previews it. Changing a tolerance never changes the model you drew.

**Tolerance modifier.** A live modifier on a body, a face, a loop of edges or a feature. It stays editable until baked, and it is attached by stable ID, so editing the model does not detach it.

| Type | Does |
|---|---|
| Clearance fit | Grows holes and slots, or shrinks tabs and pegs, by a set amount |
| Press fit | The reverse, with a negative clearance |
| Kerf and tool compensation | Offsets outlines by half the cutter width (laser kerf, router bit) on the correct side |
| Internal corner relief | Adds a dog-bone or T-bone at inside corners smaller than the tool radius, or rounds them to the tool radius |
| Shrink and expand | Scales a part for printing shrinkage, optionally per axis |
| Minimum feature | Flags walls, holes and slots below a size the process cannot make |

**Machine and material profiles.** A library of presets, each editable, for example "6 mm ply on a laser, 0.15 mm kerf", "CNC, 3.175 mm bit", "FDM, 0.2 mm clearance". A profile fills in the modifier defaults. Which profile applies is chosen per body or for the whole document, and it is saved in the `.prism` file.

**Fit Helper.** A checker, in the style of Model Health (section 1.10), that finds the places where tolerances are needed.
- **It finds** nested and mating geometry: tabs in slots, pegs in holes, parts sitting in pockets, and bodies with coincident or zero-gap faces.
- **It reports** interferences (overlapping volume), contacts with zero clearance that should have a gap, clearances tighter than the profile allows, thin walls, holes smaller than the tool, and inside corners sharper than the tool radius.
- **One click assigns** the modifier to the right side of a pair. Rule: the hole or slot side gets the clearance, and the peg or tab side stays nominal, unless you swap it.
- **Check Fit** colours the model by the smallest gap between bodies, so too-tight and too-loose areas are visible at a glance.
- **It runs** on the selection by default, so it doesn't change hidden detail by accident. Fixes are previewed first and are one undo step.

**Joints and mechanisms.** Basic constraints between components or groups, enough to check that something fits when it moves.
- **Hinge** (rotation about an axis, with limits), **Slide** (movement along an axis, with a range), **Ball** (rotation about a point, with an optional cone limit and a twist limit), and **Fixed** (locked together). Cylindrical and planar joints can follow.
- **Creating one:** pick two parts, then pick the axis (or, for a Ball joint, the centre point) with the normal inference, for example an edge, the axis of a hole or exact cylinder, a sphere's centre, or a face normal. Set the limits in the Inspector (section 1.17) or by dragging to the limit.
- **Moving:** with a joint in place, Move and Rotate on one part respect the joint, and you can type an angle or a distance in the measurement box. Saved poses (Open, Closed) are stored as scenes.
- **Sweep Motion Range** moves a joint through its whole range and reports the angle or distance where the first collision happens, and the smallest clearance across the whole sweep. Clearances from tolerance modifiers are included, so a hinge pin in a hole is tested with its real gap.
- **First-class data:** joints are entities with stable IDs, drawn as small gizmos that can be hidden, and a joint's position is an animatable property (Animation, tier 2).
- **Solving:** a tree of joints (a chain of parts) first, then closed loops such as a four-bar linkage later.

**Out of scope here:** physics-based motion, forces and stress analysis. Rigid-body dynamics belongs to Simulation (tier 3).

**Dependencies.** Planar and mesh parts can use the exact predicates from section 1.9 first. Clearance and interference checks on curved NURBS parts need the kernel's distance and intersection queries (section 2.3), so those come after Solids & CAD.

### 1.16 Rendering options

Two separate things share the word "rendering": the **viewport** (section 1.14) and the **final render**. This section covers the final render, from the Render menu and a **Render Settings** panel. Settings are saved per scene, with presets (Draft, Preview, Final, Print).

**Engine (provider).** The built-in path tracer (wgpu, runs natively and on the web), or Cycles or Blender as an external renderer where one is installed. Section 4.1 explains how Prism picks what is available.

**Quality.**
- **Sampling:** a sample count, a time limit, or a noise threshold. Adaptive sampling and progressive refinement are on by default.
- **Bounces:** separate limits for diffuse, glossy, transmission and volume. A clamp setting limits fireflies. Caustics are off by default.
- **Resolution:** size, a percentage for quick tests, an optional crop region, and DPI for print.
- **Device:** GPU or CPU, which GPU, and a memory limit, with tiling when a scene is bigger than memory.
- **Denoiser (OIDN):** on or off, a strength setting, and use of the albedo and normal passes. It can be applied only to the preview, or to the final too.

**Camera and exposure.**
- **Camera:** from a scene camera (section 1.11), focal length or field of view, sensor size, depth of field with f-stop, and a focus distance picked by clicking in the view.
- **Exposure:** EV, or ISO with shutter, plus white balance and a view transform (Standard, Filmic, AgX, ACES).
- **Post effects,** all optional: glare, vignette and motion blur (when animation exists).

**Lighting.**
- **Environment:** an HDRI with rotation and strength, or the sky and sun from the View settings (section 1.14).
- **Lights:** point, spot, area, sun, IES profiles and emissive materials. A **light mixer** lists every light with its intensity and a solo button.
- **Background:** film transparency, a shadow catcher for compositing onto a photo, and a ground plane option.

**Materials.**
- **Material overrides:** a clay render, a wireframe overlay, and ambient occlusion only, for checking form separately from colour.
- **Surface detail:** bump versus true displacement, and texture mapping from the UV tools (section 6, Rendering).

**Outputs and passes.**
- **Passes:** beauty, depth, normal, albedo, object ID and material ID, shadow, ambient occlusion and emission.
- **File formats:** PNG and JPG for finished images, and EXR (16 or 32 bit float, multilayer) for passes and compositing.
- **Output:** folder and naming, a batch over scenes, and a turntable.

**Workflow.**
- **Rendered viewport:** a "Rendered" face style shows the path tracer live in the viewport.
- **Render queue:** renders run in the background, with progress, cancel and a history you can compare side by side.
- **Section planes** cut the model in the render too.

**Blender bridge (stopgap).** Before the built-in tracer is finished, Render > Render in Blender exports the scene (USD or glTF) and runs Blender or Cycles headless as a separate program, then loads the image. Cycles is Apache licensed. Blender itself is GPL, and running it as its own program does not make Prism GPL, but copying Blender's code into Prism would (section 8).

### 1.17 Inspector panel

A single docked panel (Window > Inspector) that shows whatever is selected and lets you change it. It extends Entity Info and shares the viewport's selection and undo. It is **schema-driven**: every entity and feature declares its properties (name, type, unit, range, step, whether it previews live), and the panel draws itself from that. New tools get an Inspector automatically, and the same schema is what Animation later uses for keyframes (section 4).

**Always shown (top).** What is selected (icons and counts), its name, tag, material, visibility and lock, and a breadcrumb of the groups it sits in.

**Sections, by what is selected.**
- **Transform:** position, rotation and scale, in local or world terms, and the pivot.
- **Dimensions:** bounding-box size, length, area and volume.
- **Geometry:** exact or mesh, face and edge counts, the normal of a face, and a "make exact" or "freeze" action where it applies.
- **Material, Tolerance (1.15) and Joint (1.15):** shown when relevant.
- **Features:** the live operations on a body (below).

**Editable features.** The plan's rule is light editability without a full history tree (section 1.4). The Inspector extends it to the operations where re-editing matters most:

| Feature | Editable parameters |
|---|---|
| Boolean | Operation (Union, Subtract, Intersect, Trim, Split), operand order and a swap button, keep operands (hidden and editable) or consume them |
| Chamfer / Fillet | Distance, radius, symmetric or two-distance or distance-and-angle, which edges, and whether tangent-connected edges follow |
| Shell / Thicken | Thickness, direction, which faces stay open |
| Hole | Diameter, depth, counterbore or countersink size and angle |
| Primitives | Radius, height, and so on |
| Array and Mirror | Count and spacing, or the path, or the mirror plane and whether to merge |
| Loft and Sweep | Style, tension, end-condition weights, twist and scale (section 1.4) |
| Offset, Push/Pull, Draft | Distance or angle |
| Text | Font, size, depth, alignment |
| Section Plane | Position and direction |

- **Live until baked.** These features stay editable until you choose **Bake** (or **Explode**), which turns them into plain geometry. One-shot operations such as Move stay editable only through the measurement box (section 1.7).
- **Edge and face references** are picked in the viewport with a "pick" button on the field, using Shift and Ctrl to add and remove, or typed from a list. References are stored by stable ID with a geometric fallback, so a feature survives edits to the model.
- **Previews:** changing a value shows the ghost result, and a warning (for example "chamfer is larger than the edge") shows in red with a message instead of silently failing.

**Editing.**
- **Fields:** accept typed values with units and simple expressions (`2*25.4`), and can be scrubbed by dragging. Each committed change is one undo step.
- **Several items selected:** shared properties are shown, differing values appear as "—", and an edit applies to all of them.
- **Named parameters** (later): a small table of values such as `thickness = 6 mm` or `kerf = 0.15 mm`, usable in any field and in tolerance modifiers, so one change updates every part.

**Where live editing stops: a modifier stack per body, not a global timeline.** Booleans, fillets, shells, arrays, mirrors, tolerance modifiers and joints are *modifiers* applied in order on top of a body's base geometry. Direct edits (Line, Push/Pull, Move) change the base geometry underneath the stack, and the stack re-evaluates. If a direct edit removes something a modifier refers to, the modifier is marked broken and asks you to re-pick it or bake it. **Bake** collapses a stack into plain geometry. This keeps Plasticity-style direct modeling, with no project-wide history, while still allowing re-editing a chamfer or a boolean.

**Bodies, definitions and instances.** A *body* is a group or component that holds a closed shell (a solid). Booleans, shells and the watertight check work on solids. Modifiers and joints' geometry belong to the component *definition*, so every instance updates together. Per-instance state is the transform, a joint's current pose, a material override and the tag.

**Risks.** Keeping features live raises the "topological naming" problem: after an edit, which face is the one the chamfer pointed at? Prism uses stable IDs first and a geometric match second, and falls back to asking you to re-pick, rather than failing silently.

## 2. Research summary

### 2.1 SketchUp inference (the reference to beat)
- **Point inferences:** Endpoint, Midpoint, Arc Midpoint, Intersection, On Edge, On Face, Center, and Tangent at Vertex. Each has its own colour and screen tip.
- **Linear inferences:** On Red, Green or Blue Axis; From Point (a dotted guide in the axis colour from a remembered point); Parallel and Perpendicular to an edge (magenta); Through Point.
- **Hovering encourages an inference.** Pausing over a point makes SketchUp prioritise alignment with that point. Prism should make this explicit: an anchor memory with a visible "woken" state.
- **Locking:** Shift locks the current inference. The arrow keys lock axes (up blue, right red, left green, down parallel or perpendicular). In the Line tool, Alt or Cmd cycles through all inferences, none, and parallel/perpendicular only.
- **Weak spots to improve on:** segmented arcs make tangents approximate; tangent cues do not appear from guide points or intersections; face splitting is flaky in edge cases.

### 2.2 MoI 3D and Plasticity (NURBS with a good feel)
- MoI's "straight snap" locks to a line from the previous point when the mouse is near it. It defaults to 90°, and Shift holds it. Object snaps take priority over grid snaps. MoI shows that NURBS modeling can feel as light as SketchUp.
- Plasticity is "CAD for artists": a direct modeler on Parasolid with a Blender-like UI and no history tree. It shows the market for a kernel-strength modeler that skips parametric timelines.
- Fusion 360 T-splines: conversion to NURBS splits the surface at every star point, and trimmed NURBS cannot be converted back. Takeaway: treat SubD as a later, separate representation that converts one way into NURBS.

### 2.3 Geometry kernels
| Option | Strengths | Problems for us |
|---|---|---|
| **OpenCascade (OCCT)**, LGPL | Mature NURBS B-rep: booleans, fillets, offsets, STEP and IGES | C++. Its browser build (opencascade.js, about 4–10 MB) does not link easily into a Rust `wasm32-unknown-unknown` build; it would need its own WASM module with JS glue. API is heavy. |
| **NVIDIA usd-brep / SMLib**, Apache-2.0, newly open-sourced | Mature NURBS B-rep (booleans, fillets, offsets, healing, tessellation) | C++. New to open source. A Rust port (solidcore) is not usable yet. |
| **truck / monstertruck / arris** (pure Rust) | Native Rust, compiles to WASM, NURBS B-rep. monstertruck adds fillets, healing and STEP. arris has booleans on analytic and NURBS faces plus constant-radius fillets. | Young. Robustness of surface-surface intersection is unproven at scale. |
| **Manifold**, Apache-2.0 | Mesh booleans that are guaranteed manifold, fast, with a WASM build | Triangle meshes only, so not exact NURBS. |
| **Our own** | Full control over the data model and the feel | Building NURBS surface-surface intersection and fillets is a multi-year effort. |

**Decision: own the data model, borrow the hard maths.** Prism has its own topology (non-manifold B-rep, which SketchUp-style loose edges and faces need) and its own exact curve and surface types. Planar and polyhedral operations (sticky geometry, push/pull, intersect, planar booleans) are ours and use exact predicates (Shewchuk-style adaptive predicates; plane-based exact booleans such as EMBER and Cherchi et al. are references). Hard NURBS operations (surface-surface intersection, fillets, offsets) sit behind a `Kernel` trait. At Milestone 5 a spike will pick a backend: monstertruck or arris first (pure Rust, works in the browser), OCCT or SMLib as the fallback. **The spike (2026-11-12) picked arris** (`docs/research/nurbs-spike.md`).

### 2.4 Lines and viewport
- The standard technique is to upload only segment endpoints, expand each segment into a quad in screen space in the vertex shader (instanced), and anti-alias with a signed distance in the fragment shader. This gives any pixel width with AA independent of MSAA (Magnum's LineGL, the facett-core capsule lines). Hardware lines are unreliable above 1 px.
- SketchUp only lets you change profile thickness. Its users complain about edge flicker, invisible selected edges under MSAA, and lines on high-DPI screens. Prism should beat it: DPI-aware widths, depth bias in view space, and selection drawn as a separate highlight pass.
- Trimmed NURBS display: tessellate adaptively from screen-space error on the CPU, then draw with a z-buffer. Direct GPU trimming (Guthe et al.; Schollmeyer & Froehlich) can come later if needed.

### 2.5 Rendering, simulation and the platform
- **Path tracing on WebGPU:** feasible (a 2024 paper pairs WebGPU with OpenPBR). three-gpu-pathtracer is the most mature in the browser. Wavefront and megakernel designs are both used. Denoising: `oidn-webgpu` runs Intel's OIDN network on WebGPU, both in the browser and natively through wgpu, guided by albedo and normals.
- **Cloth:** XPBD with small steps on WebGPU compute is proven. Mass-spring cloth reaches 60 fps with 640K nodes.
- **Water:** MLS-MPM on WebGPU handles 100K particles on an integrated GPU and about 300K–1M on discrete GPUs.
- **Platform:** wgpu with Rust runs the same rendering code natively (Vulkan, Metal, DX12) and in the browser (WebGPU, with a WebGL2 fallback for basic drawing). egui integrates with wgpu. One caveat: on the web, wgpu objects are not `Send`/`Sync`, so threading has to be designed around that.

---

## 3. Platform and stack (recommendation)

**Rust + wgpu + winit, with egui for panels, built two ways from one codebase:**
- **Web build** (WASM + WebGPU), published to GitHub Pages by CI on every push. You can try each build from any desktop browser with nothing to install, the same way Prism's APK reached Releases.
- **Native desktop builds** (Windows, macOS, Linux), attached to GitHub Releases by CI. Native is needed later for heavy sims, for projection mapping (fullscreen output on a second display or projector), and for large files.

Why Rust:
- Geometry code needs speed, f64 precision and strong types.
- One language spans the kernel, the GPU host code and the UI.
- It compiles to WASM cleanly.
- Pure-Rust NURBS kernels exist.

Why not TypeScript + three.js: faster to start, but the kernel, sims and path tracer would end up in WASM anyway, splitting the codebase in two.

egui is fine for panels and the measurement box. The viewport, glyphs, guides and tool feedback are drawn by our own renderer, so their look is fully ours.

### 3.1 Packaging decision

**The native desktop app is the real product. The web build is for fast testing and for the iPad.** Both come from the same Rust codebase, as above.

| Target | How it ships | Role |
|---|---|---|
| **Windows** | Native build (winit + wgpu on D3D12/Vulkan), installer or zip on GitHub Releases | Main target. Full keyboard, no browser in the way |
| **macOS** | Native universal build (Apple Silicon and Intel, Metal), .dmg on GitHub Releases | Main target. Cmd replaces Ctrl, and menus can use the system menu bar |
| **Web** | WASM + WebGPU on GitHub Pages, redeployed by CI on every push. Installable as a PWA so it opens in its own window | Rapid testing, sharing a link, and the iPad |
| **iPad** | The hosted web build opened in Chrome or Safari (both use Apple's WebKit engine on iOS), with a keyboard and trackpad. A native iPadOS app (wgpu and winit both support iOS) is the only route to a proper installed app, and it needs Apple signing and TestFlight or the App Store | Light use and review, not the main modelling device. iOS restricts unsigned local code and heavy in-browser work, so a locally saved copy of the page will not run. Touch and pen stay out of scope (section 3) |

**Why native for hotkeys.** A browser takes keys before the page sees them (Ctrl+T, Ctrl+N, Ctrl+W, Ctrl+Tab, F-keys, Alt for its own menu), middle-click can start auto-scroll, a right-click opens the browser menu unless the page cancels it, and Esc can leave fullscreen or pointer lock. The plan binds SketchUp's keys (Ctrl+T deselects, Ctrl+Shift+W zooms to a window), so some of these collide. A native window receives every key. The PWA window removes most of the browser's shortcuts, and Chrome's keyboard-lock API can capture more in fullscreen, so the web build stays usable but is never the reference.

**Decisions that follow**
1. **One action registry (section 1.6) drives everything:** the in-app menu bar and context menus, the system menu bar on macOS (through a native-menu crate such as `muda`), hotkeys and command search. Hotkeys display as Ctrl on Windows and Cmd on macOS.
2. **Bind tool hotkeys by physical key position** (`KeyboardEvent.code` on the web, scancodes natively), so L, R, P and the rest stay in the same place on AZERTY and other layouts. Typed values in the measurement box use the typed characters.
3. **No Electron or Tauri wrapper.** The native build already has its own window and menus. A wrapper would only add a second runtime and still rely on the system webview's key handling.
4. **CI builds all of these on every tagged release:** Windows, macOS and web, with the web build also deployed to Pages on every push to main.
5. **Signing, later:** an unsigned Windows build shows a SmartScreen warning, and an unsigned macOS build needs right-click Open. Notarising the Mac build needs an Apple Developer account (paid), and an iPad App Store or TestFlight build needs the same. Not needed while it is only for you.
6. **Files:** `.prism` documents open from either platform's file manager (section 1.13). Chrome on desktop saves in place through the File System Access API. On the iPad, where Chrome uses Apple's browser engine, Save downloads a copy and Open uses the Files picker. A copy is also kept in the browser's private storage as a safety net.
8. **Updates (owner request, 2026-11-09):** Help > Check for Updates downloads the newest release build from GitHub Releases (not the source, which would need the Rust toolchain), checks it against the checksum published with the release, and swaps it in on the next start, after asking. It is a normal HTTPS download, so firewalls treat it like a browser download, and it needs no admin rights when Prism lives in a folder the user owns. On Windows the running program cannot be overwritten, so the old file is renamed and the new one put in its place; on macOS the app bundle is replaced. A signature check joins the checksum once builds are signed (decision 5). The web build already updates on every push. Chunk: **Updates**.
7. **To verify early:** that the hosted web build runs on the iPad at all (WebGPU support on iOS is recent, so check the exact iPadOS version, and WebKit's memory limits for larger models), and the two kernels' WASM size. If it does not, the iPad waits for a signed native app, which needs a paid Apple Developer account.

Input targets: mouse and keyboard first, which is how SketchUp is meant to be used, including middle-mouse orbit and a three-button mouse. Trackpad gestures come second. Pen and touch are out of scope until a decision says otherwise.

---

## 4. Architecture

A Cargo workspace:

```
crates/
  prism-math     f64 vectors and transforms, exact predicates, tolerances, units
  prism-geom     curves (line, exact arc/circle, NURBS curve) and surfaces (plane,
                 analytic, NURBS); evaluation, projection, closest point, intersection, tessellation
  prism-topo     non-manifold B-rep: Vertex, Edge, Loop, Face, Shell; stable IDs;
                 sticky-geometry rules (split, merge, face detection)
  prism-ops      push/pull, offset, follow-me, intersect, planar booleans; Kernel
                 trait for NURBS solids (booleans, fillet, shell)
  prism-infer    the inference engine: a pure function, heavily tested (section 5)
  prism-doc      document, scene graph, groups and components, materials, layers/tags;
                 transactions with undo/redo; serialization (.prism file)
  prism-render   wgpu: shaded viewport, line renderer, glyphs and guides, picking
                 acceleration, later the path tracer and denoiser
  prism-tools    tool state machines (Select, Line, Arc, Push/Pull, Move...), each
                 a pure (state, input event, inference) -> (state, preview, commands)
  prism-app      winit and egui shell, input mapping, measurement box, web and native entry points
```

Rules that keep options open for the later tiers:
- **Every edit is a command in a transaction.** That one design gives undo/redo, autosave and crash recovery, recorded test sessions, and later the animation keyframing hooks.
- **Stable entity IDs and typed properties.** Any property (a transform, a material parameter, a light intensity) can later become an animation channel (tier 2) or a node input (tier 4).
- **Materials are node graphs from day one**, even while the UI only shows a simple PBR form (base colour, roughness, metallic, and so on, following OpenPBR / Principled BSDF). Tier 4 then extends the graph instead of replacing the material system. Graphs compile to WGSL for both the rasteriser and the path tracer.
- **A GPU compute framework in `prism-render`** (buffers, dispatch, readback), shared by the path tracer (tier 1), sims (tier 3) and texture baking (tier 4).
- **Cameras are first-class scene objects** with intrinsics, so projectors (tier 5) are just calibrated cameras with an output display.
- **Precision:** geometry in f64. Rendering is relative to the eye (f32 positions offset by the camera) so large or far-from-origin models do not jitter.
- **Depth precision (thin things at a distance).** SketchUp shows back detail through thin or distant objects in every version. There are two causes, and they need two fixes.
  1. **The depth buffer runs out of resolution.** The standard depth buffer spends most of its precision close to the camera, so two surfaces a short distance apart far away get the same depth value. Fixes: a reversed-Z floating-point depth buffer (precision is spread evenly by distance), and a near and far plane recomputed every frame from the visible bounding box (hidden objects excluded, section 1.14), so the range is never wider than it needs to be.
  2. **Lines are pushed forward to avoid z-fighting with faces.** The depth bias that keeps edges from flickering against their own faces also lets edges on the far side of a thin wall win the depth test. Fix: a small bias in view-space units that is limited to a fraction of the thinnest wall in view, or lines compared against the face depth in the shader with a tolerance that scales with distance, instead of a fixed offset.
  - **Duplicate coplanar faces** (section 1.9) are a third source, and are removed by the overlap rule.
  - **Test:** a screenshot test puts a 0.5 mm plate with detail behind it at 5 m, 100 m and 1 km, and fails if anything behind it shows through.
- **Determinism:** inference, tools and ops never read the clock or the GPU, so scenario tests can replay them exactly.

---

### 4.1 Capabilities and providers

Prism adapts to where it runs. At startup it detects the **capabilities** it has: operating system, GPU and compute support, WebGPU, threads, memory, and whether Blender is installed. Each heavy feature (rendering, simulation, exact booleans, video export) is built as a set of **providers**, and the app uses the best one available.
- **Rendering:** the built-in wgpu path tracer, Cycles or Blender as an external program, or a lower-quality preview fallback.
- **Simulation:** GPU compute, a slower CPU version, or an export to Blender with the result brought back as a cached animation.
- **Unavailable features are greyed out with a reason** ("needs GPU compute", "Blender not found"), using the same registry as every menu (section 1.6). A document made on a strong desktop opens on a weaker device with the heavy features greyed instead of broken.
- **The user can override** the choice of provider in Preferences.
- **Simulation libraries:** rigid bodies could use Rapier (a Rust physics library, Apache licensed, to be confirmed before relying on it). Cloth and water follow the published methods in the plan. Blender's own simulation code cannot be copied without making Prism GPL.

## 5. The inference engine (core design)

```
infer(scene_view, camera, cursor_px, anchors, locks, tool_context) -> Inference {
    point: DVec3,          // where the click lands
    kind: Endpoint | Midpoint | Center | Intersection | OnEdge | OnFace |
          OnAxis(axis, from) | Parallel(edge) | Perpendicular(edge|face) |
          Tangent(curve, at) | GuideCross(a, b) | Free(plane) | Typed(...),
    guides: Vec<GuideLine>,   // dotted lines to draw
    glyph, label,             // feedback
    hit: Option<EntityRef>,   // what is under the cursor
}
```

**Candidate generation, all in screen space:**
1. A spatial index (BVH) over projected entities returns everything within the pick aperture.
2. Point candidates: vertices, edge and arc midpoints, arc and circle centres and quadrants, screen-space intersections of nearby edges (checked in 3D, and treated as a 3D point only if the edges really meet; otherwise a "visual intersection" on the face plane), on-edge closest points, and on-face ray hits.
3. **Anchor memory:** a short ring buffer (about 5 items) of points, edges and curves the cursor dwelled on (around 200 ms) or that the tool used (the last click, the start point). A woken anchor shows a small marker. Anchors expire after time or distance, and are pinned while a tool step uses them.
4. **Guide generation from anchors:** axis lines (global axes, plus face-local axes when the tool context has a face), parallel and perpendicular lines to anchored edges, tangent lines from anchored curves at their endpoints and at the hovered parameter, through-point lines, and perpendicular from a point onto an anchored edge or face.
5. Guide-guide and guide-geometry intersections become strong point candidates ("on red from A" crossing "on green from B").

**Scoring:** `score = type_priority − screen_distance / aperture(type)`. Starting values, to be tuned:

| Type | Capture radius | Notes |
|---|---|---|
| Vertex | 14 px | Highest priority |
| Midpoint, centre | 12 px | |
| Intersection, guide cross | 12 px | |
| Edge | 9 px | |
| Axis or guide line | 7 px | |
| Face | Under the cursor | Fallback |

Radii are in logical pixels and scaled by DPI. Hysteresis: the current snap holds until the cursor is 1.6× its capture radius away, or a clearly better candidate (by a margin) appears.

**Locks** (Shift, arrows) turn the winner into a constraint. Later candidates are then projected onto the locked line or plane, so you can lock to red and then snap to a far endpoint's red projection.

**Feedback:**
- Glyph shapes as well as colours, so the system works for colour-blind users.
- Axis-coloured dashed guides.
- A short label near the cursor.
- A status-bar hint for the tool's next step.

**Tests:** unit tests per candidate type; golden scenario tests (scene, camera and cursor path giving the expected inference sequence); property tests for stability (tiny cursor jitter never changes the snap); and recorded real sessions replayed in CI.

---

### 5.1 Smart spacing guides (proposed, from the owner's Illustrator idea)

Illustrator's Smart Guides line an object up with the edges and centres of the others, and show when a gap equals a gap that already exists. Prism has most of the machinery already (section 5), so this extends the inference engine to **whole objects**: groups, components and bodies.

**What it adds** (while moving, copying or placing a group, component or body):
- **Alignment:** the moving object's bounding-box faces, edges and centre line up with those of nearby objects, per axis. A guide line shows what matched.
- **Equal spacing:** if A and B have a gap along an axis, and the moving object C would leave the same gap to B, C snaps there and both gaps are marked with arrows and a small measurement label (for example `d: 1.5 m`). Also works in a row: equal gaps along a line of objects.
- **Equal size** (optional later): a resize that matches the width, depth or height of a neighbour.
- It uses the ordinary snap pull (the capture radius in pixels, section 5) and the same locks. Typing a number still wins.

**Cost.** The cost rule is the one Illustrator uses, plus the model's own rules:
1. **Spatial index first.** Candidates come from the bounding-box index (section 4), only those within the snap radius of the moving object along an axis, never all objects against all objects.
2. **Hidden is free** (section 1.14): hidden objects are not in the index.
3. **Time budget, not an object count.** The owner suggested switching off when more than about 20 objects are in view. A fixed count is too blunt: 20 big nearby objects are cheap with an index, while a few thousand objects crowded into the cursor's reach are not. The engine instead gets a budget per frame (start with 2 ms, a setting) and degrades in steps: (a) stop looking for equal-spacing chains and keep plain alignment, (b) look only at objects the cursor is near, (c) turn Smart Guides off and show a short notice ("Smart Guides paused: too many nearby objects. Ctrl+U to turn on"). It comes back on by itself when the load drops, unless the user switched it off.
4. **A candidate cap** (default 200 near the cursor) also bounds the worst case.
5. **Gaps are computed on bounding boxes**, with exact geometry only when a snap is accepted.
6. **GPU:** not planned. The index query is small enough on the CPU. Revisit only if a measurement says otherwise (section 7, performance).

**Controls.** A View menu toggle "Smart Guides", **Ctrl/Cmd+U** (not used by SketchUp), and its settings in the Inspector and View Settings: which guide kinds are on, the time budget, the label units. It shows as greyed until built, like every other action.

**Colours.** Illustrator uses magenta. SketchUp already uses magenta for parallel and perpendicular guides, so spacing guides get their own colour and arrows to avoid confusion. The colour is a setting.

**Where it sits.** It is an extension of Inference & Drawing and of Groups & Components, because it needs objects. It should not start before Groups & Components exist. The feel-test prototype already tries it: with the Move tool, separate connected shapes count as objects, and Help > Smart Guides Demo Scene loads a row of boxes to drag one against. It has alignment (min, centre, max per axis, teal), equal spacing with arrows and a `d:` label (orange), an 8 px pull, a 2 ms budget that pauses spacing first, and Ctrl+U to switch it off. It does not yet have equal sizes or chains longer than two neighbours. A centre-alignment guide line only appears once the centres are actually lined up (within a quarter of the pull radius); until then only the moving selection's centre dot is shown. Guides only appear for axes the object is actually moving along (a flat shape sliding on the ground is always aligned in Z, so Z never shows), and a guide joins the two nearest points of the two bounding boxes at the aligned coordinate instead of running centre to centre.

## 6. Roadmap

Each milestone ends with something you can try in the web build.

### 6.0 Work chunks (as the scope stands today)

The milestones below are broken into chunks that can be built and tried one at a time. Sizes are relative: S is a few days, M about a week or two, L a month or so, XL several months and risky. The rows are in build order: each chunk comes after everything it depends on. The feel prototype already proves Inference & Drawing and Edit Tools in the browser, and its menus, dock and registry carry over into Actions & Keys.

| Chunk | What it contains | Milestone | Depends on | Size | Where we are | You can try it when |
|---|---|---|---|---|---|---|
| **Skeleton** | Cargo workspace, wgpu window native and web, CI to Pages (web) and Releases (Windows, macOS) | M0 | — | M | Built and merged. Runs on Windows and macOS (checked); the web page and the iPad still to check (see NOTES.md) | A blank viewport opens as a Windows app, a Mac app and a web page |
| **Soft Theme** | One theme layer in Rust (Motion Kit, dark Soft glass, `docs/UI-STYLE.md`), IBM Plex fonts bundled, the debug overlay restyled, a spring helper, reduce-motion setting | M0 | Skeleton | S | Built: `prism-ui` theme layer, Plex fonts, soft controls, redraw only on change. Waiting for a person to check it on Windows and macOS (see NOTES.md) | The debug overlay looks like the kit's soft.html (raised, pressed, still glass cutouts) |
| **Core Model** | Math, tolerances, B-rep with holes and stable IDs, sticky geometry, overlap split and dedupe (1.9), commands and undo. The edge and face types are defined for exact curves and surfaces from the start (section 1.1) even though only lines and planes are implemented at first, and each planar face stores its own plane, with its vertices placed on it | M1–M2 | — | L | Built: prism-math, -geom, -topo, -ops, -doc; all verified golden cases pass at three scales, property tests for the rules, exact undo (see NOTES.md) | Unit tests only: the prototype's cases pass in Rust |
| **Viewport** | Camera navigation (1.5a), line renderer, shaded faces, picking, prehighlight, selection display | M1 | Skeleton | L | Built: `prism-render` (camera, own triangulator, picking, reversed-Z wgpu renderer with no line bias), Select clicks, test scenes; pocket and thin-plate screenshot tests pass. Waiting for a person to check it on Windows and macOS (see NOTES.md) | Orbit, pan and zoom a model with correct highlighting |
| **Actions & Keys** | Action registry, menu bar, dock, context menu, command search, keymap engine, hotkeys window (1.12), Mac system menu bar, Planned Features list | M1–M2 | Skeleton | M | Built: registry from the prototype's tables, menus, dock, right-click menu, command search, keymap engine, shortcuts window, Planned Features. Waiting for a person to check it on Windows and macOS (see NOTES.md) | Every action is reachable from a menu and rebindable |
| **Inference & Drawing** | Inference engine, Line, Rectangle, Select, measurement box with units | M2 | Core Model, Viewport | L | Built: `prism-infer` (the engine), `prism-tools` (Line, Rectangle, Select with box select, units, a replayable session), the measurement box, glyphs and guides. Waiting for the owner to try the feel (see NOTES.md) | The prototype's feel, running natively |
| **Edit Tools** | Push/Pull with new loop, Move and copy, arrays, Rotate with protractor, Scale, Offset, Eraser, Tape Measure; part 3: Intersect Faces and snapping to where pieces cross (DECISIONS #29) | M3 | Inference & Drawing | L | All three parts built. Part 3: snapping to where faces cross, and Edit > Intersect Faces with the model or the selection (DECISIONS #29). Part 1 and 2: Push/Pull, Move and copy with arrays, Rotate with the full-circle protractor and plane lock, Scale, Eraser (part 1); Offset, Tape Measure, Protractor and guides in the model (part 2) | You can model a small building |
| **Groups & Components** | Edit in context, instances, tags, outliner, Entity Info, Explode with merge | M3 | Edit Tools | L | Built in 3 parts: groups (Make Group, select whole, move/rotate/scale/copy, open and close with the rest faded, Explode with the overlap rules, `.prism` schema 2), components (linked instances, Make Unique, Flip Along, the Components window), and hiding, tags, Outliner and Entity Info (hidden is free). Waiting for the owner to try it (see NOTES.md) | The icosahedron case in 1.9 works |
| **Files & Formats** | `.prism` save and load with versioning, atomic save, autosave and recovery, STL, OBJ, 3MF, PLY and glTF import and export, drag and drop, the units dialog | M2–M3 | Core Model | M | All three parts built: `prism-file` (the `.prism` format, and STL, OBJ, 3MF, GLB and PLY in and out), save and open natively and on the web, atomic save, autosave and recovery, recent files, the units dialog, Import and Export dialogs, drag and drop, document tabs, and double-click opens in the running Prism (DECISIONS #24, #27; the Windows installer waits for packaging) | A model survives a crash and a restart, and opens in another program |
| **Model Health** | The checks and Clean envelope (1.10), validate before export | — | Core Model, Groups & Components | M | Built in 2 parts: the checks, Window > Model Health (counts, select and zoom), Orient Faces, Reverse Faces, Clean Envelope, Remove Stray Edges; problems drawn in the model while the window is open, and a check on export. Waiting for the owner to try it | Reversed faces, holes and interior faces are listed and fixable |
| **View Settings** | Show faces, lines and points, line thickness and profile lines (1.14), styles, X-ray, hidden geometry, hide and unhide, default lighting and sun with shadows, the hierarchy tree (1.14) | M1–M3 | Viewport, Groups & Components | M | Built in 2 parts: view styles (Default, Hidden Line, X-ray, Technical, Wireframe), Show toggles, line and profile thickness (profiles on), colour by axis, hidden lines, the View Settings window, saved with the file; the sun from date, time and place, shadows on faces and the ground, Show Hidden ghosts. Waiting for the owner to try it | The model can be shown as hidden line, X-ray or with a sun, and the tree matches the viewport |
| **Toolbars** | Toolbars docked on the left or top, floating in the app, rearranged, closed and reset; edit mode; the Toolbars window with drag-out; X-ray and Hidden buttons (owner request, brief 19) | M1–M3 | Actions & Keys | M | Built (2 parts): layout, docks, floating, edit mode, View toolbar; the Toolbars window with drag-out, and reshaping. Owner request, built before Updates | Toolbars can be arranged like SketchUp's |
| **Updates** | Help > Check for Updates: download the newest release build, check its checksum, swap it in on the next start (3.1, decision 8) | — | Files & Formats | S | Built (brief 20): `update.json` in each release, Help > Check for Updates, checked download and swap. Not yet tried on a real release | Prism updates itself from a menu |
| **MCP Server** | A local Model Context Protocol server: read the model, selection and health, run actions and commands with undo, screenshots, replay sessions (section 8, decision 6) | — | Actions & Keys, Model Health | M | Built (2 parts, brief 21): the local server (both protocol eras); reading the model, selection, health and geometry; running actions; using tools at model points; screenshots; replaying sessions. Not yet tried from Claude Code | Claude can drive Prism on the owner's machine |
| **Exact Curves** | Arc tools, circle, polygon, tangent inference, Follow Me, fillet and chamfer on planar edges, soften and smooth | M4 | Edit Tools | XL | Part 1 of 5 built (loops list their edges, file layout 2). Parts 1 and 2 built (part 2, brief 23: arcs and circles in the core, exact crossings, picking, snapping, Model Health, and the Circle and Polygon tools, greyed but reachable over MCP). Part 3 is built too (brief 24: the Arc tools, tangents, Offset with arcs, corner fillets). Part 4 is built too (brief 25: cylinder surfaces in the core; Push/Pull makes cylinders, shaded and picked; soften and smooth; a new radius by Push/Pull). Part 6 is built (brief 27): 6a (the segment fallback) and 6b (exact crossings with curved faces, cutting them in (θ, s), Intersect Faces, sticky geometry on curved faces, faceting cut faces) and 6c (Push/Pull booleans with curved faces; a pull and a new radius stop at curved faces; Follow Me refuses a sweep that overlaps itself) and 6d (new sizes for cones, spheres and tori; outline lines along curved faces) and 6e (the curve tools active, a release) are built: **Exact Curves is done**. Part 5 is built (brief 26): 5a (fillet and chamfer of edges), 5b (cones, spheres and tori in the core), 5c (Follow Me along lines) and 5d (Follow Me along arcs, lathes, Alt) are built. 6 parts (5 by the owner, 2026-11-09, `docs/research/exact-curves.md`; part 6 added 2026-11-11, DECISIONS #33). Its tools are active since 6e (2026-11-11), now that curved faces work with the standard tools | Round shapes stay exact, and the standard tools work on them |
| **Basic Materials** | Paint Bucket, colours and textures, UV projection, material library, the Shaded-with-textures style | M3 | Viewport | M | Part 1 (colours) built and released: 1a the core (materials in the document, paint kept through edits, schema 8), 1b the Paint Bucket and colours in the viewport, 1c the Materials window, Entity Info, Select by material, Model Health and MCP. The Fillet window is built too (brief 29, owner request: the Fillet / Chamfer tool's kind and size in the right-hand column, with a live preview). Part 2 (textures) step 2a is built: images in the document and the file (schema 9), a material's texture and size, placement on flat faces carried by Move, Rotate and Scale, exact coordinates on curved faces. Step 2b is built: textures in the viewport (texture coordinates, batches by texture, View > Faces > Textures with average colours when off, see-through faces sorted back to front that hide nothing from picking). Step 2c is built and released: the Materials window loads images (or drop one on the window), sets a texture's size, replaces or removes it, and sets opacity; Model Health counts unused images; MCP lists textures. Part 3 is built and released: Position Texture (pins) and Reset, materials in glTF out and in, OBJ's .mtl and images, 3MF colours. **Basic Materials is done.** No bundled textures (DECISIONS #34, owner) | A face can be painted and shows its texture in the viewport |
| **NURBS Spike** | Kernel choice (monstertruck or arris vs OCCT or SMLib) decided by robustness tests, including the surface-surface intersection that curved overlap splits need | M5 | Core Model | S | Done (brief 30): **arris**, by its test results (12 of 12 cases against monstertruck's 4, NURBS booleans consistent, builds for the web without threads). `docs/research/nurbs-spike.md`, DECISIONS #9 | A written decision, with test results |
| **Freeform NURBS** | Control-point curves, cage editing with the normal tools, loft, sweep, revolve, patch, adaptive tessellation, curvature display | M5 | Exact Curves, NURBS Spike | XL | Done (brief 31, six parts): NURBS curves and the Curve tool, freeform faces with adaptive tessellation, cage editing with Select, Move, Rotate and Scale, Loft, Revolve, Sweep and Patch (arris behind `prism-kernel`), Zebra Stripes and Curvature Combs. What is left is in NOTES, "Freeform NURBS: what the whole chunk left for later" | A loft you can pull into shape |
| **Fillet Joins** | Fillet / Chamfer on many edges at once: double-click a face to round all its edges (the face itself is left alone); edges that meet at angles join cleanly (mitres, ball corners, blends where faces meet at uneven angles); a line made of segments on one axis is rounded as a whole unless segments were picked by hand first | M5 | Freeform NURBS (the kernel's blends) | M | Not started (owner request 2026-11-12; moved after NURBS by the owner) | A box's top face double-clicked and filleted, with clean corners |
| **Annotation & Sections** | Dimensions, Text, the Axes tool, Section Planes, Construction Planes, guides, optional snap to grid | M3 | Edit Tools | M | Not started (guide lines and points came with Edit Tools) | A section cuts the model and a dimension updates with it |
| **Inspector & Features** | The schema-driven panel, editable booleans, chamfer and fillet, shell, hole, array and mirror, pick-in-viewport references, named parameters later (1.17) | M3–M6 | Edit Tools, Groups & Components, Exact Curves, Annotation & Sections | L | Not started | A chamfer's distance changes after the fact and the model updates |
| **Shapes & Placement** | Primitives, Mirror and live symmetry, Align and distribute, Drop to Face, Hole, Draft Angle, Bend/Twist/Taper, Unfold and Lay Flat | M3–M6 | Edit Tools, Exact Curves, Inspector & Features | L | Not started | A box, a mirrored part and a flattened sheet can be made with one tool each |
| **Fit & Mechanisms** | Tolerance modifier and profiles, Fit Helper, hinge, slide and fixed joints, motion sweep (1.15) | M3–M6 | Groups & Components, Inspector & Features | L | Not started | A hinged lid is checked through its range and a tab fits its slot |
| **Solids & CAD** | Booleans, shell, NURBS fillets, validation and healing, STEP, 3DM, 3MF, DXF/SVG | M6 | Freeform NURBS | XL | Not started (7 parts, PLAN 6.0a) | A part exports to CAM |
| **Rendering** | Materials, lights, HDRI, path tracer, denoiser, render view, the full settings in 1.16, and the Blender bridge | M7 | Viewport, Basic Materials | XL | Not started (6 parts, PLAN 6.0a) | A model renders with real materials |
| **Modes & Scenes** | The mode switcher (1.11), named cameras, saved scenes (tentative) | — | Actions & Keys | M | Not started | You can switch modes and jump between saved views |
| **Animation** | Keyframes and timeline, graph editor, camera paths and cuts, constraints, video export | Tier 2 | Rendering, Modes & Scenes | XL | Not started (5 parts, PLAN 6.0a) | A camera flies through a model |
| **Simulation** | Particles, cloth, water, baking | Tier 3 | Rendering, Animation | XL | Not started (4 parts, PLAN 6.0a) | Cloth falls on a modelled table |
| **Texture Nodes** | Node editor, procedural nodes, live shader compile, baking | Tier 4 | Rendering | L | Not started | A procedural material appears in both viewport and render |
| **Projection Mapping** | Projectors and calibration, mapping, edge blending, second-display output, compositing graph | Tier 5 | Modes & Scenes, Rendering | XL | Not started | A calibrated projector lights a modelled surface |
| **iPad App** | Native iPadOS build, signing and TestFlight | — | Skeleton | M | Not started | The app installs on the iPad |

**Where we are:** the browser prototype covers the feel of Inference & Drawing and most of Edit Tools, plus the design of the menus, dock and context menu in Actions & Keys, and it tests the rules for Core Model and Viewport. Skeleton is built and merged: a Cargo workspace with `prism-app`, a window natively and on the web, and CI for checks, Pages and Releases. It runs on the owner's Windows machine. Soft Theme is built (the `prism-ui` crate), and so are Actions & Keys (menus, dock, search and shortcuts from one registry), Core Model (the geometry core, tested against the prototype's golden cases) and Viewport (the model drawn and navigated, with hover and selection), Inference & Drawing (snapping, Line, Rectangle, the measurement box, box select), and Edit Tools (Push/Pull, Move, Rotate, Scale, Eraser, Offset, Tape Measure, Protractor and guides). Files & Formats parts 1 and 2 (`.prism` files, autosave, units, mesh import and export, drag and drop) are built, and so is Edit Tools part 3 (Intersect Faces and snapping to crossings, DECISIONS #29). Files & Formats part 3 (tabs, one running Prism) is built too. Groups & Components and Model Health are built too, and View Settings, Toolbars, Updates, and MCP Server. Exact Curves part 1 is built (faces name their edges; no visible change), and part 2 (exact arcs and circles in the core, their crossings, picking, snapping and Model Health for them, and the Circle and Polygon tools, greyed until all five parts are done). Exact Curves part 3 is built too (brief 24: the Arc tools, tangent inference, Offset with arcs, corner fillets; greyed like the Circle). Exact Curves part 4 is built too (brief 25: cylinder surfaces in the core, cylinders from Push/Pull, shaded and picked, soften and smooth, and Push/Pull on a cylinder for a new radius). Exact Curves part 5 is built too (brief 26): step 5a (fillet and chamfer of a solid's edge), step 5b (cones, spheres and tori in the core: checked, shaded, picked, saved, listed over MCP), step 5c (Follow Me along lines, greyed) and step 5d (Follow Me along arcs: lathes making cones, spheres and tori, and Alt for a face's edge). Next: part 6 (brief 27: curved faces in the standard tools; the curve tools become active at its end), step 6a (the segment fallback: approximate curves and facets for uneven Scale, Follow Me and slanted fillets) and step 6b (planes and curved faces crossing exactly, curved faces cut in (θ, s), Intersect Faces with curved faces, drawing on curved faces, Follow Me's result welded, faceting cut faces) and step 6c (Push/Pull booleans with curved faces: holes, slots and notches with exact volumes; a pull and a new radius stop at curved faces; Follow Me refuses a sweep that overlaps itself) and step 6d (Push/Pull gives cones, spheres and tori a new size; outline lines along curved faces) and step 6e (the curve tools are active: Circle, Polygon, the Arcs, Fillet / Chamfer and Follow Me) are built. **Exact Curves is done**; next is Basic Materials (the next row in the build order).

**Added after review:** Basic Materials, Annotation & Sections and Shapes & Placement were missing from the first list. Tools in section 1.4 such as Paint Bucket, Dimensions, Text, Section Plane, Primitives, Mirror, Hole and Unfold had no chunk.

**Added since:** View Settings, Inspector & Features and Fit & Mechanisms come from sections 1.14, 1.17 and 1.15. Fit & Mechanisms works first on planar and mesh parts, and curved NURBS parts wait for Solids & CAD.

**Order:** the table is the build order. It was reordered on 2026-11-08 so that every chunk comes after what it needs: Exact Curves before Inspector & Features and Shapes & Placement (fillets, holes and primitives need arcs and circles), Annotation & Sections before Inspector & Features (it edits Text and Section Planes), and Basic Materials before Rendering. Updates and MCP Server come right after View Settings (owner, 2026-11-09). NURBS Spike and Freeform NURBS come right after Basic Materials (owner, 2026-11-11): their prerequisites (Core Model, Exact Curves) are built, so nothing else moves up with them. Fillet Joins comes right after Freeform NURBS (owner, 2026-11-12), since clean corners between rounded edges need the kernel's blend surfaces. **Standing rule (owner, 2026-11-08):** chunks are built in this order, top to bottom; the next chunk is the first row not yet started. Only the owner changes the order.

### 6.0a Building chunks in parts

Every chunk is built in **parts**. A part is one brief, sized so that one session can build it, test it, push it and see CI green before the chat's context is compacted.
- Each part ends with something the owner can try. NOTES says what was built, and `docs/HANDOFF.md` is updated if the session ends there.
- **Sizes in the table above:** S and M chunks take one or two parts, L takes two or three, and XL takes four to seven.
- **Writing the briefs:** a part's brief is written just before it is built, not all at once. The split below is a starting point, and each brief may move work between parts.
- **The MCP line (owner, 2026-11-09):** every brief from Exact Curves on has an "MCP" line. Actions and tools reach the MCP server by themselves (`list_actions`, `run_action`, `use_tool`). When a chunk adds something an assistant should read or set that those cannot reach (a new kind of geometry, materials, dimensions, panel settings), the same chunk extends the MCP tools, with a test.
- **In a session:** commit and push after each step of a part, so a compaction or a restarted worker loses nothing. Untested app code goes on the session branch, not `main`, because every push to `main` makes a release.

**Done so far:**
- Model Health: 2 parts. 1. Checks, panel and fixes. 2. Problems drawn in the model, and a check on export.
- Groups & Components: 3 parts. 1. Groups. 2. Components. 3. Hiding, tags, Outliner and Entity Info.
- Edit Tools: 2 parts.
- Files & Formats: 3 parts.
  1. `.prism`, autosave, units (built).
  2. Mesh import and export, drag and drop.
  3. Tabs and double-click to open in the running Prism.

**Proposed splits for the XL chunks:**

| Chunk | Parts |
|---|---|
| Exact Curves | 1. Loops list their edges (topology and file layout 2, no visible change). 2. Arcs and circles in the core, with the Circle and Polygon tools. 3. The Arc tools, tangent inference, Offset with arcs, corner fillets. 4. Curved faces: cylinders from Push/Pull, shading and picking, soften and smooth. 5. Fillet and chamfer on planar edges; cones, spheres and tori; Follow Me along lines and arcs. 6. Curved faces in the standard tools (owner, 2026-11-11, DECISIONS #33): 6a the segment fallback (#32); 6b exact crossings of planes with cylinders, cones, spheres and tori, used by Intersect Faces, sticky crossings and snapping (Follow Me's result becomes sticky too); 6c Push/Pull booleans and stops with curved faces (a cylinder pulled into a box cuts it; a pull or a new radius stops at a curved face), and the solid overlap test, also used to refuse a Follow Me sweep that overlaps itself (further along a path, or a sphere profile of more than half a circle); 6d a new size for cones, spheres and tori by Push/Pull, and outline lines along curved faces; 6e the curve tools become active. |
| Freeform NURBS | 1. NURBS curves: the control-point curve tool, display and inference on curves. 2. Surfaces in the model, adaptive tessellation and display. 3. Cage editing with Move, Rotate and Scale. 4. Loft and revolve. 5. Sweep and patch. 6. Curvature display (zebra and curvature combs). |
| Solids & CAD | 1. Solids: validation and healing. 2. Exact planar booleans and their UI. 3. Booleans on curved faces through the kernel picked by the NURBS Spike. 4. Shell. 5. NURBS fillets. 6. STEP import and export. 7. 3DM, DXF and SVG. |
| Rendering | 1. Physically based materials and the render view. 2. The path tracer core (WebGPU compute) on meshes. 3. Lights and HDRI. 4. The denoiser. 5. The rest of the settings in 1.16. 6. The Blender bridge. |
| Animation | 1. Keyframes and the timeline. 2. The graph editor. 3. Camera paths and cuts. 4. Constraints. 5. Video export. |
| Simulation | 1. Particles. 2. Cloth. 3. Water. 4. Baking. |

**L chunks:** split when their first brief is written. Usually:
- the data and core first;
- then the tools and UI;
- then the remaining tools.



### Tier 1: modeling and rendering

**M0. Ground clearing and skeleton.**
- Remove the Android video player (the user confirmed no backup is needed).
- Create the Cargo workspace.
- Get a wgpu window rendering natively and on the web.
- CI: tests, a web build to GitHub Pages, native binaries to Releases.

**M1. Viewport and lines.**
- Camera: orbit with middle mouse, pan with Shift + middle, zoom to the cursor, zoom extents.
- Axes and ground.
- The line renderer: instanced screen-space quads, SDF anti-aliasing, DPI-aware, depth-biased, hidden and dashed modes, profile pass.
- Shaded faces with a SketchUp-like default style.
- Screen-space picking of points, edges and faces with prehighlight.
- SketchUp-style selection highlighting (section 1.2).

**M2. Inference and first tools.**
- The `prism-infer` engine as specified in section 5.
- Line tool, rectangle tool, measurement box with units.
- Sticky geometry: edge splitting and automatic face creation.
- Select tool: click, Shift/Ctrl add and remove, left-to-right window select, right-to-left crossing select, double click for the face plus its edges, triple click for everything connected.
- Undo/redo, then save and load.

**M3. The core loop.**
- Push/Pull.
- Move and copy with arrays.
- Rotate with a protractor.
- Scale, Offset, Eraser.
- Tape measure and protractor guides.
- Groups and components (edit in context, other instances fade).
- Tags/layers and an outliner.

**M4. Exact curves.**
- Arc tools (2-point, 3-point, tangent, pie), circle, polygon.
- Real tangent inference.
- Follow Me along exact paths.
- Fillet and chamfer on planar edges.
- Soft and smooth edges for a curved look.

**M5. Freeform NURBS (cage editing).**
- Kernel spike (monstertruck or arris vs OCCT or SMLib), decided by robustness tests.
- Control-point and interpolated curves, with control-point editing that uses inference.
- Mesh-like control cages on freeform faces, edited with the normal Move, Rotate and Scale tools; "Smooth" a planar region into a G1/G2 patch; insert edge loop (knot insertion).
- Extrude, revolve, loft, sweep, network/patch surfaces.
- Adaptive tessellation.
- Curvature and zebra display.

**M6. Solids.**
- Booleans (union, subtract, intersect).
- Shell, fillet and chamfer on NURBS edges.
- Solid validation and healing.
- Import/export per sections 1.3 and 1.13: STEP, IGES, 3DM, DXF/SVG. Basic mesh formats (STL, OBJ, 3MF, glTF) and the `.prism` format arrive earlier, in Files & Formats.

**M7. Rendering.**
- Materials (OpenPBR-style), textures with UV projection tools, lights, an HDRI environment.
- A progressive path tracer in wgpu compute: BVH, MIS, depth of field.
- The OIDN denoiser.
- Render view and export.

### Tier 2: animation
- Keyframes on any property.
- A timeline and a graph editor (Bézier keys).
- Camera paths and cuts.
- Constraints and parenting.
- Later: armatures and skinning for organic meshes.
- Rendered frame sequences and video export.

### Tier 3: simulation
- A particle system on GPU compute (emitters, forces, collisions against the B-rep's tessellation).
- XPBD cloth (stretch, bend, collisions, pinning, wind).
- MLS-MPM water with surface reconstruction for rendering.
- Baking to the timeline.

### Tier 4: texture nodes
- A node editor UI on the existing material graphs.
- Procedural nodes (noise, Voronoi, gradients, maths, colour ramps).
- A live WGSL compile shared by viewport and path tracer.
- Baking to image textures.

### Tier 5: projection mapping and compositing
- Projectors as calibrated cameras: point correspondences and solving intrinsics and pose.
- Mapping content onto modeled surfaces.
- Multi-projector edge blending.
- Fullscreen output to second displays (native build).
- A compositing node graph over render passes and video inputs.

---

## 7. Risks

**Checked 2026-11-06:** `docs/research/risks.md` lists the hiccups found ahead, with what to do for each. The main ones:
- the path tracer needs WebGPU, because WebGL2 has no compute shaders;
- threads on GitHub Pages need a service worker;
- the Blender bridge must stay out of process, because Blender is GPL;
- the pure-Rust NURBS kernels have few users;
- code signing costs money (DECISIONS #28).

- **NURBS robustness** is the biggest technical risk. Mitigations: the kernel is behind a trait, the M5 spike comes before any commitment, and exact predicates handle the planar cases.
- **Feel cannot be specified, only tuned.** Mitigations: every aperture, dwell time and hysteresis value is a setting from day one, there is a debug overlay that shows candidates and scores, and you test the web build often.
- **Testing GPU code headlessly.** Logic stays pure and is unit-tested. Rendering is checked with wgpu's software adapter (lavapipe or SwiftShader) for screenshot tests. WebGPU in headless Chromium needs flags and may be limited.
- **Scope.** The tiers are strictly sequential. Tier 1's feel is the product, and nothing later starts until M3 feels right. Since then the plan has grown (view settings, Inspector, fit and mechanisms, model health, scenes), so a **first usable build** needs a fixed cut: Skeleton, Core Model, Viewport, Actions & Keys, Inference & Drawing, Edit Tools and Files & Formats. Everything else waits until that build feels right.
- **The web build's limits.** Multi-threaded WASM needs cross-origin isolation headers, which GitHub Pages cannot set, so the web build needs a service-worker workaround or another host. A 32-bit WASM build is also limited to a few GB of memory, which caps model size in the browser.
- **Precision at mm to km scales, and thin things seen from far away.** Rendering relative to the eye fixes jitter in the *positions* of vertices, but it does not fix the cause of back faces and edges showing through thin walls at a distance, which is the depth buffer running out of resolution (see section 4, depth precision). It needs its own plan, and a screenshot test.
- **Performance budget: set, to be measured.** See section 8, decision 8. Picking, occlusion and hidden-line drawing need a target size (for example, how many edges and triangles stay interactive on an integrated GPU) and a design for it (a spatial index, GPU picking by ID buffer, level of detail). Without a number, Model Health, the Fit Helper and the Inspector previews have nothing to be measured against.
- **Planar faces under transforms.** After a rotation, a face with more than three vertices is no longer exactly flat. Each planar face must own its plane and its vertices must be placed on it, or faces break into triangles (the prototype's workaround).
- **Dependencies and licences are not audited.** A table of every library, its licence and whether it works in WASM is needed before the licence decision (section 8). Candidates still to check: a 2D polygon offset library for kerf and corner relief (Clipper2 is a likely fit), and a distance and contact query library for clearance and motion checks (Parry, from the Rapier family).
- **Scripting is stubbed.** There is a greyed-out Scripting menu and a clean action registry and command log, but no API yet (section 8, decision 6). SketchUp's strength is its extension ecosystem. The plan has no scripting API, macros or batch mode, which also limits testing and the Blender bridge.
- **Input conflicts.** The measurement-box separator and trackpad defaults are decided (section 8, decisions 4 and 5). Still to settle: Shift+letter hotkeys (Shift+Z, Shift+S) collide with Shift being held as a lock while a tool is active.

## 8. Decisions

1. **Platform:** decided. Rust/wgpu, with native Windows and macOS builds and a web build (section 3.1).
2. **Name:** decided. Keep "Prism".
3. **Licence:** decided. Open source, MIT or Apache-2.0 (the exact pair is picked when the first release is cut). This allows Cycles (Apache) to be built in, and Blender to be run as a separate program. Blender's own GPL code is still never copied in. OCCT (LGPL) stays a fallback only if its terms are acceptable for an open-source project.
4. **Input devices:** decided. Mouse and keyboard only for now, and pen and tablet do not shape the early design. Help menu has a **Pen and tablet input** toggle, greyed out until support exists. Trackpad defaults: two-finger scroll pans, pinch zooms at the cursor, and orbit only through the Orbit tool or middle-drag on a laptop. The wheel on a mouse only zooms.
5. **Measurement box separators:** decided. The separator follows the user's locale (a comma between values where the decimal mark is `.`, a semicolon where the decimal mark is `,`), and `;` is always accepted. Units and the separator can be changed in File > Document Settings.
6. **Scripting and automation:** decided for now. A first automation surface is planned as the **MCP Server** chunk (owner request, 2026-11-09): Prism runs a Model Context Protocol server so an AI assistant such as Claude can read the model, the selection and Model Health, run any registered action or command (each one undoable, through the same command log), take viewport screenshots and replay recorded sessions. It is off by default, listens only on the local machine with a token, and is native only (the web build has no sockets). A greyed-out top-level **Scripting** menu is a stub. The language and API are not chosen. Keep the action registry and command log clean enough to add one later.
7. **First usable build:** decided. Skeleton, Core Model, Viewport, Actions & Keys, Inference & Drawing, Edit Tools and Files & Formats. Everything else waits until that build feels right.
8. **Performance targets:** decided as a provisional target, to be measured. The owner's machine is the reference: Intel Core i7-5820K at 3.3 GHz (6 cores, 2014), 32 GB RAM, a GPU with 8 GB of video memory. SketchUp struggles there with files over about 50 MB. Prism's goal is to stay interactive (orbit at 30 fps or better, picking and inference under 16 ms) on a model that is **about twice the size of the file SketchUp struggles with**, with hidden objects costing nothing (section 1.14). The first chunk that renders real geometry measures how many triangles and edges that is, records the numbers in `NOTES.md`, and replaces this sentence with them.

---

## Appendix A: what makes SketchUp feel intuitive

No one maintains a canonical list. This one is compiled from SketchUp's documentation and quick-reference card, the official forum's "Tips, Hints and Guidelines" thread, the SketchUcation forum, and reviews. Each item is a candidate requirement for Prism. ★ marks the ones I think matter most.

**Measurement box (typed input)**
- ★ You type without clicking into a field: values go to the measurement box, and Enter applies them.
- ★ You can retype after finishing an operation to correct it. Push/Pull 1 m, then type 1.2 m and Enter, and the last operation changes.
- Typed units override the model's units (`2'6"`, `750mm`). `300,200` gives rectangle dimensions; `[x,y,z]` gives absolute coordinates and `<dx,dy,dz>` relative ones.
- After a copy, `x5` makes an array of 5 and `/5` divides the distance into 5.

**Inference and locks**
- ★ Hover a point to "wake" it, then draw aligned to it from elsewhere.
- ★ Shift locks the current inference; the arrow keys lock axes or parallel.
- Drawing on a face aligns to that face's plane, and holding Shift keeps that plane.
- The rectangle tool shows a diagonal guide for square and golden-section proportions.
- The arc tool turns cyan when the arc is tangent and magenta for a half-circle. Double-clicking a corner with the arc tool fillets it.

**Tools that do the obvious thing**
- Line tool:
  - Lines chain from the last endpoint.
  - Closing a loop makes a face.
  - A line across a face splits it.
- Push/Pull:
  - ★ Double-click repeats the last distance on another face.
  - ★ Tap Alt to toggle "new loop" mode (validated in the feel-test prototype). The face's current boundary is kept as a face and a new copy of that loop is extruded from it, with side walls up to the new cap. Without Alt, pulling a face of a closed prism stretches it, and pulling a free face makes a box. The cursor label and status hint show "New loop" while on. Typed distances and re-editing the last pull keep the mode.
  - Pushing a face flush with the back face cuts a hole through.
  - Hovering a point elsewhere matches its height.
- Move:
  - Ctrl copies.
  - ★ Moving an edge or vertex reshapes the connected faces (Alt allows auto-fold).
  - Hovering a group shows red rotate handles on its box faces, so you can rotate with the Move tool.
- Rotate:
  - The protractor auto-aligns to the face under the cursor, or you drag to define the plane.
  - Ctrl makes rotated copies or radial arrays.
- Offset: double-click repeats.
- Follow Me: select a path, then click a profile. Alt follows a face's perimeter.
- Tape Measure:
  - Makes guide lines and guide points.
  - Measuring an edge and typing a new length rescales the whole model.
- Eraser: drag across edges to erase them. Shift hides, Ctrl softens or smooths.
- Paint Bucket: Alt samples a material, Ctrl fills connected faces, Shift replaces everywhere.

**Selection**
- Double-click a face to select the face and its edges; triple-click selects everything connected.
- Dragging left to right selects what's fully inside the box; dragging right to left also selects what the box crosses.
- Ctrl adds, Shift toggles, Ctrl+Shift removes.
- Right-click to select connected faces, everything on the same tag, or everything with the same material.

**Camera**
- Middle-drag orbits, Shift + middle-drag pans, and the scroll wheel zooms to the cursor. Double middle-click re-centres.
- Zoom extents, standard views, and a perspective/parallel toggle. Typing a field of view.
- Scenes with smooth animated transitions.

**Feedback and learnability**
- A small starter toolbar, with one-letter shortcuts for the core tools: Space select, L line, R rectangle, C circle, A arc, P push/pull, M move, Q rotate, S scale, F offset, T tape measure, E eraser, B paint bucket, O orbit, H pan, Z zoom.
- A distinct cursor icon for each tool, a status-bar hint for the next step, and screen tips on inferences.
- Faces have a front colour (white) and back colour (blue-grey), so orientation is always visible.
- Esc cancels an operation midway, closes windows, and exits an opened group or component. Undo goes back as far as you need.

**Model organisation**
- Groups, components with live instancing, glue-to and cut-opening.
- The outliner, tags, hide and isolate (Shift+Space hides the rest of the model), X-ray, live section planes.
- Sticky geometry teaches you to group.

**Common complaints Prism should fix**
- Sticky geometry surprises: geometry merges when you didn't expect it. Prism should warn when an edit would merge geometry and offer to group it.
- Reversed faces cause trouble on export. Prism should orient faces automatically on closed solids.
- Inference gets lost in dense models. Prism should rank candidates better and offer the debug overlay.
- Circles are segmented, so tangents are approximate. Prism solves this with exact geometry.
- Tiny faces (under about 1 mm) fail to form, forcing the "scale the model ×1000" workaround. Prism should use exact predicates and tolerances relative to model size.
- Layers and tags are clumsy. Prism should use an outliner-first organisation.
- Rendering needs plugins. Prism has a built-in path tracer (M7).
- CAD imports don't make faces. Prism should rebuild faces on import.

---

## Sources
- SketchUp drawing basics and inferences: https://help.sketchup.com/en/sketchup/introducing-drawing-basics-and-concepts
- SketchUp quick reference (keys and locks): https://cdn.shopify.com/s/files/1/0605/0358/9072/files/sketchup_for_desktop_quick_reference_guide_2023.1_windows.pdf
- Parallel and perpendicular locks (forum): https://forums.sketchup.com/t/how-can-i-move-an-object-parallel-or-perpendicular-the-face-edge-axis/254815
- Magenta tangent means segment-aligned: https://forums.sketchup.com/t/arc-tool-preview-magenta-inference-from-a-node-point/136580
- SketchUp edge styles: https://help.sketchup.com/en/sketchup/edge-styles
- Edge and AA complaints: https://forums.sketchup.com/t/antialising-in-su-2017/51395, https://forums.sketchup.com/t/thick-outline-edges-suddenly-gone-now-only-1-pixel-wide/285184
- Sticky geometry and face-split issues: https://community.sketchucation.com/post/1316372, https://forums.sketchup.com/t/erratic-behaviour-of-edge-intersect-in-simple-geometry/7456?page=2
- MoI 3D docs (straight snap): https://moi3d.com/3.0/docs/moi_introduction.htm
- Plasticity: https://digitalproduction.com/2023/10/04/plasticity-3d-cad-for-artists/, https://www.plasticity.xyz
- Fusion T-spline conversion: https://forums.autodesk.com/t5/fusion-design-validate-document/convert-body-into-a-t-spline/m-p/10031118
- NVIDIA usd-brep / SMLib: https://develop3d.com/cad/nvidia-launches-usd-brep-geometry-kernel-for-solid-modelling/
- truck: https://github.com/ricosjp/truck; monstertruck: https://docs.rs/crate/monstertruck/latest/source/README.md; arris: https://docs.rs/crate/arris/0.9.0
- OpenCascade.js: https://ocjs.org
- Manifold: https://github.com/elalish/manifold
- Interactive and Robust Mesh Booleans (Cherchi et al.): https://arxiv.org/pdf/2205.14151.pdf
- Screen-space line rendering: https://doc.magnum.graphics/magnum/classMagnum_1_1Shaders_1_1LineGL.html, https://docs.rs/facett-core/0.1.8/src/facett_core/render/prim.rs.html
- GPU trimmed NURBS: https://www.uni-weimar.de/de/gpucast/direct-trimming-of-nurbs-surfaces-on-the-gpu, https://dcgi.fel.cvut.cz/~havran/ARTICLES/supplementaryPG21SloupHavran/supplementaryPG21SloupHavran.pdf
- WebGPU + OpenPBR path tracer: https://web3.arxiv.org/abs/2407.19977
- WebGPU denoising: https://github.com/pissang/oidn-webgpu/wiki
- XPBD cloth on WebGPU: https://github.com/ccincotti3/webgpu_cloth_simulator; mass-spring at 640K nodes: https://arxiv.org/html/2507.11794v1
- MLS-MPM on WebGPU: https://tympanus.net/codrops/2025/02/26/webgpu-fluid-simulations-high-performance-real-time-rendering/
- SketchUp tips thread: https://forums.sketchup.com/t/tips-hints-and-guidelines-for-working-with-sketchup/15067/1.md
- SketchUp Push/Pull tips: https://blog.sketchup.com/training/top-5-push-pull-tips
- SketchUp reviews (pros and cons): https://www.trustradius.com/products/sketchup/reviews/pros-and-cons, https://www.capterra.com/p/211798/SketchUp/reviews/?page=2
- "Why SketchUp?" discussion: https://community.sketchucation.com/post/1264180
- Fusion loft reference (tangency weight, rails): https://help.autodesk.com/cloudhelp/ENU/Fusion-Patch/files/SFC-REF-LOFT-DLG.htm, https://forums.autodesk.com/t5/fusion-360-ideastation-archived/tangency-control-of-rails-in-loft-operation/idi-p/5968077
- Rhino loft styles: https://docs.mcneel.com/rhino/8/help/en-us/commands/loft.htm, https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.lofttype/
- egui-wgpu: https://docs.rs/crate/egui-wgpu/latest/source/src/lib.rs
