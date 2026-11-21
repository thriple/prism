# Feature map: every planned feature and the chunk that builds it

Written 2026-11-16 (owner: "make sure every planned feature is assigned to the plan ... so nothing gets left behind"). Every chunk named here is a row in `PLAN.md` section 6.0. A test (`every_unbuilt_feature_has_a_chunk` in `prism-app`) fails if a feature in the Planned Features window is not built and has no row in table A, or if a row names a chunk that is not in the plan. When a chunk is built, its rows can stay: the window shows what is built.

## A. The Planned Features window (actions and behaviours not built yet)

| Id | Feature | Chunk |
|---|---|---|
| `exp.step` | STEP (machining, CAM) | Solids & CAD |
| `exp.3dm` | 3DM (Rhino) | Solids & CAD |
| `exp.dxf` | DXF / SVG (routing, laser, CNC) | Solids & CAD |
| `edit.weld` | Weld | Solids & CAD |
| `conv.curve` | Curve Chain ↔ NURBS Curve | Solids & CAD |
| `conv.patch` | Faces ↔ Smooth Patch | Solids & CAD |
| `tool.shell` | Shell / Thicken (joined freeform faces, brief 49: part 1 built, part 2 built for creases, inside, outside (part 2b) and a true corner of three or more faces (part 2c) all built; the real boat hull now thickens both ways end to end (NOTES "part 2c"); rim styles with handles and Inspector rows built, and walls end flush both ways, the surface carried on past its edge where a cut leans away: brief 49 is done) | Tool Pass |
| `edit.center` | Center / Align | Shapes & Placement |
| `tool.prim` | Primitives | Shapes & Placement |
| `prim.box` | Box | Shapes & Placement |
| `prim.cyl` | Cylinder | Shapes & Placement |
| `prim.cone` | Cone | Shapes & Placement |
| `prim.sphere` | Sphere | Shapes & Placement |
| `prim.torus` | Torus | Shapes & Placement |
| `prim.capsule` | Capsule | Shapes & Placement |
| `tool.mirror` | Mirror | Shapes & Placement |
| `tool.symmetry` | Live Symmetry | Shapes & Placement |
| `tool.trim` | Trim / Extend | Shapes & Placement |
| `tool.array` | Array | Shapes & Placement |
| `tool.align` | Align / Distribute | Shapes & Placement |
| `tool.drop` | Drop to Face | Shapes & Placement |
| `tool.deform` | Bend / Twist / Taper | Shapes & Placement |
| `tool.draft` | Draft Angle | Shapes & Placement |
| `tool.project` | Project Curves | Shapes & Placement |
| `tool.unfold` | Unfold / Flatten | Shapes & Placement |
| `tool.layflat` | Lay Flat | Shapes & Placement |
| `tool.hole` | Hole | Shapes & Placement |
| `tool.subd` | Smooth / Subdivide | Cage & Subdivision |
| `tol.mod` | Tolerance Modifier | Fit & Mechanisms |
| `fit.helper` | Fit Helper | Fit & Mechanisms |
| `fit.check` | Check Fit | Fit & Mechanisms |
| `tool.hinge` | Hinge | Fit & Mechanisms |
| `tool.slide` | Slide | Fit & Mechanisms |
| `tool.ball` | Ball Joint | Fit & Mechanisms |
| `tool.fixed` | Fixed Joint | Fit & Mechanisms |
| `mech.sweep` | Sweep Motion Range | Fit & Mechanisms |
| `render.settings` | Render Settings… | Rendering |
| `render.engine` | Engine | Rendering |
| `render.passes` | Output and Passes… | Rendering |
| `render.denoise` | Denoise | Rendering |
| `render.materials` | Materials… | Rendering |
| `render.lights` | Lights | Rendering |
| `render.env` | Environment | Rendering |
| `render.view` | Render View | Rendering |
| `render.queue` | Render Queue | Rendering |
| `render.blender` | Render in Blender | Rendering |
| — | Mode switcher (Model, Materials, Render, Animate, Simulate, Project) | Modes & Scenes |
| — | Scenes and named cameras (tentative) | Modes & Scenes |
| — | Tags typed in the Inspector and the Tags window, with Enter | Names, Tags & Game Roles |
| — | Game words for Beverly: typed roles on groups and components, a Surface word on materials, in the GLB | Names, Tags & Game Roles |
| `script.console` | Script Console | Scripting |
| `script.run` | Run Script… | Scripting |
| `script.docs` | Scripting Reference | Scripting |
| `help.feel` | Feel Panel | Input & Feel |
| `help.pen` | Pen and tablet input | Input & Feel |

## B. Planned in PLAN section 1, with no entry in the window

