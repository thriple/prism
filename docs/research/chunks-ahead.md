# Chunks ahead: where the plan is thin (2026-11-11)

The owner asked (2026-11-11) for research on the chunks that might feel
underprepared. This covers the next five in build order: Exact Curves 5c–5d
(Follow Me) and part 6 (curved faces in the standard tools, DECISIONS #33),
Basic Materials, Annotation & Sections, and Inspector & Features. Each
section says how ready the chunk is, what was found, and what needs the
owner's decision. Sources are at the end.

## Exact Curves 5c–5d: Follow Me

**Ready enough.** Brief 26 has SketchUp's rules. What the research adds:

- **The profile is projected, not turned.** SketchUp projects the profile
  onto the plane square to the first segment; a tilted profile comes out
  squashed. Brief 26 already says "projected". Keep it.
- **Mitres fail on wide profiles at sharp inside corners.** SketchUp then
  makes overlapping faces. Prism should refuse with a reason instead: at
  each corner, check that the profile, mitred, does not reach past the
  neighbouring corner's mitre plane (a self-crossing sweep).
- **A profile drawn at a path corner** can stop SketchUp early when the
  sweep meets the profile again. Treat a closed path as a ring with no
  ends, and never let the sweep end on its own start.
- **Faces Follow Me makes should be rectangles in (θ, s)** (one face per
  path segment and profile side, split by seams), so they take the fast
  grid that step 5b built (`prism_view::tess::turned`).

## Exact Curves part 6: curved faces in the standard tools

**Underprepared: the hardest part ahead.** Nothing in the plan said how
booleans work with curved faces. Findings:

- **Every kernel does the same four steps:** find where faces cross, cut
  both faces along those crossings, decide for each piece whether it is
  inside or outside the other solid, and keep the pieces the operation wants.
  Most boolean bugs come from the first step, which is why kernels treat
  planes specially. Prism already does all four steps for flat faces
  (Push/Pull booleans, DECISIONS #31); part 6 extends each step to curved
  faces.
- **Where a plane meets a turned surface** (known geometry):

  | Surface | Plane square to the axis | Plane through the axis | Any other plane |
  |---|---|---|---|
  | Cylinder | a circle | two lines (or one, or none) | an ellipse |
  | Cone | a circle | two lines through the apex | an ellipse, parabola or hyperbola |
  | Sphere | a circle | a circle | **always a circle** |
  | Torus | two circles (or one, or none) | two circles | a quartic (spiric section) |

  So the cases a modeller meets most (a box cut square to a cylinder's axis,
  any plane through a sphere) are **exact with the curves Prism already
  has**. Slanted cuts give ellipses and other curves: the segment fallback
  (DECISIONS #32, step 6a).
- **Cutting a curved face is a flat problem in (θ, s).** Step 5b describes
  every turned surface as (θ round the axis, s along the profile).
  There, a plane square to the axis is a straight line s = constant, and a
  plane through the axis is a line θ = constant. So cutting a curved face
  along exact crossings is the same 2D region rebuild Prism already does for
  flat faces (`regions::rebuild_plane`), done in (θ, s). Fallback crossings
  are polylines in (θ, s), which works the same way.
- **Inside or outside:** the flat code casts a ray and counts faces. Curved
  faces need exact ray hits: cylinders have them already, cones and spheres
  are a quadratic too, and tori are met through their drawn triangles
  followed by Newton steps (as picking does since 5b).
- **Two curved faces crossing** (cylinder and cylinder) give 3D curves
  except in special cases: on the same axis gives circles; parallel axes
  give lines; a sphere centred on the cylinder's axis gives circles. Do
  only those exactly; everything else uses the fallback.
- **Proposed split of part 6** (already in PLAN 6.0a, refined):
  - 6a: fallback curves as flagged polylines in the core, files, Model
    Health.
  - 6b: exact plane crossings with all four surfaces, and cutting faces
    in (θ, s); Intersect Faces and sticky geometry use them.
  - 6c: the ray test against curved faces; Push/Pull booleans and stops
    with curved faces.
  - 6d: new sizes for cones, spheres and tori, and outline lines along
    curved faces.
  - 6e: the curve tools become active.
  - 6b and 6c are each a full session; 6b may need splitting into planes
    and curved faces first, then sticky geometry.

## Basic Materials

**Underprepared on decisions, not on technique.**

- **Libraries:** Poly Haven and ambientCG are both CC0 (public domain:
  commercial use, no attribution), with seamless PBR maps. Poly Haven
  has a public API. CC0 is not on Prism's licence list, which covers code
  and fonts. **Owner decision:** allow CC0 for bundled textures (as #12
  allowed OFL for fonts)? Recommendation: yes, a small bundled set
  (about 30 materials at 1k, a few MB), with more downloadable later.
- **How rich a material is:** the plan says physically based. Store base
  colour (or texture), roughness and metalness, plus opacity, from the
  start, so the path tracer (Rendering) needs no file change. Normal maps
  can come later. The viewport shows colour, texture and opacity only.
- **Placing a texture:** flat faces use a 2D placement in the face's plane
  (offset, rotation, scale), as SketchUp does. **Curved faces get natural
  coordinates free:** (θ × radius, s) from step 5b unrolls cylinders and
  cones exactly. Spheres and tori stretch at the poles: accept it, as
  SketchUp does.
- **Files:** textures go inside the `.prism` zip, stored once by content
  hash (a new schema).
- **Web limits:** WebGL2 guarantees 2048-pixel textures (4096 on nearly
  all devices). Downscale larger imports for the viewport, and keep the
  original in the file for rendering.
- **Crates:** `image` (MIT/Apache, builds for the web) for PNG and JPEG;
  egui's own colour picker for colours.
- **Paint Bucket's modifier keys** are already in the PLAN tool table
  (replace everywhere, fill connected, sample).

## Annotation & Sections

**Underprepared: three choices are open.**

- **Dimension and text labels:** SketchUp draws them facing the screen
  and on top of everything. Prism can draw them with egui's painter at the
  projected points (as the measurement box and tool hints already are), so
  no new crate. glyphon (MIT, Apache or zlib; wgpu text) is only needed
  for text hidden behind geometry, which these are not.
- **3D Text (extruded letters):** needs glyph outlines (`ttf-parser`,
  MIT/Apache, with the bundled Plex fonts). Glyph outlines are curves that
  are not arcs, so they use the segment fallback (6a) or are fitted with
  arcs. Defer 3D Text to Shapes & Placement unless the owner wants it here.
- **Section planes:** the common real-time trick (clip in the shader, then
  fill the cut using the stencil buffer) breaks on models that are not
  closed and cannot make hatching or 2D exports. **Recommendation:** compute
  the cut exactly on the CPU from the model (each plane meeting each face,
  with part 6's exact crossings), then draw the cut faces as real geometry:
  exact caps, hatching, and a 2D export later. Clip the rest in the shader.
  This makes Annotation & Sections depend on Exact Curves part 6 (it comes
  after it in the order already).
- **Dimensions that follow the model:** a dimension refers to vertices by
  stable id. When an edit removes one, it falls back to the nearest point
  where the vertex was. If nothing is near, the dimension is marked broken,
  as PLAN 1.17 does for features.

## Inspector & Features

**Underprepared: the design exists (PLAN 1.17), but the architecture change
it needs is not planned.**

- **Re-editing is the "topological naming" problem.** No CAD system fully
  avoids it: FreeCAD's 1.0 algorithm reduces it, and commercial kernels
  hide it with matching heuristics. PLAN 1.17 already chooses stable ids
  first, a geometric match second, then asking you to re-pick. That is the
  right answer. It needs a **reference format**: an id plus a geometric
  fingerprint (an edge: midpoint, direction, length, and the normals of
  its two faces), matched within the model tolerance.
- **The missing piece: a modifier stack per body.** Today a fillet is a
  one-shot command (`Command::RoundEdge`) that changes the geometry. PLAN
  1.17 wants a body's base geometry plus modifiers applied in order, and
  re-evaluated after every direct edit. This changes the document (a body
  keeps its base geometry and stack), files (a new schema), undo, the scene
  (it draws the result), and Model Health. It is an XL piece of work on
  its own, and the chunk is sized L.
- **Booleans between bodies** (Union, Subtract and so on, between two
  groups) do not exist yet; only Push/Pull booleans do. The Inspector's
  editable boolean needs them first.
- **Proposed split** (the chunk becomes XL, 5 parts):
  1. The schema-driven panel for what exists (transform, dimensions,
     geometry, tags).
  2. The modifier stack: base geometry, evaluation, files, undo, the
     broken state, Bake.
  3. Booleans between bodies, as modifiers.
  4. Fillet and chamfer, shell, as modifiers.
  5. Array, mirror and hole.

## Decisions for the owner

1. **CC0 textures:** allow CC0 for bundled textures and materials (like OFL
   for fonts)? Recommended: yes.
2. **Section planes:** cut exactly on the CPU (recommended), or the
   stencil trick (faster to build, no hatching or export)?
3. **3D Text:** in Annotation & Sections, or later with Shapes & Placement
   (recommended: later)?
4. **Inspector & Features:** grow it to XL in 5 parts, with the modifier
   stack as its own part (recommended)?

## Sources
- [FreeCAD wiki: Topological naming problem](https://wiki.freecad.org/Topological_naming_problem/en)
- [realthunder: Topological Naming (Assembly3 wiki)](https://github.com/realthunder/FreeCAD_assembly3/wiki/Topological-Naming)
- [B-rep Boolean resulting model repair (arXiv 2310.10351)](https://arxiv.org/pdf/2310.10351)
- [BRL-CAD: NURBS Boolean evaluation development guide](https://brlcad.org/docs/cad/devguides/en/bool_eval_development.html)
- [Novedge: Boolean modeling in CAD, robustness](https://novedge.com/blogs/design-news/design-software-history-boolean-modeling-in-cad-csg-origins-b-rep-breaking-points-and-robustness-solutions)
- [glyphon (crates)](https://docs.rs/crate/glyphon/latest)
- [OpenGL capping with the stencil buffer (SIGGRAPH 99 notes)](https://ii.uni.wroc.pl/~anl/cgfiles/Sig99AdvOpenGLnotes/node21.html)
- [Cross-section capping in WebGL (BioDigital write-up)](https://shrekshao.github.io/2016/08/27/BioDigital-Intern-Review-3-Cross-Section)
- [Stencil capping limits on non-convex shapes (Trapp)](https://dspace5old.zcu.cz/bitstream/11025/6874/1/Trapp.pdf)
- [xeokit SectionPlaneCaps](https://xeokit.github.io/sdk/docs/api/classes/viewing.viewer.SectionPlaneCaps.html)
- [Poly Haven](https://polyhaven.com) and [its FAQ (licence)](https://docs.polyhaven.com/en/faq)
- [Free PBR textures guide (ambientCG, Poly Haven, CC0)](https://app.cinevva.com/guides/free-textures-hdris-materials)
- [SketchUp Help: Extruding with Follow Me](https://help.sketchup.com/en/sketchup/extruding-follow-me)
- [SketchUp forum: Follow Me oddity (mitres, perpendicular first segment)](https://forums.sketchup.com/t/follow-me-tool-oddity/71178)
- [SketchUcation: Follow Me issue (starting at a corner)](https://community.sketchucation.com/topic/159318/follow-me-tool-issue/2)
