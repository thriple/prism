# Open decisions and the defaults to use

Until the owner decides, use the default. Note in `NOTES.md` whenever a default shaped your work.

| # | Decision | Status | Default to assume |
|---|---|---|---|
| 1 | Licence of Prism itself | Decided | Open source, MIT or Apache-2.0. Add dual `LICENSE-MIT` and `LICENSE-APACHE` files when the first release is cut. Use only permissively licensed dependencies (MIT, Apache-2.0, BSD, zlib, MPL). Never copy GPL code. |
| 2 | Name | Decided | Keep "Prism". |
| 3 | Platform | Decided | Rust, wgpu and winit. Native Windows and macOS builds, plus a web build (PLAN.md 3.1). |
| 4 | Input devices | Decided | Mouse and keyboard only. The wheel only zooms. On trackpads, two-finger scroll pans, pinch zooms at the cursor, and orbit is by the Orbit tool or middle-drag only. Add a greyed-out **Help > Pen and tablet input** toggle. No pen or touch. |
| 5 | Separator in the measurement box | Decided | Locale-aware: comma between values where the decimal mark is `.`, semicolon where it is `,`. Always accept `;`. Units and separator are set in File > Document Settings. |
| 6 | Scripting and automation | Decided (stub) | Add a greyed-out top-level **Scripting** menu. No API yet. Keep the action registry and command log clean so one can be added later. |
| 7 | What the first usable build contains | Decided | Skeleton, Core Model, Viewport, Actions & Keys, Inference & Drawing, Edit Tools, Files & Formats. Nothing else until that build feels right. |
| 8 | Performance targets | Decided (provisional) | Reference machine: i7-5820K 3.3 GHz, 32 GB RAM, 8 GB VRAM. Stay interactive (orbit 30 fps or better, picking and inference under 16 ms) on a model about twice the size of a SketchUp file that struggles there (over about 50 MB). The first chunk that renders measures what that means in triangles and edges and records it in `NOTES.md`. |
| 9 | NURBS kernel | Open | Not needed yet. Design the edge and face types so exact curves and surfaces can be added (PLAN.md 1.1), but implement lines and planes only. |
| 10 | iPad | Open | Hosted web build only. Check that it runs at all and record the result in `NOTES.md`. |
| 11 | Existing Android video player in the repo | Decided | Deleted in the first commit of the plan. It is in git history. |
| 12 | Fonts under the SIL Open Font Licence | Decided | Allowed for bundled fonts (not code). Use IBM Plex Sans and IBM Plex Mono. |
| 13 | Large display type from the Motion Kit | Decided | Not used. Ignore the giant header type in `soft.html`, here and in any app screen. |
| 14 | Push/Pull of a ring face (a face with a hole, around another face) | Decided | The pulled face becomes the top; the bottom is not closed. Only Alt (new loop) keeps the original face in place. Golden case `pushpull_ring_face` is now verified. |
| 15 | What the Eraser does to the faces beside an edge | Decided | Erasing an edge deletes every face that uses it (both faces that share it), not a merge into one face. |
| 16 | Orbit, Pan and Zoom (O, H, Z) picked while another tool is active | Decided | They stay active after the mouse is released; Esc returns to the previous tool. They are not temporary. |
| 17 | What the Down arrow does while drawing | Decided | Down releases the lock (as in the feel test). It does not lock parallel or perpendicular. |
| 18 | Locking the Rotate tool's protractor | Decided | The protractor is a full circle. In Rotate, Up locks it to the plane of the face under it; you can then move the locked protractor to another place and click and drag there to start the rotation, as in SketchUp. Rotate uses the same snaps as the other tools. Built with Edit Tools. |
| 19 | Perpendicular guides while drawing | Decided | Not wanted for now: only "Parallel to Edge" guides, as in the feel test. |
