# Motion kit — UI interaction components

A small set of React interaction components that share one motion style: quick springs that
slightly overshoot, content that sharpens in from a blur, dark glass surfaces, and keycap hints
for keyboard shortcuts. They were first built for SIFT (a Windows media sorter), then made
project-neutral here.

They are original code, loosely inspired by the names of patterns in the Skiper UI catalogue
(https://aestheta.ai/skiper-ui/).

---

## Instructions for Claude (read this first)

The user drops this zip into a chat and says something like "here are some UI interaction
components we should use in this project". When that happens:

1. **Unzip and read this whole file.** Also skim `components/motion.ts` and `styles/kit.css`; they
   define the look.
2. **Check the target project's stack.**
   - **React 18/19 + Tailwind v4:** use the files as they are (see *Adding to a project*).
   - **React + Tailwind v3:** works too. Import `kit.css` normally, and add the kit folder to
     `content` in `tailwind.config` instead of using `@source`.
   - **React without Tailwind:** keep the components, but convert their `className`s to your
     CSS approach. Say so before doing it.
   - **Not React (plain HTML, Vue, Svelte…):** port the components you need. Keep the same
     springs (`motion.ts`), timings, sizes, colours and keyboard behaviour so the feel carries
     over. Say which ones you ported.
3. **Copy, don't reference.** Put `components/` in the project (e.g. `src/components/motion-kit/`)
   and `styles/kit.css` next to the global CSS. Install `motion` if it's missing.
4. **Use the kit wherever it fits** instead of hand-rolling the same thing:
   - status feedback → DynamicIsland
   - Ctrl/⌘+K → CommandPalette
   - right-click actions → ContextMenu
   - icon-button hints → Tooltip (a row/column of icon buttons → TooltipToolbar)
   - 2–5 exclusive options → SegmentedControl
   - several filters/tags picked from a fixed list → FilterSelector (`mode="light"` on light pages)
   - picking one of a fixed set of modes (camera modes, presets) → ModeDial
   - stepping through a cyclic sequence (months, chapters, slides) → Clicker
   - destructive actions → UndoSnackbar or HoldButton
   - background work → StatusDot
   - processing / "thinking" while waiting on a result → DnaThink (inline, wide) or RingThink (compact, square)
   - loading → Skeleton
   - thumbnails → ImageReveal
   - people or file groups → AvatarStack
   - primary actions → SheenButton
   - a slightly more important regular action (continue, next step) → WowButton (`mode="light"` on light pages)
   - icons → `<Icon n={…} />` (numbers match the demo's Icons grid); icons that should react on hover → `<AnimatedIcon n={…} />` (numbers match the demo's Animated icons grid)
   - celebratory / premium accents (upgrade buttons, new badges, success) → Sparkle
   - a simple operation finished, or a map/terrain marker that must catch the eye → DigitalSparkle
   - small status/info popover off a round button → GooeyMenu
   - short codes, PINs, big-number entry → AnimatedInput
   - bottom/app dock whose sections open in place → DockPanel
   - a whole-app workspace of docked, movable, resizable panels (editors, tools) → PanelLayout
   - media apps: playback buttons → Transport (+ `useTransportKeys`); times → Timecode (`formatTC`/`parseTC`);
     a scrubbable time ruler with an In/Out range → TimelineRuler; audio levels → LevelMeter; audio shape →
     Waveform (`computePeaks`); exports and long jobs → RenderProgress

   - neumorphic / soft-UI pages, or "frosted glass over an aurora" → the *Soft glass* set (`SoftButton`,
     `SoftToggle`, `SoftSlider`, `SoftSelect`, `GlassWell` for any new cutout); see *Soft glass*

   Don't force a component where it doesn't fit.
5. **Match the motion language in any new UI you write for the project:** reuse `SPRING`,
   `SPRING_SNAPPY`, `SPRING_SOFT`, `BLUR_IN` and `POP_IN` from `motion.ts`, and show keyboard
   shortcuts with `<Kbd>`.
6. **Retheme through the `--kit-*` CSS variables**, not by editing each component. Only change
   component internals when the project needs different behaviour.
7. **Keep what's built in:** keyboard support (Esc, ↑/↓/Enter), focus handling, `role`/`aria`
   attributes, and `prefers-reduced-motion` handling.
8. **Keep `demo.html` current.** After changing or adding components, add them to `demo/src/App.tsx`
   and run `npm run build` in `demo`. Then hand the user the updated zip, so they can view it by
   double-clicking.
9. **Effects.** Every animated component has an `effect` (see *Effects*). "Motion kit segmented control
   with color flash" means `<SegmentedControl effect="color-flash" … />`. "Use the glitch effect" for
   a whole project means wrapping the app in `<EffectProvider effect="glitch">`. No effect named →
   `default`. When writing new UI for a project that uses an effect, play it with `useFx` too.
10. **Missing interaction?** If the user wants one the kit doesn't have (carousel, preloader,
   magnetic button…), write it in the same style and offer to add it to the kit. If they want
   Skiper's exact version, point them to skiper-ui.com.

