# Open decisions and the defaults to use

Until the owner decides, use the default. Note in `NOTES.md` whenever a default shaped your work.

| # | Decision | Status | Default to assume |
|---|---|---|---|
| 1 | Licence of Prism itself | Open | Do not add a licence file. Use only permissively licensed dependencies (MIT, Apache-2.0, BSD, zlib, MPL). Never copy GPL code. |
| 2 | Name | Keep "Prism" | Use "Prism". |
| 3 | Platform | Decided | Rust, wgpu and winit. Native Windows and macOS builds, plus a web build (PLAN.md 3.1). |
| 4 | Input devices | Open | Mouse and keyboard. The wheel only zooms. Pinch zooms. Treat two-finger trackpad scroll as zoom until the trackpad defaults are decided. No pen or touch. |
| 5 | Separator in the measurement box | Open | A comma between values (`300,200`), as in the prototype. Also accept `;`. Decimals use `.` only. |
| 6 | Scripting and automation | Open | None. Keep the action registry and command log clean so a scripting layer can be added later. |
| 7 | What the first usable build contains | Proposed | Skeleton, Core Model, Viewport, Actions & Keys, Inference & Drawing, Edit Tools, Files & Formats. Nothing else until that build feels right. |
| 8 | Performance targets | Open | Aim for 1 million triangles and 200,000 edges at 60 fps on an integrated GPU. Record real numbers when a chunk measures them. |
| 9 | NURBS kernel | Open | Not needed yet. Design the edge and face types so exact curves and surfaces can be added (PLAN.md 1.1), but implement lines and planes only. |
| 10 | iPad | Open | Hosted web build only. Check that it runs at all and record the result in `NOTES.md`. |
| 11 | Existing Android video player in the repo | Needs confirmation | Do not delete it. Ask first. |
