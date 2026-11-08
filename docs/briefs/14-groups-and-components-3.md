# Chunk 8: Groups & Components (part 3: hiding, tags and panels)

**Goal.** Groups and components can be hidden, named and tagged, and seen as a tree. A hidden object costs memory and nothing else (PLAN 1.14, "hidden must be free").

**Read first:**
- PLAN 1.14 ("Hidden geometry", "Hidden must be free", "Hierarchy tree") and 1.6;
- NOTES "Groups & Components (part 1)" and "(part 2)".

## Requirements
- **Instances get:**
  - a name (optional, editable);
  - a tag (optional);
  - a hidden flag.
  Instance ids are unique in the document, so commands name an instance by its id wherever it is in the tree.
- **Tags:** the document has named tags, each shown or hidden. A hidden tag hides every instance on it.
- **Hide and unhide (Edit menu, right-click menu):**
  - Hide Selected hides the selected groups and instances.
  - Unhide All shows every hidden instance in the open group.
  - Unhide Last shows what the last Hide hid.
  - Isolate hides every other instance in the open group.
  - Each is one undo step.
- **Hidden must be free:**
  - A hidden instance, or one on a hidden tag, is not built into the scene. So it is not drawn, picked, snapped to, box-selected or taken by Select All.
  - It is not exported (mesh export writes what is visible).
  - It still costs memory, saving and undo.
  - A test checks that a model with 90% hidden builds its scene in about the time of the visible 10%.
- **Entity Info (Window menu):** for the selected group or instance:
  - its name (editable);
  - its definition (group, or component name);
  - its tag (pick from the list);
  - hidden or not.
  For loose geometry, it shows the counts and the face area.
- **Tags window (Window menu):**
  - lists tags with a show or hide toggle and the number of instances on each;
  - adds a tag;
  - renames a tag;
  - puts the selected groups on a tag.
- **Outliner (Window menu):**
  - The tree of instances, nested, by name (or "Group" or the component's name), with an eye to hide or show each.
  - A click selects that instance (in the viewport too, opening its parent group if needed).
  - A double-click opens it.
  - Hidden rows are greyed.
- **Files:** `groups.json` keeps instance names, tags and hidden flags, and the tag list. Part 1 and 2 files open with none.
- **Menus:** Hide, Unhide All, Unhide Last, Isolate, Outliner, Entity Info and Tags become active (DECISIONS #20).

## Out of scope
- Hiding loose faces and edges (PLAN 1.14 recommends hiding groups; loose hidden geometry and its overlap rules come later).
- Show Hidden (ghosted), Lock, scenes remembering tags.
- Dragging rows in the Outliner to regroup; search and filters.

## Tests
- **Data:**
  - Hide, unhide, unhide last and isolate, with exact undo.
  - Tags hide their instances.
  - Names and tags round-trip through a file.
- **Session:**
  - A hidden group is not picked, not snapped to and not in Select All.
  - Unhide brings it back.
- **Speed:** the 90% hidden test (written to NOTES).
- **Export:** hidden groups are left out.

## Acceptance
- [ ] Hiding a heavy group makes the scene as quick as if it were not there.
- [ ] The Outliner, Entity Info and Tags windows work with the viewport selection.
- [ ] PLAN 6.0 is updated, and NOTES lists what is left.
