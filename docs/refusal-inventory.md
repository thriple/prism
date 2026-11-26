# Refusal inventory (proposal 54, step 1)

Status: **for the owner to check** (2026-11-26). Generated from the code: the refusals the geometry code and tools can give (every "Refused…" message, and the error messages of the geometry crates), with the file, the constant, and a proposed kind from proposal 54 (`docs/briefs/54-warn-dont-refuse.md`). The kinds are guessed from the wording and need checking; "?" means a decision is needed (most are "not supported yet" limits: kind 1 until the tool can do it, or kind 3 if something can be built anyway).

- **1. Not an edit:** stays a refusal, said at the cursor.
- **2. As far as it goes:** build the largest that works and show what stopped it.
- **3. Built, check it:** build it anyway, flag the doubtful parts orange. "3 / red": kernel failures, red only if nothing can be built.
- **4. Partly:** build what works, mark the rest.

145 messages in 29 files. Already done: Shell / Thicken (warns, DECISIONS #41) and Fillet / Chamfer sizes (kind 2, brief 55). Next in order: Push/Pull and Offset.

Proposed kinds: 1: 34, 2: 10, 3 / red: 13, 4: 5, ?: 83

| File | Constant | Message | Proposed kind |
|---|---|---|---|
| `prism-doc/src/assembly.rs` | `NESTED` | Refused: a group with groups inside cannot be used yet. Explode them first. | ? |
| `prism-kernel/src/lib.rs` | `NO_SURFACE` | Refused: no surface could be made from these curves. | ? |
| `prism-kernel/src/lib.rs` | `ON_AXIS` | Refused: the curve lies on the axis it would turn about. | ? |
| `prism-kernel/src/solid.rs` | `NOT_CLOSED` | Refused: these faces do not close a solid. | ? |
| `prism-kernel/src/solid.rs` | `FAULTY` | Refused: the kernel found a fault in this solid (faces crossing, or pointing inward). | 3 / red |
| `prism-kernel/src/solid.rs` | `SURFACE_NOT_YET` | Refused: a face's surface cannot go to the kernel. | 3 / red |
| `prism-kernel/src/solid.rs` | `CURVE_NOT_YET` | Refused: an edge of this solid cannot go to the kernel. | 3 / red |
| `prism-kernel/src/solid.rs` | `OFF_FACE` | Refused: an edge does not lie on the face it bounds. | ? |
| `prism-kernel/src/solid.rs` | `CANNOT_STORE` | Refused: the kernel made a shape Prism cannot store yet. | 3 / red |
| `prism-kernel/src/solid.rs` | `TOO_LARGE` | Refused: the size is too large for a face beside an edge, or for a round next to it. | 2 |
| `prism-kernel/src/solid.rs` | `NO_BLEND` | Refused: edges can be rounded only between flat, cylindrical and conical faces so far. | ? |
| `prism-kernel/src/solid.rs` | `NOT_ROUNDED` | Refused: the kernel could not round these edges. | 3 / red |
| `prism-kernel/src/solid.rs` | `NOT_IN_SOLID` | Refused: an edge to round is not an edge of this solid. | 1 |
| `prism-kernel/src/step.rs` | `NOTHING` | Nothing to export: no solid or sheet could go to STEP. | 1 |
| `prism-kernel/src/step.rs` | `NOT_WRITTEN` | Refused: the STEP writer could not write these shapes. | 3 / red |
| `prism-kernel/src/step.rs` | `NOT_A_SHEET` | Refused: these faces do not join into one surface. | ? |
| `prism-ops/src/boolean.rs` | `NOT_SOLID` | Refused: one of these is not a solid. Solid > Validate and Heal says why. | 1 |
| `prism-ops/src/boolean.rs` | `UNSURE` | Refused: Prism could not tell inside from outside here. | 3 / red |
| `prism-ops/src/bridge.rs` | `APPROXIMATE` | Refused: an edge of this solid is an approximation, which the kernel cannot take. | 3 / red |
| `prism-ops/src/cage.rs` | `NO_CAGE` | Select a freeform curve or face first: lines, arcs and flat faces have no control points. | 1 |
| `prism-ops/src/cage.rs` | `SIDE_OF_FACE` | Refused: this curve is the side of a freeform face. Move the face's control points instead. | ? |
| `prism-ops/src/cage.rs` | `FLAT_BESIDE` | Refused: a flat face beside it would bend. Move these points within its plane, or erase that face first. | ? |
| `prism-ops/src/cage.rs` | `NEIGHBOUR` | Refused: another curved face or curve is joined here and would come apart. | ? |
| `prism-ops/src/cage.rs` | `CLOSED_ADD` | Refused: control points cannot be added round a closed curve or surface yet. | ? |
| `prism-ops/src/cage.rs` | `COLLAPSED` | Refused: the curve or face would shrink to nothing. | 1 |
| `prism-ops/src/cage.rs` | `TRIMMED` | Refused: a face on this surface has been cut; editing its control points comes later. | ? |
| `prism-ops/src/convert.rs` | `NOTHING` | Select a chain of lines and arcs, or one NURBS curve, then Convert. | 1 |
| `prism-ops/src/convert.rs` | `NOT_A_CHAIN` | Refused: Convert takes edges joined end to end in one chain, without branches. | ? |
| `prism-ops/src/convert.rs` | `MIXED` | Refused: Convert takes a chain of lines and arcs, or one NURBS curve, not both. | ? |
| `prism-ops/src/convert.rs` | `MEETS` | Refused: other edges meet this chain partway; its points between would have to stay. | ? |
| `prism-ops/src/convert.rs` | `CURVED` | Refused: a curved face runs along this chain; Convert takes chains with flat faces or none. | ? |
| `prism-ops/src/convert.rs` | `FAILED` | Refused: Prism could not make this curve. | 3 / red |
| `prism-ops/src/convert.rs` | `NO_FACES` | Select a grid of four-sided faces, or one freeform face, then Convert. | 1 |
| `prism-ops/src/convert.rs` | `NOT_A_GRID` | Refused: Convert takes four-sided flat faces joined edge to edge in rows and columns, or one freeform face. | ? |
| `prism-ops/src/convert.rs` | `BORDERS` | Refused: other faces or edges meet this grid's sides; Convert takes a grid standing on its own. | ? |
| `prism-ops/src/convert.rs` | `TRIMMED` | Refused: this freeform face is cut from its patch; Convert takes a whole one. | ? |
| `prism-ops/src/curves.rs` | `NOTHING` | Select edges joined end to end, then Weld. | 1 |
| `prism-ops/src/curves.rs` | `NOT_A_CHAIN` | Refused: Weld joins edges end to end in one chain, without branches; these are not. | ? |
| `prism-ops/src/curves.rs` | `ALREADY` | These edges are one curve already. | ? |
| `prism-ops/src/deform.rs` | `AXIS` | A deform needs an axis to work about. | ? |
| `prism-ops/src/deform.rs` | `SPAN` | The two ends of a deform must be different places along its axis. | ? |
| `prism-ops/src/deform.rs` | `SCALE` | A taper's far end must be a positive size, not zero or less. | 1 |
| `prism-ops/src/deform.rs` | `DIR` | A bend must bend towards a direction square to its axis. | ? |
| `prism-ops/src/deform.rs` | `FOLD` | This bend is too tight: part of the shape would fold through itself. | 2 |
| `prism-ops/src/deform.rs` | `CURVED` | Cylinders, cones, spheres and tori cannot be bent, twisted or tapered yet. | ? |
| `prism-ops/src/deform.rs` | `FAILED` | Prism could not fit the deformed shape closely enough. | 3 / red |
| `prism-ops/src/draft.rs` | `NOT_FLAT` | Draft Angle works on flat faces | ? |
| `prism-ops/src/draft.rs` | `ANGLE_RANGE` | A draft angle is between -45 and 45 degrees, and not zero | 1 |
| `prism-ops/src/draft.rs` | `PARALLEL` | A face parallel to the neutral plane has nothing to lean about | 1 |
| `prism-ops/src/draft.rs` | `CORNER` | A corner of these faces meets other than three faces: Draft Angle needs three at each | 4 |
| `prism-ops/src/draft.rs` | `CANNOT` | These faces cannot be leaned by that much (their corners no longer meet) | ? |
| `prism-ops/src/draft.rs` | `NOTHING` | Select the faces to lean first | 1 |
| `prism-ops/src/fillet.rs` | `NOT_A_CORNER` | Only a corner of a face between two straight edges can be rounded. | ? |
| `prism-ops/src/fillet.rs` | `TOO_SHORT` | The edges at this corner are too short for that radius. | 2 |
| `prism-ops/src/follow.rs` | `NO_PATH` | Select a path first (lines and arcs joined end to end), then click the profile face. | 1 |
| `prism-ops/src/follow.rs` | `NOT_A_PATH` | The path must be lines and arcs joined end to end in one chain. | ? |
| `prism-ops/src/follow.rs` | `FREEFORM_PATH` | Refused: Follow Me does not follow a freeform curve. Select the profile and the path, then use Draw > Surface > Sweep. | 1 |
| `prism-ops/src/follow.rs` | `FREEFORM_PROFILE` | Refused: Follow Me does not take a profile with a freeform curve. Use Draw > Surface (Sweep, Loft or Revolve) with its curves. | ? |
| `prism-ops/src/follow.rs` | `NOT_A_PROFILE` | The profile must be a flat face. | ? |
| `prism-ops/src/follow.rs` | `ALONG_PATH` | The profile lies along the path's first line: it must cross it. | ? |
| `prism-ops/src/follow.rs` | `NOT_SQUARE` | The profile is not square to the path where it starts, and it is joined to other faces (or has arcs), so it cannot be moved there. | ? |
| `prism-ops/src/follow.rs` | `ARCS` | A profile with arcs cannot turn a sharp corner of the path or be tilted to it: its arcs would become ellipses. | ? |
| `prism-ops/src/follow.rs` | `CROSSES_ITSELF` | The profile is too wide for a corner of this path: the sweep would cross itself. | 2 |
| `prism-ops/src/follow.rs` | `OVERLAPS` | The sweep would run into itself further along the path: the profile is too wide for it. | 2 |
| `prism-ops/src/follow.rs` | `RING_TWISTS` | Round this closed path the profile would not come back to where it started. | ? |
| `prism-ops/src/follow.rs` | `NOT_TANGENT` | Where the path meets an arc it must turn smoothly (tangent): a corner there would need an ellipse. | ? |
| `prism-ops/src/follow.rs` | `OFF_AXIS` | On an arc of the path, the profile must lie in a plane through the arc's centre and axis. | ? |
| `prism-ops/src/follow.rs` | `CROSSES_AXIS` | On an arc of the path, the profile must stay on one side of the arc's axis. | ? |
| `prism-ops/src/hole.rs` | `TOO_SMALL` | A hole needs a size above 0.01 mm | ? |
| `prism-ops/src/hole.rs` | `BORE_SMALLER` | A counterbore must be wider than the hole | ? |
| `prism-ops/src/hole.rs` | `BORE_DEEPER` | A counterbore must be shallower than the hole | ? |
| `prism-ops/src/hole.rs` | `SINK_SMALLER` | A countersink must be wider than the hole | ? |
| `prism-ops/src/hole.rs` | `SINK_ANGLE` | A countersink's angle is between 10 and 170 degrees | ? |
| `prism-ops/src/hole.rs` | `SINK_DEEPER` | A countersink must be shallower than the hole | ? |
| `prism-ops/src/loft.rs` | `TWO_CURVES` | Select two or more curves (or lines or arcs) to loft through. | 1 |
| `prism-ops/src/loft.rs` | `MIXED` | Refused: open and closed curves cannot be lofted together. Select all open or all closed. | 1 |
| `prism-ops/src/loft.rs` | `NOT_ALIKE` | Refused: closed curves need the same number of control points to be lofted (or use circles for all of them). | ? |
| `prism-ops/src/loft.rs` | `RING` | These curves join end to end in a ring: that is a boundary to fill, not sections to loft through. Use Draw > Surface > Patch (four sides for now). | ? |
| `prism-ops/src/loft.rs` | `DEGENERATE` | Refused: the curves touch or cross, so the surface would pinch to a point. | ? |
| `prism-ops/src/loft.rs` | `NO_AXIS` | Select the outline or curves to turn (any shape, open or closed) and one straight line to turn them about (a line that touches none of the others). | 1 |
| `prism-ops/src/loft.rs` | `TOUCHES_AXIS` | Refused: a curve touches or crosses the axis. For shapes that close on the axis, use Follow Me with lines and arcs. | ? |
| `prism-ops/src/offset.rs` | `CURVED_OFFSET` | Curved faces cannot be offset yet. | ? |
| `prism-ops/src/offset.rs` | `NO_OFFSET` | That face cannot be offset: an edge would vanish or cross. | ? |
| `prism-ops/src/primitives.rs` | `TOO_SMALL` | That size is too small (at least 0.01 mm) | 1 |
| `prism-ops/src/primitives.rs` | `TORUS_TUBE` | A torus's tube must be thinner than its ring is wide | ? |
| `prism-ops/src/pushpull.rs` | `CURVED_PULL` | This pull would bend a curved face, or a face beside a cylinder. | ? |
| `prism-ops/src/radius.rs` | `FREEFORM` | Refused: a freeform face is not pushed or pulled. Move its control points instead. | ? |
| `prism-ops/src/radius.rs` | `RADIUS_GONE` | The radius would reach zero. | 1 |
| `prism-ops/src/radius.rs` | `TUBE_AXIS` | The torus's tube would reach its axis. | 2 |
| `prism-ops/src/round.rs` | `NOT_AN_EDGE` | Only a straight edge between two flat faces at an angle, with plain corners at its ends, can be rounded. | ? |
| `prism-ops/src/round.rs` | `TOO_WIDE` | The faces beside this edge are too narrow for that size. | 2 |
| `prism-ops/src/round.rs` | `SLANTED_END` | A fillet needs the faces at the edge's ends square to it (a slanted end would be an ellipse). | ? |
| `prism-ops/src/rounds.rs` | `NOTHING` | Refused: pick the edges of a solid to round. | 1 |
| `prism-ops/src/rounds.rs` | `NO_SIZE` | Refused: the size must be more than zero. | 1 |
| `prism-ops/src/rounds.rs` | `TWO_SOLIDS` | Refused: the edges are not all on one closed solid. | ? |
| `prism-ops/src/rounds.rs` | `ONE_FACE` | Refused: an edge needs one face on each side to round. | 1 |
| `prism-ops/src/rounds.rs` | `OPEN` | Refused: off a closed solid, only straight edges between flat faces round so far, and not two that meet. | ? |
| `prism-ops/src/rounds.rs` | `FLAT_LINE` | Refused: this line is drawn on a flat face, so there is no corner to round. | 1 |
| `prism-ops/src/shelling.rs` | `NOTHING` | Refused: click a face to shell or thicken. | 1 |
| `prism-ops/src/shelling.rs` | `ZERO` | Refused: a thickness of zero makes nothing. | 1 |
| `prism-ops/src/shelling.rs` | `ALL_OPEN` | Refused: every face is left open, so there is nothing to thicken. | 1 |
| `prism-ops/src/shelling.rs` | `FREEFORM` | Refused: an arc's moved copy here is not round the same axis, so Shell / Thicken cannot wall the gap between them yet. | ? |
| `prism-ops/src/shelling.rs` | `MIXED` | Refused: these faces face different ways. Solid > Heal turns them the same way. | ? |
| `prism-ops/src/shelling.rs` | `BRANCHES` | Refused: more than two faces meet at an edge here, and they cannot be parted into sheets that each thicken on their own (or their shells could not be joined). | 3 / red |
| `prism-ops/src/shelling.rs` | `CORNER` | Refused: more than three faces meet at a corner on the rim here, or round a curved face, and their moved copies do not meet at one point. Not yet. | ? |
| `prism-ops/src/shelling.rs` | `TOO_THICK` | Refused: that thickness turns a face inside out. Try a smaller one. | 2 |
| `prism-ops/src/shelling.rs` | `RIM_STYLE` | Refused: a rim style other than Square works on joined faces with a freeform one, not on this shape yet. | ? |
| `prism-ops/src/shelling.rs` | `FLUSH_OFF` | Refused: a flush rim's plane must pass through the edge it ends. | ? |
| `prism-ops/src/shelling.rs` | `FLUSH_PARALLEL` | Refused: that plane lies along the face at its rim, so a wall cannot end in it. Pick another plane or Square. | 1 |
| `prism-ops/src/shelling.rs` | `FLUSH_REACH` | Refused: to end flush in that plane the surface would have to be carried on a long way past its own edge. Use a plane closer to square, or Square. | 2 |
| `prism-ops/src/shelling.rs` | `SHARP` | Refused: these faces fold right back onto each other here, and thickening into the fold (or a fold that ends at a corner of other faces) is not built yet. Try the other way. | 4 |
| `prism-ops/src/shelling.rs` | `BRANCHED` | More than two faces meet at an edge here: each sheet was thickened on its own and their shells were joined where they overlap. | 4 |
| `prism-ops/src/shelling.rs` | `BRIDGED` | These faces fold right back onto each other here, so their offsets never meet: the gap between them is bridged round the fold instead of mitred. | 4 |
| `prism-ops/src/shelling.rs` | `SQUARED` | That rim style cannot be built on this shape here, so this rim was cut square instead. | ? |
| `prism-ops/src/shelling.rs` | `SQUARED_OFF` | The flush plane chosen for this rim does not pass through its edge, so the rim was cut square instead. | ? |
| `prism-ops/src/shelling.rs` | `SQUARED_ALONG` | The flush plane chosen for this rim lies along the face there, so the rim was cut square instead. | ? |
| `prism-ops/src/sides.rs` | `NOT_JOINED` | These edges do not join end to end in one run. | ? |
| `prism-ops/src/sides.rs` | `TOO_FEW` | Pick two or more sides: sections to loft through, or a ring to fill. | 1 |
| `prism-ops/src/sides.rs` | `RING_OF_TWO` | Two sides that close a ring cannot be filled: split one so the ring has three or more sides. | ? |
| `prism-ops/src/sides.rs` | `OPEN` | The sides neither close a ring nor stand apart: some touch and some do not. | 4 |
| `prism-ops/src/sides.rs` | `CLOSED_SIDE` | A closed curve can only be lofted through, not used as a side of a ring. | ? |
| `prism-ops/src/sides.rs` | `PERIODIC_PART` | Part of a closed smooth curve cannot be a side yet; pick all of it. | 1 |
| `prism-ops/src/sweep.rs` | `TWO_FOR_SWEEP` | Select two curves to sweep: a profile and a path (lines and arcs count as curves). | 1 |
| `prism-ops/src/sweep.rs` | `BOTH_CLOSED` | Refused: two closed curves cannot be swept. One must be open (the path). | ? |
| `prism-ops/src/sweep.rs` | `FOUR_FOR_PATCH` | Select curves joined end to end in one ring to patch. | 1 |
| `prism-ops/src/taper.rs` | `AXIS` | Taper needs an axis to shrink or grow toward | ? |
| `prism-ops/src/taper.rs` | `SPAN` | The two ends of a taper must be different places along the axis | ? |
| `prism-ops/src/taper.rs` | `SCALE` | A taper's far end must be a positive size, not zero or less | 1 |
| `prism-ops/src/taper.rs` | `CANNOT` | This cannot be tapered exactly: a slanted face, a diagonal edge, or a curved face not on the axis | ? |
| `prism-ops/src/thicken.rs` | `NO_OFFSET` | Refused: Prism could not move this freeform face out that far within the tolerance. | 3 / red |
| `prism-ops/src/thicken.rs` | `ROUND` | Refused: a freeform face that goes all the way round cannot be pushed or pulled yet. | ? |
| `prism-ops/src/transform.rs` | `ARC_WOULD_BEND` | Refused: an arc would bend. Move its whole curve, or scale it evenly. | ? |
| `prism-ops/src/transform.rs` | `SPLINE_WOULD_BEND` | Refused: one end of a curve cannot move alone. Move the whole curve, or its control points. | ? |
| `prism-ops/src/transform.rs` | `ARC_FACE_WOULD_FOLD` | Refused: a face with arcs would fold. Keep it flat, or move all of it. | ? |
| `prism-ops/src/transform.rs` | `HOLE_FACE_WOULD_FOLD` | Refused: a face with a hole would fold. Keep it flat, or move all of it. | ? |
| `prism-ops/src/transform.rs` | `CURVED_WOULD_BEND` | Refused: a curved face would bend. Move all of it. | ? |
| `prism-ops/src/transform.rs` | `CUT_CURVED_SCALE` | Refused: this curved face cannot be made into facets. Scale it evenly. | ? |
| `prism-ops/src/trim.rs` | `NOT_LOOSE` | That edge bounds a face: use the Eraser, or Trim a loose edge | ? |
| `prism-ops/src/trim.rs` | `NOT_STRAIGHT` | Extend works on straight edges for now | ? |
| `prism-ops/src/trim.rs` | `END_JOINED` | That end is joined to something: Extend works from a free end | ? |
| `prism-ops/src/trim.rs` | `NOTHING_IN_THE_WAY` | Nothing in the way: carried on, the edge meets nothing | 1 |
| `prism-ops/src/trim.rs` | `NOT_FLAT` | Project onto a flat face | ? |
| `prism-ops/src/trim.rs` | `NOTHING_TO_PROJECT` | Select lines (or arcs turned the same way as the face) to project first | 1 |
| `prism-ops/src/validate.rs` | `KERNEL_NOT_YET` | A face round a pole or a tip, or one that goes all the way round, cannot go to the kernel yet: checked by Prism's rules only. | 3 / red |
| `prism-ops/src/validate.rs` | `KERNEL_TOO_LONG` | Freeform faces that meet along many curve pieces take the kernel too long to check yet: checked by Prism's rules only. | 2 |