---

## Adding to a project

Needs React 18 or 19, `motion` (v11+, imported as `motion/react`), and Tailwind CSS v4.

```
npm install motion
```

Copy `components/` to e.g. `src/components/motion-kit/`, and `styles/kit.css` to `src/styles/`. Then, in the global CSS:

```css
@import 'tailwindcss';
@import './styles/kit.css';
@source './components/motion-kit';   /* only needed if Tailwind doesn't already scan that folder */
```

Import from the folder's index:

```tsx
import { DynamicIsland, useIslandToast, CommandPalette, Kbd } from '@/components/motion-kit'
```

The components assume a dark background. White text is the default.

## Theme

Set these on `:root` (or any wrapper) to retheme everything:

| Variable | Default | Used for |
| --- | --- | --- |
| `--kit-accent` | `#f5dc4a` | highlights, active items, Undo text |
| `--kit-danger` | `#ff5e3a` | destructive buttons, rings |
| `--kit-ok` / `--kit-busy` / `--kit-warn` | green / amber / coral | StatusDot tones |
| `--kit-ink` | `#0a0809` | island, tooltips |
| `--kit-panel` | near-black 94% | menus, palette, snackbar |
| `--kit-glass` | dark 52% | `.glass` cards |
| `--kit-line` | white 14% | hairline borders |

## Components

