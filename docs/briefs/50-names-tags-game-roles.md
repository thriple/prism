# Names, Tags and Game Roles (owner request, 2026-11-21)

**Goal.** Three things the owner asked for while connecting Prism to Beverly (their Bevy game editor):
1. **A new component asks for its name** straight away: the Inspector opens with the Name field ready for typing, and Enter finishes.
2. **Tags can be typed in.** The owner asked what the method for entering tags is; today it is clumsy (below), so this plans a better one.
3. **Game roles for Beverly:** mark a group or component as a **collider** or **breakable**, so Beverly gets the information in the file instead of setting it all again by hand.

**Read first:** brief 35 (the GLB export for Beverly; "Beverly-specific settings in the file (collision, health, breakable)" was left out of it on purpose, and this brief takes that part up), brief 48 (the Inspector), NOTES "Export for Beverly", "Groups & Components part 3" (tags).

## How tags work today (found 2026-11-21)
- A tag is a document object (`TagId`, a name and a visible flag). An instance (group or component) has **one tag or none**. Tags only decide what is shown or hidden (like SketchUp's layers); loose faces and edges cannot carry one.
- **Entering one:** Window > Tags. **Add tag** makes a tag named "Tag" at once (no prompt, so two clicks make two tags both called "Tag"); to name it, press **Rename**, type, press **Save** (Enter does not save). To use it, select groups and press **Put selected**, or choose it in the Inspector's **Tag** row or in Entity Info (a choice among the tags that exist: a new tag cannot be typed there).
- Gaps: no typing a new tag where it is needed; no Enter to finish; "Tag" placeholders pile up; the name clash is not checked; there is no way to give a loose set of faces a tag without grouping first.

## Part 1: name a component when it is made (S)
- **Make Component** (Edit menu, context menu, hotkey) makes it exactly as now, then **opens the Inspector** if it is closed, selects the new component, and puts the cursor in the **Name** field with the default word ("Component") selected. Typing replaces it; **Enter** keeps the name and returns the keys to the viewport; **Esc** keeps the default name (the component is still made).
- The making and the naming are **one undo step** (the transaction stays open until Enter or Esc, as a typed Push/Pull distance does).
- **Default:** a name another component already has is refused in red ("A component is already called Wall") and the field stays open. Groups are not asked (they are unnamed by default); a preference "Ask for a name when making a group" is left for later.
- Tests: the field is focused with the default selected, Enter names it, Esc keeps the default, one undo step removes both, a clashing name is refused. Applies through MCP too (`run_action edit.makecomp` then `set_inspector name`).

## Part 2: tags you can type (S)
- **Inspector:** the **Tag** row becomes a text field with the tags that exist listed under it. Type a name and press Enter: an existing tag is used; a new name **makes the tag and puts it on the selection** in one undo step. Empty or "None" takes it off.
- **Tags window:** **Add tag** opens a new row with its name field focused (no "Tag" placeholder); **Enter** saves, Esc drops it. Rename also takes Enter. A name already used is refused in red. Entity Info gets the same field.
- **Default kept:** one tag per group or component (SketchUp's behaviour); several tags on one thing is not planned. Tags stay a showing and hiding tool: Beverly's roles are separate (part 3), so a tag never changes what a file says to a game.
- Tests: typing a new tag in the Inspector makes and applies it in one step, Enter in the window, the clash refused, undo, MCP `set_inspector tag`.

## Part 3: game roles for Beverly (M)
- **A Role on a group or component:** *None*, *Collider* (the physics shape: it need not be drawn) or *Breakable* (it breaks apart: a wall in chunks has each chunk, or the whole wall, marked). Set in the Inspector (a choice row **Game role**), in Entity Info, and from **Edit > Game Role** (also the right-click menu), so it works on several selected groups at once. One undo step.
- **Where it is kept:** on the **component** (every copy shares it: the definition) and on a **group** (its own). A copy of a component can be given its own role later if Beverly needs it (not now).
- **Faces and shapes:** a role belongs to a group or component, not to a loose face. **Edit > Game Role** on loose faces first **makes a group of them** (named after the role, "Collider" or "Breakable", for the owner to rename) and sets the role in the same undo step, so "this shape is a collider" is two clicks.
- **In the GLB** (File > Export > Game engine): the node's `extras` gets `"role": "collider"` or `"breakable"` next to `kind` and `definition`; a node with no role has none (so files without roles are byte for byte as now). An export option **Put the role in the name** adds a suffix for engines that read names (`Wall_collider`, `Chunk_breakable`; the one place the owner's "with a group or component name" lands), off by default. The GLB import reads either back (the `extras`, or the suffix, which it takes off the name) and sets the role.
- **In Prism:** a small badge in the Outliner and Entity Info; **View > Game Roles** (off by default) draws a collider's outline in green and a breakable's in orange so the roles can be checked at a glance. A collider is **not hidden** in Prism (Beverly decides what to draw).
- **File:** the role is stored with the group or component in `groups.json` (a new optional field, a schema bump with a no-op migration).
- **MCP:** `get_inspector` and `set_inspector` rows `role`; `list_groups` shows each role.
- Tests: set on a group, a component (all copies), several at once, loose faces grouped, undo; the GLB `extras` and the suffix option, a round trip through export and import; the file round trip; the old files unchanged.

## Questions for the owner (defaults are used until answered)
1. **Names:** are `collider` and `breakable` the exact words Beverly reads, and does it read `extras` or the node name? (Default: both, the name part off.)
2. **Both at once?** Can one thing be a collider and breakable? (Default: no, one role each.)
3. **Breakable on the wall or on each chunk?** Does Beverly want the role on every chunk, or on a parent group whose children are the chunks? (Default: on whichever group carries it; the export writes it on that node only, children inherit nothing, so mark each chunk, or the parent, as Beverly needs.)
4. **Other settings** (health, collision layers, mass) stay Beverly's own, as in brief 35, unless asked.

## Not in this brief
Several tags on one object; a role on a single copy of a component; physics settings; Beverly reading `.prism` files directly (brief 35's non-goal still stands).

## Order
Parts 1 and 2 are small and independent; part 3 is the one Beverly needs. The owner chooses the order.
