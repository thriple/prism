# Tolerance and geometry-rule audit (2026-11-26)

Asked by the owner (DECISIONS #42): how does Prism decide that two things
are "close enough" to be the same, which clean-up steps run after an edit,
and do any of them override each other where they should not? This is a
report only: no code was changed for it. It ends with a proposal for the
owner to accept, change or turn down.

## Summary

1. **There is one central tolerance, but it is not one number.** It is a
   millionth of the size of whatever model asks for it, worked out again
   every time it is asked. So every group has its own, a model's
   tolerance grows as it grows, and drawing one far-away point loosens
   welding for the very line being drawn.
2. **391 hard-coded thresholds** sit in the program's own code (tests not
   counted), 193 of them in the operations crate. Most are harmless (guards
   against dividing by zero, curve-parameter nudges), but some are real
   "same point" or "same direction" decisions that ignore the tolerance.
3. **"Parallel" is judged by a fixed angle, "same point" by a distance.**
   The two do not agree on long faces: two faces 10 m long count as
   parallel even when one end is 0.45 mm off, 45 times the distance
   tolerance of a 10 m model. That is the test that decides whether faces
   are coplanar and merge.
4. **The clean-up steps after an edit have no written order and no rule
   about what each may undo.** Each edit runs its own sequence. Several of
   this week's bugs were one step quietly undoing another's decision, or a
   step written for the simple case meeting a harder one.
5. **Curve fitting has its own tolerances** (a tenth or a hundredth of the
   distance tolerance, or a fixed fraction of a size), chosen per tool.
   They are mostly finer than the merge tolerance, which is the safe way
   round, but nothing ties them together.

## 1. Where "close enough" comes from today

| Source | What it is | Where |
|---|---|---|
| `Tol::for_extent` | 1e-6 of a size, clamped to 1e-12 m … 1 cm | `prism-math/src/tol.rs` |
| `Model::tol()` | that, for the model's own bounding box, recomputed on every call (scans every point) | `prism-topo/src/lib.rs` |
| `sticky::tol_with` | the model's points **plus the new ones being drawn** | `prism-ops/src/sticky.rs` (9 callers) |
| `Scene::tol` | the drawn scene's own bounds (groups merged in) | `prism-view/src/scene.rs` |
| Mesh import | the imported points' own extent | `prism-file/src/mesh/build.rs` |
| `ANGLE` | 1e-9 as 1 − cos: about 0.0026°, fixed, for "parallel" | `prism-math/src/tol.rs` |
| `perpendicular` | √ANGLE, about 0.0018 as a cosine | same |

The tolerance is asked for 109 times by name (`.tol()`) in the operations,
tools, document and view crates, each time from whichever model is at
hand: the whole model, a group's model, a scratch copy, a slab being
built, the union of two pieces in a boolean. Each of those has its own
size, so each gets its own tolerance.

**What that means in practice:** a 10 m model gets 0.01 mm. A 1 cm part in
its own group gets 0.00001 mm. Join them (explode, boolean, intersect) and
the two "same point" rules meet. Draw a point 1 km away inside the 10 m
model and that model's tolerance becomes 1 mm, for everything in it.

## 2. The hard-coded thresholds

391 literal thresholds in program code (tests excluded):

| Crate | Count |
|---|---|
| prism-ops | 193 |
| prism-tools | 58 |
| prism-file | 38 |
| prism-geom | 31 |
| prism-view | 29 |
| prism-doc | 13 |
| others | 29 |

By what they compare (rough sort by reading each line):

| Kind | Count | Verdict |
|---|---|---|
| Absolute comparisons (`< 1e-9`, `<= 1e-12`) | 145 | Mixed: most guard a division; some are distance decisions (below) |
| Relative to a size (`len * 1e-9`) | 88 | Usually right in spirit; should come from the tolerance |
| Angles and directions | 49 | Should come from one angle rule tied to the distance tolerance |
| Curve parameters (`t > 1e-9`) | 41 | Mostly fine: parameter space, not distance |
| Scaled from the tolerance (`tol.dist * 0.1`) | 29 | Right |
| Other | 39 | Case by case |

Most common values: 1e-9 (157), 1e-12 (83), 1e-6 (50), 1e-3 (27).

**Ones that are real "same point" or "same direction" decisions outside
the tolerance** (the ones that matter):