| Component | What it does | Key props |
| --- | --- | --- |
| `DynamicIsland` + `useIslandToast()` | Black status pill that springs open for each action, then settles back | `toast`, `children` (resting content), `idleSize`, `toastSize` |
| `CommandPalette` (+ `fuzzyScore`) | Ctrl/⌘+K dialog: blurred scrim, spring-in box, fuzzy search, ↑ ↓ Enter Esc | `open`, `onClose`, `items: PaletteItem[]`, `placeholder` |
| `ContextMenu` | Right-click menu that pops from the pointer and stays on screen; Esc/click-away closes | `at: {x,y} \| null`, `title`, `items` ({label, keys, danger, disabled, hint, onSelect} or `'separator'`), `onClose` |
| `Tooltip` | Springs up on hover or focus, with key hints | `label`, `keys`, `side` |
| `TooltipToolbar` | Icon toolbar with one shared tooltip that slides between buttons; its labels scroll inside it | `items: {label, keys?, icon, onSelect?}[]`, `label`, `orientation: 'horizontal' \| 'vertical'` |
| `GooeyMenu` | Round trigger; on hover/click/focus its panel buds off like a liquid drop, then grows above it | `icon`, `label`, `children` (panel), `width`, `height`, `size` |
| `AnimatedInput` | Text input whose characters spring up as typed and drop away when deleted | `value`, `onChange(value)`, `className` for size, plus normal input props (`placeholder`, `maxLength`, `aria-label`…) |
| `DockPanel` | Icon dock that grows into a panel; the active tab becomes a labelled pill, content slides between tabs; click outside/Esc closes | `tabs: {label, icon, content}[]`, `active` (index or null), `onChange`, `label`, `width` |
| `ModeDial` | Camera-style mode dial: fixed tick arc with an index mark; the ring of modes turns (eased, ~0.5 s a step) so the chosen one sits under it. Drag, scroll, click, ← → Home End | `items: {label, kind?: 'disc' \| 'pill' \| 'text', text?, icon?}[]`, `value`, `onChange`, `label`, `size` |
| `Clicker` | Big wheel with its centre off to the right; labels and ticks on the rim, an `AnimatedIcon` per slot inside. Each click springs it one slot; the slot at 9 o'clock is in focus and its icon plays. Wraps round. Drag, scroll, click, ↑ ↓ Home End | `items: {label, icon}[]` (`icon` = AnimatedIcon number), `value`, `onChange`, `label`, `width`, `height` |
| `FilterSelector` | Filter chips + a + button. The + grows into a menu of the options not yet applied (from its corner, flips left near the screen edge); a pick lands as a chip from its menu row and the + slides on (wrapping lines). × fades a chip out and the rest reflow. ↑ ↓ Esc, click-away closes | `options: {id, label, icon?}[]`, `value` (ids), `onChange(ids)`, `label`, `mode: 'dark' \| 'light'` (default `dark` = kit surfaces; `light` = white chips) |
| `SegmentedControl` | Options with a sliding white indicator | `options: {label, hint?}[]`, `value`, `onChange`, `label` |
| `UndoSnackbar` | "Doing X in 8 s · Undo" with a countdown ring; you own the timer | `until` (epoch ms or null), `duration`, `message`, `onUndo`, `className` for position |
| `HoldButton` | Press and hold to confirm, with ring and fill; release cancels; can be driven by a key | `onComplete`, `duration`, `holding` (external, e.g. Ctrl+Del held) |
| `StatusDot` | Pill with a dot that pulses while busy | `tone: 'ok' \| 'busy' \| 'warn'`, `label` |
| `Skeleton` | Shimmer placeholder | `className` for size |
| `ImageReveal` | Zooms, and wipes across to a second image, on hover | `src`, `hoverSrc`, `className` |
| `AvatarStack` | Overlapping circles that fan out on hover, with +N | `items: {key, src?, label}[]`, `max`, `size`, `ring` |
| `SheenButton` | Button with a light sweep on hover | normal button props |
| `WowButton` | Outlined pill with a dot before the label; on hover/keyboard focus the dot swells to fill the button, the label slides left and an arrow comes in | normal button props, `children` (label), `mode: 'dark' \| 'light'` (default `dark` = white on dark; `light` = black on white) |
| `DnaThink` | Processing indicator: 11 dot pairs with faint rungs; a wave runs left→right as each pair snaps into one gooey capsule, holds, and pulls apart (1.7 s per pair) | `width` (default 120), `label` (omit = decorative), `className` (colour = text colour) |
| `RingThink` | Processing indicator: two sparks at 12 and 6 o'clock sweep clockwise into glowing crescents, heads leading, tails catching up, then shrink back (1.57 s loop) | `size` (default 40), `label`, `className` (colour = text colour) |
| `Kbd` | Keycap | children |
| `Icon` | Numbered icon set from `icons-motion-kit.svg`: 1–48 solid, 49–79 line; takes the text colour | `n`, `size`, `title` (omit = decorative) |
| `AnimatedIcon` | 50 animated line icons (reference-sheet order): shields pop, pins lie down, arrows spin, the send icon flies off and back… Plays when its parent is hovered, when `trigger` changes, or with `loop`; stroke in the text colour | `n` (1–50), `size`, `loop`, `delay`, `gap`, `trigger`, `hover`, `strokeWidth`, `title` |
| `DigitalSparkle` | Crisp sprite-like burst in white + one accent, ~1.3 s (1 cross-hairs → orange X → ring · 2 three waves of cyan/white rays · 3 green pinwheel that bursts and returns white). For a finished action, or `loop` + `tilt` as a marker on 3D terrain | `n` (1–3), `size`, `loop`, `delay`, `gap`, `color` (accent), `tilt` (deg) |
| `Sparkle` | Animated sparkle for accent decoration (1 long star · 2 soft diamond · 3 tilted star · 4 flare · 5 plus + dots · 6 burst + satellites); glow = `--kit-accent` | `n` (1–6), `size`, `loop`, `delay`, `gap`; position it yourself (e.g. `absolute -top-3 -right-3`) |
| `PanelLayout` (+ `split`, `tabs`, `movePanel`) | Docked workspace like Premiere: panels tile the space, never float. Drag dividers to resize; drag a tab onto another panel to join it (centre) or split beside it (edge). The layout is a plain tree you own | `layout: PanelNode`, `onChange`, `panels: Record<id, {title, content}>` |
| `Transport` + `useTransportKeys` | Go to start, step back, play/pause (icon morphs), step forward, go to end, loop; a speed badge when shuttling. The hook binds Space, J/K/L, ← → (Shift = 10 frames), Home/End | `playing`, `onToggle`, `onStep(frames)`, `onJump('start'\|'end')`, `loop`, `onLoop`, `rate` |
| `Timecode` (+ `formatTC`, `parseTC`) | HH:MM:SS:FF readout; click to type a time (full timecode, partial "3:04", frames "120", or "2.5s"); Enter jumps, Esc cancels | `time` (s), `fps`, `onChange` |
| `TimelineRuler` | Ticks and timecode labels that adapt to zoom (down to single frames), a draggable playhead, and a two-handle render-range strip (In/Out) | `duration`, `fps`, `pxPerSec`, `time`, `onSeek`, `range`, `onRange`, `snap` |
| `LevelMeter` | Per-channel dB meter (−60…0), green/amber (−12)/red (−3), peak hold | `levels` (array, or a function polled every frame), `height` (px or CSS length), `barWidth` |
| `Waveform` (+ `computePeaks`) | Mirrored waveform on a canvas that fills its box; slice with `from`/`to` for a trimmed clip | `peaks`, `from`, `to`, `gain`, `color`, `className` |
| `Dialog` | Modal: dims the page, springs a panel in with title, body and footer buttons; Esc / click outside close (unless `locked`), focus moves in and back | `open`, `title`, `onClose`, `children`, `footer`, `width`, `locked` |
| `RenderProgress` | Export/job card: title, %, bar, detail line, Cancel; `done` and `error` states | `progress`, `title`, `detail`, `onCancel`, `done`, `error` |
| `EffectProvider` | Sets the effect for everything inside | `effect: KitEffect` |
| `Ring` | Progress ring 0–1 | `progress`, `size`, `color` |