| Feature (PLAN section) | Chunk |
|---|---|
| Loft styles (Normal, Loose, Tight, Straight, Uniform, Developable) with a ghost of each; tension; refit tolerance; closed loft; match start and end tangents (1.4) | Loft & Sweep Controls |
| Loft end conditions as arrows dragged in the viewport (weights) (1.4) | Loft & Sweep Controls |
| Dragging a loft's surface adds a guide curve; rails and a spine; dragging profile start points (1.4) | Loft & Sweep Controls |
| Extrude a curved face in a straight direction or along a path into a solid (Follow Me takes flat profiles only; owner, 2026-11-16) | Loft & Sweep Controls |
| The loft stays live: moving a profile updates it until exploded (1.4) | Loft & Sweep Controls |
| Sweep twist and scale (1.4) | Loft & Sweep Controls |
| Arrays: polar and along a path, not only Move's linear copies (1.4) | Shapes & Placement |
| "Smooth" a flat cage region into a patch; insert edge loop as a tool (1.1) | Cage & Subdivision |
| Glue to and Cut opening for components, with the glue sign and toggle; a components library browser (1.2) | Groups & Components 2 |
| USD / USDZ and FBX; mesh export controls (chord and angle tolerance, edge length) with a preview and triangle count (1.3) | More Formats |
| Text, left over after part 3 (three bundled fonts, shaping, outline / face / solid as a group): exact glyph curves (they are short straight pieces now), text along a path, bold and italic, alignment, a font list beyond the three (the owner chose three bundled fonts, no system fonts, 2026-11-18), editable text (it is baked into a group when set down), text on curved faces, screen labels (1.4) | Annotation & Sections |
| Keymap presets for Blender and Fusion (1.5a); the Mouse and navigation tab and a printable cheat sheet (1.12) | Input & Feel |
| The Mac system menu bar, and a key to open the menu bar (Alt or F10) (1.6) | Input & Feel |

## C. Leftovers in NOTES (from each chunk's "Left over" list)

Done since and left out: the camera items (Everyday Commands), Intersect within a group, Model Health's crossing faces, crossings with curved and freeform faces, file import and export part 2, the Toolbars window, Push/Pull of freeform faces, the kernel for solids, a component's own axes (Group Axes), and Lock.

