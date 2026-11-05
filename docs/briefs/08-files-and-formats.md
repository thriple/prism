# Chunk 7: Files & Formats (part 1)

**Goal.** Your work is safe. Models save to and open from `.prism` files, natively and on the web; a crash or a closed tab loses at most a few seconds; and lengths show in the units you pick. Mesh import and export (STL, OBJ, 3MF, glTF, PLY) and drag and drop follow in part 2.

**Read first:**
- PLAN.md 1.13;
- DECISIONS #7, #24 and #25;
- `docs/research/files-and-formats.md`: crates, licences, and what winit and the browsers can and cannot do;
- `crates/prism-app/src/storage.rs`: where settings are kept today.

## Requirements
- **Exact numbers first.** Turn on serde_json's `float_roundtrip` feature in the workspace, with a test that f64 values come back from JSON bit for bit. The research found about 11% did not.
- **A new pure crate, `prism-file`.** It turns a document into `.prism` bytes and back. It reads no clock, does no file input or output, and gives the same bytes for the same document. Add it to the purity test.
  - **Container:** a zip, deflate only. Use `zip` without default features, and `flate2` with its Rust backend, as listed in the research. Every entry gets a fixed timestamp, so the file is reproducible.
  - **`manifest.json`:** the format name, the schema version, the app version that wrote it, the units, and a list of the entries.
  - **`document.json`:** the camera, units and display settings, and anything else that is not geometry. Fields this version does not know are kept and written back.
  - **`geometry/model.bin`:** the model in postcard. Stable ids and the id counters are kept, so ids stay the same after a save and an open. The binary layout is its own versioned struct in `prism-file`, not the in-memory `Model`, so the two can change separately.
  - **`thumbnail.png`:** optional (the app may add one later). The reader must not need it.
  - **Versions:** a file from a newer schema is refused with a clear message. Older schemas open through migration functions, one per version step. The framework is built now, with a test that uses a made-up version 0.
  - **Errors:** an unreadable file is refused with a reason, and never panics. Test this with damaged and cut-short files.
- **The undo history is not saved** (PLAN 1.13). After Open, Undo has nothing to undo.
- **File menu:**
  - New, Open…, Save, Save As… and Recent work, with their shortcuts.
  - The window title shows the file's name, with a dot when there are unsaved changes.
  - New, Open and closing the window ask "Save changes?" when there are unsaved changes: Save, Don't Save, Cancel.
  - On the web, closing the tab with unsaved changes shows the browser's own warning.
- **Saving natively:**
  - Open and save dialogs from `rfd`.
  - An atomic save: write `name.prism.tmp` in the same folder and flush it; keep the old file as `name.prism.bak`; rename the new file into place.
  - A save that fails leaves the old file untouched, and the status bar says why.
  - Recent files (up to 10) are kept in the config folder.
- **Saving on the web:**
  - Chrome and Edge save in place through the File System Access API (`showSaveFilePicker`, then `createWritable`).
  - Other browsers download a copy.
  - Open uses the file picker in every browser.
- **Autosave and recovery:**
  - While there are unsaved changes, every 10 seconds (and at most once a second) Prism writes a recovery journal. The journal is the model at the last save or open, plus the commands run since, which is the log `prism-doc` already keeps. The base model is written once per save; after that only the commands are rewritten.
  - Natively it lives in the config folder's `recovery` folder. On the web it lives in IndexedDB.
  - A clean save, or closing without saving, clears it.
  - At start-up, a journal left behind gives a prompt: "Prism closed without saving. Restore your work?" Restore replays the commands with `Document::replay`. Discard clears the journal.
- **Opening from the command line** (DECISIONS #24): `prism path/to/file.prism` opens the file. The Mac app's `Info.plist` declares the `.prism` document type. The Apple event handler and Windows file-type registration wait for a packaging step.
- **Units (File > Document Settings…):**
  - Pick the display unit: mm, cm, m, inches, or feet and inches.
  - Pick how many decimals to show.
  - Lengths show in that unit everywhere: the measurement box, pills and labels.
  - Typing still accepts any unit.
  - The setting is saved in the document. The decimal mark follows DECISIONS #5 as now.
- **Greyed out until built (DECISIONS #20):** New, Open, Save, Save As, Recent and Document Settings become active now. Import stays greyed until part 2.
- **DEPENDENCIES.md** gets a row for each new crate, with its licence and whether it builds for the web.

## Out of scope
- Mesh import and export, drag and drop, the import dialog and Model Health after import: part 2.
- Groups and components in the file: they come with Groups & Components. The format must leave room for one geometry entry per definition.
- Thumbnails, file-type registration on Windows, the Mac Apple event handler, and iPad specifics.

## Tests
- **`prism-file`:**
  - A round trip of the test scenes, the Large Scene, a model with guides and a model with holes gives the same geometry, ids and counters, bit for bit.
  - The same document gives the same bytes twice.
  - A newer schema is refused.
  - A version 0 file migrates.
  - Unknown fields in `document.json` survive a round trip.
  - Damaged, cut-short and empty files are refused without a panic. Use a property test that flips random bytes.
- **Golden files:** `docs/acceptance/files/`, with a few `.prism` files written by this version. CI opens each one, checks the model, saves it again and compares the bytes. Later versions keep these files and must still open them.
- **App (native):**
  - The atomic save leaves a `.bak` and no `.tmp`.
  - A save into a folder that cannot be written leaves the old file as it was.
  - Recovery: write a journal, start a new app state from it, and get the same model.
- **Units:** `NumberFormat` shows and reads each unit, including feet and inches (`2' 6 1/2"`, or `2'6.5"` for decimals).

## Acceptance
- [ ] A model saved natively opens on the web, and the other way round.
- [ ] Killing the app mid-edit and starting it again offers the work back.
- [ ] Unsaved changes are never lost without a question.
- [ ] PLAN 6.0 "Where we are" is updated, and NOTES lists what is left.