CSS-only versions of some effects are in `kit.css` for when a component is overkill: `.glass`,
`.skel`, `.reveal` (inside a `.group`), `.stack`, `.sheen`, `.kc`, `.thin-scroll`.

### Usage snippets

```tsx
// Island feedback
const [toast, show] = useIslandToast()
<DynamicIsland toast={toast}><span className="truncate">{fileName}</span></DynamicIsland>
show({ glyph: '+1', label: 'Next frame', sub: '00:17:23:14' })         // tone: 'accent' | 'danger' | 'plain'

// Undo grace period — perform the action when `until` passes; clear it on undo
const [until, setUntil] = useState<number | null>(null)
useEffect(() => { if (!until) return; const t = setTimeout(() => { doDelete(); setUntil(null) }, until - Date.now()); return () => clearTimeout(t) }, [until])
<UndoSnackbar until={until} duration={8000} message="Deleting 3 files" onUndo={() => setUntil(null)} className="fixed bottom-6 left-1/2 -translate-x-1/2" />

// Context menu
<div onContextMenu={(e) => { e.preventDefault(); setAt({ x: e.clientX, y: e.clientY }) }} />
<ContextMenu at={at} onClose={() => setAt(null)} title={file.name}
  items={[{ label: 'Open', keys: ['↵'], onSelect: open }, 'separator', { label: 'Delete', danger: true, onSelect: del }]} />
```

`demo/src/App.tsx` uses every component and is the best full example.

## Effects

The components' normal motion (springs, blurs, slides) is the **Default** effect. The other nine are
stepped, FUI-style entrances taken from a reference sheet. An effect replaces a component's
*appear or change* moment. Layout motion (the gooey drop, panels resizing) stays as it is. Exits become
a quick cut. Effects are skipped entirely under `prefers-reduced-motion`.

| Effect | `effect=` | What happens (~0.3–0.5 s, ~55 ms beats) |
| --- | --- | --- |
| Default | `'default'` | the kit's springs + blur |
| Blinky | `'blinky'` | flickers off / half / full before settling |
| Glitch | `'glitch'` | stretched, shifted, sliced fragments snap together; small aftershock ~0.9 s later |
| Trail | `'trail'` | streaks in from the left, stretched and dim, then brightens |
| Stroke-n-fill | `'stroke-n-fill'` | outline flickers, then the fill blinks in (text: outlined glyph, then solid) |
| Shuffle | `'shuffle'` | placeholder frames — crossed box, "!", hatching — then the real thing (text: random characters) |
| Crash-n-blink | `'crash-n-blink'` | lands oversized, snaps to size, blinks |
| Slide-n-blink | `'slide-n-blink'` | rises ~10 px while blinking |
| Color flash | `'color-flash'` | red / cyan / green / violet, then its real colour |
| Build on | `'build-on'` | assembles from random cells of a 4×4 grid |

