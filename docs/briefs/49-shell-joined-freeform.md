# Shell / Thicken of joined freeform faces (owner request, 2026-11-19)

**Goal.** Shell / Thicken works on shapes like a boat hull: several freeform (NURBS) faces joined along their edges, with flat faces beside them, open or closed. Today it refuses them ("takes a freeform face on its own, but not one joined to other faces"). Blender (Solidify) and Fusion 360 (Thicken, Shell) do this: every face is offset, and the offsets are joined at the corners.

**Read first:** brief 42 (the tool), `prism-ops/src/shelling.rs` and `thicken.rs` (a lone freeform face, and `offset_surface`), brief 48 part 5a (the live shell), NOTES "Live shell, part 5a". The owner's test file is `docs/acceptance/models/boat-half-hull.prism` (a half hull: three NURBS faces and two flat faces, an open sheet, two instances of one component that touch along the centre plane).

## Size
XL. Owner chose (2026-11-19) to build the smooth and creased joins together, with sharp mitres by default. Parts, each pushed and tested on its own:
1. **Offset a joined set of freeform faces (done).** Each face is offset (as a lone face is now); where two faces join smoothly (the same surface direction along the shared edge, within a tolerance) their offsets meet along the offset of the shared edge. The sheet becomes a slab with square rims. Refusals stay specific. Freeform joined to flat is not in this part (still refused); that is folded into part 2.
2. **Creases (done).** Where joined faces meet at an angle (hull to flat face, patch to patch with a crease), the two offsets are trimmed to meet in a sharp mitre, found by marching the shared edge and solving where both offsets meet exactly at each point. An inside corner (the mitre trims back within each face's own edge) works. An outside corner (the mitre would reach past a freeform face's own edge, which a NURBS surface cannot do without being refit bigger there) now works too (part 2b): each face keeps its own straight offset edge instead of the mitre, and a small flat face bridges the gap between them, closed at either end by a cap and a small triangle against the rim risers there. Where a third face (or more) also meets at that end (part 2c), the shared corner still stands for whichever of them it is truly on (a flat or other analytic face has no edge of its own to miss, so it never needs to split); only a face that truly misses it splits away on its own, closed against the other's own riser where there is one, or a plain shared edge where the corner is fully interior (no rim at all). More than one face missing it there is refused by the same name as a true multi-face corner, rather than built wrong. Rounded corners are not in this part either.
3. **Where each new edge goes** (the owner's open question, below): rim styles, a ghost, and handles with snapping.
4. **Live and Inspector.** It is the live Shell feature (brief 48 part 5a), so the thickness, the open faces and the rim style are editable afterwards.

## Everything physical (owner, 2026-11-19)
Typing a number is the fallback, not the way. The live shell already has a **thickness handle** (brief 48 part 5b): drag it along the wall. For joined freeform faces the same handle works from a point on the offset surface, and the rim handles below place the edges. The ghost shows the whole result while any handle is dragged.

## Where each new edge goes
At an open edge of a sheet, or where one part touches another, the new wall's edge can be cut in different ways. The owner asked for a ghost of what is about to happen, then handles with snapping. A drawing of the three styles at the half hull's keel was shown to the owner.

**Decided (owner, 2026-11-19):** Flush is picked automatically as below; the Custom handle only **tilts** the cut about the edge (no sliding: a cut moved off a plane is made by snapping flush to a guide placed there); the style is set **per open boundary**, not edge by edge.

The design:
- **Rim style, for each open boundary** (a whole loop of open edges, such as the keel line or the gunwale line): *Square* (cut across the surface, the default for a free edge), *Flush to a plane* (the wall ends in a plane: the plane of the face it touches, a guide, a work plane or an axis plane), and *Custom* (the plane is set by a handle).
- **Defaults are chosen for the user:** a boundary that lies in a plane (as the half hull's centre line does) or touches another part defaults to *Flush to that plane*, so two halves meet exactly with no double wall. Everything else is *Square*.
- **Ghost:** while the Shell window is open and a thickness is being set, the result is drawn as a ghost (the tool already does this for flat faces); rims show in their style, and a rim that the default placed by itself is marked.
- **Handles:** each open rim has a handle (like the Fillet tool's). Dragging it tilts the cutting plane about the edge; the drag snaps to the planes of nearby faces, guides, axes and work planes with the usual inference marks. Letting go sets the style to *Custom* (one undo step).
- **Live:** the style and plane are stored in the Shell feature, so they stay editable (Inspector rows and the same handle).

## Not in this brief
Rounded corners (a style choice later), Union of freeform solids (the touching halves are joined by leaving the touching face open, then a Union once that works), shelling a closed freeform solid inward when its offset self-intersects (refused with a reason), and anything for hole, array and mirror.

## Tests
- A loft-like sheet of three smooth NURBS faces thickens to a valid solid whose volume is close to area x thickness.
- A sheet with a crease (a flat face and a freeform face at an angle) gets a sharp mitre; the result is valid.
- An outside corner between two freeform faces (part 2b) bridges cleanly; by hand, for a square fold, the gap it closes is a right triangle of legs `d`, so the volume is `(2|d| + d^2/2) x` the fold's length.
- Three faces at a corner (part 2c), two of its creases outside and one inside: only the odd face out splits away; by hand, since the other two are flat and keep the one shared point, the whole thing is exactly a unit cube's own corner thickened outward, `(1 + d)^3 - 1`.
- The half-hull file thickens inward and outward, valid, both rims square; with the centre-plane face left open it is flush to that plane. **Not yet reachable** (2026-11-19, part 2): two pre-existing issues found while testing this, both fixed (NOTES "Offset-fit issue", "A found-while-testing false 'too sharp' refusal") — a trimmed patch misreading a shared knuckle knot, and `corner` refusing a vertex where a crease fades to tangent. Part 2b then got the hull past its own two simple outside corners, onto a third, harder one: a true corner of three faces at the hull's bow (part 2c, above), now reached the same way. Past that, the hull is still not genuinely shellable: fitting the real file's own outside corner (not the hand-built one above, which is clean) turns up a face with a bad winding — a further, separate limitation, found but not chased this time (NOTES "Brief 49 part 2c"). The hull itself is still not shellable end to end, each time for a reason the brief expected or a new, named, honestly-left one — not an accident.
- A refused case says why and changes nothing; the live feature runs again after a size change.

## MCP
Shell is a tool, so the MCP tools drive it as they do the others (select the faces, choose the open ones, give the thickness); the rim style and plane are `set_inspector` rows on the live feature (`feature.N.rim`).
