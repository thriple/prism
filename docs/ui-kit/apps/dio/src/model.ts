// Project model and pure edit operations. Every operation mutates a draft project (see `edit` in store.ts).

export type MediaKind = 'video' | 'audio' | 'image'
export type Media = { id: string; name: string; kind: MediaKind; duration: number; width: number; height: number; hasAudio: boolean; size?: number; modified?: number }
export type TrackKind = 'video' | 'audio'
export type Track = { id: string; kind: TrackKind; muted?: boolean; hidden?: boolean }
/** `start` on the timeline, `in` into the source, `dur` long (seconds). `link` pairs a video clip with its audio. */
export type Clip = {
  id: string; trackId: string
  /** '' for text clips */
  mediaId: string
  start: number; in: number; dur: number
  link?: string; color?: number
  props?: Props
  fx?: Fx[]
  text?: TextStyle
}

// ── Keyframes ──
/** A property: a plain value, or keyframes (times are source time, so keys travel with the content when trimmed/moved). */
export type Key = { t: number; v: number }
export type Anim = { v: number; k?: Key[] }
export type PropName = 'x' | 'y' | 'scale' | 'scaleY' | 'rot' | 'opacity' | 'volume'
export type Props = Record<PropName, Anim> & { lock: boolean }
export const defaultProps = (): Props => ({
  x: { v: 0 }, y: { v: 0 }, scale: { v: 100 }, scaleY: { v: 100 }, rot: { v: 0 }, opacity: { v: 100 }, volume: { v: 0 }, lock: true
})
export const propsOf = (c: Clip): Props => c.props ?? defaultProps()
export const ease = (u: number): number => u * u * (3 - 2 * u) // ease in-out between keys
/** Value of a property at source time `st`. */
export function valueAt(a: Anim, st: number): number {
  const k = a.k
  if (!k?.length) return a.v
  if (st <= k[0].t) return k[0].v
  for (let i = 1; i < k.length; i++) if (st <= k[i].t) { const u = (st - k[i - 1].t) / (k[i].t - k[i - 1].t || 1); return k[i - 1].v + (k[i].v - k[i - 1].v) * ease(u) }
  return k[k.length - 1].v
}
/** Set a property: with keyframes on, adds/updates the key at `st`; otherwise sets the plain value. */
export function setValue(a: Anim, st: number, v: number): void {
  if (!a.k?.length) { a.v = v; return }
  const hit = a.k.find((x) => Math.abs(x.t - st) < 1e-4)
  if (hit) hit.v = v; else { a.k.push({ t: st, v }); a.k.sort((x, y) => x.t - y.t) }
}
/** Source time for a timeline time, clamped into the clip. */
export const srcTime = (c: Clip, t: number): number => c.in + Math.min(c.dur, Math.max(0, t - c.start))

// ── Effects ──
export type Fx =
  | { id: 'blur'; amount: number }
  | { id: 'hsl'; hue: number; sat: number; light: number }
  | { id: 'curves'; rgb: number[]; r: number[]; g: number[]; b: number[] }
  | { id: 'beats'; sensitivity: number; gap: number }
export const FX_NAMES: Record<Fx['id'], string> = { blur: 'Blur', curves: 'Color Curves', hsl: 'Hue / Saturation', beats: 'Beat Highlighter' }
const LINE = [0, 0.25, 0.5, 0.75, 1]
export const newFx = (id: Fx['id']): Fx =>
  id === 'blur' ? { id, amount: 8 }
  : id === 'hsl' ? { id, hue: 0, sat: 100, light: 100 }
  : id === 'curves' ? { id, rgb: [...LINE], r: [...LINE], g: [...LINE], b: [...LINE] }
  : { id, sensitivity: 1.5, gap: 0.25 }

// ── Text ──
export type TextStyle = { content: string; font: string; size: number; align: 'left' | 'center' | 'right'; bold: boolean; italic: boolean; underline: boolean; color: string }
export const defaultText = (): TextStyle => ({ content: 'Title', font: 'Segoe UI', size: 96, align: 'center', bold: true, italic: false, underline: false, color: '#ffffff' })

/** Colour labels for keys 1–8. */
export const LABELS = ['#e5484d', '#f76b15', '#ffc53d', '#46a758', '#12a594', '#0090ff', '#8e4ec6', '#d6409f']
/** `transFrames`: length of a new Default transition. */
export type Settings = { width: number; height: number; fps: number; transFrames: number }
/** Crossfade centred on the seam where clip `a` ends and clip `b` starts (same track). */
export type Transition = { id: string; a: string; b: string; dur: number }
export type Project = {
  settings: Settings
  media: Media[]
  /** Index 0 is V1 (drawn first, shown lowest in the timeline). */
  video: Track[]
  /** Index 0 is A1 (shown first under the video tracks). */
  audio: Track[]
  clips: Clip[]
  transitions: Transition[]
  range: { in: number; out: number }
}

