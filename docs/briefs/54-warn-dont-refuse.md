# Proposal 54: warn, don't refuse

Status: **proposal, for the owner's decision** (2026-11-26). Nothing here is built yet. Recorded as DECISIONS #45 (open).

## Why

Owner, 2026-11-26, after a fillet drag blanked out at 0.028 m:

> "A refusal doesn't SHOW what isn't allowed, or what MIGHT be broken, which isn't intuitive. The user needs choices more than limits."

Shell / Thicken already works this way (DECISIONS #41). It builds what it can and marks in orange where to check. This proposal extends that to every tool.

Today, a refusal throws the preview away and puts one line of grey text in a side window. You can't see:
- which face or edge caused it;
- how far you can go;
- what would have been made.

There are about 220 named messages across about 60 files. Some are hints, but most of them stop an edit. The biggest groups are:
- Shell (29, mostly warnings since #41);
- Follow Me (14);
- Convert (10);
- the geometry kernel (10);
- Loft, Deform and Cage (7 each);
- Trim, Transform, Sides, Hole and Draft (6 each).

## The rule

Each refusal becomes one of four kinds:

| Kind | When | What happens |
|---|---|---|
| **1. Not an edit** | There's nothing to do: zero size, nothing picked, a number that isn't a number, a solid tool on something that isn't a solid. | Stays as now, but said at the cursor, not only in the window. |
| **2. As far as it goes** | A size the shape can't take: a round, an offset or a pull past its limit. | The largest size that works is built and shown. The face, edge or corner that stopped it is drawn in orange. The cursor label reads, for example, *"Made at 0.108 m, the most this edge takes: the line on the end face stops it"*. The size you asked for is drawn as a faint ghost, so you can see the difference. A click commits what's shown, with the warning. |
| **3. Built, check it** | A result exists but may have faults. Examples: shell face by face; a boolean that wasn't sure of inside and outside; a deform fitted just outside the tolerance. | Built anyway. The doubtful faces and edges are drawn orange in the preview and after the click. The warning stays in the window and the status bar, and Model Health lists it. |
| **4. Partly** | Many targets, some impossible: several edges to round, a loft with one bad section, Follow Me along a path with one bad turn. | What works is built. The rest is drawn orange and listed: *"3 of 4 edges rounded; this one is too short"*. |

Red is kept for "nothing could be built at all", for example when the kernel fails inside. Even then, the last good preview stays on screen.

**For every tool:**
- The preview never blanks mid-drag. This is done for rounds since `f896753`.
- The reason shows next to the cursor, in the warning colour.
- Whatever is drawn orange is the reason. You don't have to read anything to find it.

## Fillet / Chamfer: first, as the example

1. **Too large** becomes **as far as it goes**, plus a limiter highlight:
   - Prism finds the largest size that works by halving the step (about 10 tries with the kernel).
   - While dragging, a coarse search; on release, the exact one. This uses the coarse-then-exact preview work.
   - The ghost of the size you asked for is cheap to draw: on two flat faces it's just the two tangent lines, so no kernel call is needed.
   - What stopped it is the nearest corner, line or face end along the setback on each face; that gets drawn orange.
2. **Several edges, some too short** becomes **partly**: round the ones that work and mark the rest.
3. **"Only between flat, cylindrical and conical faces so far"** stays a refusal (kind 1). There's nothing honest to build yet, but it should say so at the cursor and outline the face it can't round against.
4. **The kernel made a shape Prism can't store** becomes red, with the last good preview kept.

## Order of work

1. **Inventory:** a table of every refusal, with its tool, its kind (1–4), and what gets built instead. This is a short document for you to check before anything changes.
2. **Shared parts:**
   - a "largest that works" search helper;
   - the limiter highlight;
   - the ghost of the asked-for size;
   - the cursor label (already started for rounds).
3. **Tool by tool:**
   - **Fillet / Chamfer first.**
   - **Then Push/Pull and Offset** (the most used).
   - **Then Follow Me, Loft, Sweep, Deform/Taper and Booleans.**

   Each step gets tests, and the rule-order notes (#43) are updated.

## Questions for you

1. **Commit on click.** When a size is clamped, should a click commit the clamped size (as proposed), or do nothing until you come back within range?
2. **The ghost.** Show the asked-for size as a faint ghost (proposed), or only the limit line?
3. **Red.** Keep red for "nothing at all could be built" (proposed), or use orange everywhere?
4. **Kind 3 and undo.** Should a "built, check it" result also offer a one-click "undo and show me why"? Or is the orange marking plus Model Health enough?
5. **Shell.** Should Shell's face-by-face fallback adopt kind 2 for "too thick"? That would mean building at the thickest that works, instead of face by face. This was option (c) in the session-17 notes.
