# Prism to Beverly: what a GLB carries (addendum to Prism's brief 50, owner request, 2026-11-21)

Both programs follow this file: Prism when it writes and reads a GLB with the "Game engine" preset, Beverly when it
imports one (roadmap feature `prism-import`). It answers brief 50's four questions and widens its part 3 ("Game
roles") from one role to a short list of words, because Beverly now plans to read many words, not only collider and
breakable. Brief 35 (named groups and components, shared meshes, stable names) is unchanged and assumed.

## Goal

The owner marks what things are while modelling, in words: a floor group with a `collider` inside it, a crate that is
`breakable` and `loose`, a material that is `stone`. Beverly reads those words on import and sets the piece up (collision,
physics, footstep sounds, surface tags, doors, lights...), with the owner's changes in Beverly winning over them.
Prism only stores and exports the words; Beverly decides what each one means.

## Where the words come from (Beverly reads all three)

1. **Game roles** on a group or component (brief 50 part 3, widened below): the main way.
2. **Prism's tag** on a group or component: Beverly also reads the tag's name as a word, so a tag called `Collider` or
   `Stone` works. Tags keep their job in Prism (showing and hiding); exporting the name is all that's needed.
3. **Names:** the words in a group's, component's or material's name, split at spaces, `_`, `-` and `.`, ignoring case
   and the ` 2`, ` 3` Prism adds to repeated names. So `Floor collider`, `Floor_collider` and `Crate breakable` work
   with no roles set, and so do GLBs from other tools.

## Changes to brief 50 part 3

- **Roles become a list.** A group or component has any number of roles (brief 50 question 2: yes, several at once, for
  example `breakable` + `loose` + `wood`). The Inspector's **Game role** row becomes a row of chips with an add field:
  type a word and press Enter, or pick from the suggestions (the list below). Any word is allowed: Beverly keeps words it
  doesn't know as tags of the project's own, for the owner's game to use. One undo step per change. Edit > Game Role and
  the right-click menu add or take off a role on everything selected; on loose faces they group them first, as brief 50
  says.
- **Kept** as brief 50 says: on the component (every copy shares it) and on a group (its own), in `groups.json` as a new
  optional field `roles: ["collider"]` (left out when empty, so old files and files without roles stay byte for byte
  the same).
- **A material can say what it's made of:** a new optional **Surface** row on a material (a word with suggestions:
  stone, wood, metal, grass, dirt, sand, gravel, water, snow, carpet, glass, ice, rubber; any word allowed), stored
  with the material as an optional field and left out when empty. This is the "material tag" the owner asked for; a
  material name that already says it (`Stone wall`) works without it.
- **View > Game Roles** (brief 50) colours by the first role it knows: collider green, breakable orange; others show as
  the badge only.

## In the GLB

On each node, next to brief 35's `kind` and `definition`:

```json
"extras": {"prism": {"kind": "group", "tag": "Walls", "roles": ["breakable", "stone"]}}
```

- `roles`: the group's or component's roles, in the order the owner gave them; left out when there are none.
- `tag`: the name of the instance's Prism tag; left out when it has none.
- A node carries only its own roles and tag: children inherit nothing in the file (Beverly decides what passes down).

On each material, next to the existing `prism_texture_size_m`:

```json
"extras": {"prism_texture_size_m": 2.0, "prism": {"surface": "stone"}}
```

- Material names stay as they are now, exported verbatim, and stable between exports.

**Import (Prism reading a GLB back):** `roles`, `tag` (the tag is found by name, or made) and `surface` are read back,
so a Prism to GLB to Prism round trip keeps them. Brief 50's name suffix is read back as a role too.

**Brief 50's "Put the role in the name" option** stays, off by default, for engines that only read names. Beverly reads
`extras` and doesn't need it; it also ignores role words when matching pieces on a re-import, so turning the option on
doesn't break the owner's changes in Beverly.

## Answers to brief 50's questions

1. **Names:** Beverly reads `extras` first, and also the words in node and material names (always, whatever the
   suffix option). The words are lower case in the file; Prism may show them capitalised.
2. **Both at once:** yes, a list (above).
3. **Breakable:** on the parent group whose children are the chunks: the parent is the thing that breaks, its child
   groups are its pieces. A breakable group with no groups inside breaks as one piece. Nothing is inherited in the file.
4. **Other settings** (health, mass, collision layers) stay Beverly's own, as brief 35 says. Beverly works out weight
   from size and the material word.

## The words Beverly knows (Prism's suggestions)

| Words | What Beverly does with them |
|---|---|
| `collider` | The node is the collision shape, drawn in Beverly only as an outline when selected. It is the collision for the group it sits in (its parent node), or for the whole model at the top level. If a model has any collider, only colliders collide. |
| `walkthrough`, `trigger`, `nocollide` | Walk-through, a trigger volume, or drawn only with no collision. |
| `loose` | Loose physics: it tumbles and can be pushed. |
| `breakable` | Breaks into its child groups (Destructible props). |
| `climbable`, `ladder`, `slippery`, `water`, `ground`, `diggable`, `grind rail`, `perch`, `paths ignore` | Beverly's surface tags. |
| `stone`, `wood`, `metal`, `grass`, `dirt`, `sand`, `gravel`, `water`, `snow`, `carpet`, `glass`, `ice`, `rubber` | What it's made of: footstep sounds, weight, grip; later impact sounds, marks and effects. Usually on a material's Surface, but allowed as a role too. |
| `door`, `button`, `lift`, `platform` | A door that turns 90 degrees around the node's own origin when used, an E button, a moving part. The group's axes (Group Axes) are the pivot. |
| `light`, `socket`, `seat` | A light at the node's origin, coloured by its glowing material; an attachment point. |
| `lod1`, `lod2`, `editor` | Simpler versions of the parent, and pieces shown only in the editor. |

This list grows on Beverly's side; Prism only needs it for suggestions, and any word passes through unchanged.

## Tests (Prism)

- Several roles on a group, on a component (all copies share them), on several selected things at once, on loose faces
  (grouped first); undo.
- The GLB: `roles` and `tag` on the right nodes only, `surface` on the material, nothing written when there are none
  (old output byte for byte); a round trip through export and import keeps all three.
- `groups.json` and the material file: the new fields round trip; old files open unchanged.
- MCP: `get_inspector` / `set_inspector` rows `roles` and (for a material) `surface`; `list_groups` shows the roles.

## Question for the owner (a default is used until answered)

- **Hidden colliders:** collision shapes are often hidden while modelling. Should the Game engine export include
  `collider` nodes even when they are hidden or on a hidden tag? (Default: yes, since a collider is never meant to be
  seen; everything else follows the export scope as now.)