export const IMAGE_DUR = 5 // seconds, default length of a still

let n = 0
export const uid = (p: string): string => `${p}${Date.now().toString(36)}${(n++).toString(36)}`

export function newProject(): Project {
  return {
    settings: { width: 1920, height: 1080, fps: 30, transFrames: 30 },
    media: [],
    video: [{ id: uid('v'), kind: 'video' }],
    audio: [{ id: uid('a'), kind: 'audio' }],
    clips: [],
    transitions: [],
    range: { in: 0, out: 0 }
  }
}

export const end = (c: Clip): number => c.start + c.dur
export const frame = (t: number, fps: number): number => Math.round(t * fps) / fps
export const trackOf = (p: Project, id: string): Track | undefined => p.video.find((t) => t.id === id) ?? p.audio.find((t) => t.id === id)
export const mediaOf = (p: Project, id: string): Media | undefined => p.media.find((m) => m.id === id)
export const partner = (p: Project, c: Clip): Clip | undefined => (c.link ? p.clips.find((o) => o.link === c.link && o.id !== c.id) : undefined)
export const projectEnd = (p: Project): number => p.clips.reduce((m, c) => Math.max(m, end(c)), 0)
/** How far a clip can extend at each side before running out of source (images: unlimited). */
export function room(p: Project, c: Clip): { before: number; after: number } {
  const m = c.text ? undefined : mediaOf(p, c.mediaId)
  if (!m || m.kind === 'image') return { before: Infinity, after: Infinity }
  return { before: c.in, after: m.duration - (c.in + c.dur) }
}

/** Overwrite edit: make room for [a, b) on a track by trimming, splitting or removing other clips. */
export function clearRange(p: Project, trackId: string, a: number, b: number, keep: Set<string>): void {
  const eps = 1e-6
  const out: Clip[] = []
  for (const c of p.clips) {
    if (c.trackId !== trackId || keep.has(c.id) || c.start >= b - eps || end(c) <= a + eps) { out.push(c); continue }
    const e = end(c)
    if (c.start < a - eps) out.push({ ...c, dur: a - c.start }) // left part survives
    if (e > b + eps) out.push({ ...structuredClone(c), id: c.start < a - eps ? uid('c') : c.id, link: c.start < a - eps ? undefined : c.link, start: b, in: c.in + (b - c.start), dur: e - b })
  }
  p.clips = out
}

/** Split a clip (and its linked partner) at time t. The right-hand pieces are linked to each other. */
export function cutAt(p: Project, clipId: string, t: number): void {
  const c = p.clips.find((x) => x.id === clipId)
  if (!c) return
  const group = [c, partner(p, c)].filter((x): x is Clip => !!x && x.start < t - 1e-6 && end(x) > t + 1e-6)
  const link = group.length > 1 ? uid('l') : undefined
  for (const g of group) {
    const right: Clip = { ...structuredClone(g), id: uid('c'), start: t, in: g.in + (t - g.start), dur: end(g) - t, link }
    g.dur = t - g.start
    p.clips.push(right)
  }
}

/** Snap `t` to the nearest candidate within `tol` seconds; returns [snapped time, candidate or null]. */
export function snapTo(t: number, candidates: number[], tol: number): [number, number | null] {
  let best: number | null = null
  for (const c of candidates) if (Math.abs(c - t) <= tol && (best === null || Math.abs(c - t) < Math.abs(best - t))) best = c
  return [best ?? t, best]
}

/** Snap targets: clip edges on every track (except `exclude`), plus extra points (playhead, In/Out, 0). */
export function snapPoints(p: Project, exclude: Set<string>, extra: number[]): number[] {
  const pts = [0, ...extra]
  for (const c of p.clips) if (!exclude.has(c.id)) pts.push(c.start, end(c))
  return pts
}

/** The empty stretch on a track around time t, or null if t is inside a clip or after the last one. */
export function gapAt(p: Project, trackId: string, t: number): { a: number; b: number } | null {
  const cs = p.clips.filter((c) => c.trackId === trackId)
  if (cs.some((c) => c.start <= t && end(c) > t)) return null
  const a = Math.max(0, ...cs.filter((c) => end(c) <= t).map(end))
  const after = cs.filter((c) => c.start > t).map((c) => c.start)
  if (!after.length) return null
  return { a, b: Math.min(...after) }
}

/** Close Gap: pull everything after the gap on that track (and linked partners) left to meet the previous clip. */
export function closeGap(p: Project, trackId: string, t: number): void {
  const g = gapAt(p, trackId, t)
  if (!g) return
  const d = g.b - g.a
  const moving = new Set<string>()
  for (const c of p.clips) if (c.trackId === trackId && c.start >= g.b - 1e-6) { moving.add(c.id); const o = partner(p, c); if (o) moving.add(o.id) }
  for (const c of p.clips) if (moving.has(c.id)) c.start = Math.max(0, c.start - d)
}

