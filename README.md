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
| `docs/acceptance/golden-cases.json` | 19 test cases from the prototype, for Core Model |
| `docs/acceptance/core-model-acceptance.md` | The same cases in words, plus the rules every model must satisfy |
| `docs/prototype/` | The prototype's code and a guide to reading it |
| `docs/UI-STYLE.md` | The look every screen must use: Motion Kit, dark Soft glass (tokens, motion, which component to copy) |
| `docs/ui-kit/` | The Motion Kit itself (React, for reference). Open `soft.html` and `demo.html` in a browser |

## Putting it in the repo

Copy everything into the root of the `thriple/prism` repository, commit it, and then start Claude Code in that folder. (I could not push to the repo myself because I do not have access to it.)

## What to say to Claude Code

Start with chunk 1 and only move on when its acceptance list is done.

**Chunk 1.** "Read CLAUDE.md, PLAN.md and docs/DECISIONS.md. Then build the chunk in docs/briefs/01-skeleton.md."

**Chunk 2.** "Read CLAUDE.md, then build docs/briefs/02-core-model.md. The golden cases in docs/acceptance/golden-cases.json must all pass."

**Chunk 3.** "Read CLAUDE.md, then build docs/briefs/03-actions-and-keys.md."

Chunks 2 and 3 do not depend on each other, so they can run in separate sessions once chunk 1 is merged. Both need chunk 1.

## What is deliberately not here yet

Briefs for Viewport, Inference & Drawing and Edit Tools. They depend on how Core Model turns out, so they are better written after it. The same goes for the NURBS test, the dependency and licence table, and the performance targets (see DECISIONS.md).
