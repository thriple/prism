# How NURBS modellers fill three-sided and n-sided corners

Research for brief 34 (the Loft tool), 2026-11-13, at the owner's request.

## 1. A pole: one side shrunk to a point
- Rhino's NetworkSrf and Loft, Fusion's three-sided loft (one edge and a
  point), and Curviloft with three curves all do this: a four-sided
  surface whose fourth side is a single point (a singularity).
- It works and is simple, but the tools that use it warn about it. Fusion
  lists singularities among its "deadly sins": they cause trouble later,
  above all when offsetting (shell, thicken). Rhino users report fillets
  and Unroll failing at the point, and fix it by cutting the point out
  and rebuilding that piece.

## 2. A bigger four-sided surface, trimmed to the boundary
- Fusion's Boundary Patch "always creates a four-sided surface that is
  trimmed to the input geometry, which avoids singularities"; Fusion
  recommends it over a three-sided loft. Rhino's Patch works the same way.
- No point, any number of sides. But the face is trimmed (Prism has no
  trimmed freeform faces until Solids & CAD), and the surface is fitted
  to the boundary within a tolerance rather than exactly through it.

## 3. Split into four-sided patches from a centre point
- The usual method in the literature (Hwang and Chuang; Gregory and Zhou;
  Piegl and Tiller, "Filling n-sided regions with NURBS patches", 1999):
  join a centre point to the midpoint of each side, then fill each corner
  with a four-sided patch. Three sides give three patches, five give five.
- Exact along every side, no point, no trimming, any number of sides.
  The work is making the inner seams smooth (matching tangents across
  them).

## 4. Curviloft
- Its author says it works with four contours (a Coons patch). A closed
  loop is cut into three or four pieces at sharp corners, or at small
  "breaker" segments the user adds; with three, one side becomes a point.
  More sides need a different plugin.

## Corners of fillets
- Where three fillets meet (the owner's rounded box corner), Alias has
  Corner Blend and Ball Corner, and Rhino's FilletEdge now builds the
  corner patch itself. That belongs with Fillet Joins.

## Recommendation
Method 3 for three and five-plus sides: exact sides, no singularity to
trip Solids & CAD later (offset, shell, booleans through the kernel), and
it fits Prism's four-sided freeform faces today. Four sides stay one
Coons patch. Method 2 can be added once trimmed faces exist.

## Sources
- https://discourse.mcneel.com/t/another-fillet-problem/26310
- https://discourse.mcneel.com/t/how-to-fix-converging-isocurve-surfaces/93700
- https://discourse.mcneel.com/t/corner-fillet-fix/137042
- https://www.autodesk.com/products/fusion-360/blog/fusion-360-3d-modeling-singularities-non-manifold-topology/
- https://help.autodesk.com/cloudhelp/2024/ENU/Alias-Tool-Palette-Reference/files/Surfaces-palette/GUID-01C85806-6EE3-4CAD-8570-DF74C4CE3F85.html
- https://community.sketchucation.com/post/1179383
- https://ir.lib.nycu.edu.tw/bitstream/11536/27575/1/000186572400008.pdf
- https://link.springer.com/article/10.1007/s00371-010-0468-4
- https://core.ac.uk/download/52631242.pdf
