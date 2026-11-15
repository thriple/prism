# Brief 35: Export for Beverly: groups and components as named objects (owner request, 2026-11-15)

**Goal.** A model exported from Prism opens in Beverly (the owner's Bevy game editor) with its parts still separate and named: each group and component instance is its own named object in the GLB, nested as in Prism, and each component's geometry is stored once and shared. Today the GLB writer flattens everything into one mesh, which loses exactly what a game needs.

**Why (what Beverly does with it).**
- **Destructible props:** the owner models a wall whole (file A), then cuts it into chunks left in place (file B, one group per chunk). Beverly needs every chunk as its own object to simulate and bake the fall.
- **Characters:** a character's moving parts (legs, joints, turrets) must be separate objects whose own origin is the pivot they turn around.
- **Copies:** a component used 40 times should arrive as one shared mesh plus 40 placements, so the game can draw it as instances.

**Read first:** PLAN 1.3 (export presets) and the import/export formats table; `crates/prism-file/src/mesh/gltf.rs` (`write`, and the reader); `crates/prism-file/src/mesh/mod.rs` (`Mesh`, `to_mesh`); `crates/prism-doc/src/assembly.rs` (`flattened`, `flattened_visible`); brief 09 (files and formats) and the Groups & Components brief.

## Size
M: two parts. Part 1 is export; part 2 is reading the same structure back.

## Requirements (part 1: export)
- **One node per group and per component instance**, named after it: the group's name, or the component definition's name, with ` 2`, ` 3`, ... added when siblings would share a name. Nesting follows Prism's (a group inside a group is a child node). Loose geometry at the top level goes into one node named after the document.
- **Each node's transform is its group's or instance's own axes** (origin and orientation), and its mesh is in those local coordinates, so the object turns around the origin the owner set in Prism. If Prism has a way to set a group's axes, this is where it pays off; if not, note it for later.
- **Components share one glTF mesh** per definition; each instance is a node pointing at it (with its own transform, including mirroring).
- **Materials** as now: one primitive per material, base colour, roughness, metalness, texture and opacity.
- **Units and axes:** metres, Y up (glTF's convention). Hidden things follow the export scope (visible only by default).
- **Names are stable:** exporting the same document twice gives the same names in the same places, so Beverly can match pieces when a model is re-exported.
- **`extras` on each node:** `{"prism": {"kind": "group" | "component", "definition": "<component name>"}}`. Beverly ignores what it doesn't know; this leaves room to pass more later.
- **Preset:** the Export dialog gets a "Game engine (Beverly, Godot, Unity)" preset: GLB, Y up, metres, structure kept. A **Merge into one mesh** option keeps today's flattened output for targets that want it. Keeping the structure is valid glTF for every program, so it's the default for GLB.

## Requirements (part 2: import)
- Reading a GLB with nodes gives groups (one per node, named after it, nested, with its transform as the group's axes), and nodes sharing a mesh become instances of one component. A flat GLB still imports as today.
- A Prism → GLB → Prism round trip keeps names, nesting, axes, components and materials.

## Out of scope
- Skeletons, skinning and animation (Animation is tier 2).
- Beverly-specific settings in the file (collision, health, breakable). Beverly sets those itself.
- Beverly reading `.prism` files directly.

## Tests
- A document with two nested groups and a component used three times exports: four or more nodes with the right names and nesting, one shared mesh for the component, transforms that put every vertex where it was in Prism (compared with the flattened export's positions).
- Duplicate sibling names get ` 2`, ` 3`; two exports of one document give identical node names.
- A group's origin away from its geometry: the node's translation is the origin, and the mesh's local vertices are offset to match.
- Mirrored component instances keep their facing (no inside-out faces).
- Merge into one mesh gives today's output byte for byte.
- Round trip (part 2): names, nesting, axes, instance count and materials survive.

## Acceptance
- [ ] A wall cut into chunks exports from Prism and opens in Beverly with each chunk as its own named object (owner).
- [ ] A model with a component used many times opens in Beverly with one shared mesh (owner).
- [x] PLAN and NOTES are updated.
