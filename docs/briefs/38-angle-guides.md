# Angle Guides: Rotate matches any angle a neighbour's edges make (owner request, 2026-11-15)

**Goal.** While a group, component or whole body is turned with Rotate, when one of its straight edges comes near the direction of a neighbour's straight edge, a Smart Guide shows along both and the turn snaps softly to it, so the object can be matched to a neighbour at any angle (a wall drawn at 30°), as in Illustrator. Today Rotate's guides match only the same heading or a quarter turn from it.

**Read first:** `prism-tools/src/smart.rs` (`snap_turn`), `prism-tools/src/session/smart.rs` (`smart_begin_step`, `smart_turn`, `smart_overlay`), NOTES "Closing gaps: Smart Guides in Scale and Rotate".

## Size
S: one part.

## Requirements
- At the start of a turn, the moving object's straight edges and each neighbour's (groups and components in the open group from their own geometry, placed; loose bodies) are listed: up to 12 per object, longest first, one per direction.
- While turning about the protractor's normal, edges are compared only where both lie (nearly) in the plane of the turn. When one of the object's edges, turned, is within the pull of parallel to a neighbour's edge, the angle snaps to make them parallel.
- The guide: a dashed teal line along each of the two edges (through their middles, a little longer than the edges), and the label **Parallel**.
- The heading match (Parallel / Square) stays. When both catch, the one nearer the cursor's angle wins; equal, the heading. The protractor's 15° soft steps win when nearer, as now.
- What wins over it, as with the other guides: a snapped point, typing an angle, Shift (45° steps). The time budget lowers it with the others (Near: neighbours within three sizes; Paused: none).
- Curved edges are not compared (their direction changes along them).

**MCP:** nothing new to read or set: Rotate is reached by `use_tool`, and the angle it lands on is in the model.

## Out of scope
- Angle guides in Move, Line or Scale; matching curved edges' tangents; a list of preset angles (the protractor's soft steps cover it).

## Tests
- `snap_angle`: an edge at 28° pulled to a neighbour's 30° edge; one at 40° not; one standing out of the plane not; paused gives none.
- A session test: a box turned to 35.5° beside a triangle whose longest side runs along x and another side at 37° lands at 37° (the edge, not the heading).
- The heading test (`a_turned_group_faces_the_way_its_neighbour_does`) still passes. Shift skips the guides before they are asked, as before.

## Acceptance
- [ ] A box turned until it sits square with a wall drawn at 30° (owner).
- [ ] PLAN, NOTES and the Planned Features behaviours line are updated.
