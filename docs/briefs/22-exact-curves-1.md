# Exact Curves, part 1: loops list their edges

**Goal.** Each face loop names the edges between its vertices, so that later parts can join two vertices by more than one edge (a half circle and its diameter). No visible change: every existing test passes as it is.

**Read first:** `docs/research/exact-curves.md`; DECISIONS #9 and #32; `crates/prism-topo/src/lib.rs`; `crates/prism-file/src/geometry.rs` and `migrate.rs`.

## Parts of Exact Curves (owner, 2026-11-09: 5 parts)
1. **Loops list their edges** (this brief).
2. Arcs and circles in the core, with the Circle and Polygon tools.
3. The Arc tools, tangent inference, Offset with arcs, corner fillets.
4. Curved faces: cylinders from Push/Pull, shading and picking, soften and smooth.
5. Follow Me along lines and arcs; fillet and chamfer on planar edges.

**Greyed until done (owner, 2026-11-09):** the curve tools (Circle, Polygon, the Arc tools, Follow Me, fillet, chamfer, soften and smooth) stay greyed in menus, toolbars and releases until all five parts are finished, even where a part has built one. Each part is tried through tests, the MCP server and builds made by hand.

## Requirements
- **Topology (`prism-topo`):**
  - A face keeps its vertex loops (`outer`, `holes`) and gains the matching edges: edge `i` of a loop joins vertex `i` to vertex `i + 1`.
  - The model keeps them in step: adding a face finds its edges (it may be given them); splitting an edge puts both halves into every loop that used it; editing a face's loops goes through model methods that relink it.
  - `loop_edges(face)` (and friends) give a loop's edges with their direction. Code that walked a loop and looked up `edge_between(a, b)` uses them instead.
  - A model check: every loop's edges exist and join its vertices in order. It runs wherever the model checks run (tests, Model Health's bookkeeping check).
- **File:** geometry layout 2 stores each loop's edges. Layout 1 files are read by finding the edges, as now (a migration). Files written by this version open in this version only (the schema number goes up).
- **MCP:** no change (`get_geometry` already lists edges and faces).

## Out of scope
Arcs, new curve types, any new tool or visible change (parts 2 to 5).

## Tests
- All existing tests pass unchanged.
- Topology: a face added from vertices gets the right edges; splitting an edge relinks every face using it; removing an edge drops faces that used it, as now; the new check finds a loop whose edges are out of step.
- File: a model round-trips through layout 2; a layout 1 file (from the test fixtures) opens with the same faces and edges.
- The property tests also run the new check after every random operation.

## Acceptance
- [ ] No visible change, all tests and CI green.
- [ ] PLAN and NOTES are updated.
