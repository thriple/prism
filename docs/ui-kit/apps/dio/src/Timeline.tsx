import { useEffect, useRef, useState, type DragEvent, type PointerEvent as RPointerEvent, type ReactNode } from 'react'
import { ContextMenu, TimelineRuler, Timecode, Tooltip, Waveform, type MenuItem } from '../../../components'
import { detectBeats } from './beats'
import { engine } from './engine'
import {
  IMAGE_DUR, LABELS, addTransition, clearRange, defaultText, liveTransitions, maxTransition, propsOf, seamAt, closeGap, cutAt, end, frame, gapAt, mediaOf, partner, projectEnd, room, snapPoints, snapTo,
  trackAt, trackOf, uid, type Clip, type Media, type Project, type TrackKind
} from './model'
import { PEAKS_PER_SEC, clock, dragMedia, edit, onCenterPlayhead, runtime, useProject, useTime } from './store'

export type Tool = 'select' | 'cut' | 'slip'
const HEAD = 96 // track header width
const ROW = { video: 58, audio: 50, zone: 34 }
const SNAP_PX = 8

type Row = { id: string; kind: TrackKind; label?: string; zone?: boolean; muted?: boolean; hidden?: boolean }
type Preview = Record<string, { start?: number; in?: number; dur?: number; row?: string }>

const rowsOf = (p: Project): Row[] => [
  { id: 'new-video', kind: 'video', zone: true },
  ...p.video.map((t, i) => ({ id: t.id, kind: 'video' as const, label: `V${i + 1}`, hidden: t.hidden })).reverse(),
  ...p.audio.map((t, i) => ({ id: t.id, kind: 'audio' as const, label: `A${i + 1}`, muted: t.muted })),
  { id: 'new-audio', kind: 'audio', zone: true }
]
const rowIndex = (p: Project, row: string, kind: TrackKind): number => {
  const list = kind === 'video' ? p.video : p.audio
  return row === `new-${kind}` ? list.length : list.findIndex((t) => t.id === row)
}
const kindOfRow = (p: Project, row: string): TrackKind => (row === 'new-video' || p.video.some((t) => t.id === row) ? 'video' : 'audio')

/** Beat marks (timeline times) of audio clips that have the Beat Highlighter, per clip. */
export function beatsOf(p: Project, c: Clip): number[] {
  const f = c.fx?.find((x) => x.id === 'beats')
  const env = f && runtime.get(c.mediaId)?.bass
  if (!f || f.id !== 'beats' || !env) return []
  return detectBeats(env, f.sensitivity, f.gap, c.mediaId).filter((b) => b >= c.in && b < c.in + c.dur).map((b) => c.start + b - c.in)
}
const allBeats = (p: Project): number[] => p.clips.flatMap((c) => beatsOf(p, c))

/** Text tool (T): a default text clip at the playhead, on the lowest video track with 5 s free there (or a new one). */
export function addText(): string {
  const id = uid('c')
  edit((d) => {
    const t = frame(clock.get(), d.settings.fps)
    let i = d.video.findIndex((tr) => !d.clips.some((c) => c.trackId === tr.id && c.start < t + IMAGE_DUR && end(c) > t))
    if (i < 0) i = d.video.length
    d.clips.push({ id, trackId: trackAt(d, 'video', i).id, mediaId: '', start: t, in: 0, dur: IMAGE_DUR, text: defaultText() })
    if (d.range.out <= d.range.in) d.range = { in: 0, out: projectEnd(d) }
  })
  return id
}

