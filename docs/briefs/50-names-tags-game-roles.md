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

## Part 3: game words for Beverly (M)

**Widened by Beverly's addendum, `docs/briefs/50a-prism-to-beverly-glb.md` (read it: both programs follow it), and the owner's answers (2026-11-21).** Prism only **stores and exports words**; Beverly decides what each one means (collision, physics, footstep sounds, doors, lights...), and the owner's changes in Beverly win. There is **no Collider button** and no fixed list of roles: the owner types the words (`collider`, `breakable`, `loose`, `stone`, anything), with suggestions offered.

- **A list of roles on a group or component.** The Inspector's **Game role** row is a row of chips with an add field: type a word and press Enter, or pick a suggestion (`collider`, `breakable`, `loose`, `trigger`, `door`, `stone`, `wood`, `metal`... the list in 50a). Click a chip's × to take it off. Any word is allowed and passes through unchanged. One undo step per change. Also in Entity Info and over MCP; **Edit > Game Role** adds or takes off a role on everything selected, and on loose faces it groups them first (named after the word) in the same undo step.
- **Kept on the component** (every copy shares it) and **on a group** (its own), in `groups.json` as a new optional field `roles: ["collider"]`, left out when empty (so files without roles are byte for byte the same; schema bump, no-op migration).
- **A material can say what it is made of:** an optional **Surface** word on a material (suggestions: stone, wood, metal, grass, dirt, sand, gravel, water, snow, carpet, glass, ice, rubber; any word allowed), in the Materials window and the Inspector, stored with the material, left out when empty.
- **Beverly also reads Prism's tag and the words in names** (50a): a tag called `Collider`, or a group called `Floor collider`, works with no role set. Nothing to build for that on Prism's side beyond exporting the tag's name.
- **In the GLB** (File > Export > Game engine), on each node next to `kind` and `definition`: `"extras": {"prism": {"kind": "group", "tag": "Walls", "roles": ["breakable", "stone"]}}`; `roles` in the order given, `tag` the instance's tag name, each left out when there is none, and a node carries only its own (children inherit nothing in the file). On each material, next to `prism_texture_size_m`: `"prism": {"surface": "stone"}`. Material names stay verbatim. **Hidden colliders:** a node with the role `collider` is exported even if it is hidden or on a hidden tag (default until the owner answers; everything else follows the export scope as now).
- **Import** (Prism reading a GLB back) reads `roles`, `tag` (found by name, or made) and `surface`, so a round trip keeps them.
- **"Put the role in the name"** (an export option, off by default) adds the roles to the name as a suffix, for engines that read only names; the import reads it back as roles. Beverly reads `extras` and also ignores role words when matching pieces on a re-import, so turning it on breaks nothing.
- **In Prism:** a small badge for the roles in the Outliner and Entity Info; **View > Game Roles** (off by default) outlines by the first role it knows, `collider` green and `breakable` orange; other words show as the badge only. A collider is not hidden in Prism.
- **MCP:** `get_inspector` and `set_inspector` rows `roles` (a list of words) and, for a material, `surface`; `list_groups` shows the roles.
- **Tests:** several roles on a group, on a component (all copies share them), on several things at once, on loose faces (grouped first), undo; the GLB (`roles` and `tag` on the right nodes only, `surface` on the material, nothing written when there are none: old output byte for byte; a hidden collider is still exported; a round trip through export and import keeps all three); `groups.json` and the material file (new fields round trip, old files unchanged); the name-suffix option and its read back; MCP rows.

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
