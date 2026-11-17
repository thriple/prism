# Solids & CAD part 4: Convert and Weld (2026-11-17)

**Goal.** Two edit commands from PLAN section 1:
- **Weld** joins chained edges into one curve (picked, moved and erased as one).
- **Convert** turns a curve chain into one NURBS curve and back, and faces into one smooth patch and back.

Shell / Thicken, once part 4 here, moved to the Tool Pass (brief 42).

**Read first:**
- PLAN section 1 ("Weld edges", "Convert");
- `prism-ops/src/sticky.rs` (`break_curves`: where curves part);
- `prism-geom/src/nurbs.rs` (`Nurbs::line`, `arc`, `elevated`, `joined`, `through`, `fitted`);
- `prism-geom/src/patch.rs` (`NurbsSurface::fitted`);
- `prism-ops/src/loft.rs` and `sweep.rs` (patches from curves).

## Size
XL: three parts, each pushed and tested on its own.

## Rules
- **A curve is the edges that share a curve id.** They are picked, moved and erased together. A drawing that cuts into a curve parts it there (NOTES "Curves part where a drawing cuts them").
- **Edit commands work on the selection,** and say why when they refuse, in the status line.
- **Every command is one undo step,** recorded in the log.

## Parts
- **Part 4a. Weld (Edit > Weld, and the right-click menu with edges selected).**
  - The selected edges must join end to end in one chain, open or closed, with no branch. They become one curve.
  - Refused, with a reason: nothing selected; edges not in one chain; already one curve.
  - `Command::Weld { edges }`.
- **Part 4b. Curve chain ↔ NURBS curve (Solid > Convert).**
  - A welded or selected chain of lines and arcs becomes one NURBS curve edge, exactly: each line and arc is a rational quadratic, joined, with knots where they met, so corners stay sharp. The points in between go, and the faces along it keep it as their side.
  - One NURBS curve becomes a chain again: the pieces that are lines or arcs come back as those, exactly. The rest become lines within the tolerance.
  - Faces on curved surfaces along the chain: refused at first (flat faces and loose curves only).
- **Part 4c. Faces ↔ smooth patch (Solid > Convert).**
  - A grid of faces (rows and columns of four-sided faces, as the Loft tool's rows make) becomes one freeform face through their points.
  - A freeform face becomes a grid of flat faces along its spans, with soft and smooth edges between them.

**MCP:** nothing new. `run_action` runs Weld and Convert on the selection, and the log replays them.

## Out of scope
- Explode Curve (taking a welded curve apart); a line drawn across a curve already parts it.
- Smoothing that does not pass through the faces' points (a subdivision surface): that is Smooth / Subdivide.

## Tests
- **4a:**
  - a chain of lines welds into one curve, picked and erased as one;
  - a closed loop welds;
  - a branch is refused, and so are edges apart;
  - a welded run past a box's corners stays whole when lines are drawn from those corners.
- **4b:** a chain of lines and arcs converts and back with the same geometry, and its faces keep their area.
- **4c:** a lofted grid converts to one patch through its points, and back to a grid.

## Acceptance
- [x] Weld and both Converts are built and tick themselves in Planned Features.
- [x] PLAN, NOTES and HANDOFF are updated after each part.
