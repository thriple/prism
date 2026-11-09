# Handoff for the next Claude Code session

Written 2026-11-06, updated 2026-11-09 (Guam). Read `CLAUDE.md` first, as always. This file is the short version of where things stand and how the owner likes to work.

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
- **State:** `main` = session branch. Last push: Exact Curves 2b
  (crossings). Check its CI first (all jobs, Windows included).
- **Also fixed 2026-11-09:** View Settings moved to the Window menu;
  hidden-geometry ghosts are edges only, drawn like hidden lines.
- **Exact Curves part 1 is built** (brief 22: faces name their edges, file
  schema 3 / geometry layout 2; NOTES has what is left for part 2).
- **Exact Curves part 2** is briefed (brief 23, four steps). Steps 2a
  (arcs in the core) and 2b (exact crossings) are built. **Next: step 2c**
  (picking, inference, Eraser, Model Health). Where to start for 2c:
  - picking and hover already use drawn points (`prism-view/src/pick.rs`,
    `prism-tools/src/select.rs`); selecting an arc should take every edge
    with its `curve_id`, and the Eraser the whole curve;
  - inference is `prism-infer` (`Kind::Midpoint`, `Kind::OnEdge`): add arc
    ends, centre (while hovering the arc), arc midpoints, circle quadrants,
    and "on arc" with `Curve::closest_point`;
  - Model Health: list arcs whose ends left their circle, and repair by
    moving the ends back (`circle_distance`);
  - tools should say why a move was refused (an arc would bend, or a face
    with arcs would fold); brief 23 says 2c/2d.
- **Exact Curves conventions (2a, 2b):** crossings go through
  `sticky::meet` / `on_curve_inside` (exact for lines and arcs), never the
  straight-only `crossing` alone; `Curve::Arc` runs counter-clockwise
  about its axis from the edge's `a` to `b`; use `add_curve_edge` (never
  `add_edge`) for arcs; edit face loops with `edit_face` or `Face::reverse`
  so edges stay with their sides; polygons for faces come from
  `loop_points`, not the vertices. Exact Curves is 5 parts (owner, 2026-11-09); the design
  is in `docs/research/exact-curves.md`. Curves that are not lines or arcs
  fall back to fine segments, flagged (DECISIONS #32), built after part 5.
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
- **Waiting on the owner to try by hand (2026-11-09):** Push/Pull stopping
  at faces, Option/Alt/Ctrl taps, the web wheel, Toolbars part 2, ghosts,
  Updates, MCP Server.
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

## How the owner works
- **Times:** the owner is in Guam (UTC+10). Give times in Guam time.
- **Parts:** every chunk is built in parts that each fit one session (PLAN 6.0a, which has the proposed splits for the XL chunks).
- **Order of work:** brief, then build, then tests, then NOTES and PLAN 6.0, then push to `main`, then check CI and tell the owner.
  - Large chunks are split into parts, each with its own brief.
  - Unless the owner says otherwise, follow the order in PLAN 6.0 ("Suggested order").
- **Language:** plain English in commits, docs and messages; short.
- **Feedback:** the owner tests releases and the web page, then sends feedback in batches. Each answer becomes a DECISIONS row; feedback rounds so far are #16–#26.
- **Releases:** every push to `main` builds a release (`latest`, plus a numbered build) whatever CI says. The web page https://thriple.github.io/prism/ updates only when CI is fully green.
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
