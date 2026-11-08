# Chunk 8: Groups & Components (part 2: components)

**Goal.** A component is a named definition with any number of instances. Editing one instance changes them all; Make Unique splits one off. Instances can be mirrored. Drawing many instances of one definition does not cost a rebuild of each (PLAN 1.2).

**Read first:**
- PLAN 1.2 (Components);
- NOTES "Groups & Components (part 1)";
- `crates/prism-doc/src/assembly.rs` and `crates/prism-tools/src/session/groups.rs`.

## Requirements
- **What a definition is:**
  - A definition is a group's (one instance, copies are independent: DECISIONS #22) or a component's (named, shared by its instances).
  - Component names are unique in the document ("Component", "Component#2", ...), and can be renamed.
- **Make Component (Edit menu, right-click menu):**
  - Like Make Group, but the new definition is a component named "Component" (the next free name).
  - The new instance is selected.
- **Copies of a component instance** share its definition. Move with Ctrl, arrays and Rotate copies all make linked instances.
- **Editing a component:**
  - Open an instance as a group is opened. Edits go to the definition, so every instance changes.
  - While one is open, the other instances of the same definition are drawn normally (not faded), so the change shows on all of them.
- **Make Unique (Edit menu, right-click menu):** each selected instance gets its own copy of the definition, as a new component ("Name#2").
- **Flip Along red, green or blue (Edit menu, right-click menu):** mirrors the selected groups and instances about their own centre, along that world axis. Their faces still face out (the definition's geometry is not changed; the placement is).
- **Explode** of a component instance leaves the definition if other instances use it.
- **Drawing:** each definition's scene is built once per change and placed for each instance, instead of rebuilding the geometry of every instance. (GPU instancing, one draw per definition, can come later if this is not enough; measure with many instances.)
- **Components window (Window > Components):**
  - lists the document's components, each with its number of instances;
  - selects all instances of one (in the open group);
  - renames one;
  - shows nothing for groups.
- **Files:** `groups.json` records each definition's kind and name. Files from part 1 (no kind) open as groups.
- **Menus:** Make Component, Make Unique and Flip Along become active (DECISIONS #20); Window > Components opens the window. Planned Features ticks "Groups and components, open and exit".

## Out of scope
- Glue to and Cut opening (DECISIONS #23).
- A component's own axes and insertion point (the Axes tool), and the component library browser.
- Outliner, Entity Info, tags and hiding (part 3).

## Tests
- **Data:**
  - Copies of a component share its definition; editing inside one changes the others.
  - Make Unique gives an independent definition with a fresh name.
  - Flip keeps the model valid and faces facing out after Explode.
  - Explode of one instance keeps the definition for the others.
  - Undo and redo are exact.
- **Session:** make a component, copy it, open the copy, draw a line, and see the other change; Make Unique, then edit and see the other unchanged.
- **Files:** a component golden file; a part-1 file still opens as groups.
- **Speed:** many instances of one component rebuild in about the time of one plus the placing (written to NOTES).

## Acceptance
- [ ] Editing one instance of a component changes all of them.
- [ ] Make Unique and Flip Along work, with undo.
- [ ] The Components window lists and renames components.
- [ ] PLAN 6.0 is updated, and NOTES lists what is left.
