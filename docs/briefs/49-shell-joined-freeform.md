# Shell / Thicken of joined freeform faces (owner request, 2026-11-19)

**Goal.** Shell / Thicken works on shapes like a boat hull: several freeform (NURBS) faces joined along their edges, with flat faces beside them, open or closed. Today it refuses them ("takes a freeform face on its own, but not one joined to other faces"). Blender (Solidify) and Fusion 360 (Thicken, Shell) do this: every face is offset, and the offsets are joined at the corners.

**Read first:** brief 42 (the tool), `prism-ops/src/shelling.rs` and `thicken.rs` (a lone freeform face, and `offset_surface`), brief 48 part 5a (the live shell), NOTES "Live shell, part 5a". The owner's test file is `docs/acceptance/models/boat-half-hull.prism` (a half hull: three NURBS faces and two flat faces, an open sheet, two instances of one component that touch along the centre plane).

## Size
XL. Owner chose (2026-11-19) to build the smooth and creased joins together, with sharp mitres by default. Parts, each pushed and tested on its own:
1. **Offset a joined set of freeform faces (done).** Each face is offset (as a lone face is now); where two faces join smoothly (the same surface direction along the shared edge, within a tolerance) their offsets meet along the offset of the shared edge. The sheet becomes a slab with square rims. Refusals stay specific. Freeform joined to flat is not in this part (still refused); that is folded into part 2.
2. **Creases (inside corners done; outside corners still open).** Where joined faces meet at an angle (hull to flat face, patch to patch with a crease), the two offsets are trimmed to meet in a sharp mitre, found by marching the shared edge and solving where both offsets meet exactly at each point. An inside corner (the mitre trims back within each face's own edge) works now. An outside corner (the mitre would reach past a freeform face's own edge, which a NURBS surface cannot do without being refit bigger there) is refused by name for now; it needs its own small piece of work, maybe a small bridging face at the corner rather than an extended surface. Rounded corners are not in this part either.
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
- The half-hull file thickens inward and outward, valid, both rims square; with the centre-plane face left open it is flush to that plane. **Not yet reachable** (2026-11-19, part 2): two pre-existing issues found while testing this, both fixed (NOTES "Offset-fit issue", "A found-while-testing false 'too sharp' refusal") — a trimmed patch misreading a shared knuckle knot, and `corner` refusing a vertex where a crease fades to tangent. The hull itself now reaches, and is blocked only by, part 2b's own outside-corner limit (`EXTEND`), in both directions — the real remaining gap, not an accident.
- A refused case says why and changes nothing; the live feature runs again after a size change.

## MCP
Shell is a tool, so the MCP tools drive it as they do the others (select the faces, choose the open ones, give the thickness); the rim style and plane are `set_inspector` rows on the live feature (`feature.N.rim`).
