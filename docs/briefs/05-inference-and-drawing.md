# Chunk 5: Inference & Drawing

**Goal.** The feel of the prototype, running natively: the inference engine (snaps, woken points, guides, locks), the Line and Rectangle tools, the measurement box with units, and the rest of the Select tool. This is the part the plan calls "monumental", so it gets the most care and the most tests.

**Read first:** PLAN.md section 1 (items 2–6, 8, 9), 1.4 (Select, Line, Rectangle), 1.7 (the Select, Line and Rectangle rows), section 5 (the engine), Appendix A ("Measurement box" and "Inference and locks"). DECISIONS #4 and #5. NOTES.md, "For the next Claude Code session" item 3 (box select) and the Viewport section.

## The prototype: same behaviour, different programming
As in the Viewport chunk, the feel test (`docs/prototype/Main.dc.html`) shows **what** to build, and its numbers are the starting values. Do not port its code. In particular, its tolerances are absolute numbers (`5e-4`, `2e-3`, `500`, `1e-3`), and it reads the clock with `setTimeout`. Prism's engine uses the model's relative tolerance and pixel distances, and gets the time as an input.

**Take from the prototype** (`infer(`, `guideLines(`, `trackDwell(`, `wakeCheck(`, `GLYPH`, `onKey(`, `enter(`, `parseLen(`, `commitSeg(`, `rotNormal(`, `rectBasis(`, `commitBox(`, `drawOverlay(`):

- **Candidates.** Priority and capture radius at aperture 1, in pixels:

  | Candidate | Priority | Radius (px) |
  |---|---|---|
  | Endpoint | 6 | 14 |
  | Guide Cross | 5.5 | 12 |
  | Midpoint | 5 | 12 |
  | Intersection (a guide crossing an edge) | 4.6 | 12 |
  | Guide (on an axis or parallel guide) | 3.3 | 8 |
  | On Edge | 3 | 9 |
  | On Face | 1 | under the cursor |
  | On Ground | 0.8 | under the cursor |

  Score = priority − distance / radius. Points hidden behind faces are not candidates; guides are.
- **Hysteresis.** The current snap stays a candidate out to 1.6 times its radius (the "snap release" slider). It keeps winning unless another candidate scores at least 0.5 more.
- **Woken points.** Hovering an Endpoint, Midpoint or On Edge for 220 ms (the "hover wake" slider) wakes it. At most 5 are kept, oldest dropped first. A point is drawn as a small diamond outline; an edge is drawn in magenta.
  - Each woken point casts red, green and blue guides, labelled "Red from Point" and so on.
  - The tool's start point casts axis guides labelled "On Red Axis".
  - Each woken edge casts a magenta "Parallel to Edge" guide from the start point.
  - Waking resets when a segment is committed or the tool changes. Esc with nothing else to do also clears them.
- **Locks.**
  - **Shift**, held while the snap is on an axis guide, locks to that axis. Releasing Shift unlocks.
  - **Up, Right and Left** toggle a lock to blue, red and green. **Down** releases.
  - While locked, a point snap (Endpoint, Midpoint, Intersection, Guide Cross) is projected onto the locked line, so you can lock to red and pick up a far point's red position.
  - The status bar shows the lock in a chip.
- **Free point** (nothing to snap to): with a start point, on the plane through the start that faces the view's main axis; otherwise at the target's distance along the ray.
- **Feedback.**
  - **Glyphs** at the snap point, by shape and colour: Endpoint green circle, Midpoint cyan triangle, On Edge red square, On Face blue diamond, On Ground grey diamond, Intersection black cross, Guide Cross magenta cross. They take the axis colour when on an axis.
  - **Label** in a small dark pill near the cursor (Plex Mono 12).
  - **Target highlight:** the edge snapped to gets a wide faint glow and a 2.8 pt line in the snap colour; the vertex snapped to gets a ring.
  - **Guides** the snap sits on are drawn dashed (5 and 4 px, 1.6 pt) in their colour, from their origin to a little past the snap.
- **Line tool (L).**
  - Click a start, then click each next point. Lines chain from the last end.
  - The line stops chaining when the segment just drawn closed a face. Esc stops at any time.
  - The rubber band is 1.8 pt, in the axis colour when on an axis, with its length in a pill at its middle.
  - Type a length and Enter: the segment goes along the current direction, or along the locked axis.
- **Rectangle tool (R).**
  - Click the first corner, then the opposite one. The rectangle is drawn as a tinted preview with its size in a pill.
  - **The plane comes from**, in order: the locked axis (arrows before the first click), the face under the cursor, the ground, else the view's main axis. The two sides line up with the world axes where they can (`rectBasis(`).
  - Type `w,h` or one number for a square, and Enter. Each value keeps the side the cursor is on.
- **Measurement box.**
  - It sits at the right of the status bar, as in the prototype: a label (Length, Size) and a Soft well with the value in Plex Mono 14. While you type, the value shows in the accent colour with a caret.
  - Typing goes straight to it with no field to click: digits, `.`, `-`, a separator, then letters and `'` `"` for units. Backspace deletes, Enter applies, Esc clears what was typed.
  - Units (`parseLen(`): `mm`, `cm`, `m` (the default), `in` or `"`, `ft` or `'`, and feet-and-inches such as `2'6"`. Values are shown with 3 decimals in metres (`1.234 m`) until Document Settings exists.
  - Separator, from DECISIONS #5: `,` between values where the decimal mark is `.`, `;` where it is `,`, and `;` is always accepted. Use the operating system's locale where Rust can read it; otherwise assume `.`. Record which in NOTES.
