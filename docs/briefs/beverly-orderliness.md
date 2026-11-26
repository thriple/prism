# Orderliness: one set of rules for "close enough" and "what runs after what"

A brief for **Beverly** (the owner's Bevy game editor), from what Prism
learned on 2026-11-26. It is written to apply to any editor that changes
shared data (geometry, a level, a scene of entities). It names no Prism
code; Prism's own versions are `docs/TOLERANCE-AUDIT.md` and DECISIONS
#41–#44 in the Prism repository, as a worked example.

## Why

Prism grew tool by tool. Each tool was right on its own, and the bugs
came from between them:

- **"Close enough" meant something different in each place.** One central
  tolerance existed, but it was recomputed from whatever was asking (a
  group's size, the points being drawn), and about 400 other thresholds
  had been written in by hand. Two steps judging "the same point" by
  different numbers produced duplicates, slivers and joins that failed.
- **Clean-up steps undid each other.** Several steps run after every edit
  (merge, split, rebuild, tidy, smooth). Nothing said what order they run
  in or what each may change, so a later step quietly overwrote an earlier
  one's decision (a line that should stay hidden turned visible).
- **Rules written for the simple case.** A rule that assumes walls are
  upright, or that an edge has two faces, works until a user does
  something ordinary that it did not imagine.
- **Refusals with no feedback.** A tool that says "no" without showing
  where or why leaves the user guessing; the owner found this more
  frustrating than an imperfect result with a warning.
- **Slow for reasons of its own.** A join took minutes because a tidy-up
  step ran the whole health check to find one kind of problem, each lookup
  scanned the whole model, and the same work was done twice. None of it
  showed until it was timed.

None of these is a Prism-only problem. Any editor where several systems
touch the same data (snapping, merging, physics shapes, navigation data,
prefab overrides, transform hierarchies) gets them in the same way.

## The principles

### 1. One "close enough", fixed, per project

- **One tolerance per project (or per scene), stored with it, the same
  everywhere in it.** Not worked out from the size of whatever is asking,
  and not changed by what is being added.
- **Every other threshold is derived from it by a stated ratio.** Fits and
  bakes (a collider fitted to a mesh, a navmesh, a simplified LOD) are
  finer than the merge tolerance, so a fitted thing never misses what it
  should meet. Angle tests allow the tolerance over the length involved,
  not a fixed angle (a fixed angle lets the far end of a long wall drift
  much further than the tolerance).
- **A setting for extreme scales,** with presets and a typed value.
- **Display allowances are separate.** How far a line may sit behind a
  surface and still draw is about pixels, not data, and has its own
  numbers.
- **Guards against dividing by zero are not tolerances.** Name them as
  constants and leave them; they decide nothing about the data.

### 2. A written order of resolution rules

After any edit, the steps that tidy the result run in one written order,
and each says what it may and may not change. Prism's draft (DECISIONS
#44) is: exact tests first, with no tolerance; one tolerance; then weld,
split, rebuild, cut, remove duplicates, merge seams, soften, and the
user's own direct request last.

For Beverly the steps are different, but the shape is the same. For
example, after moving a prefab instance: resolve the transform hierarchy;
snap; merge or split level geometry it now touches; rebuild colliders it
changed; re-bake navigation it changed; apply overrides; and only then
what the user asked for directly. Write down which step owns which
decision, such as "only the override step may change a prefab instance's
material" or "snapping never changes a collider's shape". **No step undoes
a decision another step owns.** When two steps disagree, the order
decides, not whichever ran last.

### 3. The rules stay current

Every tool or system that changes the data says which steps run after it,
in what order, and which tolerance it uses for each judgement. **A piece
of work that adds such a tool, or changes one in a major way, is not done
until that is written down and tested.** Put this in the project's
definition of done (Prism: CLAUDE.md, "Finish a chunk properly"; DECISIONS
#43).

### 4. Build and warn; refuse only when there is nothing to build

- When an action cannot be done as asked, do the nearest thing that can be
  done, and say plainly what was done differently and why.
- Show where: mark the affected parts on the preview, before the user
  commits, so they can see which way will not work.
- Keep the warning visible after the commit, until the next action.
- Refuse only when there is nothing to make (nothing selected, a size of
  zero).
- Hand-held tools (a drag handle) may keep the last good value and say why
  instead, because a drag is a question, not a commit.

### 5. Preview fast, commit exact

A preview is not saved, so it may be coarse or a moment late. Commit must
be exact and repeatable.

- **While dragging:** a cheap version (the last finished result, a coarser
  version, pieces not yet joined), rebuilt when the pointer pauses or
  moves a meaningful amount.
- **On release or click:** the exact result, computed once.
- Background threads can help on native builds; a web build without threads
  falls back to rebuilding on pause.

### 6. Performance hygiene

- **Time each stage before guessing.** Prism's twelve-minute join was four
  separate wastes, none of them the expensive maths everyone suspected.
- **No whole-world check inside an inner step.** A tidy-up that needs one
  answer computes that answer, not the full health report.
- **Index what is looked up:** points by position, edges by their ends,
  entities by spatial cell. A lookup that scans everything is fine once
  and fatal in a loop.
- **Do shared work once.** The same intersection computed from each side,
  or a "can this run?" check that does the whole job and throws it away,
  doubles the cost.
- **Cache what is reused across pairs** (bounding boxes, layouts), and
  skip pairs whose boxes do not meet.
- **Deterministic still:** budgets are counts (at most N pieces joined),
  never clock time, so results replay the same.

### 7. Old data keeps behaving

Data saved before a rule changed was made under the old rule. When it
opens, give it the settings it was made with (Prism: the precision an old
file was built at), finer only where its own details need it, and say so.
New projects start at the new defaults. A file at the default saves byte
for byte as before.

### 8. Decisions are recorded, and the newest wins

Each owner decision is written down with its date. When a later one
contradicts an earlier one, the earlier is marked as replaced, and the
docs that repeated it (plan, project instructions) are updated in the same
change. The order of authority is: the project's hard rules, then the
owner's decisions (newest first), then defaults chosen while a decision is
open (noted where used).

## How to apply it to Beverly

Work in stages, each tested and released, as Prism did:

1. **Audit, report only.** No code changes. List:
   - every place "close enough" is decided: snapping distances, vertex
     welds, contact and penetration slops in physics, navmesh cell sizes
     and agent radii, float comparisons in transforms and serialisation;
     where each value comes from, and which ignore the project's;
   - every step that runs after an edit, per kind of edit, in order, and
     what each changes (transforms, colliders, navigation, overrides,
     materials, names and tags);
   - known bugs, each traced to the step that caused it;
   - slow paths, timed by stage.
   End with a proposal: the default tolerance, derived thresholds, the
   setting and its fallbacks, a draft rule order, and stages.
2. **The owner decides.** Record each decision.
3. **Stage 1: one tolerance in the project**, read everywhere through one
   accessor. This alone removes the biggest source of inconsistency.
4. **Stage 2: derived thresholds**, length-aware direction tests, stray
   distance checks moved onto the tolerance.
5. **Stage 3: the rule order enforced,** with a test per rule ("snapping
   never changes a collider", "an override survives a re-bake").
6. **Stage 4: the setting, presets and warnings** in the editor.
7. **Stage 5: sweep the rest:** name the guards, delete duplicates.

Throughout: build and warn rather than refuse, preview fast and commit
exact, and time before optimising.

## What to watch for

- A test that only passes at one exact tolerance is guarding luck, not
  behaviour. Prism found two (an offset through itself, a fold); both are
  real bugs in the logic, not settings to tune.
- "Same at every scale" tests stay valid only if the project's tolerance
  scales with the test's model, as a user's would.
- Fixing the order of rules can change long-standing results. Compare old
  saved projects before and after each stage.
