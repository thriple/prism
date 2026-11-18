# Completion Effects: a short line animation when a tool finishes

**Goal.** When a tool finishes an edit, the shape it just made answers with a soft, brief line animation. It lasts about a second, has no fill, and never changes the model, the selection or the input. A toggle in the viewport's top-right corner turns it off.

**Read first:** PLAN 1.14 (hidden is free), 4 (overlay, depth), `docs/UI-STYLE.md` (motion, reduce motion), `prism-view/src/overlay.rs`, `prism-view/src/camera.rs` (`Motion`: how the session eases without reading the clock), `prism-app/src/app.rs` (redraw only when something changes).

## Size
M: two parts. Part 2 is optional.

## The rule for what animates
Tools do not add code one by one. One general rule picks the shape. After a command that a **tool** committed, the shape is the edges that are new or whose curve changed, in world coordinates. This covers lines, rectangles, circles, arcs, Push/Pull, Offset, Follow Me, Loft, booleans, Move, Rotate and Scale alike.
- **Not animated:** undo, redo, file open, MCP edits, and selection changes.
- **Not drawn:** hidden objects, and parts cut away by the active section plane.
- **Large edits:** above a cap (for example 2,000 segments), only the outer boundary of the new faces animates. Above a second cap, nothing animates.
- **Effects off:** the session does not even work out the shape, so off costs nothing.

## Part 1: the Echo effect, the toggle, the plumbing
- **Echo.** The new edges flash bright, then a copy of them grows a little outward and fades.
  - Each face's outline grows in its own plane about its centre, to about 6 % larger.
  - Lone edges grow from their midpoint.
  - It takes 1 s, with an ease-out: full brightness at the start, nothing at the end.
  - It uses lines only, in the accent colour, and is drawn over the model like the inference lines.
- **Pure and timed from outside.** The session stores a `Made { edges, faces' planes, seq }` when a tool commits. A pure `prism-view::effects::lines(&Made, t) -> Vec<Line>` turns it into lines for any `t` from 0 to 1. The app owns the clock and passes the time in, the same way as the camera's `Motion`. So the tool crates stay deterministic, and tests can ask for any frame.
- **Redraws.** The app draws frames only while an effect is running, then stops. An idle window draws nothing extra.
- **Overlapping effects.** A new edit starts its own effect. At most 4 run at once, and the oldest goes first.
- **Reduce motion.** When the system or the setting reduces motion, there is no growth. The outline only fades, over 0.4 s.
- **The toggle.** A small round Soft-glass icon button sits in the top-right corner of the viewport well. It shows lit when on.
  - It is the registered action `view.effects` ("Completion Effects", View menu), so it also works from Command Search and can take a hotkey.
  - It is saved in the app's settings, not per style. It is on by default.
  - Turning it off stops any effect that is running.
- **MCP:** no new tools. MCP edits do not animate, and `view.effects` can be toggled through the existing action call.
- **Tests.**
  - `lines(&Made, t)`: full at t = 0; growth and fade at t = 0.5 match the formula; nothing at t ≥ 1; it is the same every time.
  - Session:
    - drawing a rectangle makes a `Made` with its 4 edges;
    - Push/Pull makes the new side edges and the moved face's edges;
    - undo, redo and MCP edits make none;
    - with effects off, none is made;
    - hidden geometry is left out;
    - a huge edit falls back to the boundary, or to nothing.
  - App: `view.effects` has a menu path; the setting round-trips.
  - GPU screenshot: a rectangle at t = 0.3.

## Part 2 (optional): one variant per kind of edit
These keep the same rule and plumbing and only change the drawing:
- **Trace** for lines, arcs and curves: a bright tip runs along the new edges, then fades.
- **Sweep** for Push/Pull and Follow Me: a faint outline runs from the start face to the end face.
- **Ghost** for Move, Rotate and Scale: the old outline fades out where it was.

The owner picks which to keep after seeing part 1.

## Out of scope
- Sound.
- Effects in the path-traced renderer or in exports.
- Particles or fills.
- Effects for selection.
- A setting for each tool.

## Acceptance
- [ ] Drawing, Push/Pull and Move each give a short echo that feels calm, not busy (owner).
- [ ] The toggle in the top-right corner turns it off at once. Off, the app draws no extra frames.
- [ ] PLAN, NOTES, the Planned Features list and the feature map are updated.