- `prism-ops/src/cut.rs:196` merges points closer than **1e-9**, and
  `cut.rs:649` and `cut.rs:802` merge points closer than **1e-6**, in the
  same file, all absolute, in a curved face's flattened layout (metres).
  The same operation uses three different "same point" rules, none of them
  the model's.
- `prism-ops/src/primitives.rs:28`: points within 1e-9 are the same.
- `prism-ops/src/band.rs:149, 232`: knots and axes compared at 1e-9
  (added this week for cylinder ends; should use the tolerance).
- `prism-ops/src/regions.rs:401`, `sides.rs:119`, `taper.rs:26`,
  `loft.rs:115`: "parallel" as a cross product under 1e-9, a third
  parallel rule besides `Tol::parallel` and the plane grid's 1e-4.
- `prism-ops/src/settle.rs:87`: planes are grouped on a grid of 1e-4 in
  their normals (with neighbours searched, so it only narrows the search;
  the decision is `same_surface`).
- `prism-file/src/mesh/build.rs:145`: imported points weld at 1e-6 of
  their distance from the origin, not of the model's size.
- `prism-geom/src/nurbs.rs:918, 939`: a curve is closed if its ends are
  within 1e-7 of its size.

## 3. The clean-up steps after each edit

What each kind of edit runs afterwards, in order, as the code stands:

