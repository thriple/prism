# Brief 30: NURBS Spike

PLAN 6.0 row "NURBS Spike" (M5, size S). Owner: build it right after
Basic Materials (2026-11-11), and start NURBS today (2026-11-12).

## Goal
Pick the NURBS kernel behind Prism's `Kernel` trait (PLAN 2.3) by
robustness tests, including the surface-surface intersection that curved
overlap splits need. Deliverable: a written decision with test results.

## Scope
- Candidates: the pure-Rust kernels first (monstertruck, arris), as PLAN
  2.3 says; OCCT only if both fail (it is LGPL, which CLAUDE.md does not
  allow by default), SMLib likewise (C++).
- One test program, `spikes/nurbs/`, outside the app's build: the same
  cases through each kernel, each checked against a hand-worked volume:
  planar, plane x cylinder (circles and ellipses), cylinder x cylinder
  (two loops, and the tangent Steinmetz case), plane x sphere, sphere x
  sphere, sphere x cylinder, plane x torus, a fillet, fillets meeting at
  corners, random tilted holes, and NURBS faces in booleans.
- Each kernel built for `wasm32-unknown-unknown`: does it build, how big
  is it, does it need threads; and the licences of everything it brings.
- The decision in `docs/research/nurbs-spike.md`; DECISIONS #9 and PLAN
  updated.

## Out of scope
Using the kernel in the app: Freeform NURBS (the next chunk) does that.
No change to the app's code or dependencies.

## Done when
The decision is written with its results, and pushed.
