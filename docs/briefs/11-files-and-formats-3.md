# Chunk 7: Files & Formats (part 3)

**Goal.** Several models are open at once, in tabs in one window. Double-clicking a `.prism` file opens it in the Prism that is already running, never a second copy of the program (DECISIONS #24, #27).

**Read first:**
- DECISIONS #24 and #27;
- `docs/research/files-and-formats.md`, the section "Tabs and one running Prism";
- `docs/research/risks.md`, items 8 and 9;
- `crates/prism-app/src/files.rs` and `recovery.rs`.

## Requirements
- **Tabs:**
  - A tab bar along the top of the viewport. Each tab shows its file's name, with a dot while there are unsaved changes, and a close button. A "+" makes a new tab.
  - Each tab has its own:
    - document, undo history, selection and camera;
    - tool step in progress;
    - file name and path;
    - autosave journal.
  - Clicking a tab switches to it.
  - Ctrl+Tab and Ctrl+Shift+Tab go to the next and previous tab. They are actions in the registry, so they can be remapped.
- **What opens a tab:**
  - File > New makes a new, empty tab.
  - File > Open, Open Recent and dropping a `.prism` file open a new tab.
  - If the current tab is untitled and unchanged, the file opens in it instead, so starting Prism and opening a file leaves one tab, not two.
  - A file that is already open switches to its tab.
- **Closing:**
  - Closing a tab asks "Save changes?" for that document only.
  - Closing the window goes through the tabs with unsaved changes, asking each in turn. Cancel stops the close.
  - Closing the last tab leaves one empty tab. It does not quit.
- **Recovery:**
  - Each tab keeps its own journal: a folder per tab natively, keys per tab in IndexedDB on the web.
  - At start, every journal left behind is offered back, one tab each.
- **One running Prism (native):**
  - **Windows and Linux:** at start, Prism tries to reach a running Prism through a local socket (a named pipe on Windows, a Unix socket elsewhere). If one answers, it hands over the files named on the command line and quits. Otherwise it listens, and files it is sent open in new tabs.
  - **macOS:** a file double-clicked in Finder, opened with "Open With", or dropped on the Dock icon arrives as an Apple event (`application:openURLs:`). winit does not pass it on, so Prism adds the method to the app delegate at run time and opens each file in a tab. A second copy does not start on macOS.
  - A window brought forward by a hand-over comes to the front.
  - Registering `.prism` with Windows still needs an installer (the packaging step).
- **The web:** tabs work the same. A second browser tab is a separate Prism, as now.

## Out of scope
- Dragging tabs to reorder them, or out into new windows.
- More than one window.
- The Windows installer and file-type registration.
- Cross-tab copy and paste. That comes with the clipboard, which is not built yet.

## Tests
- **Tabs, as pure app logic:**
  - New and Open make tabs;
  - an untitled, unchanged tab is reused;
  - an open file switches to its tab;
  - closing asks only for that tab;
  - closing the window asks each dirty tab and stops on Cancel;
  - closing the last tab leaves one empty tab.
- **Recovery:** two journals left behind come back as two tabs.
- **Hand-over:** sending a path over the local socket reaches the listening Prism (a test with two endpoints in one process).
- **macOS:** the Apple event handler compiles on macOS in CI. The owner tries the double-click by hand.

## Acceptance
- [ ] Two files open side by side in tabs, each with its own undo.
- [ ] Double-clicking a `.prism` file in Finder or Explorer (with "Open With > Prism") opens it in the running Prism.
- [ ] PLAN 6.0 is updated, and NOTES lists what is left.