/** Put a library item on the timeline: video (+ linked audio on the matching audio track), audio, or a still. */
export function placeMedia(d: Project, m: Media, row: string, start: number): void {
  const rowKind = kindOfRow(d, row)
  const idx = Math.max(0, rowIndex(d, row, rowKind))
  const dur = m.kind === 'image' ? IMAGE_DUR : m.duration
  const out: Clip[] = []
  const link = m.kind === 'video' && m.hasAudio ? uid('l') : undefined
  if (m.kind !== 'audio') out.push({ id: uid('c'), trackId: trackAt(d, 'video', rowKind === 'video' ? idx : 0).id, mediaId: m.id, start, in: 0, dur, link })
  if (m.kind === 'audio' || m.hasAudio) {
    const ai = m.kind === 'audio' ? (rowKind === 'audio' ? idx : 0) : idx
    out.push({ id: uid('c'), trackId: trackAt(d, 'audio', ai).id, mediaId: m.id, start, in: 0, dur, link })
  }
  for (const c of out) clearRange(d, c.trackId, c.start, end(c), new Set())
  d.clips.push(...out)
  if (d.range.out <= d.range.in) d.range = { in: 0, out: projectEnd(d) }
}

export function Timeline({ tool, setTool, pps, setPps, selected, setSelected }: {
  tool: Tool
  setTool: (t: Tool) => void
  pps: number
  setPps: (n: number) => void
  selected: string[]
  setSelected: (ids: string[]) => void
}) {
  const p = useProject()
  const t = useTime()
  const scroller = useRef<HTMLDivElement>(null)
  const [pv, setPv] = useState<Preview | null>(null)
  const [snapLine, setSnapLine] = useState<number | null>(null)
  const [ghost, setGhost] = useState<{ row: string; start: number; dur: number } | null>(null)
  const [menu, setMenu] = useState<{ at: { x: number; y: number }; items: MenuItem[] } | null>(null)
  const fps = p.settings.fps
  const dur = Math.max(projectEnd(p) + 30, 60)
  const tol = SNAP_PX / pps
  const view = useRef({ pps, fps })
  view.current = { pps, fps }

  /** Scroll so the playhead sits in the middle of the track area. */
  const center = (): void => {
    const el = scroller.current
    if (el) el.scrollLeft = clock.get() * view.current.pps - (el.clientWidth - HEAD) / 2
  }
  useEffect(() => onCenterPlayhead(center), [])
  // While playing, keep the playhead centred; otherwise it may go off screen.
  useEffect(() => { if (engine.playing) center() }, [t])

  // Wheel: over the ruler it moves the playhead (Shift = one frame); over the tracks it scrolls left/right.
  useEffect(() => {
    const el = scroller.current!
    const onWheel = (e: WheelEvent): void => {
      const tgt = e.target as HTMLElement
      const d = e.deltaY || e.deltaX
      if (!d) return
      if (tgt.closest('[data-ruler]')) {
        e.preventDefault()
        const { pps: z, fps: f } = view.current
        engine.seek(clock.get() + Math.sign(d) * (e.shiftKey ? 1 / f : Math.max(1 / f, 10 / z)))
      } else if (tgt.closest('[data-row]')) {
        e.preventDefault()
        el.scrollLeft += d
      }
    }
    el.addEventListener('wheel', onWheel, { passive: false })
    return () => el.removeEventListener('wheel', onWheel)
  }, [])

  const timeAt = (clientX: number): number => {
    const r = scroller.current!.getBoundingClientRect()
    return Math.max(0, (clientX - r.left + scroller.current!.scrollLeft - HEAD) / pps)
  }
  const rowAt = (x: number, y: number): string | null =>
    (document.elementFromPoint(x, y) as HTMLElement | null)?.closest<HTMLElement>('[data-row]')?.dataset.row ?? null
  const extras = (): number[] => [clock.get(), p.range.in, p.range.out, ...allBeats(p)]

  /** Generic pointer drag: `move` gets the pointer's time offset since pointerdown; `up` runs once at the end. */
  const drag = (e: RPointerEvent, move: (dt: number, ev: PointerEvent) => void, up: (moved: boolean) => void): void => {
    e.preventDefault(); e.stopPropagation()
    const t0 = timeAt(e.clientX), x0 = e.clientX, y0 = e.clientY
    let moved = false
    const mv = (ev: PointerEvent): void => { if (moved || Math.hypot(ev.clientX - x0, ev.clientY - y0) > 3) { moved = true; move(timeAt(ev.clientX) - t0, ev) } }
    const u = (): void => { window.removeEventListener('pointermove', mv); window.removeEventListener('pointerup', u); setSnapLine(null); up(moved) }
    window.addEventListener('pointermove', mv)
    window.addEventListener('pointerup', u)
  }

  const groupOf = (c: Clip): Clip[] => [c, partner(p, c)].filter((x): x is Clip => !!x)

  const onClipDown = (e: RPointerEvent, c: Clip, handle?: 'front' | 'rear'): void => {
    if (e.button !== 0) return
    if (tool === 'cut') {
      e.stopPropagation()
      const [at] = snapTo(timeAt(e.clientX), [clock.get()], tol)
      edit((d) => cutAt(d, c.id, frame(at, fps)))
      return
    }
    if (tool === 'slip') return slip(e, c)
    if (handle) return trim(e, c, handle)
    // Select (with linked partner); Shift toggles. Then drag to move.
    const g = groupOf(c).map((x) => x.id)
    const already = selected.includes(c.id)
    const sel = e.shiftKey ? (already ? selected.filter((id) => !g.includes(id)) : [...selected, ...g]) : already ? selected : g
    setSelected(sel)
    move(e, c, sel)
  }

  const move = (e: RPointerEvent, grab: Clip, sel: string[]): void => {
    const ids = new Set(sel)
    for (const id of sel) { const c = p.clips.find((x) => x.id === id); const o = c && partner(p, c); if (o) ids.add(o.id) }
    const moving = p.clips.filter((c) => ids.has(c.id))
    const kind = trackOf(p, grab.trackId)!.kind
    const cands = snapPoints(p, ids, extras())
    const minStart = Math.min(...moving.map((c) => c.start))
    let last: Preview = {}
    drag(e, (dt, ev) => {
      const [s, cs] = snapTo(grab.start + dt, cands, tol)
      const [en, ce] = snapTo(end(grab) + dt, cands, tol)
      if (cs !== null && (ce === null || Math.abs(s - grab.start - dt) <= Math.abs(en - end(grab) - dt))) { dt = s - grab.start; setSnapLine(cs) }
      else if (ce !== null) { dt = en - end(grab); setSnapLine(ce) } else setSnapLine(null)
      dt = frame(Math.max(dt, -minStart), fps)
      const over = rowAt(ev.clientX, ev.clientY)
      const delta = over && kindOfRow(p, over) === kind ? rowIndex(p, over, kind) - rowIndex(p, grab.trackId, kind) : 0
      const list = kind === 'video' ? p.video : p.audio
      last = {}
      for (const c of moving) {
        const ck = trackOf(p, c.trackId)!.kind
        let row = c.trackId
        if (ck === kind && sel.includes(c.id)) {
          const i = Math.min(list.length, Math.max(0, rowIndex(p, c.trackId, kind) + delta))
          row = i === list.length ? `new-${kind}` : list[i].id
        }
        last[c.id] = { start: c.start + dt, row }
      }
      setPv(last)
    }, (moved) => {
      setPv(null)
      if (!moved) { if (!e.shiftKey) setSelected(groupOf(grab).map((x) => x.id)); return }
      edit((d) => {
        const lens = { video: d.video.length, audio: d.audio.length }
        const placed: Clip[] = []
        for (const c of d.clips) {
          const q = last[c.id]
          if (!q) continue
          const k = kindOfRow(d, c.trackId)
          c.trackId = q.row === `new-${k}` ? trackAt(d, k, lens[k]).id : q.row!
          c.start = q.start!
          placed.push(c)
        }
        for (const c of placed) clearRange(d, c.trackId, c.start, end(c), ids)
      })
    })
  }

  const trim = (e: RPointerEvent, c: Clip, side: 'front' | 'rear'): void => {
    const g = groupOf(c)
    const ids = new Set(g.map((x) => x.id))
    const cands = snapPoints(p, ids, extras())
    const min = 1 / fps
    let d = 0
    drag(e, (dt) => {
      const edge = side === 'front' ? c.start : end(c)
      const [s, hit] = snapTo(edge + dt, cands, tol)
      setSnapLine(hit)
      d = frame(s - edge, fps)
      for (const x of g) {
        const r = room(p, x)
        d = side === 'front' ? Math.min(Math.max(d, -r.before, -x.start), x.dur - min) : Math.max(Math.min(d, r.after), min - x.dur)
      }
      const next: Preview = {}
      for (const x of g) next[x.id] = side === 'front' ? { start: x.start + d, in: x.in + d, dur: x.dur - d } : { dur: x.dur + d }
      setPv(next)
    }, (moved) => {
      setPv(null)
      if (!moved || !d) return
      edit((dd) => {
        for (const x of dd.clips) {
          if (!ids.has(x.id)) continue
          if (side === 'front') { x.start += d; x.in += d; x.dur -= d } else x.dur += d
          clearRange(dd, x.trackId, x.start, end(x), ids)
        }
      })
    })
  }

  /** Scrub (slip): the clip stays put; its content slides inside it. */
  const slip = (e: RPointerEvent, c: Clip): void => {
    const g = groupOf(c)
    const base = new Map(g.map((x) => [x.id, x.in]))
    drag(e, (dt) => {
      let d = -frame(dt, fps)
      for (const x of g) { const r = room(p, x); d = Math.max(-r.before, Math.min(r.after, d)) }
      if (!isFinite(d)) return
      edit((dd) => { for (const x of dd.clips) if (base.has(x.id)) x.in = base.get(x.id)! + d }, `slip:${c.id}`)
      // Show the first frame of the clip's new content while slipping.
      if (clock.get() < c.start || clock.get() >= end(c)) engine.seek(c.start)
    }, () => {})
  }

  const onLaneContext = (e: React.MouseEvent, row: Row): void => {
    e.preventDefault()
    if (row.zone) return
    const at = timeAt(e.clientX)
    // A seam (two touching clips) near the pointer: transitions.
    const seam = seamAt(p, row.id, at, 6 / pps)
    if (seam) {
      const max = maxTransition(p, seam.a, seam.b)
      const frames = Math.min(p.settings.transFrames, Math.floor(max * fps + 1e-6))
      const has = p.transitions.find((x) => x.a === seam.a.id && x.b === seam.b.id)
      const items: MenuItem[] = [{
        label: 'Add Default Transition',
        hint: frames > 0 ? `Cross dissolve · ${frames} frames` : 'Needs spare frames past both clip handles',
        disabled: frames <= 0,
        onSelect: () => edit((d) => { const a = d.clips.find((c) => c.id === seam.a.id)!, b = d.clips.find((c) => c.id === seam.b.id)!; addTransition(d, a, b) })
      }]
      if (has) items.push({ label: 'Remove Transition', danger: true, onSelect: () => edit((d) => { d.transitions = d.transitions.filter((x) => x.id !== has.id) }) })
      setMenu({ at: { x: e.clientX, y: e.clientY }, items })
      return
    }
    const clip = p.clips.find((c) => c.trackId === row.id && c.start <= at && at < end(c))
    if (clip) {
      const sel = selected.includes(clip.id) ? selected : groupOf(clip).map((x) => x.id)
      setSelected(sel)
      const items: MenuItem[] = []
      if (clip.link) {
        const l = clip.link
        items.push({ label: 'Unlink', onSelect: () => edit((d) => { for (const c of d.clips) if (c.link === l) c.link = undefined }) })
      } else {
        // Link: exactly one unlinked video clip and one unlinked audio clip selected.
        const pair = p.clips.filter((c) => sel.includes(c.id))
        const kinds = pair.map((c) => trackOf(p, c.trackId)?.kind).sort().join()
        if (pair.length === 2 && kinds === 'audio,video' && pair.every((c) => !c.link)) {
          items.push({ label: 'Link', onSelect: () => { const l = uid('l'); edit((d) => { for (const c of d.clips) if (sel.includes(c.id)) c.link = l }) } })
        }
      }
      items.push({ label: 'Delete', keys: ['Del'], danger: true, onSelect: () => deleteClips(sel) })
      setMenu({ at: { x: e.clientX, y: e.clientY }, items })
      return
    }
    if (gapAt(p, row.id, at)) setMenu({ at: { x: e.clientX, y: e.clientY }, items: [{ label: 'Close Gap', onSelect: () => edit((d) => closeGap(d, row.id, at)) }] })
  }

  const deleteClips = (ids: string[]): void => {
    const all = new Set(ids)
    edit((d) => { for (const c of d.clips) if (all.has(c.id)) { const o = partner(d, c); if (o) all.add(o.id) } d.clips = d.clips.filter((c) => !all.has(c.id)) })
    setSelected([])
  }

  // ── Drag from the library ──
  const onDragOver = (e: DragEvent, row: Row): void => {
    const m = dragMedia.id ? mediaOf(p, dragMedia.id) : undefined
    if (!m) return
    e.preventDefault()
    const len = m.kind === 'image' ? IMAGE_DUR : m.duration
    const [s, hit] = snapTo(frame(timeAt(e.clientX), fps), snapPoints(p, new Set(), extras()), tol)
    setSnapLine(hit)
    setGhost({ row: row.id, start: s, dur: len })
  }
  const onDrop = (e: DragEvent): void => {
    e.preventDefault()
    const m = dragMedia.id ? mediaOf(p, dragMedia.id) : undefined
    if (m && ghost) edit((d) => placeMedia(d, m, ghost.row, ghost.start))
    setGhost(null); setSnapLine(null); dragMedia.id = null
  }

  const live = liveTransitions(p)
  const shown = (c: Clip): Clip & { row: string } => ({ ...c, ...pv?.[c.id], row: pv?.[c.id]?.row ?? c.trackId })
  const rows = rowsOf(p)
  const W = dur * pps

  const toolBtn = (id: Tool, label: string, key: string, icon: ReactNode) => (
    <Tooltip label={label} keys={[key]} side="bottom">
      <button type="button" aria-label={label} aria-pressed={tool === id} onClick={() => setTool(id)}
        className={`grid size-8 place-items-center rounded-[8px] ${tool === id ? 'bg-[var(--kit-accent)] text-[var(--kit-ink)]' : 'text-white/70 hover:bg-white/10'}`}>{icon}</button>
    </Tooltip>
  )
  const svg = (d: string) => <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d={d} /></svg>

  return (
    <div className="flex h-full flex-col" onDragLeave={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node)) { setGhost(null); setSnapLine(null) } }}>
      <div className="flex h-10 shrink-0 items-center gap-1 border-b border-[var(--kit-line)] px-2">
        {toolBtn('select', 'Select', 'V', svg('M4 2.5 12.5 9 8.4 9.6 10.6 13.8 9 14.6 6.8 10.4 4 13z'))}
        {toolBtn('cut', 'Cut', 'C', svg('M5 12.5a1.8 1.8 0 1 0 0-.01M11 12.5a1.8 1.8 0 1 0 0-.01M6.3 11.2 12 2.5M9.7 11.2 4 2.5'))}
        {toolBtn('slip', 'Scrub (slip)', 'Y', svg('M2 8h12M4.5 5.5 2 8l2.5 2.5M11.5 5.5 14 8l-2.5 2.5M6 3.5h4v9H6z'))}
        <Tooltip label="Text: add a text clip at the playhead" keys={['T']} side="bottom">
          <button type="button" aria-label="Add text" onClick={() => setSelected([addText()])} className="grid size-8 place-items-center rounded-[8px] font-serif text-[15px] font-bold text-white/70 hover:bg-white/10">T</button>
        </Tooltip>
        <div className="mx-2 h-5 w-px bg-white/10" />
        <Tooltip label="Zoom out" keys={['−']} side="bottom"><button type="button" aria-label="Zoom out" onClick={() => setPps(Math.max(2, pps / 1.5))} className="grid size-8 place-items-center rounded-[8px] text-white/70 hover:bg-white/10">−</button></Tooltip>
        <Tooltip label="Zoom in" keys={['=']} side="bottom"><button type="button" aria-label="Zoom in" onClick={() => setPps(Math.min(600, pps * 1.5))} className="grid size-8 place-items-center rounded-[8px] text-white/70 hover:bg-white/10">+</button></Tooltip>
        <div className="flex-1" />
        <Timecode time={t} fps={fps} onChange={(s) => engine.seek(s)} />
      </div>

      <div ref={scroller} className="relative min-h-0 flex-1 overflow-auto thin-scroll">
        <div className="relative" style={{ width: HEAD + W, minHeight: '100%' }}>
          {/* Ruler row */}
          <div data-ruler className="sticky top-0 z-30 flex">
            <div className="sticky left-0 z-10 h-[40px] shrink-0 border-r border-[var(--kit-line)] bg-[var(--kit-ink)]" style={{ width: HEAD }} />
            <TimelineRuler duration={dur} fps={fps} pxPerSec={pps} time={t} onSeek={(s) => engine.seek(s)}
              range={p.range} onRange={(r) => edit((d) => { d.range = r }, 'range')}
              snap={(s) => snapTo(s, snapPoints(p, new Set(), [p.range.in, p.range.out, ...allBeats(p)]), tol)[0]} />
          </div>

          <div className={`relative ${pv ? '[&_[data-clip]]:pointer-events-none' : ''}`}>
            {rows.map((row, ri) => (
              <div key={row.id} className={`flex ${row.kind === 'audio' && rows[ri - 1]?.kind === 'video' ? 'border-t-2 border-white/15' : ''}`} style={{ height: row.zone ? ROW.zone : ROW[row.kind] }}>
                <div className="sticky left-0 z-20 flex shrink-0 items-center gap-1.5 border-b border-r border-[var(--kit-line)] bg-[#101217] px-2 text-[12px]" style={{ width: HEAD }}>
                  {row.zone ? <span className="text-[10px] leading-tight text-white/30">Drop for new {row.kind} track</span> : (
                    <>
                      <span className="w-6 font-semibold text-white/70">{row.label}</span>
                      {row.kind === 'video' ? (
                        <Tooltip label={row.hidden ? 'Show track' : 'Hide track'}>
                          <button type="button" aria-label={`${row.hidden ? 'Show' : 'Hide'} ${row.label}`} aria-pressed={!!row.hidden}
                            onClick={() => edit((d) => { const tr = d.video.find((x) => x.id === row.id)!; tr.hidden = !tr.hidden })}
                            className={`grid size-7 place-items-center rounded-[6px] hover:bg-white/10 ${row.hidden ? 'text-white/30' : 'text-white/80'}`}>
                            {row.hidden ? svg('M2 8s2.2-4 6-4 6 4 6 4-2.2 4-6 4-6-4-6-4zM3 13 13 3') : svg('M2 8s2.2-4 6-4 6 4 6 4-2.2 4-6 4-6-4-6-4zM8 9.6a1.6 1.6 0 1 0 0-3.2 1.6 1.6 0 0 0 0 3.2z')}
                          </button>
                        </Tooltip>
                      ) : (
                        <Tooltip label={row.muted ? 'Unmute track' : 'Mute track'}>
                          <button type="button" aria-label={`${row.muted ? 'Unmute' : 'Mute'} ${row.label}`} aria-pressed={!!row.muted}
                            onClick={() => edit((d) => { const tr = d.audio.find((x) => x.id === row.id)!; tr.muted = !tr.muted })}
                            className={`grid size-7 place-items-center rounded-[6px] text-[12px] font-bold ${row.muted ? 'bg-[var(--kit-danger)] text-[var(--kit-ink)]' : 'text-white/70 hover:bg-white/10'}`}>M</button>
                        </Tooltip>
                      )}
                    </>
                  )}
                </div>
                <div
                  data-row={row.id}
                  className={`relative flex-1 border-b border-[var(--kit-line)] ${row.zone ? 'bg-white/[0.015]' : ''} ${ghost?.row === row.id && row.zone ? 'bg-[var(--kit-accent)]/10' : ''}`}
                  onPointerDown={(e) => { if (e.button === 0 && tool === 'select') setSelected([]) }}
                  onContextMenu={(e) => onLaneContext(e, row)}
                  onDragOver={(e) => onDragOver(e, row)}
                  onDrop={onDrop}
                >
                  {p.clips.map(shown).filter((c) => c.row === row.id).map((c) => (
                    <ClipView key={c.id} c={c} p={p} pps={pps} tool={tool} selected={selected.includes(c.id)} dragging={!!pv?.[c.id]} onDown={onClipDown} />
                  ))}
                  {live.filter((tr) => tr.A.trackId === row.id && !pv).map((tr) => (
                    <button
                      key={tr.id}
                      type="button"
                      aria-label={`Transition, ${Math.round(tr.dur * fps)} frames`}
                      onPointerDown={(e) => { e.stopPropagation(); if (e.button === 0) setSelected([tr.id]) }}
                      onContextMenu={(e) => { e.preventDefault(); e.stopPropagation(); setMenu({ at: { x: e.clientX, y: e.clientY }, items: [{ label: 'Remove Transition', danger: true, onSelect: () => edit((d) => { d.transitions = d.transitions.filter((x) => x.id !== tr.id) }) }] }) }}
                      className={`absolute bottom-1 z-[5] h-[45%] overflow-hidden rounded-[5px] border text-[9px] font-semibold text-white ${selected.includes(tr.id) ? 'border-[var(--kit-accent)] ring-1 ring-[var(--kit-accent)]' : 'border-black/50'}`}
                      style={{ left: (tr.cut - tr.dur / 2) * pps, width: Math.max(8, tr.dur * pps), background: 'linear-gradient(to bottom right, rgba(255,255,255,.55), rgba(20,20,30,.85))' }}
                    >
                      {tr.dur * pps > 46 && 'Dissolve'}
                    </button>
                  ))}
                  {ghost?.row === row.id && (
                    <div className="pointer-events-none absolute inset-y-1 rounded-[6px] border-2 border-dashed border-[var(--kit-accent)]" style={{ left: ghost.start * pps, width: ghost.dur * pps }} />
                  )}
                </div>
              </div>
            ))}
            {/* Playhead + snap guide through all tracks */}
            <div className="pointer-events-none absolute inset-y-0 z-10 w-[2px] bg-[var(--kit-danger)]" style={{ left: HEAD + t * pps - 1 }} />
            {snapLine !== null && <div className="pointer-events-none absolute inset-y-0 z-10 w-px bg-[var(--kit-accent)]" style={{ left: HEAD + snapLine * pps }} />}
          </div>
        </div>
      </div>
      <ContextMenu at={menu?.at ?? null} items={menu?.items ?? []} onClose={() => setMenu(null)} />
    </div>
  )
}

