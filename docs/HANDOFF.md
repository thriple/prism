# Handoff for the next Claude Code session

Written 2026-11-06, updated 2026-11-09 and 2026-11-10 (Guam). Read `CLAUDE.md` first, as always. This file is the short version of where things stand and how the owner likes to work.

## Start here (2026-11-12, Guam)
- **State:** `main` = session branch, clean. Basic Materials part 1
  (colours) is done and released. Since then, owner feedback, all
  released: Follow Me works as in SketchUp (press on a face, drag along
  the joined edges; it stops where the inference engine snaps on the
  path; a face of a body swept out grows the body); the Paint Bucket
  opens Window > Materials; Push/Pull, Follow Me and Scale use the
  inference engine (edges, arcs, crossings, guides, points lined up).
  NOTES: "Inference in every tool that places a point" and "Owner
  feedback on release 193".
- **CI:** the Linux job skips the Mesa install until the owner says
  otherwise (screenshot tests skip there unless `PRISM_REQUIRE_GPU` is
  set); they run locally before each push. Recent runs were green.
- **Latest (2026-11-12, after the Fillet window), all pushed:** face
  style buttons (Wireframe, Base Colour, Colour/Material beside X-ray;
  NOTES "Face style buttons"), sliders smooth instead of only their ends
  (NOTES "Sliders only gave their ends"), Planned Features' behaviours
  list brought up to date. CI was green for all of them.
- **Release notes:** `packaging/release-notes.md` is the text every
  release shows; rewrite its top for each `[release]` (it was stale from
  the Fillet window until 2026-11-13).
- **Planned Features window:** actions tick themselves when built; the
  "Behaviours and feel" list is ticked by hand in `BUILT_BEHAVIOURS`
  (`registry.rs`). Owner: keep it up to date with each step.
- **The action table** (`actions/table.rs`) is edited by hand now; the
  import script no longer reproduces it, so do not rerun it.
- **The Fillet window is built** (brief 29, released): a right-column
  window with Fillet / Chamfer and the size, and a live preview on the
  hovered edge. NOTES: "The Fillet window".
