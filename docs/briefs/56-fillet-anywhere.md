# Brief 56: Fillet / Chamfer anywhere

Status: **plan, for the owner** (2026-11-26). Owner: "make a plan for the fillet / chamfer tool to get it to work on situations like the session 29 body. You can generate an end-cap face if the fillcham needs to cut into another face. Fillcham should work on the session 29 body, that's why it's a good test." And: "I haven't even tried a more complicated 3D path, but I know those are possible too."

**Goal:** any edge with a face on each side rounds (fillet or chamfer), on real models drawn the SketchUp way, with lines drawn on faces, small leftovers from earlier edits, openings, and chains of edges that turn in 3D. A refusal is left only where there is no corner at all (a line on a flat face) or where nothing can be built. A size past what fits is handled as "as far as it goes" (DECISIONS #45).

## Why the session 29 body fails

The recording is `crates/prism-tools/tests/data/owner-session-29-open-fillet.json`; at event 118 the hovered edge is refused.

The body (42 faces) is closed, but the kernel does not take it as a solid, for two reasons. Both are in one face: the flat end face at x = 1.386, face 329.

1. **A notch.** A line and an arc 0.24 mm long run between the same two points, and nothing fills the space between them. Some earlier edit left it.
2. **A slit.** A line about 0.37 m long, in three pieces, is drawn into the face from its edge without dividing it. The face walks it out and back, so those edges have the same face on both sides.

The kernel needs every edge to have exactly two different faces. So the whole body counts as "not a closed solid". Every edge then falls back to Prism's own round, which only takes one straight edge between flat faces whose ends are plain three-edge corners. Most of the body's edges are not like that: they meet other rounded edges, sit beside cylinders, or end at busier corners.

What the existing round already does:
- lines drawn on faces in the round's way are taken off and drawn back (session 22);
- split points along a line are joined;
- edges in pieces are rounded as one;
- openings that lie in one plane are capped for the kernel (session 29, general case);
- rounds are made in turn where the kernel cannot do them all at once;
- the tip of an apex is cut off first.

None of that reaches face 329, because the slit and the notch make the whole body refused before any of it runs.

## The plan

### 1. A copy of the body the kernel can take

A round works on a copy of the body, prepared for the kernel. Prism's own model is changed only by the round itself: every repair below is undone or kept the way the user drew it, except where the step says otherwise.

| What | On the copy | Afterwards |
|---|---|---|
| Lines drawn on a face in the round's way | taken off (done) | drawn back where they still lie on a face (done) |
| A **slit**: edges one face walks out and back | taken off | drawn back, as far as they still lie on the face |
| A **notch or sliver**: two short edges between the same two points, under 100 × the precision | the two points welded | kept welded: the shape moves no more than the notch is long. Model Health also lists them, so Heal can fix them anywhere |
| An **opening that lies in one plane** | a flat cap (done) | cap taken away (done) |
| An **opening that does not lie in one plane** | a cap from the Loft ring fill (the same surface Loft's fill makes) | cap taken away |
| Faces turned the wrong way | turned on the copy | the round's new faces face the way their neighbours do |
| Other geometry in the same group that does not touch the body | left out of the copy | untouched |

Test: on the session 29 body, every edge between two faces that are not of one plane rounds at 0.093 m (the size in the owner's screenshot), as a fillet and as a chamfer. Edges whose faces are too narrow say so as "as far as it goes", not as a refusal. Face 329's slit is still there afterwards. Model Health is no worse.

### 2. End caps where a round cuts into another face (Prism's own round)

Prism's own round (`round.rs`) is the fallback when the kernel will not take even the prepared copy. Today it needs plain three-edge corners at both ends and an end face square to the edge. Owner: "you can generate an end-cap face if the fillcham needs to cut into another face". So each end of the round is closed as follows:

- **Plain corner, square end face:** as today: the end face gets the round's profile (a line or an arc).
- **Slanted end face:** the profile is cut by that face's plane. An ellipse for a fillet; a line for a chamfer. Today a slanted end is refused for fillets.
- **End at a busier corner, or where the round's setback runs into a neighbouring face:** an **end-cap face**. This is a flat face in the plane square to the edge, at the point where the round's setback lines leave the faces beside it. It is bounded by the profile and by the lines where that plane meets the neighbouring faces. The neighbouring faces are cut back to the cap, the way sticky geometry splits faces.

The end cap is always a flat face, so the result is a valid solid, and the user can push/pull or erase the cap like any face.

Tests:
- a box with a line drawn into the end face (a slit at the end);
- a wedge with a slanted end;
- an edge ending at a four-edge corner;
- a round whose setback crosses a short neighbouring face;
- each as fillet and chamfer, checked by volume (worked out by hand), Model Health clean.

### 3. Rounding locally when the whole body cannot go to the kernel

If the prepared copy is still not a solid (for example, an edge with three faces somewhere else on the body), round **just the neighbourhood**:

1. Take the faces beside the edges, and the faces at their ends.
2. Close them into a small solid with end-cap faces (step 2's caps, on the copy).
3. Round there through the kernel.
4. Put the round's new faces and the trimmed neighbours back into the model.

A problem elsewhere on the body then no longer blocks a round here.

Test: the session 29 body with one more fault added far from the edge (a third face on some edge). The edge still rounds.

### 4. Chains of edges that turn in 3D (the owner's "more complicated 3D path")

- **Tangent chains** (a line running into an arc, a Follow Me path): already rounded as one by the kernel on solids. On open bodies and fallback rounds they are taken as one chain too: the whole run is collected with one click (as straight runs are now), and the round follows it.
- **Chains with corners** (a 3D polyline path, the edges of a twisted extrusion): each corner is closed by the corner rules (DECISIONS #46): in turn, or the corner cut off first. With Prism's own round, two rounds meeting at a corner are mitred: both are cut by the plane that halves the angle between them, and the cut is filled.
- **Edges on freeform (NURBS) faces:** the kernel rounds only between flat, cylindrical and conical faces. For freeform faces, the round is fitted within the document's precision (DECISIONS #40, as Deform does): a rolling-ball surface sampled along the edge and fitted as a NURBS face.

Tests:
- a Follow Me tube along a 3D path with corners, all its outer edges filleted;
- a swept profile along a 3D curve;
- a lofted shape's edge between a NURBS face and a flat one.

### 5. The smooth corner patch (DECISIONS #46)

Replaces the small flat facet at an apex: an n-sided patch that meets each round's slope at its edge.

Tests: a pyramid tip; a cube corner with three different sizes (once rounds can have per-edge sizes).

### 6. The tool

- **Hover:** shows what the round would make, including end caps, and in orange what it could not do.
- **Message:** the cursor label and Fillet window say which step was taken where it matters. For example: "the end is closed by a flat cap", or "a notch 0.24 mm long was welded".
- **Session 29 replay:** the owner's recording goes into the tests once step 1 is in: the hovered edge's run rounds at 0.093 m.

## Order of work

1. Step 1 (slits, notches, non-flat openings), with session 29 as the test. **This is what makes the owner's body work.**
2. Step 2 (end caps).
3. Step 3 (local round).
4. Step 4 (3D chains, freeform faces).
5. Step 5 (corner patch).
6. Step 6 (tool messages) as each lands.

Each step:
- tests (unit tests, and session replays where there is one);
- the rule-order note in DECISIONS #44 updated: what the round does to a copy, and what it leaves changed;
- release notes;
- a release.

## Questions for the owner

1. **The notch weld.** Keep it in the model (proposed: it is a defect, and moves the shape no more than 0.24 mm)? Or weld only on the copy, and leave the notch for Heal?
2. **End caps.** A flat cap square to the edge (proposed)? Or a cap in the plane of the face the round runs into, which looks like the round "dies into" that face, as some CAD programs do?
3. **A line drawn across a round's path.** It is drawn back on the faces it still lies on (as now). Should it also continue across the round's own face, as a curve on it?
