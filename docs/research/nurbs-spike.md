# NURBS Spike: which kernel (2026-11-12)

PLAN 6.0's NURBS Spike: pick the backend behind Prism's `Kernel` trait
(PLAN 2.3) by robustness tests, "including the surface-surface
intersection that curved overlap splits need". Deliverable: a written
decision with test results. This is it.

## Decision

**arris** (0.10, MIT OR Apache-2.0, pure Rust) is Prism's NURBS kernel,
behind Prism's own `Kernel` trait. Prism keeps its own data model,
topology, planar operations and exact curves (PLAN 2.3: "own the data
model, borrow the hard maths"); arris does what is hard to own:
surface-surface intersection, booleans on curved and freeform faces,
fillets, chamfers, offsets, shells, lofts, sweeps and STEP.

monstertruck is not used. OCCT is ruled out by its licence (LGPL, which
CLAUDE.md does not allow) and by its web build; SMLib (Apache-2.0, C++)
does not build for `wasm32-unknown-unknown` and is not needed now that a
pure-Rust kernel passes. Both stay the fallback named in PLAN 2.3 if arris
falls short later.

## The tests

`spikes/nurbs/` (its own Cargo workspace, not part of the app's build)
runs the same solids through both kernels and checks each result against
a volume worked out by hand: valid solid (arris's checker at Level::Full;
monstertruck's closed-shell condition), and the volume within 1 %
(arris measures exactly; monstertruck from a fine tessellation). Run with
`cargo run --release` in that folder. Results on this machine (4 cores,
rustc 1.99.1, release build):

| case | tests | arris | ms | monstertruck | ms |
|---|---|---|---|---|---|
| box - box | planar boolean | ok (0.000 %) | 4 | ok (0.000 %) | 3 |
| box - cylinder (hole) | plane x cylinder: circles | ok (0.000 %) | 11 | refused: output shell not closed | 59 |
| box - tilted cylinder | plane x cylinder: ellipses | ok (0.007 %) | 44 | ok (0.014 %) | 128 |
| cylinder AND cylinder, equal radii | cylinder x cylinder, tangent branches (Steinmetz) | ok (0.000 %) | 60 | refused: output shell not closed | 361 |
| cylinder AND cylinder, radii 1 and 0.6 | cylinder x cylinder, two loops | ok (0.000 %) | 125 | ok (0.042 %) | 222 |
| cylinder OR cylinder, equal radii | union with tangent branches | ok (0.000 %) | 86 | refused: output shell not closed | 268 |
| sphere AND box (half) | plane x sphere | ok (0.000 %) | 4 | refused: empty output | 22 |
| sphere OR sphere | sphere x sphere | ok (0.000 %) | 71 | refused: empty output | 85 |
| sphere - cylinder | sphere x cylinder, off centre | ok (0.001 %) | 168 | refused: empty output | 103 |
| torus - box (half) | plane x torus | ok (0.000 %) | 5 | refused: output shell not closed | 59 |
| fillet one cube edge | constant-radius fillet | ok (0.000 %) | 3 | ok (0.001 %) | 12 |
| fillet all 12 cube edges | fillets meeting at corners | ok (0.000 %) | 11 | **panic** (out of bounds) | 3 |

- **Passed:** arris 12 of 12, monstertruck 4 of 12 (one panic).
- **Random tilted holes** (40 cylinders of random radius through a box at
  random tilts and offsets): arris 40 valid, monstertruck 39; where both
  succeeded they agree within 1 % every time.
- **Freeform (NURBS) faces in booleans** (arris only: smooth lofts through
  twisted squares, cut by a box and crossed with each other): consistent to
  2.5e-11 (V(A) = V(A∩B) + V(A−B); V(A∪B) = V(A) + V(B) − V(A∩B)). They
  took **12 s** together, which is the one worry (below).

Some of monstertruck's refusals may come from how its solids were built
here (a sphere turned from a half disc touches its axis), but the plain
hole and the torus fail too, and the twelve-edge fillet panics. Its fillet
is a sampled rolling ball (5 divisions), not exact. arris's fillets are
exact surfaces, and its fillets meeting at a corner close cleanly, which
the owner's Fillet Joins step needs.

## The web and the rules

| | arris | monstertruck |
|---|---|---|
| Builds for `wasm32-unknown-unknown` | yes | yes |
| Size, release, LTO, opt-level s | 2.9 MB (0.98 MB gzipped) | 1.8 MB (0.49 MB gzipped) |
| Threads | none | rayon and crossbeam (Prism's web build has no threads) |
| Licences of everything it brings | all MIT, Apache-2.0, Zlib or Unlicense | permissive, some dual with LGPL, one BSL-1.0 |
| Crates brought in | 47 | 99 |
| `unsafe` | none (its own claim) | some |
| Deterministic | yes: the same ids and bytes on every platform (its own claim; Prism's rule for core crates) | not stated |
| Cancelling a long operation | every operation takes a `Control` it polls | no |
| Errors | typed refusals naming the entities, never an invalid shape | errors, and a panic above |

## Risks and how Prism handles them

- **Pre-1.0:** arris's API breaks between minor versions. Pin the version,
  and keep every call inside one crate (`prism-kernel`, behind the
  `Kernel` trait), so an upgrade touches one place.
- **Speed on freeform faces:** NURBS-on-NURBS booleans took seconds. Run
  kernel operations off the interface thread natively; on the web, show
  progress and use arris's `Control` to cancel. Prism's own exact planar,
  cylinder, cone, sphere and torus paths stay in Prism and stay fast; the
  kernel is called only where they cannot answer.
- **Two models:** Prism's topology is non-manifold (loose edges and faces,
  as in SketchUp) and arris's is solid B-rep. Convert at the boundary:
  closed regions go to arris as solids, results come back as Prism faces
  with exact surfaces. Freeform NURBS part 2 designs this conversion.
- **Size on the web:** about 1 MB gzipped. Acceptable; load it only when
  first needed if it matters.
- **Few users:** the spike's cases become Prism tests, so a regression in
  a new arris version shows before it ships.

## What this changes

- DECISIONS #9 (NURBS kernel): decided, arris.
- PLAN 2.3's kernel table: arris is the pick.
- Freeform NURBS (the next chunk) builds on it; the Solids & CAD chunk's
  curved booleans, shells, NURBS fillets and STEP go through it too.
- `docs/DEPENDENCIES.md` gets arris's row when the app first depends on
  it (Freeform NURBS part 1), not now.