| Edit | Steps, in order |
|---|---|
| Draw a line or shape (`add_segment`) | tolerance from the model **plus the new points** → weld the ends onto existing points → split edges the line crosses or touches → rebuild the faces on that plane (regions) → (drawn by the user only) show a hidden line it runs over |
| Push/Pull | weld the new points → coplanar rule on touched planes (rebuild regions, then the curved-face rule: cut curved faces, remove duplicates) → (new loop only) cut what it passes through, remove duplicates → remove duplicates → (curved, plain push) soften the old outline where curved faces meet smoothly → soften new seams between curved faces |
| Move / Rotate / Scale | arcs that would bend become exact ellipses or segments → plan curved bands → move → re-settle faces off their surfaces → rebuild bands → weld points that landed on others (soft if either line was) → coplanar rule for rigid groups → remove duplicates |
| Boolean (Union/Subtract/Intersect, Push/Pull into solids, Shell's face-by-face joins) | cut each where the other crosses it → classify faces inside/outside → drop faces inside both → keep the wanted ones → join them → remove unused lines → merge seams between coplanar faces |
| Shell / Thicken | turn mismatched faces round → pick rim styles → build the slab → (if it cannot) square the rims → (if it still cannot) face by face, joined by booleans → remove the old faces → join the slab in |
| Erase | remove → merge coplanar faces left either side |

**What each step may change that an earlier one decided is written
nowhere.** The cases that bit this week:

| Bug | Which step overrode which |
|---|---|
| A hidden seam turned hard after a push | splitting an edge copied the new line's look onto an existing soft line (`split_edge`, fixed 2026-11-25) |
| Hover lit only part of a curved face | coplanar and cut steps left hard lines inside one smooth surface; nothing softened them afterwards (fixed: softening runs last in Push/Pull) |
| The truck cab would not shell | the rim-corner rule assumed upright walls (fixed 2026-11-26) |
| Face-by-face shell dropped most faces | a join that produced a non-solid made every later join fail (fixed 2026-11-26) |
| Joins took minutes | removing unused lines ran the whole Model Health check (fixed 2026-11-26) |

## 4. Other findings

- **The angle and distance rules disagree on long faces.** `Tol::parallel`
  allows about 4.5e-5 rad. Over 10 m that is 0.45 mm; the distance
  tolerance of a 10 m model is 0.01 mm. `Plane::same_surface` (the test
  for "coplanar, so merge") uses this angle plus one point on the plane, so
  two large faces can merge while their far edges are well apart, or a
  face slightly tilted by rounding is still "the same plane" far from its
  origin. A parallel test should allow the distance tolerance over the
  length involved, not a fixed angle.
- **Fitting tolerances are chosen per tool:** `thicken` and `deform` fit to
  a tenth of the tolerance, crossing curves trace to a hundredth or the
  full tolerance, cylinder bands use 1e-9 of their size, NURBS closure
  1e-7 of its size. Fits finer than the merge tolerance is the safe way
  round (a fitted edge never misses what it should meet); they should be
  stated as one ratio.
- **Recomputing the tolerance costs time:** every `model.tol()` scans every
  point. With 109 call sites, some inside loops, that is measurable on
  large models. A fixed value stored once removes it.
- **Display slack is separate and should stay separate.** The viewport's
  line test has its own allowances (a curved face drawn as flat facets
  strays in front of its exact edge). Those are about pixels, not geometry,
  and are not part of this proposal.

## 5. Proposal

### 5.1 The tolerance

- **One fixed distance per document, stored in the file, the same for
  every group.** Proposed default: **0.01 mm (1e-5 m)**. That is what a
  10 m model gets today, so most models behave as now. It is far above the
  precision of Prism's numbers anywhere up to about 10 km from the origin,
  and fine enough for parts with 0.1 mm details.
- **A document setting** (File > Document Settings > Precision), with
  presets: Fine 0.001 mm (jewellery, small parts), Standard 0.01 mm,
  Coarse 0.1 mm (sites, terrain), or a typed value between 1e-9 m and
  1 cm.
- Older files open at Standard unless their smallest edges are shorter
  than about 10 tolerances, in which case a finer preset is chosen and
  Prism says so (see 5.3).

### 5.2 Everything else derived from it

| Threshold | Rule |
|---|---|
| Same point | distance < tolerance |
| Zero length / zero area | < tolerance / < tolerance² |
| Parallel / coplanar | the two directions differ by less than tolerance over the length involved (a face's size, an edge's length), not a fixed angle |
| Point on curve or surface | distance < tolerance |
| Curve and surface fits | to a tenth of the tolerance (one ratio, everywhere) |
| Crossing tracing | to a tenth of the tolerance |
| Sliver (Model Health) | thinner than 100 tolerances and 100 times longer than wide (as now) |
| Guards against dividing by zero | stay as they are (they are not geometry decisions) |
| Curve-parameter nudges | stay, written as one named constant |

### 5.3 Graceful fallbacks

- **Drawing below the tolerance:** an edge shorter than the tolerance would
  collapse. Prism refuses nothing: it draws it, warns "smaller than this
  document's precision (0.01 mm): it may merge", and offers Fine.
- **Running out of precision:** if geometry sits so far from the origin
  that the numbers cannot hold the tolerance (about 1e9 tolerances away:
  10 km at Standard), Model Health warns, as its "far from origin" row
  already does, and suggests moving the model nearer the origin or
  choosing Coarse.
- **Opening a file:** if its smallest edges are under about 10
  tolerances, the finer preset is chosen and the status bar says so.
- **Changing the setting:** making it coarser runs Model Health first and
  lists what would merge before it is applied.

### 5.4 The order of geometry rules (draft for DECISIONS)

1. **Exact tests come first and use no tolerance:** which side of a plane,
   which way a loop turns (exact predicates).
2. **One tolerance, the document's.** No step works out its own.
3. **After any edit, in this order:** weld points → split lines where they
   cross or touch → rebuild faces on each touched plane (coplanar rule) →
   cut curved faces by lines on them → remove duplicate faces → merge
   coplanar seams (Erase, booleans) → soften seams between smoothly
   meeting faces → user-intent last (a line the user drew shows).
4. **What a step may change:**
   - Welding and splitting never change how a line looks (soft, hard,
     hidden) or which curve it belongs to.
   - Only the softening step makes a line soft; only a line the user draws
     (or erase/unsmooth by the user) makes a soft line hard.
   - Removing duplicates keeps the face the edit did not make.
   - No step deletes a face another step just made, except duplicates.
5. **Every tool that edits geometry states which steps it runs and in what
   order** (DECISIONS #43), and its tests cover the cases where a step
   would otherwise undo another.

### 5.5 How to get there (suggested stages, each tested and released)

1. Store the tolerance in the document (default Standard; old files
   choose as in 5.3). `Model::tol()` returns it instead of computing it.
   Groups use the document's. This alone fixes the per-group and
   far-point problems.
2. Replace the parallel/coplanar test with the length-aware one; move the
   real distance decisions found in section 2 (`cut.rs`, `primitives.rs`,
   `band.rs`, the cross-product parallel tests, mesh import) onto the
   tolerance.
3. Write the order of rules into DECISIONS and make each edit follow it,
   with a test per rule in 5.4.
4. The setting, its presets and the fallbacks in the app.
5. Sweep the remaining thresholds: name the divide-by-zero guards and
   parameter nudges as constants, delete duplicates.

Stage 1 is the smallest and removes the largest source of inconsistency.