**Choosing:** `<EffectProvider effect="…">` for everything inside it, or `effect="…"` on one component
(the prop wins). `EFFECTS` and `EFFECT_LABELS` list them. The demo's square in the bottom-left corner
switches it live, and *Export theme* records the chosen effect.

**What each component does with an effect:**

| Component | Effect plays on |
| --- | --- |
| Tooltip, ContextMenu, CommandPalette, UndoSnackbar | the panel as it opens |
| TooltipToolbar | the tooltip, which snaps to each hovered button instead of sliding |
| FilterSelector | the menu as it opens, and each newly added chip |
| SegmentedControl | the indicator, which jumps to the new option instead of sliding |
| ModeDial | the newly chosen mode (the ring still turns) |
| Clicker | the focused label on each click (the wheel still turns; the icon still plays) |
| DockPanel | the content on each tab change (swaps in place) and the active pill |
| DynamicIsland | each new message (the pill still springs to size) |
| GooeyMenu | the panel content, once the drop has spread |
| AnimatedInput | each typed character (glyph version) |
| UndoSnackbar | also the seconds digit on every tick |
| HoldButton | the button on completion; progress fills in 8 steps |
| StatusDot | the dot on tone change; while busy it replays every 1.4 s instead of pulsing |
| Skeleton | replays every 1.4 s instead of shimmering |
| ImageReveal | the second image on hover, instead of the wipe |
| AvatarStack | each avatar in turn as the stack fans out |
| SheenButton | the button on hover, instead of the sweep |
| WowButton | the fill on hover (it fills at once instead of swelling from the dot) |
| Kbd, Ring | nothing — they don't animate |
| Dialog | the panel as it opens |
| PanelLayout, Transport, Timecode, TimelineRuler, LevelMeter, Waveform, RenderProgress | nothing — working tools that should respond instantly |
| DnaThink, RingThink | nothing — their loop is the whole point, like Sparkle and DigitalSparkle |

**For new UI:** `useFx(ref, trigger, { effect?, kind?, enabled?, delay? })` plays the active effect on
`ref`'s element when it mounts and whenever `trigger` changes. It returns the active effect, so you
can skip default motion when it isn't `'default'`. Use `kind: 'glyph'` for single text characters.
`playEffect(el, effect)` is the imperative version. `useTick(ms)` gives a trigger for looping.
The engine is `components/effects.tsx`; its overlay needs the `[data-kit-fx]::after` rule in `kit.css`.

## Soft glass (`soft.html`, `components/soft/`)

A second look for the kit: neumorphic surfaces (everything is one opaque material, raised or pressed
by paired light/dark shadows) with **cutouts**: recessed windows through the surface onto a slowly
morphing aurora, frosted with a milky tint and film grain. Same springs as the rest of the kit.

| Component | Opaque (raised) | Glass cutout | Motion |
| --- | --- | --- | --- |
| `SoftButton` | whole button | none | hover lifts, press sinks (shadows flip to inset), release overshoots back |
| `SoftToggle` | knob | recessed track | frost clears when on; knob squashes/stretches in travel; accent dot lights |
| `SoftSlider` | handle (+ value bubble) | rail; filled side is clear glass, rest frosted | handle trails pointer on a spring, lifts while held |
| `SoftSelect` | trigger, highlight pill | list background | trigger sinks while open; pill pops out of the glass and slides between items |

```tsx
import '…/styles/soft.css'
<SoftButton onClick={save}>Save scene</SoftButton>
<SoftToggle label="Lights" checked={on} onChange={setOn} />
<SoftSlider label="Brightness" value={v} onChange={setV} format={v => `${v}%`} />
<SoftSelect label="Scene" options={['Reading', 'Cinema', 'Dinner', 'Night']} value={s} onChange={setS} />
```

