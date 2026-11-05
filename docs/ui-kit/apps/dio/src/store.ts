import { useSyncExternalStore } from 'react'
import { computePeaks } from '../../../components'
import { bassEnvelope } from './beats'
import { newProject, uid, type Media, type Project } from './model'

// ── Project store: one immutable project; `edit` clones, mutates the draft, and publishes. ──
let project: Project = newProject()
const subs = new Set<() => void>()
export const store = {
  get: (): Project => project,
  set(p: Project): void { project = p; subs.forEach((f) => f()) },
  sub(f: () => void): () => void { subs.add(f); return () => { subs.delete(f) } }
}
// ── Undo: the last 20 edits. Rapid edits sharing a `key` (a drag, a slip) collapse into one step. ──
const UNDO_LIMIT = 20
const undoStack: Project[] = []
const redoStack: Project[] = []
let lastKey: string | undefined, lastAt = 0
const hsubs = new Set<() => void>()
const hemit = (): void => hsubs.forEach((f) => f())

export const edit = (fn: (draft: Project) => void, key?: string): void => {
  const now = performance.now()
  const merge = key !== undefined && key === lastKey && now - lastAt < 1000
  lastKey = key; lastAt = now
  const d = structuredClone(project)
  fn(d)
  if (!merge) {
    undoStack.push(project)
    if (undoStack.length > UNDO_LIMIT) undoStack.shift()
    redoStack.length = 0
    hemit()
  }
  store.set(d)
}
export function undo(): void {
  const prev = undoStack.pop()
  if (!prev) return
  redoStack.push(project); lastKey = undefined
  store.set(prev); hemit()
}
export function redo(): void {
  const next = redoStack.pop()
  if (!next) return
  undoStack.push(project); lastKey = undefined
  store.set(next); hemit()
}
/** Start a fresh history (after New / Load). */
export function resetHistory(): void { undoStack.length = 0; redoStack.length = 0; lastKey = undefined; hemit() }
export const history = {
  sub(f: () => void): () => void { hsubs.add(f); return () => { hsubs.delete(f) } },
  get: (): string => `${undoStack.length}|${redoStack.length}`
}
export const useProject = (): Project => useSyncExternalStore(store.sub, store.get)

// ── "Bring the playhead into view" requests (Home/End keys); the timeline listens. ──
const csubs = new Set<() => void>()
export const centerPlayhead = (): void => csubs.forEach((f) => f())
export const onCenterPlayhead = (f: () => void): (() => void) => { csubs.add(f); return () => { csubs.delete(f) } }

// ── Playhead time: separate store so playback only re-renders what shows the time. ──
let time = 0
const tsubs = new Set<() => void>()
export const clock = {
  get: (): number => time,
  set(t: number): void { if (t !== time) { time = t; tsubs.forEach((f) => f()) } },
  sub(f: () => void): () => void { tsubs.add(f); return () => { tsubs.delete(f) } }
}
export const useTime = (): number => useSyncExternalStore(clock.sub, clock.get)

// ── Media files live outside the project (not serialisable): object URLs, thumbnails, waveform peaks. ──
export type MediaRuntime = { file: File; url: string; thumb?: string; peaks?: Float32Array; bass?: Float32Array; img?: HTMLImageElement }
export const runtime = new Map<string, MediaRuntime>()
export const PEAKS_PER_SEC = 100
/** The library item being dragged (HTML drag data can't be read during dragover). */
export const dragMedia: { id: string | null } = { id: null }

const once = (el: HTMLElement, ev: string): Promise<void> =>
  new Promise((res, rej) => {
    el.addEventListener(ev, () => res(), { once: true })
    el.addEventListener('error', () => rej(new Error(`Can't read this file`)), { once: true })
  })

let decoder: AudioContext | null = null

/** Read a dropped/picked file: kind, duration, size, thumbnail and (for anything with sound) waveform peaks.
 *  Pass `id` to relink a file to existing media (after loading a project). */
export async function importFile(file: File, id?: string): Promise<Media> {
  const url = URL.createObjectURL(file)
  const kind = file.type.startsWith('video/') ? 'video' : file.type.startsWith('audio/') ? 'audio' : file.type.startsWith('image/') ? 'image' : null
  if (!kind) throw new Error(`${file.name}: not a video, audio or image file`)
  const m: Media = { id: id ?? uid('m'), name: file.name, kind, duration: 0, width: 0, height: 0, hasAudio: false, size: file.size, modified: file.lastModified }
  const rt: MediaRuntime = { file, url }

  if (kind === 'image') {
    const img = new Image()
    img.src = url
    await img.decode()
    m.width = img.naturalWidth; m.height = img.naturalHeight
    rt.img = img
    rt.thumb = thumbOf(img, img.naturalWidth, img.naturalHeight)
  } else {
    const el = document.createElement(kind)
    el.preload = 'auto'
    el.muted = true
    el.src = url
    await once(el, 'loadedmetadata')
    m.duration = el.duration
    if (el instanceof HTMLVideoElement) {
      m.width = el.videoWidth; m.height = el.videoHeight
      el.currentTime = Math.min(1, el.duration * 0.1)
      await once(el, 'seeked')
      rt.thumb = thumbOf(el, el.videoWidth, el.videoHeight)
    }
    try {
      decoder ??= new AudioContext()
      const buf = await decoder.decodeAudioData(await file.arrayBuffer())
      rt.peaks = computePeaks(buf, PEAKS_PER_SEC)
      rt.bass = bassEnvelope(buf)
      m.hasAudio = true
    } catch {
      m.hasAudio = false // no audio track (or a codec the browser can't decode)
    }
    if (kind === 'audio' && !m.hasAudio) throw new Error(`${file.name}: can't decode this audio`)
  }
  runtime.set(m.id, rt)
  return m
}

function thumbOf(src: CanvasImageSource, w: number, h: number): string {
  const c = document.createElement('canvas')
  c.height = 90
  c.width = Math.max(1, Math.round((90 * w) / Math.max(1, h)))
  c.getContext('2d')!.drawImage(src, 0, 0, c.width, c.height)
  return c.toDataURL('image/jpeg', 0.7)
}

/** Forget all imported media (New / Load). */
export function clearRuntime(): void {
  for (const r of runtime.values()) URL.revokeObjectURL(r.url)
  runtime.clear()
}

// ── Project file name (shown in the header; used for Save and Export names). ──
let docName = 'Untitled'
const nsubs = new Set<() => void>()
export const doc = {
  get: (): string => docName,
  set(n: string): void { docName = n; document.title = `${n} · Dio`; nsubs.forEach((f) => f()) },
  sub(f: () => void): () => void { nsubs.add(f); return () => { nsubs.delete(f) } }
}
export const useDocName = (): string => useSyncExternalStore(doc.sub, doc.get)
