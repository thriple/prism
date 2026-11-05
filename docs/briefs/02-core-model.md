# Chunk 2: Core Model

**Goal.** The geometry core, with no window and no UI: vertices, edges, faces with holes, sticky geometry, overlap splitting, Push/Pull, transforms and undo. It must reproduce the browser prototype's behaviour (`docs/acceptance/golden-cases.json`), and be stronger than it where the prototype was a shortcut.

**Read first:** PLAN.md sections 1, 1.1, 1.9 and 4, and `docs/acceptance/core-model-acceptance.md`.

## Crates to create
`prism-math`, `prism-geom`, `prism-topo`, `prism-ops`, and the command and undo part of `prism-doc`.

## Requirements

**Data model** (a sketch; improve it if you can):
```rust
struct VertexId(u32); struct EdgeId(u32); struct FaceId(u32);   // stable, never reused

enum Curve   { Line }                    // Arc and Nurbs come later, but the enum exists now
enum Surface { Plane { origin: DVec3, normal: DVec3 } }   // others come later

struct Edge  { a: VertexId, b: VertexId, curve: Curve }
struct Face  { outer: Vec<VertexId>, holes: Vec<Vec<VertexId>>, surface: Surface }
```
- A planar face **owns its plane**, and its vertices are placed on it. After a rotation or a scale, vertices are re-projected onto the plane (or the face is split), instead of triangulating. The prototype triangulates non-planar faces as a shortcut; do not copy that.
- Non-manifold is allowed: loose edges, loose faces, more than two faces on an edge.
- Iteration order is deterministic (use ordered maps). IDs are assigned deterministically.

**Tolerance** (`prism-math`)
- One `Tol` value computed from the model's extent: about 1e-6 of the size, clamped. Everything asks it. No other epsilons in operations code.
- Orientation and side-of-plane tests use exact predicates (`robust` or equivalent).
- After any transform, run a weld step that merges vertices closer than `Tol`.

**Sticky geometry** (`prism-ops`)
- `insert_point`: reuse an existing vertex within tolerance, or split the edge the point lies on.
- `add_segment`: split at every existing vertex it passes through and at every crossing with an existing edge, then add the pieces that are new.
- **Face detection:** when edges are added, find every closed loop that contains a new edge and is an *atomic region*, meaning no edge touches the loop and passes through its interior. Make a face for each. **Use proper planar-graph tracing** (half-edges sorted by angle), not the prototype's depth-first search, which gives up beyond 14 vertices or 4000 steps.
- **Overlap split** (PLAN.md 1.9): coplanar faces never overlap. Overlaps become one face each, leftovers become their own faces. An existing face that new faces fully subdivide is removed.
- **Holes:** a face wholly inside another coplanar face, with no shared vertices, becomes a hole in it. The inner face stays separate. When a face is drawn around existing islands, the islands become its holes and are removed from the old host.
- **Duplicates:** identical faces, edges and vertices collapse to one.

**Operations**
- `push_pull(face, distance, loop: bool)`: stretches a face that closes a prism, extrudes otherwise, adds a base under a standalone face, handles faces with holes (walls for every loop), and with `loop = true` keeps the original boundary as a face and extrudes a new loop from it.
- `translate`, `rotate`, `scale` of a set of vertices, with the planar-face rule above.
- `remove` (erase): removing an edge removes the faces that use it. Removing an edge of a hole closes the hole.
- `duplicate` a selection, `connected(vertex)` returns everything reachable.

**Commands and undo** (`prism-doc`)
- Every edit is a command with an inverse (or a snapshot made cheap with persistent data). Undo and redo restore the model exactly (serialise both and compare).

## Out of scope
Rendering, UI, inference, groups and components, NURBS, file formats. Do not add them.

## Acceptance
- [ ] Every case in `docs/acceptance/golden-cases.json` with status "verified" passes through a test runner that reads the file. Cases with another status are listed and skipped, with a note in `NOTES.md` about what you decided.
- [ ] The invariants in `core-model-acceptance.md` hold, checked by property tests with random rectangles and lines.
- [ ] The golden cases give the same topology when every coordinate is scaled by 1e-4, 1 and 1e3.
- [ ] Undo and redo of every operation restore the model exactly.
- [ ] The crates have no dependency on `wgpu`, `winit` or the clock, and the web build still compiles.
- [ ] Faces never get triangulated to cope with rotation.

## Prototype pointers
`docs/prototype/README.md` lists where each function lives. The prototype's `Model` class is the reference for behaviour, not for structure.