/** Q / R: move a clip's front (Q) or rear (R) handle to time t, for the given clips and their partners. */
export function trimTo(p: Project, ids: string[], t: number, side: 'front' | 'rear'): void {
  const all = new Set(ids)
  for (const id of ids) { const c = p.clips.find((x) => x.id === id); const o = c && partner(p, c); if (o) all.add(o.id) }
  for (const c of p.clips) {
    if (!all.has(c.id) || c.start >= t - 1e-6 || end(c) <= t + 1e-6) continue
    if (side === 'front') { c.in += t - c.start; c.dur -= t - c.start; c.start = t } else c.dur = t - c.start
  }
}

/** Track for an index, creating one past the end if needed (dropping into the empty zone makes a new track). */
export function trackAt(p: Project, kind: TrackKind, i: number): Track {
  const list = kind === 'video' ? p.video : p.audio
  while (list.length <= i) list.push({ id: uid(kind === 'video' ? 'v' : 'a'), kind })
  return list[i]
}

// ── Transitions ──
const EPS = 1e-4
/** Two clips on one track that touch near time t (within `tol` s): the seam there. */
export function seamAt(p: Project, trackId: string, t: number, tol: number): { a: Clip; b: Clip } | null {
  const cs = p.clips.filter((c) => c.trackId === trackId)
  for (const a of cs) {
    if (Math.abs(end(a) - t) > tol) continue
    const b = cs.find((c) => c !== a && Math.abs(c.start - end(a)) < EPS)
    if (b) return { a, b }
  }
  return null
}
/** Longest crossfade the hidden frames allow: half on each side of the seam. 0 = none possible. */
export const maxTransition = (p: Project, a: Clip, b: Clip): number => 2 * Math.min(room(p, a).after, room(p, b).before)

/** Transitions whose clips still exist and still touch, with durations clamped to what the clips allow now. */
export function liveTransitions(p: Project): (Transition & { A: Clip; B: Clip; cut: number })[] {
  const out: (Transition & { A: Clip; B: Clip; cut: number })[] = []
  for (const tr of p.transitions) {
    const A = p.clips.find((c) => c.id === tr.a), B = p.clips.find((c) => c.id === tr.b)
    if (!A || !B || A.trackId !== B.trackId || Math.abs(end(A) - B.start) > EPS) continue
    const dur = Math.min(tr.dur, maxTransition(p, A, B), A.dur * 2, B.dur * 2)
    if (dur > EPS) out.push({ ...tr, dur, A, B, cut: B.start })
  }
  return out
}

/**
 * Add the Default transition at a seam; also across the linked audio seam (or linked video seam) when the
 * partners touch too. Returns false if there are no spare frames.
 */
export function addTransition(p: Project, a: Clip, b: Clip): boolean {
  const pairs: [Clip, Clip][] = [[a, b]]
  const pa = partner(p, a), pb = partner(p, b)
  if (pa && pb && pa.trackId === pb.trackId && Math.abs(end(pa) - pb.start) < EPS) pairs.push([pa, pb])
  let any = false
  for (const [x, y] of pairs) {
    const max = maxTransition(p, x, y)
    if (max <= EPS) continue
    p.transitions = p.transitions.filter((t) => !(t.a === x.id && t.b === y.id))
    p.transitions.push({ id: uid('x'), a: x.id, b: y.id, dur: Math.min(p.settings.transFrames / p.settings.fps, max) })
    any = true
  }
  return any
}

/** What a track shows at time t: one clip at full strength, or two mid-crossfade (eased) with their weights. */
export function mixAt(p: Project, trackId: string, t: number, live = liveTransitions(p)): { c: Clip; w: number }[] {
  for (const tr of live) {
    if (tr.A.trackId !== trackId) continue
    const a = tr.cut - tr.dur / 2, b = tr.cut + tr.dur / 2
    if (t >= a && t < b) { const e = ease((t - a) / tr.dur); return [{ c: tr.A, w: 1 - e }, { c: tr.B, w: e }] }
  }
  const c = p.clips.find((x) => x.trackId === trackId && x.start <= t && t < end(x))
  return c ? [{ c, w: 1 }] : []
}

/** Time span a clip plays, including frames borrowed by transitions on either side. */
export function playSpan(c: Clip, live: ReturnType<typeof liveTransitions>): [number, number] {
  let a = c.start, b = end(c)
  for (const tr of live) { if (tr.B.id === c.id) a = tr.cut - tr.dur / 2; if (tr.A.id === c.id) b = tr.cut + tr.dur / 2 }
  return [a, b]
}