**How the glass works (`aurora.ts`, `Glass.tsx`).** One aurora is drawn per frame into a small offscreen
canvas the size of the viewport ÷ 8 (blurry on purpose). Each cutout (`<GlassWell>`) owns a canvas and
copies the part of the aurora behind its own screen rect, so every cutout looks onto the *same*
viewport-fixed layer and stays aligned while it moves, scales or scrolls. The loop only runs while a
cutout is mounted. Colours: `--soft-aurora-base` and `--soft-aurora-1..5`; theme changes ease across.
New glass parts: wrap them in `GlassWell` (`frost` 0–1 or a MotionValue; `tint` to replace the frost
layer, as the slider does). New opaque parts: `softShadow(depth, px)` / `useSoftShadow(motionValue, px)`,
depth 1 raised → 0 flat → −1 pressed (always four shadows, so it animates through flat).

**Theme:** `styles/soft.css`, `--soft-*` variables. Light by default; `data-soft-theme="dark"` on `<html>`.
Surface colours are `@property`-registered, so switching themes crossfades.

Source for `soft.html` is `soft/` (same setup as `demo/`: `npm install`, `npm run dev`, `npm run build`).
All four accept `effect`. Reduced motion: aurora freezes, springs become instant.

## Motion language (`components/motion.ts`)

| Preset | Values | Use for |
| --- | --- | --- |
| `SPRING` | stiffness 420, damping 32 | containers changing size (island) |
| `SPRING_SNAPPY` | 520 / 34 | menus, tooltips, the segmented indicator, the palette |
| `SPRING_SOFT` | 400 / 30 | toasts and snackbars sliding in |
| `BLUR_IN` | opacity 0→1, blur 6px→0, scale .96→1 | content appearing or swapping |
| `POP_IN` | adds y 8→0 and blur 8px | panels opening from a trigger |

`AnimatedInput` uses its own bouncier spring (600 / 18) so landing characters visibly overshoot.

Other conventions: corners mix big and small radii (`rounded-[14px_14px_14px_4px]`) for an
asymmetric feel; hit targets are at least 40–44 px; shortcuts are always shown with `<Kbd>`.

## Demo

**Just look:** double-click `demo.html`. It's a single self-contained file with every component
running, so no install or server is needed, and it works offline.

**Edit and watch changes live:** run `npm install` once in `demo`, then `npm run dev`. Leave it
running; the page updates the moment you save a file.

**Pick a theme:** the *Theme* section at the bottom of the demo recolours every `--kit-*` variable
live (swatch picker, or type hex / rgb). *Export theme* downloads `motion-kit-theme.txt`, a `:root`
block. **Claude:** when the user sends that file, copy its values into `styles/kit.css` (or the
project's global CSS) rather than editing components.

**Refresh `demo.html` after changes:** run `npm run build` in `demo`. It rebuilds `demo.html` at the kit's top level.

## Apps built with the kit

### `apps/dio/` — Dio, a browser video editor (Chrome/Edge), in progress

Open `dio.html` (top level, one self-contained file), or `npm install` + `npm run dev` in `apps/dio`.
It imports the kit from `../../components`; `npm run build` there regenerates `dio.html`.

**Done (stages 1–2):** docked panels (Project Library, Program monitor, Timeline, Audio meter); import videos, images and
audio (button or drop files); drag items to the timeline (videos with sound become a linked video + audio pair; drop in
the empty zone above/below the tracks to create a new track); monitor keeps the project aspect and shrinks to fit;
clips: move (across tracks too), trim handles when selected, magnetic snapping to clip edges on all tracks, playhead and
In/Out; overwrite edits; Close Gap (right-click a gap); Delete (right-click or Del); hide video tracks (eye), mute audio
tracks (M); waveforms on audio clips; playback with J/K/L shuttle and a stereo level meter; undo/redo (last 20 edits;
a drag or slip counts as one); colour labels 1–8 on selected clips; right-click a linked clip → Unlink, or select one
unlinked video + one unlinked audio clip → Link; mouse wheel over the tracks scrolls left/right, over the ruler moves
the playhead (Shift = one frame); Home/End (keys) centre the playhead in the timeline, and playback keeps it centred.

**Keys:** V Select · C Cut · Y Scrub (slip: content slides inside the clip) · I / O set In / Out at the playhead ·
T add text · Q / R move the front / rear handle of the clip(s) under the playhead to it (selected ones if any) · Space play/pause ·
J K L shuttle · ← → frame (Shift ×10) · Home End (and centre) · = − zoom · 1–8 colour label · Ctrl+Z undo ·
Ctrl+Shift+Z / Ctrl+Y redo · Del delete · Esc deselect.

**Done (stage 3):** Effect Controls panel for the selected clip (or linked pair):
- Transform (video, images, text): Position X/Y, Scale (🔒 locked, or separate width/height), Rotation, Opacity. Audio: Volume (dB, −60 = silent).
  Every property is keyframable: the stopwatch turns keys on (click again removes them all); with keys on, any change
  records a key at the playhead; ‹ ◆ › jump to / add-remove / jump to keys. Keys ease in and out, live in *source* time
  (they stay with the content when a clip is moved or trimmed), and show as diamonds on the selected clip.
  Numbers: drag left/right (Shift ×10) or click to type.
- Text tool (T or the T button): a 5 s text clip at the playhead on the lowest free video track (or a new one). Text,
  font (common list, or "All fonts…" to list installed fonts in Chrome/Edge), size, colour, alignment, bold/italic/underline.
- Effects, added with the + in the Effects section (FilterSelector: chips = applied effects, × removes): Blur,
  Color Curves (RGB/R/G/B, five points each; double-click resets a channel), Hue / Saturation (and lightness) for
  visual clips; Beat Highlighter for audio clips: marks detected beats (bass onsets) on the clip, shows count and
  approximate BPM, adjustable sensitivity and minimum spacing; beats are snap targets for every drag.

**Done (stage 4):**
- Default transition: right-click a seam (two touching clips) → Add Default Transition — an eased cross dissolve centred
  on the cut, using spare frames past each clip's handle (greyed out, with the reason, when either side has none; shorter
  than the default if only a little is spare). Linked audio seams get one too; two touching audio clips can have one on
  their own. Click a transition to change its length (frames) in Effect Controls; right-click or Del removes it.
  Video dissolves blend additively (no dip in brightness); audio fades are linear-gain (a slight level dip mid-fade).
