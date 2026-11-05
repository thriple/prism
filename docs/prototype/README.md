# The browser prototype (reference only)

`Main.dc.html` is the "Prism Feel Test", a single file that runs in a browser. It is a mesh-only test of how the main tools feel. **Do not port its code line by line.** Use it to see the intended behaviour and to read the rules it encodes.

It is a Design-artifact page: the template and CSS are at the top, and the logic is in one `<script type="text/x-dc">` block that defines `class Component`. To try it, open the published page (ask the owner for the link), or load the file in the design tool.

## Where things are in `Main.dc.html`

| What | Where (search for) |
|---|---|
| Geometry model: points, edges, faces with holes, sticky geometry, Push/Pull | `class Model`, then `insertPoint`, `addSegment`, `detectFaces`, `attachHoles`, `pushPull`, `remove`, `dup` |
| Ear-clipping triangulation (the shortcut Core Model must not copy) | `function triangulate` |
| Inference engine: candidates, scoring, guides, locks, dwell | `infer(`, `guideLines(`, `trackDwell(` and the `GLYPH` table |
| Camera: zoom to cursor, orbit about the point under the cursor, pan | `onWheel(`, `surfacePoint(`, `orbitBy(`, `panBy(` |
| Tools: Select, Line, Rectangle, Push/Pull, Move, Rotate, Scale, Eraser | `toolDown(`, `rectCorners(`, `commitScale(`, `commitErase(` |
| Action registry, menus, dock, icons | `const ACT`, `const MENUBAR`, `const DOCK`, `const IC` |
| Menu, dock and context menu generation | `buildChrome(`, `menuHTML(`, `onContext(` |
| Planned Features checklist | `openFeatures(`, `const BEHAVIOURS` |
| Drawing: faces, edges, snap highlights | `drawFaces(`, `drawEdges(`, and the glyph code in `draw(` |

## Behaviour worth knowing

- **Inference priorities and capture radii (px):** Endpoint 14, Midpoint 12, On Edge 9, axis guides 7, face as fallback. The pick aperture, snap release (hysteresis) and hover wake time are sliders.
- **Locks:** Shift locks the current inference. Up, right and left arrows lock blue, red and green. Down releases.
- **Typed values** go to a measurement box: `2.5`, `40cm`, `45`, `2,1.5`.
- **Camera:** the wheel only zooms (toward the surface under the cursor). Middle-drag orbits about the point under the cursor, object first and then the ground. Shift+middle pans. Orbit (O) and Pan (H) are tools that orbit and pan on left-drag.
- **Push/Pull:** Alt toggles "new loop".
- **Overlaps:** coplanar overlaps split, faces inside faces become holes (see `core-model-acceptance.md`).
- **Relative tolerance:** one millionth of the model's size, clamped.

## Known limits
Mesh only. No groups. Faces are planar polygons, and non-planar ones are triangulated. The ground shadow ignores holes. Hidden objects, the Inspector, mechanisms and most menu items are not built, and show as greyed in the menus.