function ClipView({ c, p, pps, tool, selected, dragging, onDown }: {
  c: Clip & { row: string }
  p: Project
  pps: number
  tool: Tool
  selected: boolean
  dragging: boolean
  onDown: (e: RPointerEvent, c: Clip, handle?: 'front' | 'rear') => void
}) {
  const m = mediaOf(p, c.mediaId)
  const rt = runtime.get(c.mediaId)
  const kind = trackOf(p, c.trackId)?.kind ?? 'video'
  const orig = p.clips.find((x) => x.id === c.id)!
  const label = c.text ? `T  ${c.text.content.split('\n')[0]}` : m?.name
  const beats = kind === 'audio' ? beatsOf(p, c) : []
  // Keyframe times (any property) inside the clip, for the selected clip.
  const keys = selected ? [...new Set(Object.values(propsOf(c)).flatMap((a) => (typeof a === 'object' ? a.k ?? [] : []).map((k) => k.t)))].filter((t) => t >= c.in && t <= c.in + c.dur) : []
  const cursor = tool === 'cut' ? 'cursor-crosshair' : tool === 'slip' ? 'cursor-ew-resize' : 'cursor-grab'
  return (
    <div
      data-clip
      onPointerDown={(e) => onDown(e, orig)}
      className={`absolute inset-y-1 overflow-hidden rounded-[6px] border ${cursor} ${selected ? 'border-[var(--kit-accent)] ring-1 ring-[var(--kit-accent)]' : 'border-black/40'} ${dragging ? 'opacity-80' : ''}`}
      style={{
        left: c.start * pps,
        width: Math.max(2, c.dur * pps),
        backgroundColor: c.color ? `color-mix(in srgb, ${LABELS[c.color - 1]} ${kind === 'video' ? 70 : 60}%, #111)` : c.text ? '#5b3a8c' : kind === 'video' ? '#2b4a7a' : '#2d6b4f',
        backgroundImage: !c.text && !rt ? 'repeating-linear-gradient(45deg, #5a1d1d 0 8px, #3a1212 8px 16px)' : kind === 'video' && rt?.thumb ? `url(${rt.thumb})` : undefined,
        backgroundSize: 'auto 100%',
        backgroundRepeat: 'repeat-x'
      }}
      title={c.text ? c.text.content : m?.name}
    >
      {c.color && kind === 'video' && <div className="pointer-events-none absolute inset-x-0 top-0 h-[5px]" style={{ background: LABELS[c.color - 1] }} />}
      {c.color && kind === 'video' && rt?.thumb && <div className="pointer-events-none absolute inset-0 opacity-35" style={{ background: LABELS[c.color - 1] }} />}
      {kind === 'audio' && rt?.peaks && (
        <Waveform peaks={rt.peaks} from={c.in * PEAKS_PER_SEC} to={(c.in + c.dur) * PEAKS_PER_SEC} className="absolute inset-0 h-full w-full text-white/55" />
      )}
      <span className="pointer-events-none absolute left-1.5 top-0.5 max-w-[calc(100%-12px)] truncate rounded-[4px] bg-black/55 px-1 text-[10px] text-white/90">
        {c.link ? '🔗 ' : ''}{label}{!c.text && !rt ? ' · offline' : ''}
      </span>
      {beats.map((b) => <div key={b} className="pointer-events-none absolute inset-y-0 w-[2px] bg-[var(--kit-accent)]/85" style={{ left: (b - c.start) * pps - 1 }} />)}
      {keys.map((k) => (
        <svg key={k} width="9" height="9" viewBox="0 0 10 10" className="pointer-events-none absolute bottom-0.5" style={{ left: (k - c.in) * pps - 4.5 }}>
          <path d="M5 .5 9.5 5 5 9.5.5 5z" fill="var(--kit-accent)" stroke="#000" strokeWidth=".8" />
        </svg>
      ))}
      {selected && tool === 'select' && (['front', 'rear'] as const).map((h) => (
        <div
          key={h}
          role="slider"
          aria-label={h === 'front' ? 'Trim start' : 'Trim end'}
          onPointerDown={(e) => onDown(e, orig, h)}
          className={`absolute inset-y-0 w-[9px] cursor-ew-resize bg-[var(--kit-accent)] ${h === 'front' ? 'left-0 rounded-l-[5px]' : 'right-0 rounded-r-[5px]'}`}
        >
          <div className="absolute inset-y-[35%] left-[3px] w-[3px] rounded-full bg-[var(--kit-ink)]/60" />
        </div>
      ))}
    </div>
  )
}

