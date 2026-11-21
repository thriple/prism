# Names, Tags and Game Roles (owner request, 2026-11-21)

**Goal.** Three things the owner asked for while connecting Prism to Beverly (their Bevy game editor):
1. **A new component asks for its name** straight away: the Inspector opens with the Name field ready for typing, and Enter finishes.
2. **Tags can be typed in.** The owner asked what the method for entering tags is; today it is clumsy (below), so this plans a better one.
3. **Game words for Beverly:** words typed on a group or component (and a Surface word on a material) such as `collider`, `breakable`, `stone`, written into the GLB so Beverly can set the piece up instead of the owner doing it again by hand. Beverly's own brief for what the file carries is `50a-prism-to-beverly-glb.md`.

**Read first:** brief 35 (the GLB export for Beverly; "Beverly-specific settings in the file (collision, health, breakable)" was left out of it on purpose, and this brief takes that part up), brief 48 (the Inspector), NOTES "Export for Beverly", "Groups & Components part 3" (tags).

## How tags work today (found 2026-11-21)
- A tag is a document object (`TagId`, a name and a visible flag). An instance (group or component) has **one tag or none**. Tags only decide what is shown or hidden (like SketchUp's layers); loose faces and edges cannot carry one.
- **Entering one:** Window > Tags. **Add tag** makes a tag named "Tag" at once (no prompt, so two clicks make two tags both called "Tag"); to name it, press **Rename**, type, press **Save** (Enter does not save). To use it, select groups and press **Put selected**, or choose it in the Inspector's **Tag** row or in Entity Info (a choice among the tags that exist: a new tag cannot be typed there).
- Gaps: no typing a new tag where it is needed; no Enter to finish; "Tag" placeholders pile up; the name clash is not checked; there is no way to give a loose set of faces a tag without grouping first.

## Part 1: name a component when it is made (S) — built 2026-11-21
- **Make Component** (Edit menu, context menu, hotkey) makes it exactly as now, then **opens the Inspector** if it is closed, selects the new component, and puts the cursor in the **Name** field with the default word ("Component") selected. Typing replaces it; **Enter** keeps the name and returns the keys to the viewport; **Esc** keeps the default name (the component is still made).
- The making and the naming are **one undo step** (the transaction stays open until Enter or Esc, as a typed Push/Pull distance does).
- **Default:** a name another component already has is refused in red ("A component is already called Wall") and the field stays open. Groups are not asked (they are unnamed by default); a preference "Ask for a name when making a group" is left for later.
- Tests: the field is focused with the default selected, Enter names it, Esc keeps the default, one undo step removes both, a clashing name is refused. Applies through MCP too (`run_action edit.comp` then `set_inspector name`).
- **Built as:** `Session::make_component` records what it was made from (`Naming`) and raises a one-shot `take_name_request()`, which the window takes each frame to open the Inspector and put the keys in the Name field with the default selected (`Panels.inspector_focus`). Enter sends `set_inspector name`; with nothing else done since, `name_new_component` undoes the making and does it again with the name, so it is one undo step (after another step it is an ordinary rename). Esc in the field (any Inspector text field now drops what was typed on Esc) sends `name_cancelled`: the default stays. A component name another component has (case aside) is refused in the Inspector with "A component is already called X", for any rename of a component.

## Part 2: tags you can type (S) — built 2026-11-21
- **Built as:** the Inspector's Tag row is `FieldKind::Suggest` (a text field with the existing tags and None offered as buttons); a new name makes the tag and puts the selection on it in one `Batch` (AddTag + SetTag, one undo step); a name matches an existing tag case aside; empty or None takes it off. Tags window: Add tag opens a name field with the keys (Enter makes it, Esc drops it), Rename takes Enter and Esc too; `Session::tag_command` returns `Result` and a taken or empty name shows in red (`Panels.tag_error`). Entity Info has the typed field too. Still one tag per object.
- **Inspector:** the **Tag** row becomes a text field with the tags that exist listed under it. Type a name and press Enter: an existing tag is used; a new name **makes the tag and puts it on the selection** in one undo step. Empty or "None" takes it off.
- **Tags window:** **Add tag** opens a new row with its name field focused (no "Tag" placeholder); **Enter** saves, Esc drops it. Rename also takes Enter. A name already used is refused in red. Entity Info gets the same field.
- **Default kept:** one tag per group or component (SketchUp's behaviour); several tags on one thing is not planned. Tags stay a showing and hiding tool: Beverly's roles are separate (part 3), so a tag never changes what a file says to a game.
- Tests: typing a new tag in the Inspector makes and applies it in one step, Enter in the window, the clash refused, undo, MCP `set_inspector tag`.

## Part 3: tags for Beverly (owner's decision, 2026-11-21) — built

**This replaces the "game roles" design of Beverly's addendum (`50a`).** The owner: it should just be a tag system, and Beverly interprets the words; Prism only provides the context. So there is **no separate "role" concept**, no Game role row, no Edit > Game Role and no View > Game Roles. Tags are the words.

- **Several tags per object.** An object can be on any number of tags (it used to be one). It is hidden if any of its tags is hidden. The Inspector's **Tags** row takes them typed, comma between (`Walls, breakable, stone`; Enter sets exactly that list in one undo step; None clears), with the tags that exist as buttons: a click puts that tag on or takes it off and leaves the rest (`tag.toggle`). Entity Info has the same. Commands: `TagOn`, `SetTags` (and `SetTag`, the old "only this one"). A file writes the first tag as before (`tag`) and the rest as `more_tags`, so a file with one tag is byte for byte what it was.
- **A Beverly list in the Tags window.** A collapsible **Beverly** header lists the words Beverly acts on (`prism_tools::BEVERLY_TAGS`, from 50a: collision, physics, surface, material, parts); click a word to put it on the selection (the tag is made if it is not one yet, in the same undo step), click again to take it off. Prism does not know what any word means, and any other word can be typed. The list grows on Beverly's side.
- **In the GLB** each node's `extras.prism.tags` is the list of tag names (left out when there are none; old output is the same). **This is a change from 50a, which said `tag` (one name) and `roles`: Beverly should read `extras.prism.tags` (a list of strings) and the node and material names. There is no `roles` and no material `surface`.** Prism reads `tags` back on import (and the older single `tag`): found by name, or made.
- **Material tags** (a word such as `stone` on a material) and **the name-suffix option** were part of the old design and are not built; the material's name already works for Beverly ("Stone wall"). Say if you want tags on materials too.
- **Hidden colliders:** nothing special now. A hidden object is left out of the export as before (tags do not change that); if Beverly needs hidden collision shapes exported, say so.
- **MCP:** `get_inspector` / `set_inspector` row `tag` (comma-separated names) and `tag.toggle`; the selection shows `tags` (a list).
- **Tests:** several tags typed and toggled, hidden if any tag is hidden, a Beverly word toggled on and off and refused with nothing selected, one undo step, the GLB `tags` on the right node only and read back, a file with several tags.

## Answers so far (owner, 2026-11-21) and what is still open
1. **Exact words, `extras` or names?** Not settled on the owner's side. Beverly's addendum says it reads `extras` first and also the words in names, so both ways work; the default (write both, the name suffix off) stands.
2. **Both at once?** Yes: a list of words; Prism just saves them.
3. **Where does breakable go?** Anywhere: the whole exported model (a window, a door, a wall segment) is what breaks, so it does not matter which node carries the word, and Beverly can override it. Complex models (mechs) will more likely use groups and components as the segments than per-piece words. (Beverly's addendum: on the parent group; a breakable group with no groups inside breaks as one piece.)
4. **Other settings** (health, mass, collision layers) stay Beverly's own.
5. **Open:** should the export include `collider` nodes that are hidden? (Default: yes.) Beverly already makes meshes collidable by default, so the `collider` word is mostly for the cases where only some of a model should collide.

## Not in this brief
Several tags on one object (roles are the several words); a role on a single copy of a component; physics settings; Beverly reading `.prism` files directly (brief 35's non-goal still stands).

## Order
Parts 1 and 2 are small and independent; part 3 is the one Beverly needs. The owner chooses the order.
