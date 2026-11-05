# Prism handoff package

What is in this folder, and how to use it with Claude Code.

| File | What it is |
|---|---|
| `CLAUDE.md` | Working rules. Put it in the root of the repo. |
| `PLAN.md` | The full plan. Put it in the root of the repo. |
| `docs/DECISIONS.md` | Open decisions, with the defaults to use until you decide |
| `docs/briefs/01-skeleton.md` | Chunk 1: native and web window, CI |
| `docs/briefs/02-core-model.md` | Chunk 2: the geometry core, with tests |
| `docs/briefs/03-actions-and-keys.md` | Chunk 3: action registry, menus, dock, hotkeys |
| `docs/briefs/04-viewport.md` | Chunk 4: the wgpu viewport, camera, lines, faces, picking and highlighting |
| `docs/briefs/05-inference-and-drawing.md` | Chunk 5: the inference engine, Line, Rectangle, the measurement box, Select |
| `docs/briefs/06-edit-tools.md` | Chunk 6: Push/Pull, Move and copy, Rotate, Scale, Eraser |
| `docs/acceptance/golden-cases.json` | 19 test cases from the prototype, for Core Model |
| `docs/acceptance/core-model-acceptance.md` | The same cases in words, plus the rules every model must satisfy |
| `docs/prototype/` | The prototype's code and a guide to reading it |
| `docs/UI-STYLE.md` | The look every screen must use: Motion Kit, dark Soft glass (tokens, motion, which component to copy) |
| `docs/ui-kit/` | The Motion Kit itself (React, for reference). Open `soft.html` and `demo.html` in a browser |

## Putting it in the repo

This repository is the handoff. Start Claude Code in a clone of it.

## What to say to Claude Code

Start with chunk 1 and only move on when its acceptance list is done.

**Chunk 1.** "Read CLAUDE.md, PLAN.md and docs/DECISIONS.md. Then build the chunk in docs/briefs/01-skeleton.md."

**Soft Theme (before Chunk 3).** "Read CLAUDE.md and docs/UI-STYLE.md, open docs/ui-kit/soft.html and demo.html in a browser, then build docs/briefs/02a-soft-theme.md."

**Chunk 2.** "Read CLAUDE.md, then build docs/briefs/02-core-model.md. The golden cases in docs/acceptance/golden-cases.json must all pass."

**Chunk 3.** "Read CLAUDE.md, then build docs/briefs/03-actions-and-keys.md."

**Chunk 4.** "Read CLAUDE.md, then build docs/briefs/04-viewport.md. The prototype shows the behaviour; build it the way the brief says, not the prototype's way."

**Chunk 5.** "Read CLAUDE.md, then build docs/briefs/05-inference-and-drawing.md."

**Chunk 6.** "Read CLAUDE.md, then build docs/briefs/06-edit-tools.md."

Soft Theme, Chunk 2 and Chunk 3 can run in separate sessions (Chunk 3 should come after Soft Theme, since it builds the menus in that look). Chunks 2 and 3 do not depend on each other, so they can run in separate sessions once chunk 1 is merged. Both need chunk 1.

## What is deliberately not here yet

The brief for Edit Tools part 2 (Offset, Tape Measure). They depend on how Core Model turns out, so they are better written after it. The same goes for the NURBS test, the dependency and licence table, and the performance targets (see DECISIONS.md).

## Building and running

```
cargo run -p prism-app          # native window
trunk serve                     # web build at http://127.0.0.1:8080 (install with: cargo install trunk --locked)
```

CI runs `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, Windows and macOS builds and the web build on every push. Pushes to `main` publish the web build to GitHub Pages. Every push also rebuilds the **latest** release: https://github.com/thriple/prism/releases/tag/latest (Windows zip and macOS `.app` zip). Each build is also kept as "Prism build N" on the Releases page (newest 30), so an older one can be downloaded again.
