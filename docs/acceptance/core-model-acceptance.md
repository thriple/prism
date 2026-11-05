# Core Model: acceptance

## Golden cases

`golden-cases.json` has 19 cases. Each one starts from an empty model (or the starting cube), applies a list of operations, and gives the expected vertex, edge and face counts, the faces by area, and the bounding box. Operations are `segment`, `rect` (four segments), `eraseEdge`, and `pushpull` (the face is the one containing `faceAt`).

The expected values come from the browser prototype. Cases marked "verified" were also checked by hand against PLAN.md section 1.9.

| Case | What it checks |
|---|---|
| triangle_makes_face | A closed loop of segments makes a face |
| rect_makes_face | A rectangle makes one face |
| line_across_face_splits | A line across a face splits it into two |
| identical_rect_twice | No duplicates |
| adjacent_rects_share_edge | Shared edges and vertices |
| partial_overlap_splits | Overlap, A-only and B-only: 3 faces |
| edge_cross_overlap | A rectangle crossing a side: 3 faces |
| cross_shape_overlap | Two rectangles crossing like a plus: 5 faces |
| rect_inside_makes_hole | A face inside a face becomes a hole in it |
| outer_rect_after_inner | The same, drawn in the other order |
| three_level_nesting | Two faces with holes and one plain face |
| erase_hole_edge_closes_hole | Erasing a hole's edge removes the inner face and closes the hole |
| pushpull_hole_inner | Extruding the inner face; the host keeps its hole |
| pushpull_ring_face | Extruding a face that has a hole (**needs review**) |
| pushpull_standalone_rect | A standalone face pulls into a closed box |
| pushpull_cube_top_stretches | Stretching a prism |
| pushpull_cube_top_alt_loop | Alt: the original boundary stays, a new loop extrudes |
| scale_mm_partial_overlap | The overlap case at 0.4 mm |
| scale_km_partial_overlap | The overlap case at 4 km |

## Rules every model must satisfy (property tests)

1. **No overlaps.** No two coplanar faces have interiors that overlap.
2. **No duplicate points.** No two vertices are closer than the tolerance. No two edges join the same pair. No two faces have the same loop.
3. **Closed bookkeeping.** Every edge of every face loop (outer and holes) exists in the edge set. Every vertex is used by an edge or is loose by design.
4. **Holes are inside.** Every hole loop lies strictly inside its outer loop, and holes do not touch each other or the outer loop.
5. **Area is conserved.** In any plane, overlap operations do not change the total covered area.
6. **Idempotent.** Adding a segment that already exists, or a rectangle that already exists, changes nothing.
7. **Order independent.** Drawing the same rectangles in a different order gives the same faces by area.
8. **Scale independent.** Scaling every coordinate by 1e-4, 1 or 1e3 gives the same topology.
9. **Undo is exact.** Undo then redo of any operation gives a model that serialises identically.
10. **Deterministic.** The same operations give the same IDs and the same serialisation, every run.
11. **Planar faces stay planar.** After rotating or scaling, every face's vertices are within tolerance of its plane, and no face was triangulated to make that true.

## Known prototype limits (do not reproduce)
- Face detection is a depth-first search that stops at 14 vertices and 4000 steps.
- Non-planar faces are triangulated by ear clipping instead of being kept planar.
- One fixed set of epsilons for orientation and the like, not exact predicates.
- Holes are only attached for a face wholly inside another (no touching islands).
- No groups: geometry is one flat model.
