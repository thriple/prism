# Research: Files & Formats (the next chunk by the plan's order)

Written 2026-11-06 (Guam), before the brief. Nothing here is built yet. PLAN 1.13 is the spec; this file records what was checked so the brief can be short.

## Order

PLAN 6.0 "Suggested order" puts Files & Formats after Edit Tools, then Groups & Components and Model Health. DECISIONS #7 also puts it in the first usable build. So the next brief is `docs/briefs/08-files-and-formats.md`.

## Findings that change the plan or need a decision

1. **JSON does not give back the same numbers by default.**
   - Tested with serde_json 1.0.154 in a scratch project: 11,279 of 100,000 ordinary coordinates (metre-scale points on a circle) came back from JSON one bit different. Random bit patterns were worse: 59,200 of about 200,000.
   - With serde_json's `float_roundtrip` feature, every value came back exactly.
   - Prism already writes models and recorded sessions as JSON (`Model::to_json`, `docs/acceptance/sessions/`). The replay test passes today only because it compares the geometry *written* after the replay, not geometry read back.
   - **Fix:** turn on `float_roundtrip` in the workspace `serde_json` line. It is one line, and it should go in before saving is built, or in the first commit of the chunk. It makes parsing a little slower.
2. **winit does not deliver dropped files on the web.** winit 0.30 has `DroppedFile` on Windows, macOS and X11, but found no sign of it for the canvas on the web. Other projects (notan, bevy_web_file_drop) add their own `dragover`/`drop` listeners on the canvas with `web-sys`. Plan for that: a small web-only module.
3. **winit does not pass on "open this file" on the Mac.**
   - Double-clicking a `.prism` file in Finder, "Open With" and dropping a file on the Dock icon all arrive as an Apple event (`application:openURLs:`). winit does not handle it (winit issue #1751).
   - Known fixes: add that method to the app delegate at run time (as kui-native does), or register an Apple event handler early (as Zed does, because of timing).
   - The app's `Info.plist` must also declare the `.prism` document type (`CFBundleDocumentTypes`), or macOS never sends the event.
   - On Windows, a double-clicked file arrives as a command-line argument. Registering the file type needs an installer or registry entries, and we ship a zip today.
   - In PLAN 1.13 now, as DECISIONS #24 (open, with this as the default): this chunk opens a file given on the command line on both platforms and sets up the Mac plist. The Apple event handler and Windows registration wait for a packaging step.
4. **Saving in place on the web.**
   - `showSaveFilePicker` and `FileSystemFileHandle.createWritable()` work in Chrome and Edge on the desktop. web-sys has bindings for the handle; the picker itself may need a call through `js_sys::Reflect`.
   - Firefox and Safari have no save picker, so Save downloads a copy there (as PLAN 1.13 already says for iPad).
   - `rfd` covers open everywhere, and save as a download on the web.
5. **Autosave and recovery on the web.**
   - The browser's private file system (OPFS) can be written from the main thread with promises. Safari added the main-thread writer only recently (Safari 26, per a WebKit bug comment, not confirmed).
   - IndexedDB works everywhere. Suggest IndexedDB for the recovery journal on the web, through `web-sys` or a small crate such as `idb` (MIT/Apache).
   - We have no WASM threads, so the worker-only sync file API is out.
6. **3MF:** the `threemf` crate is 0BSD. That is more permissive than BSD, but it is not on our licence list (CLAUDE.md). A 3MF file is a zip with one XML model file, so writing it with `zip` and `quick-xml` (MIT) is small. DECISIONS #25 (decided): write our own 3MF.

## Crates checked (crates.io, 2026-11-06)

All are pure Rust. A scratch project with all of them built natively and for `wasm32-unknown-unknown` with no C code.

| Crate | Version | Licence | Use |
|---|---|---|---|
| `zip` | 9.4.0 | MIT | the `.prism` container. Turn **off** default features (they pull in zstd and bzip2, which are C) and use `deflate-flate2` |
| `flate2` | 1.1.10 | MIT or Apache-2.0 | deflate, with `default-features = false, features = ["rust_backend"]` (miniz_oxide) |
| `postcard` | 1.1.3 | MIT or Apache-2.0 | binary geometry chunks; f64 stored as raw bits, so exact |
| `serde_json` | 1.x | MIT or Apache-2.0 | manifest and scene graph, with `float_roundtrip` (finding 1) |
| `rfd` | 0.17.2 | MIT | open and save dialogs; on Linux CI it needs its `xdg-portal` feature (pure Rust) |
| `tempfile` | 3.27.0 | MIT or Apache-2.0 | atomic save: write a temporary file in the same folder, then `persist` (rename). Native only |
| `stl_io` | 0.11.0 | MIT | STL read and write (binary and ASCII) |
| `tobj` | 4.0.5 | MIT | OBJ read. OBJ write is a few lines of our own |
| `gltf`, `gltf-json` | 1.4.1 | MIT or Apache-2.0 | glTF and GLB. The `import` feature pulls in the `image` crate; parse with `Gltf::from_slice` and load buffers ourselves to keep it small |
| `ply-rs-bw` | 4.0.1 | MIT | PLY. The original `ply-rs` has not been updated since 2020; `ply-rs-bw` is the maintained fork |
| `quick-xml` | 0.42.0 | MIT | 3MF's XML |
| `idb` | 0.6.5 | MIT or Apache-2.0 | IndexedDB on the web (finding 5) |

Licences of the whole dependency tree (116 crates): all MIT, Apache-2.0, Zlib, Unlicense or BSD-3-Clause, or a choice that includes one of them. One crate offers LGPL only as one choice of three; we take MIT or Apache.

## Format sketch (for the brief)

- `.prism` = zip:
  - `manifest.json`: schema version, app version, units, tolerance, and a list of the chunks;
  - `document.json`: scene graph, guides, camera, settings;
  - `geometry/*.bin`: postcard, one chunk per definition once groups exist;
  - `thumbnail.png`.
- Postcard does not describe its own fields, so "unknown fields are kept" (PLAN 1.13) can only hold for the JSON parts. Each binary chunk carries its own version, and old versions are read through old structs and migrated. Suggest saying this in the brief.
- Stable ids and the id counters are saved, so ids stay stable across save and load. The undo history is not saved (PLAN 1.13).
- Golden files: save a few models with this version and keep them in `docs/acceptance/files/`. CI opens, saves and compares them.
- Atomic save:
  - write `name.prism.tmp` in the same folder;
  - flush it to disk;
  - rename the old file to `name.prism.bak`;
  - rename the new file over `name.prism`.
  `std::fs::rename` replaces an existing file on Windows as well. The temp file must be in the same folder, so the rename is not a copy across drives.
- Autosave: journal the command log (prism-doc already keeps it) every few seconds or edits. Recovery replays the log from the last full save, which `Document::replay` already does.

## Sources
- winit DroppedFile on web: https://docs.rs/crate/winit/0.30.1/source/src/changelog/v0.10.md, https://docs.rs/crate/notan_web/latest/source/src/files.rs, https://docs.rs/crate/bevy_web_file_drop/0.0.5/source/src/lib.rs
- Mac open-file events: https://github.com/rust-windowing/winit/issues/1751, https://docs.rs/winit-appkit/0.31.0-beta.2/src/winit_appkit/lib.rs.html, https://docs.rs/crate/kui-native/latest/source/src/macos_open.rs, https://git.secluded.site/zed/commit/5be9dc1781ef6d2cbfbdbcd417edd0935cbb83a8, https://doc.qt.io/qt-6/qfileopenevent.html
- File System Access in web-sys: https://docs.rs/web-sys/latest/web_sys/struct.FileSystemFileHandle.html
- OPFS: https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system, https://bugs.webkit.org/show_bug.cgi?id=231706
- Crate data: the crates.io API, 2026-11-06.