- **Basic Materials is done and released** (parts 1 to 3; NOTES "Basic
  Materials (part ...)"). **The NURBS Spike is done** (brief 30): the
  kernel is **arris** (`docs/research/nurbs-spike.md`, DECISIONS #9; the
  test program is `spikes/nurbs/`, run with `cargo run --release`).
  **Freeform NURBS is done** (brief 31, 6 parts; NOTES "Freeform NURBS
  (part ...)" says what each part built, and "what the whole chunk left
  for later"). `Curve`, `Surface` and `Edge` are no longer `Copy`. The
  kernel is in `prism-kernel` (arris, pinned).
- **The order changed on 2026-11-13** (owner request, PLAN 6.0 "Order"):
  **Everyday Commands is done** (brief 33, 4 parts, released): clipboard,
  Lock and With Context; the camera; the Rotated Rectangle, Freehand,
  Preferences, About and the Tool Hint Panel; Smart Guides (NOTES
  "Everyday Commands (part 1)" to "(part 4)"). **The Loft Tool is done**
  (brief 34, released; NOTES "Loft Tool (brief 34)"). NOTES "Known gaps
  after the Loft Tool" lists what is left over across the program. The
  owner asked for those gaps to be closed before Fillet Joins, in this
  order: curves, cage editing, camera and clipboard, smaller items. **All
  done** (NOTES "Closing gaps: curves", "... cage editing", "... camera
  and clipboard", "... smaller items"); Walk collision and the web
  clipboard are not needed for now (owner). Smart Guides in Scale (equal
  sizes) and Rotate (matching angles) followed (NOTES "Closing gaps:
  Smart Guides in Scale and Rotate"). **Fillet Joins is under way**
  (brief 32; DECISIONS #37 to #39's defaults until the owner answers):
  steps A (the kernel bridge) and B (`RoundEdges`) are built (NOTES
  "Fillet Joins step A" and "step B"). **Next: step C**, picking in the
  tool (double-click a face, selections, whole lines); then Solids & CAD, which was moved up to follow
  it. The owner also floated a chat window in Prism (NOTES "Owner idea
  for later"): not scheduled.
- **Disk:** `target/debug` grows past the session's allowance after many
  builds. Build with `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` (no
  debug info: a full build and test run then takes about 3 GB, not 20);
  if space runs out, delete `target/debug` and rebuild.
- **Keep Rust current:** CI uses the newest stable Rust, and its linter
  gains rules. Run `rustup update stable` at the start of a session, or a
  commit that is clean here can fail CI's clippy (it did, 2026-11-12:
  parts 4 to 6 of Freeform NURBS).
- **Fillet Joins** (fillet many edges, clean corners, a face's edges at
  once, whole lines): owner request, moved after Freeform NURBS by the
  owner. NOTES: "Fillet on many edges".
- **Owner:** testing the latest release. Standing asks: every tool needs
  a logical way of use, and every tool that places a point uses the
  inference engine.
- **Checking the app by eye:** `Xvfb :77`, run `target/debug/prism-app`
  with a file, drive it with `xdotool`, grab with `import -window root`
  (MCP's screenshot shows the viewport only). Run `xdotool windowfocus`
  on the window first, or text fields will not take typing.
- **After Basic Materials: NURBS Spike, then Freeform NURBS** (owner
  moved them up, 2026-11-11).
- **Owner's answers (2026-11-12):** the Fillet panel comes after part 1
  of Basic Materials; new loop keeps lasting one pull; no illustrated
  Push/Pull page; no bundled textures (DECISIONS #34). #35 and #36 still
  use their defaults.
- **Owner's recordings:** Help > Record Session JSON files. Each saves the
  real final model (`result`): compare it with `Session::replay` (via
  `Recording::ends_with`) before trusting a replay. A replay can differ
  when state at the recording's start was not saved (as new loop was).
- **Materials code:** `prism_doc::material` (Material, Principled,
  FaceSide, paint, connected_same, sides_with), `Assembly` (materials,
  painted instances, replace, purge), `Face.paint` (`prism_topo::Paint`),
  `regions::inherited` (merged faces take the largest piece's paint),
  schema 8 (`materials.json`, geometry layout 7).
- **Part 6 tools to know:** `cut`, `crossing`, `shell`, `meet` (where a
  moving face first meets another), `radius.rs` (new sizes for every
  turned surface), `prism_view::outline`, `approx::facet`. Renderer:
  `CURVED_OWN` lines are depth-tested against their own curved face.

## Where the project is
Updated 2026-11-09 (Guam), before a context compaction.
- **Built, in order:** Skeleton, Soft Theme, Actions & Keys, Core Model,
  Viewport, Inference & Drawing, Edit Tools (3 parts), Files & Formats
  (3 parts), Groups & Components (3 parts), Model Health (2 parts), View
  Settings (2 parts, briefs 17 and 18), Toolbars (2 parts, brief 19), Updates (brief 20), MCP Server (2 parts, brief 21). PLAN
  6.0 has the details; NOTES has a section per part.
- **Since the last compaction (2026-11-08 to 09), owner feedback fixed:**
  - the plan was put in build order, with a standing rule to follow it;
  - Move keeps a neighbour's straight edge straight;
  - Offset goes through a snapped point;
  - the group box follows Move and is padded when a group is open;
  - double- and triple-click work inside groups, and Push/Pull and Offset
    ignore closed groups;
  - the side windows dock in a right-hand column;
  - a face never hides a line in its own plane (general fix for
    z-fighting corner lines);
  - `Command::Duplicate` was removed (it made CI hang);
  - Updates and MCP Server were added to the plan, right after View
    Settings.
- **State:** `main` = session branch, clean, last code push 3170270
  (the Scale box). CI and releases green on both branches through it
  (the branch's Release run is cancelled when `main` gets the same
  commit; `main`'s runs). GitHub release downloads gave 504s in the Web
  build on 2026-11-09 (not our code): re-run; if it repeats, make the
  download steps retry (`curl --fail --retry`).
- **Owner bug fixes, 2026-11-09 evening (NOTES has a section each):**
  - session 5 wedge: an edge slides only when its neighbour faces away
    from the pulled face; a face with free sides drawn against a solid
    keeps its base; retracing a side of a flat loop fills it;
  - undo property test finds: an arc drawn within 10 tolerances of an
    existing circle snaps onto it; loops drop a point repeated next to
    itself; a move that would bend a face with holes is refused
    (`HOLE_FACE_WOULD_FOLD`); 9000 random undo cases clean after;
  - **the Scale tool** (owner's request): a dashed box 5% off the
    selection with grips (corners even, edges two axes, faces one), no
    inference, Alt (tap) about the centre, Shift switches face/edge
    grips; `session/scalebox.rs`. PLAN's Scale row updated. Waiting on
    the owner to try it by hand. Later (owner): scaling from two picked
    points with inference.
  - The owner compared our shape rules with SketchUp's (faces, sticky
    geometry, healing, two-sided faces): they match; kept on purpose:
    Push/Pull booleans (#31), lines between coplanar faces stay after a
    pull, a pulled ring keeps no base (#14).
- **Owner's latest instruction:** "Let's fix bugs" before more features;
  ask whether to go on with Exact Curves 5b or more bug fixes.
- **Also fixed 2026-11-09:** View Settings moved to the Window menu;
  hidden-geometry ghosts are edges only, drawn like hidden lines.
- **Exact Curves: parts 1 to 4 are built** (briefs 22 to 25; NOTES
  has a section per step). Part 4's steps
  4a–4d are all built (`Surface::Cylinder`, `plane()` is an
  `Option`; Push/Pull makes cylinders, shaded and picked; soft and smooth
  edges, file schema 5; Push/Pull on a side changes the radius,
  `prism_ops::radius`). **Part 5 is under way** (brief 26, steps
  5a–5e): 5a is built (`prism_ops::round`, `Command::RoundEdge`, the
  Fillet / Chamfer tool `tool.fillet`, greyed). 5b is built (2026-11-10:
  `Cone`, `Sphere`, `Torus` in `prism-geom/src/revolved.rs`, all turned
  surfaces described by `Revolution` in (θ, s), `Model::param_loop`,
  exact volumes, file layout 5 / schema 6, grid shading
  `prism_view::tess::turned`, picking onto the surface, MCP; NOTES has
  the details and gaps). 5c is built (2026-11-11: `prism_ops::follow`,
  `Command::FollowMe`, the tool `tool.followme`, greyed; NOTES has its
  sorted gaps). 5d is built (2026-11-11: paths of lines and arcs, turned
  about each arc's axis; lathes make cones, spheres and tori; Alt follows
  the selected face's edge; NOTES has its sorted gaps): **part 5 is
  done**. Then **part 6** (owner, 2026-11-11, DECISIONS
  #33; brief `docs/briefs/27-exact-curves-6.md`, written 2026-11-11; **6a is built**, `prism_ops::approx`, `Edge.approx`, schema 7; **6b-1 is built**, `prism_ops::cut` (curved faces split in (θ, s)) and `prism_ops::crossing` (crossings traced in (θ, s), exact lines and arcs else fine segments), Intersect Faces with curved faces; **6b-2 is built**, drawing and Follow Me stick to curved faces (`cut::split_along`, `follow::stick`), crossings meeting mid-face are split, `approx::facet` facets any cut face; **6c-1 is built**, `prism_ops::shell` (the inside test with exact arcs and curved faces) and Push/Pull booleans with curved faces; **6c-2 is built**, `prism_ops::meet` (stops at curved faces for a pull and a new radius) and `follow::OVERLAPS`; **6d is built**, new sizes for cones, spheres and tori in `radius.rs` and outline lines in `prism_view::outline`; **6e is built**, the curve tools are active, so Exact Curves is done): the
  segment fallback (#32), exact plane crossings with curved faces,
  Intersect Faces, sticky crossings, Push/Pull booleans and stops with
  curved faces, a new size for cones/spheres/tori, then the curve tools
  become active (6e, moved from 5e). **Standing rule (#33):** at the end of
  every chunk, sort its known gaps; each serious one gets a step in the
  plan, not only a NOTES line. The
  DECISIONS #32 fallback is step 6a.
  - Part 4 lessons: `Face::new` links edges by corners, which is wrong
    when two arcs join the same two points: make faces with
    `Face::with_edges`. Keep edge creation order in Push/Pull (recorded
    sessions compare edge ids). The test `Driver::look` starts a new
    session (no undo history), so set the camera first. The file is
    postcard: a new field needs a new geometry layout and schema
    (schema-N copies of the golden files, `PRISM_WRITE_GOLDEN=1`).
  - Known gaps (NOTES 4a–4d): a new radius is not stopped by faces in
    its way; Push/Pull into a solid does not cut it when
    curved faces are involved (`Swept`/`keep_boundary` skipped);
    `reach::limit` meets curved faces only by corners and edges;
    tessellation is a fixed 128 steps per turn; no profile lines along a
    cylinder's outline. (Intersect Faces cuts curved faces since 6b-1.)
  - The curve tools (`GREYED_TOOLS` in `prism-tools/src/session/shapes.rs`:
    Circle, Polygon, `tool.arc`, `arc.2`, `arc.3`, `arc.c`, `arc.p`) are
    built but greyed in menus; only MCP `use_tool` reaches them.
  - Push/Pull refuses pulls that would bend a curved face, and a new
    radius that would bend a neighbour (`CURVED_PULL`) or reach zero
    (`RADIUS_GONE`). `face_at` goes by corners (misses a
    lone disc). A lone face pushed back now makes a box (it used to leave
    one face): the owner should try it.
  - Not yet checked by eye in the app: cylinders from Push/Pull and a new
    radius, soft seams, Eraser Ctrl/Ctrl+Shift, Edit > Soften/Smooth, arc snaps,
    tangents, the Arc tools,
    fillets, refusal hints. Briefs 23 and 24 ask the owner to try them over
    MCP in a build.
- **Exact Curves conventions:** crossings go through
  `sticky::meet` / `on_curve_inside` (exact for lines and arcs), never the
  straight-only `crossing` alone; `Curve::Arc` runs counter-clockwise
  about its axis from the edge's `a` to `b`; use `add_curve_edge` (never
  `add_edge`) for arcs; edit face loops with `edit_face` or `Face::reverse`
  so edges stay with their sides; polygons for faces come from
  `loop_points`, not the vertices; arcs are added with `add_arc` /
  `add_circle` (they cut and are cut like segments); a loop as lines and
  arcs comes from `face_pieces` (private, in `prism-ops/src/offset.rs`); refusals because of arcs give
  their reason through `Command::refusal`. Exact Curves is 5 parts (owner, 2026-11-09); the design
  is in `docs/research/exact-curves.md`. Curves that are not lines or arcs
  fall back to fine segments, flagged (DECISIONS #32), built as step 6a.
  The curve tools stay greyed in releases until all five parts are done
  (owner, 2026-11-09).
- **Also done 2026-11-09:** Push/Pull stops at the first face in its way
  (DECISIONS #31 changed); Ctrl/Alt/Option taps reach the tools (they never
  did); the web wheel sorts scrolls by gesture. The owner should try all
  three by hand.
- **MCP Server** waits on the owner connecting Claude Code to it.
- **Window title** names the build ("Prism - Build N"), so the owner can see
  an update took. Updates still needs a real try (install a build, push,
  Check for Updates).
- **Waiting on the owner to try by hand (2026-11-09):** a lone face
  pushed back (now a box), cylinders and soft edges over MCP, Push/Pull
  stopping at faces, Option/Alt/Ctrl taps, the web wheel, Toolbars part 2, ghosts,
  Updates, MCP Server, and the curve tools over MCP (Exact Curves 2–3).
- **Disk in cloud sessions:** the build cache can fill the session's disk
  (a linker bus error, or "No space left on device"). Delete
  `target/debug` and build with `CARGO_INCREMENTAL=0` (about 5 GB then,
  against 27 GB with stale copies).
- **Work on `main`** (owner, 2026-11-10); keep the session branch level
  with it.
- **MCP spec:** 2026-07-28 is the newest release (stateless, no
  `initialize`); the server speaks it and the older handshake. The spec
  site is blocked here; the spec repository on GitHub is reachable
  (`git clone --sparse`).
- **Updates** needs a real try: install a release build, push something,
  then Help > Check for Updates once the new release is out.
- **Toolbars, as the owner shaped them:** no name labels (the name is a
  tooltip over a toolbar's empty space, the kit's tooltip), faint dividers
  between toolbars; 1 button thick up to 5 tools, 2 from 6; a dock is as
  thick as its thickest toolbar; the top dock takes no space when empty;
  floating toolbars are tight. Floating ones reshape by their right or
  bottom edge, and keep that shape docked (turned along the dock).
- **Waiting on the owner to try by hand:** the toolbars, View Settings
  (styles, X-ray, the sun and shadows, Show Hidden), the side windows,
  groups and components, Model Health, and the Push/Pull cases. The web
  build has not been run since the face-plane render target was added
  (WebGL2 should allow it; installing trunk here was blocked).
- **MCP line in every brief** (owner, 2026-11-09; PLAN 6.0a): a chunk
  that adds something an assistant should read or set beyond actions and
  tool clicks extends the MCP tools in the same chunk, with a test.
- **Owner's standing instructions:** push tested work to `main` (everyone
  on one branch); break up XL chunks into parts; split any part, or
  research first, when not prepared enough to build it in one go;
  double-check work before pushing.
- **Standing rule (owner, 2026-11-08): follow the plan in order.** Build
  the chunks in the order of the PLAN 6.0 table, top to bottom: the next
  chunk is the first row not yet started. Do not ask which chunk is next.
  Only the owner changes the order (Toolbars was the owner's own request,
  put before Updates).
- **Checking UI here:** the app runs under Xvfb with Mesa
  (`apt-get install -y mesa-vulkan-drivers xdotool libxkbcommon-x11-0`),
  driven by `xdotool` and captured with `import -window root`. Set
  `XDG_CONFIG_HOME` to a scratch folder so saved settings stay out of the
  way. The app redraws only on change: move the mouse a little before a
  capture.
- **GPU tests here:** install Mesa's software Vulkan driver first
  (`apt-get install -y mesa-vulkan-drivers`), then run with
  `PRISM_REQUIRE_GPU=1`; without it they skip silently.
- **Platform code can be checked here:** `cargo clippy -p prism-app --target aarch64-apple-darwin` and `--target x86_64-pc-windows-msvc` both run in this container, without a Mac or a Windows machine.
- **Risks ahead:** `docs/research/risks.md` (checked 2026-11-06).
- **Chunks ahead:** `docs/research/chunks-ahead.md` (2026-11-11): Follow Me,
  Exact Curves part 6 (how curved booleans work), Basic Materials,
  Annotation & Sections, Inspector & Features; four decisions wait on the
  owner there. Read it before writing those briefs.

## How the owner works
- **Times:** the owner is in Guam (UTC+10). Give times in Guam time.
- **Parts:** every chunk is built in parts that each fit one session (PLAN 6.0a, which has the proposed splits for the XL chunks).
- **Order of work:** brief, then build, then tests, then NOTES and PLAN 6.0, then push to `main`, then check CI and tell the owner.
  - Large chunks are split into parts, each with its own brief.
  - Unless the owner says otherwise, follow the order in PLAN 6.0 ("Suggested order").
- **Language:** plain English in commits, docs and messages; short.
- **Feedback:** the owner tests releases and the web page, then sends feedback in batches. Each answer becomes a DECISIONS row; feedback rounds so far are #16–#26.
- **Releases:** only a push to `main` whose commit message contains `[release]` builds a release (`latest`, plus a numbered build), whatever CI says; it can also be run by hand (Actions tab, Release, Run workflow). Use `[release]` when a feature is finished or for a fix the owner should try, not for work in progress (owner, 2026-11-11). Pushes to the session branch no longer build releases. The web page https://thriple.github.io/prism/ updates only when CI is fully green.
  - So do not push untested app code to `main`.
  - Work in progress goes on the session's `claude/…` branch first, as was done this session.
- **"Full auto":** the owner has asked for it before, overnight ("try not to chase too many rabbits"). Keep to the brief and report in the morning.
- **Off limits:** do not edit `CLAUDE.md` (it was refused before). Suggest rule changes to the owner instead.

## Standing rules learned the hard way
- **DECISIONS #20 (greyed until built):** when something is built, even partly, make it active in the same change. Tools come from `prism_tools::session::TOOLS`; other actions from `run_of` in `crates/prism-app/src/actions/registry.rs`.
- **Menus:** the menu and action tables are generated by `node scripts/import_prototype.js` from the prototype. Add Rust-only actions in that script, never by editing `table.rs` by hand. Run `cargo fmt` after.
- **JSON:** the workspace `serde_json` has `float_roundtrip` on. Without it, about 11% of coordinates come back one bit off. Keep it on.
- **New core crates** go in the purity test, `crates/prism-ops/tests/purity.rs`.
- **Tests that only run natively** start with `#![cfg(not(target_arch = "wasm32"))]`: proptest does not build for the web, and CI runs clippy for the web target with `--all-targets`.
- **Recorded sessions** (`docs/acceptance/sessions/*.json`) replay in CI. To write a new one, run its test with `PRISM_WRITE_SESSIONS=1`. Golden `.prism` files are rewritten with `PRISM_WRITE_GOLDEN=1`, but only when the format changes on purpose, with a new schema.

## Environment gotchas from this session
- **Disk:** the build folder reaches about 27 GB and fills the session's disk allowance. The symptom is the linker dying with "Bus error". Run `cargo clean`; it is only build output.
- **GitHub:** once refused pushes to `main` for a while ("fatal error in commit_refs"), while the network proxy was fine. Retry later, or push to the session branch meanwhile.
- **Web build checks:** headless Chromium with Playwright works for the web build:
  - `trunk build --release`;
  - serve `dist/` with `python3 -m http.server`;
  - launch with `--use-angle=swiftshader --enable-unsafe-swiftshader`.
  File pickers cannot be driven there.
- **GPU tests:** they run with Mesa's software renderer in CI (`PRISM_REQUIRE_GPU=1`), and locally too.

## Waiting on the owner
- **Try by hand (older):**
  - native Save, Open and Save As dialogs on Windows and macOS;
  - "Save changes?" when closing the window;
  - recovery after killing the app;
  - saving in place in Chrome;
  - double-clicking a `.prism` file on the Mac, and "Open With > Prism" on
    Windows, while Prism runs.
- **Try by hand (2026-11-08):** see "Where the project is" above.
