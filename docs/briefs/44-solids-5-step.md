# Solids & CAD part 5: STEP import and export (2026-11-17)

**Goal.** A part made in Prism exports as a STEP file that CAM programs (Fusion CAM and others) read as exact solids. A STEP file from another CAD program imports as exact faces. PLAN 1.13: "A round trip through STEP keeps NURBS surfaces and solids".

**Read first:**
- PLAN 1.13 (the formats table and the import rules);
- `prism-kernel/src/solid.rs` (`Region`, `Solid::build`, `Solid::region`);
- `prism-ops/src/bridge.rs` (`solid_of`, `region`, `put_back`);
- `prism-file/src/mesh/mod.rs` and `scene.rs` (export options, units, groups and components out and in);
- `prism-app/src/mesh_files.rs` (picking, saving, drops);
- arris-io 0.10.0 `step` module docs (what it writes and reads).

## The library
- **arris-io 0.10.0**, from the kernel we already use (arris, pinned at 0.10.0). MIT OR Apache-2.0, pure Rust, no clock.
- It writes and reads STEP AP214:
  - solids, and sheets as surface models;
  - planes, cylinders, cones, spheres, tori and NURBS;
  - assemblies with names and placements;
  - colours;
  - units.
- It reads files from other programs, heals small gaps, and refuses each solid it cannot take by name, without failing the file.
- Every call to it stays in `prism-kernel`, as for the rest of arris.
- AP242 files with B-rep solids use the same entities, so they read too. A mesh-only (tessellated) AP242 solid is refused by name.

## Size
XL: three parts, each pushed and tested on its own.

## Rules
- **What goes out is what you see.** Hidden objects are not exported (PLAN 1.14).
  - With a selection, only the selection is exported, as the mesh exports do.
- **Each closed solid is one STEP solid.** A group or component is one product, named as it is in Prism. A component used twice is one product placed twice. Loose closed bodies go out as products named "Body 1", "Body 2".
- **Open faces** (a sheet, not closed) go out as a surface model.
- **Colours go out and come back:** a face's material colour out, and a file's colours in as materials.
- **Units.** Export writes the model's unit (Feet and inches write inches). Import converts from the file's unit.
- **Refusals are counted, never silent.** After an export or import, the notice says what went and what did not, and why, e.g. "Exported 3 solids. Left out 1: an edge is approximate (see Model Health)".
- **An import is one undo step.**

## Parts
- **Part 5a. The bridge.**
  - Add arris-io (and arris-mesh, which it needs) to `docs/DEPENDENCIES.md`, and check that the web build still compiles.
  - `prism_kernel::step::write(parts, options) -> String` and `read(text, unit) -> Read`. They take and give `Region`s, with names, placements and colours, so nothing outside `prism-kernel` sees arris types.
  - Sheets: `Region` gains an open flag, so an open sheet can be built as an arris sheet body.
  - Tests round-trip every surface kind, alone and as an assembly, in mm and inches.
- **Part 5b. Export (File > Export > STEP).**
  - `exp.step` is built and renamed "STEP (machining, CAM)". IGES is left for later (Out of scope).
  - The model, or the selection, goes out as above: solids at their placements, components shared, names and colours.
  - Approximate edges (DECISIONS #32) and anything the kernel refuses are left out, counted in the notice.
  - Deterministic: the same model writes the same bytes. The header's time stamp is empty.
- **Part 5c. Import (File > Import, and dropping a `.step` or `.stp` file).**
  - Each solid comes in as a group, named from the file's product.
  - An assembly comes in as nested groups. A product placed more than once comes in as one component, placed each time.
  - Colours come in as materials, and the file's unit is converted (the unit is shown before the import is accepted, as for meshes).
  - Refused solids are counted in the notice by reason.
  - The geometry comes in exactly: planes, cylinders, cones, spheres and tori as Prism's own surfaces, NURBS as freeform faces, cut faces as cut faces. Lines, arcs and splines stay exact.

**MCP:** two new tools.
- `export_step` returns the STEP text of the model or the selection, with the notice.
- `import_step { text }` imports STEP text, as File > Import does.

Both replay through the command log, like the other imports.

## Out of scope
- IGES (a later row; few CAM tools need it now).
- AP242 product data beyond shape (tolerances, notes) and tessellated AP242.
- Editing a STEP file's history or features: STEP carries shape only.

## Tests
- **5a:** a box, a cylinder, a cone, a sphere, a torus, a lofted freeform solid and a cut freeform face go to STEP and back with the same faces, surfaces and volume. The same holds for inches, and for a two-part assembly with names, placements and colours.
- **5b:**
  - a model of two groups and a component used twice writes two products plus one shared one;
  - hidden objects and an approximate edge are left out and counted;
  - two writes are byte-identical;
  - golden file `docs/acceptance/` with a small part.
- **5c:**
  - the 5b file imports as the same groups, component, names, colours and volumes, in one undo step;
  - a hand-written file in inches with a refused faceted solid imports the rest and counts the refusal;
  - a drop of a `.stp` file imports.

## Acceptance
- [ ] A Prism part exports to STEP and reads back exactly; a STEP part from elsewhere imports as exact faces.
- [ ] STEP import and export tick themselves in Planned Features.
- [ ] PLAN, NOTES and HANDOFF are updated after each part.