- **Select tool (Space)**, on top of the click, double-click and triple-click already built:
  - **Shift** toggles items in and out of the selection; **Ctrl** (Cmd on a Mac) adds.
  - **Box select:** dragging left to right selects what is wholly inside (window, solid outline). Dragging right to left selects what the box touches (crossing, dashed outline). A drag that starts on geometry is still a box. Drags under 6 px count as clicks. Shift and Ctrl work with the box too.
  - Faces count as inside or touched by their outline; points need to be in the box for both kinds.

## Requirements beyond the prototype (from the plan)
- **New crates.**
  - `prism-infer`: the engine as a pure function. It takes the scene, the camera, the cursor, the woken points, the locks, the tool's context, the previous snap and the time. It returns the snap point and its kind, the guides to draw, the glyph, the label and what is under the cursor (PLAN 5). It holds no state of its own; the woken-point memory is a small value the caller keeps and passes in.
  - `prism-tools`: the Line, Rectangle and Select tools as state machines, `(state, input, inference) → (state, preview, commands)`. They never touch the GPU or the clock.
  - Both go in the purity test with the core crates.
- **Every edit is a command.**
  - A committed segment is one `Command::Segment`; a rectangle is one command, so it undoes in one step. Add a `Command` for a rectangle with any four corners on a plane, since the current `Rect` is flat on the ground only.
  - Esc cancels the step in progress (the Esc rule's "operation" case) and leaves the tool active.
- **Edit the last operation (Appendix A ★).** Right after a Line segment or a Rectangle is committed, typing a new value and Enter changes that segment's length or that rectangle's size. It works as undo plus a new command.
- **Rectangle memory (PLAN 1.7).** The last width and height show in the measurement box when the tool is picked. Enter after the first click reuses them.
- **Tool hints** in the status bar, from the prototype's `hintText(`.
- **Tuning sliders** in the debug panel: the pick aperture (already there), snap release (1.6) and hover wake (220 ms). They are the prototype's Feel Panel.
- **Recorded sessions (PLAN 5, "recorded real sessions replayed in CI").** A debug action records the mouse and keys in the viewport with the camera and model to a JSON file (natively in Downloads, on the web as a download). A test replays every file in `docs/acceptance/sessions/` and checks the model it ends with. This lets the owner send a session that went wrong.
- **Speed (DECISIONS #8).** Inference under 16 ms per mouse move on the Large Scene, measured and recorded in NOTES. A brute-force scan is fine if it meets that; otherwise add a simple index here.

## Out of scope
- Smart Guides (they wait for groups; NOTES, "For the next Claude Code session" item 2).
- Face-local axes, tangent guides, arcs and circles.
- Edge-to-edge intersections that are not guides.
- Alt to cycle inference modes, Rectangle from centre (Ctrl) and the 3-point rectangle (Alt).
- `[x,y,z]` and `<dx,dy,dz>` coordinates and arithmetic in the measurement box.
- Push/Pull, Move, Rotate, Scale and Eraser (Edit Tools).
- Document Settings.

## Tests
- **Each candidate kind,** each at scales 1e-4, 1 and 1e3:
  - It is found where expected and missed outside its radius.
  - It is not found when hidden behind a face.
  - Priority order holds where candidates overlap.
- **Hysteresis and stability:**
  - A snap holds out to 1.6 times its radius and lets go after.
  - A property test: a cursor jitter of under 1 px never changes the snap.
- **Woken points:** they wake only after the dwell time, at most 5 are kept, and their guides and guide crosses appear. The time is passed in; no real waiting.
- **Locks:** Shift and the arrows lock and release as described, and a far point projects onto the locked axis.
- **Scenario tests** (golden): a scene, a camera and a cursor path give an expected sequence of snaps. Include the classic SketchUp moves:
  - wake a corner, then draw from elsewhere until "Red from Point" crosses "On Green Axis";
  - lock red and pick up a far endpoint;
  - draw a line across a face and split it;
  - close a loop and get a face.
  Where the prototype can be run headless (as for the golden cases), take the expected sequences from it.
- **Measurement box parsing:**
  - Every unit, and feet-and-inches.
  - Both separators, and bad input rejected.
  - Typed length, typed rectangle size, and editing the last operation, each giving exact geometry.
- **Box select:** window and crossing on the cube and the pocket box, with Shift and Ctrl.
- **Session replay** for at least one recorded session.

## Acceptance
- [ ] Drawing a box from lines and rectangles feels like the prototype, natively and on the web: snaps, woken points, guides, locks, glyphs and labels.
- [ ] Typed lengths and sizes give exact geometry, in every unit, and the last operation can be retyped.
- [ ] Select does click, double, triple, Shift, Ctrl and both box kinds.
- [ ] Every edit undoes and redoes in one step.
- [ ] Inference timing on the Large Scene is in NOTES.
- [ ] PLAN 6.0 "Where we are" is updated, and NOTES lists what is left.

## Decided by the owner
- **Down arrow** releases the lock (DECISIONS #17).
- **Up arrow in Rotate** locks the protractor to a face's plane (DECISIONS #18). Rotate is built in Edit Tools, but the engine here must let a tool lock its plane and keep snapping while the lock is moved, so build the lock as a plane as well as an axis.

## Open (default used)
- **Perpendicular guides.** The plan lists them; the prototype has only "Parallel to Edge". *Default: parallel only, as in the prototype.*
