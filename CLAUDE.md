# Prism: instructions for Claude Code

Prism is a 3D modeling program. It combines SketchUp's tool feel, exact curved geometry, and later a path-traced renderer and simulations. The tool **feel** comes before everything else.

Read these first, in this order:
1. `PLAN.md` (the full plan; sections 1, 3.1, 4, 5 and 6.0 matter most at the start)
2. `docs/DECISIONS.md` (open decisions, and the defaults to use until they are made)
3. The brief for the chunk you were asked to build, in `docs/briefs/`
4. `docs/prototype/README.md` (a browser prototype that shows the intended behaviour)

## How to work

1. **One chunk at a time.** Build only the chunk you were asked for, as described in its brief. If you notice something outside the brief, add a line to `NOTES.md` and carry on. Do not start the next chunk.
2. **Say what you did in plain English.** Keep commit messages, docs and comments short and clear.
3. **Ask before anything destructive:** deleting existing files, force pushes, rewriting history.
4. **When a decision is open,** use the default in `docs/DECISIONS.md`, note that you did, and keep going. Do not invent a new decision silently.
5. **Finish a chunk properly:** tests pass, CI is green, the "Where we are" column in `PLAN.md` section 6.0 is updated, and `NOTES.md` lists anything left over.

## Rules that apply everywhere

- **Pure, deterministic logic.** The crates for math, geometry, topology, operations, inference and tools never read the clock, the GPU, the network or an unseeded random source. The same inputs give the same outputs, so scenarios can be replayed in tests.
- **Geometry is f64.** Rendering is f32 relative to the eye (section 4).
- **Tolerance is relative, and lives in one place** (`prism-math`). It is a small fraction of the model's size (section 1.9), not a fixed number. Do not scatter epsilons through the code. Orientation and side-of-plane tests use exact predicates (the `robust` crate or equivalent).
- **Every edit is a command in a transaction,** so undo, redo, autosave and test replay all work from the same log.
- **One action registry** drives menus, the dock, context menus, hotkeys and command search (section 1.6). A test must fail if any action has no menu path.
- **Hidden must be free** (section 1.14): hidden objects cost memory and nothing else, in every system.
- **Depth precision** (section 4): reversed-Z, near and far fitted to the visible scene, no fixed line bias.
- **Licences.** Use only dependencies under MIT, Apache-2.0, BSD, zlib or MPL by default. Do not add GPL or LGPL code, and never copy code from Blender or SketchUp. Before adding a dependency, add a row to `docs/DEPENDENCIES.md` with its licence and whether it builds for WASM.
- **Platforms.** Windows and macOS native builds are the main target. The web build must compile at every commit. No WASM threads yet (see Risks in section 7).
- **Tests come with every change:** unit tests, the golden cases in `docs/acceptance/`, and property tests for geometry. CI must pass `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and the web build.
- **Look of the UI:** dark "Soft" style, IBM Plex Sans and IBM Plex Mono, as in the prototype. The viewport is drawn by our own renderer, not by egui.

## Layout (from PLAN.md section 4)

```
crates/
  prism-math    vectors, transforms, exact predicates, tolerance, units
  prism-geom    curves and surfaces (lines and planes first)
  prism-topo    vertices, edges, loops, faces with holes, stable IDs
  prism-ops     sticky geometry, overlap split, push/pull, transforms
  prism-infer   the inference engine
  prism-doc     document, groups, components, undo, serialization
  prism-render  wgpu renderer
  prism-tools   tool state machines
  prism-app     winit + egui shell, actions, menus, keys
docs/
  briefs/       one brief per work chunk
  acceptance/   golden test cases
  prototype/    the browser prototype, for reference only
```
Create a crate only when a chunk needs it.