- File menu: New (asks first if there are edits), Save (Ctrl+S; a .dio JSON file — saves back to the same file in
  Chrome/Edge), Load… (Ctrl+O; then Relink: pick the media files, matched by name), Project Settings (frame size presets
  or custom, fps, default transition length), Export… (Ctrl+M).
- Export: the In–Out range (whole timeline if unset) to .mp4 — H.264 High profile at the project size/fps (bitrate
  4–40 Mbps) + AAC 128 kbps stereo 48 kHz. Frame-accurate: every frame seeks the sources and renders with the same
  compositor as the monitor. Progress, time left, Cancel. Offline media is left out (the dialog warns).
- Offline media (after Load, before relink): striped red clips, "Offline" in the library, and a Relink button there.
- Developer note: `dio.html?test-codecs` exports VP9 + Opus instead (for browsers without H.264/AAC encoders, such as
  Linux Chromium); it's how the export pipeline was verified in testing.

**Model notes for future sessions:** `src/model.ts` holds the project types and pure edit operations (overwrite =
`clearRange`, `cutAt`, `closeGap`, `trimTo`, snapping). `src/store.ts` has the project store (`edit(draft => …)`), the
playhead clock, and imported media (object URLs, thumbnails, peaks — not saved in the project). `src/engine.ts` plays
back: one media element per clip synced to the clock, and a Web Audio graph (per-clip gain from Volume). `src/render.ts`
`renderFrame` composites a frame (transforms, text, effects; curves via SVG filters) — the monitor uses it, and export
will too. `src/beats.ts`: bass envelope at import + onset detection. `src/EffectControls.tsx`: the properties panel.
Transitions: `Project.transitions` ({a, b, dur}); `liveTransitions` drops ones whose clips no longer touch; `mixAt` gives
the clip(s) and weights a track shows at time t; `playSpan` includes borrowed frames. `src/files.ts`: New/Save/Load/
relink. `src/export.ts`: WebCodecs + mp4-muxer. `src/FileMenu.tsx`: menu and dialogs.

## Files

```
components/   the components (index.ts exports everything) + motion.ts presets + effects.tsx engine
styles/       kit.css — theme variables and CSS-only effects
demo.html     every component running, one file, open it in any browser
soft.html     the Soft glass set (neumorphic + frosted aurora cutouts), one file
dio.html      Dio, the video editor app, one file
apps/         apps built on the kit (see *Apps built with the kit*)
soft/         source for soft.html (same setup as demo/)
demo/         source for demo.html (Vite + React); `npm run dev` for live editing, `npm run build` to regenerate demo.html
```
