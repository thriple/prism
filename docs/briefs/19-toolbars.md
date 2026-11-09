# Toolbars (owner request, 2026-11-09)

**Goal.** The tool dock becomes a set of toolbars (Principal, Draw, Edit, Construct, Camera, View and the rest) that can be docked on the left rail or in a top dock, floated as small windows inside the app, rearranged, closed and brought back.

**Read first:** `crates/prism-app/src/chrome/dock.rs` and `frame.rs`; the `DOCK` table in `actions/table.rs`; `docs/UI-STYLE.md`.

## Parts
1. **Layout, docks, floating, edit mode** (this brief).
2. **The Toolbars window:** a list of every toolbar in a window in the middle of the app, greyed if it is already showing; a closed one can be dragged out of the list to make that toolbar. Floating toolbars can be reshaped by dragging their edge (more or fewer columns).

## Requirements (part 1)
- **New buttons:** X-ray and Hidden Geometry, in a new **View** toolbar, as toggle buttons that show their state.
- **Layout model** (pure, tested, in `prism-app`): each toolbar is closed, docked (left rail or top dock, with an order), or floating (a position). It is saved in the user's settings (not in documents), and **Reset Toolbars** puts back the default (everything on the left rail, as now).
- **Shapes** (owner, 2026-11-09: as flexible as possible, so a short toolbar is not spread over two lines):
  - Every toolbar is 1 button thick with 5 tools or fewer, and 2 with 6 or more. This applies floating and docked alike.
  - A dock is as thick as its thickest toolbar. Down the left rail, toolbars fill rows; across the top dock they take the same shape turned sideways. So Principal alone makes a one-row top dock.
  - A dock that is too long scrolls (left) or wraps onto another line (top).
  - Part 2 lets the user reshape a toolbar by dragging its edge.
- **Docks:**
  - the left rail;
  - a top dock between the menu bar and the document tabs. It takes no space when empty. While a toolbar is being dragged, an empty dock shows a thin strip to drop on.
- **Floating toolbars:** small windows inside the app (not OS windows), with less padding than the rest of the UI. A slim bar at the top drags them. They snap to each other's edges and to the viewport's edges within a few points.
- **Edit mode:** right-click empty space in a dock or a floating toolbar for **Edit Toolbars**, **Reset Toolbars** and **Toolbars…** (part 2). In edit mode:
  - each toolbar gets a rounded box, a title bar to drag it by, and an X to close it;
  - dropping on a dock docks it at the place it was dropped (before or after the others), and dropping elsewhere floats it;
  - Esc or a "Done" button leaves edit mode.
- Every toolbar action still goes through the one action registry.

## Out of scope (part 1)
The Toolbars window and drag-out, and resizing floating toolbars (part 2). Docking on the right or bottom. Toolbars of the user's own choosing.

## Tests
- Layout:
  - default and reset;
  - closing and reopening;
  - docking at a given place;
  - floating;
  - the shape rules (1 or 2 columns, docks transposed);
  - snapping (edges within the distance meet; farther ones do not);
  - a saved layout reads back, and unknown toolbar names are dropped.
- The View toolbar's buttons run `view.xray` and `view.hidden`.
- Screenshot or hand check: the top dock takes no space when empty.

## Acceptance
- [ ] Toolbars can be docked on the left or top, floated, rearranged, closed and reset, and the layout is kept between runs.
- [ ] X-ray and Hidden have buttons.
- [ ] PLAN and NOTES are updated.
