# Brief 55: Fillet / Chamfer handles and corners

Status: **in progress** (2026-11-26). Owner: "actual handles on the chamfer/fillet. Simply dragging on the highlight is not working out." / "A proper widget with handles, highlight. There are plenty of times the preview disappears on me." / "All you really need for a chamfer or fillet is two faces minimum." Task 100. Decisions: #44 (rule order), #45 (warn, don't refuse), #46 (corners).

## Done

1. **Edges the tool refused that it should round** (commit `c0176c6`):
   - lines drawn across the faces beside an edge, and points where it was split, no longer stop a round anywhere along it (owner's session 24: seven edges, refused at every size, now fillet together);
   - shallow edges take small sizes (the kernel refuses them; Prism's own round makes them; owner's Xby4);
   - edges not on a closed solid round with the two faces beside them (straight edges between flat faces, not two that meet);
   - a line on a flat face says there is no corner there.
2. **The size handle on collected edges:**
   - the collected edges are drawn highlighted (selected colour, thicker);
   - a square handle sits across the face beside the longest of them (the face facing you most), at the round's size from the middle of the edge, joined to it by a dashed line;
   - by it, the kind and size ("Fillet 0.2 m"), in orange with the reason when that size cannot be made;
   - dragging it sizes the round shown (the last size that worked stays if one cannot be made, with the reason at the cursor); letting go makes nothing; Enter makes it. A press on the edge itself still drags from the edge (a small size puts the handle close to it).
   - After Enter, the round is a feature with its own square handle (as before).

## Next

3. **Kind 2 for rounds (#45):** a size past the limit builds the largest that works and draws what stopped it in orange, instead of keeping the last size reached.
4. **Partly (#45 kind 4):** of several edges, round those that can be and mark the rest.
5. **Corner patches (#46):** where rounded edges meet at a corner the kernel cannot close, Prism's own n-sided patch, else a corner fitted within the precision. No corner in the owner's files fails after part 1, so this waits for a failing case (or is built against made-up corners: three rounds of different sizes, four edges at a corner).