| Leftover (NOTES section) | Chunk |
|---|---|
| A spatial index and an edge map: drawing, picking, inference and Intersect Faces scan everything (Core Model, Viewport, Inference, Edit Tools 3) | Performance & Scale |
| Previews and edits rebuild the whole scene; draw a moving part by a GPU transform; incremental rebuilds (Edit Tools 1, Move on a big STL) | Performance & Scale |
| A Push/Pull drag on a lofted (NURBS) face runs at about 7 fps: each move redoes the pull and re-tessellates every trimmed NURBS face. Cache tessellation per face, skip an unchanged distance, coarser preview while dragging (NOTES "Other leads from replaying the owner's recordings", session 9) | Performance & Scale |
| GPU instancing, one draw per component definition (Groups & Components 2) | Performance & Scale |
| Kernel calls and Validate off the interface thread (DECISIONS #39; Solids & CAD 1 and 3c) | Performance & Scale |
| Loops that touch themselves at one corner ("pinched"), and the property test's rare failures (Core Model, the solid rule) | Drawing & Edit Polish |
| The coplanar and solid rules after Offset, Import, Move, Rotate and Scale; an opening an edit did not close (Push/Pull edge by edge, the solid rule) | Drawing & Edit Polish |
| The Eraser can step over an edge on a fast drag; Shift to hide in the Eraser (Edit Tools 1) | Drawing & Edit Polish |
| Guides: select, move and delete them; Move and Rotate take them along (Edit Tools 2) | Drawing & Edit Polish |
| Offset of edge chains; Tape Measure's "type a length to resize the model" (Edit Tools 2) | Drawing & Edit Polish |
| Retyping the previous segment while a line chains; measurement-box arithmetic and `[x,y,z]` / `<dx,dy,dz>` (Inference & Drawing) | Drawing & Edit Polish |
| Smart Guides: equal sizes, chains of more than two, Rotate and Scale, hidden behind faces, overlapping labels, settings in View Settings (Everyday Commands 4) | Drawing & Edit Polish |
| Angle Guides for curved edges and in Move, Line and Scale (Angle Guides) | Drawing & Edit Polish |
| Typed values in the Axes tool (Group Axes); Freehand's Shift for a 3D stroke (Everyday Commands 3) | Drawing & Edit Polish |
| A curve that crosses itself is cut there; a midpoint snap on a spline (Freeform NURBS 1) | Drawing & Edit Polish |
| Edit tools on a closed group say "double-click the group to open it" (the Fillet window) | Tool Pass |
| Explode selects what it exploded; arrays (x3, /4) of groups; the Eraser drags over closed groups (Groups & Components 1) | Groups & Components 2 |
| The Components window places new instances; the Outliner drags rows, searches and filters; hiding loose faces and edges (Groups & Components 2 and 3) | Groups & Components 2 |
| A component holding groups pasted into its own document reuses its definition; "Unhide Selected" for something hidden by its tag; picking a ghost inside a visible group (Everyday Commands 1, View Settings 2) | Groups & Components 2 |
| Accurate (order-independent) X-ray; profiles on edges of three or more faces and behind faces; points of faded groups; "save as new style" (View Settings 1) | View & Materials Polish |
| Shadow cascades; fog, ground opacity, below ground, edge shadows, a time slider (View Settings 2) | View & Materials Polish |
| Dashes that restart at the screen edge; guide lines that end when zoomed far out (Viewport, Edit Tools 2) | View & Materials Polish |
| A cylinder's texture turning with it; texture places on a patch; Position Texture on curved faces; pins on more than one face and typed values (Basic Materials 2a, 3a, Freeform NURBS 2) | View & Materials Polish |
| Purge Unused counts images; image thumbnails in the Materials window; see-through faces sorted per triangle; textured batches kept between rebuilds (Basic Materials 2b, 2c) | View & Materials Polish |
| Curvature combs on a face's sides and inner lines, comb scale, stripe width and direction (Freeform NURBS 6) | View & Materials Polish |
| Box select of control points; Rotate and Scale of points with the protractor and grips; adding points round a closed curve; moving trims in cage editing; cage handles across a fill's seams (Freeform NURBS 3, Solids & CAD 3a, Loft Tool) | Cage & Subdivision |
| Surfaces that come to a point: three-sided faces, poles, two- and three-sided patches, arcs as patch sides (Freeform NURBS 4, 5) | Cage & Subdivision |
| Caps on lofts and on the pipe; picking a loft's closing edges as a chain of several runs; thin corners where sides meet at a cusp (Freeform NURBS 4, 5, Loft Tool) | Loft & Sweep Controls |
| Edges crossing in a freeform face's middle are split where they cross; a closed spline crossed twice close together leaves a sliver (Solids & CAD 3a, 3b) | Solids & CAD |
| Booleans: nested groups and loose geometry as inputs, keeping the inputs, keeping a component link, a hover highlight of the whole group, Split's placing (Solids & CAD 2) | Solids & CAD |
| Heal inside a closed group, gaps wider than ten tolerances, holes whose edges are not flat (Solids & CAD 1) | Solids & CAD |
| Rounding several edges by a drag; a true largest fit; rounds beside freeform faces, variable radius and corner styles; a piece of a straight line rounding alone (the Fillet window, Fillet Joins D) | Inspector & Features |
| Live features, left: a typed-size preview for a group seen from outside and for a group's own Position and Size, paint on a feature's faces surviving a re-run, the Fillet tool's handle on a feature with curved edges or no flat face beside it, other feature kinds (hole, array, mirror); the live shell's open faces cannot be re-picked, and Make Group bakes it, the live boolean's cutter cannot be moved by dragging (only typed) | Inspector & Features |
| Materials and colours in every format, back materials, OBJ's other maps, imported materials merged by name, per-instance materials on a shared mesh (Files & Formats 2, Basic Materials 3b, 3c, Export for Beverly 2) | More Formats |
| Exact curves recovered from meshes; joining imports to geometry; a progress bar; exports that keep holes (Files & Formats 2) | More Formats |
| Section planes, left over: corners and edges on the cut-away side still snap (only crossing guides are dropped); a smooth cap for cut curved faces; caps in the ground shadow (Annotation & Sections 1) | Annotation & Sections |
| Construction planes, left over after the gap pass: a grid drawn on the plane, naming planes in a list window, inference snapping to the plane's own origin, a plane square to an edge (1.4); the model's axes: typed values, the camera's up following tilted axes | Annotation & Sections |
| Walk collision (Everyday Commands 2); double-clicks timed by frame on a slow renderer (Viewport) | Input & Feel |
| The hint panel's pictures (Everyday Commands 3) | Input & Feel |
| The Windows installer and file-type registration; the Mac Apple event handler; a thumbnail in the file; recent files on the web (Files & Formats 1, 3) | Packaging & App Polish |
| Dragging tabs to reorder, more than one window, the system clipboard and dragging between windows (Files & Formats 3, Everyday Commands 1) | Packaging & App Polish |
| Preferences: reset units, colours and autosave timing (Everyday Commands 3) | Packaging & App Polish |
| `cargo-deny` in CI; the kit's blur-in and popup closing animation; docked panels (Skeleton, Soft Theme) | Packaging & App Polish |
