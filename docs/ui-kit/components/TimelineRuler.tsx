import { useRef, type PointerEvent as RPointerEvent } from 'react'
import { formatTC } from './Timecode'

export type TimeRange = { in: number; out: number }

// Label spacing candidates, in seconds (frame-based ones are added per fps).
const STEPS = [1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600]

/**
 * Time ruler for a timeline: tick marks and timecode labels, a playhead you can grab or click to
 * scrub, and (with `range`) a two-handle strip on top marking the part to render (In / Out).
 * It is `duration * pxPerSec` wide; put it in the same horizontal scroller as the tracks.
 *   <TimelineRuler duration={60} fps={30} pxPerSec={80} time={t} onSeek={seek} range={io} onRange={setIo} />
 */
export function TimelineRuler({ duration, fps, pxPerSec, time, onSeek, range, onRange, snap }: {
  duration: number
  fps: number
  pxPerSec: number
  time: number
  onSeek: (sec: number) => void
  range?: TimeRange
  onRange?: (r: TimeRange) => void
  /** Optional: adjust a dragged time (e.g. magnetic snapping). */
  snap?: (sec: number) => number
}) {
  const el = useRef<HTMLDivElement>(null)
  const width = Math.max(1, duration * pxPerSec)
  // Major step: the smallest one that keeps labels ≥ 90 px apart (frames first when zoomed right in).
  const steps = [1 / fps, 5 / fps, 10 / fps, ...STEPS]
  const major = steps.find((s) => s * pxPerSec >= 90) ?? 3600
  const minor = major / (major >= 1 && major % 5 === 0 ? 5 : major >= 1 ? 4 : Math.max(1, Math.round(major * fps)))
  const at = (e: { clientX: number }): number => {
    const r = el.current!.getBoundingClientRect()
    return Math.min(duration, Math.max(0, (e.clientX - r.left) / pxPerSec))
  }
  const frame = (s: number) => Math.round(s * fps) / fps
  const adj = (s: number) => frame(snap ? snap(s) : s)

  const drag = (e: RPointerEvent, move: (s: number) => void): void => {
    e.preventDefault()
    e.currentTarget.setPointerCapture(e.pointerId)
    move(at(e))
    const target = e.currentTarget as HTMLElement
    const onMove = (ev: PointerEvent) => move(at(ev))
    const onUp = () => { target.removeEventListener('pointermove', onMove); target.removeEventListener('pointerup', onUp) }
    target.addEventListener('pointermove', onMove)
    target.addEventListener('pointerup', onUp)
  }

  const labels = []
  for (let s = 0; s <= duration + 1e-6; s += major) {
    labels.push(<span key={s} className="absolute top-[13px] pl-1 font-mono text-[10px] text-white/45" style={{ left: s * pxPerSec }}>{formatTC(s, fps)}</span>)
  }

  return (
    <div ref={el} className="relative h-[40px] select-none bg-[var(--kit-ink)]" style={{ width }}>
      {/* Render range strip */}
      <div className="absolute inset-x-0 top-0 h-[10px] bg-white/[0.03]">
        {range && (
          <>
            <div className="absolute inset-y-0 bg-[var(--kit-accent)]/35" style={{ left: range.in * pxPerSec, width: Math.max(0, range.out - range.in) * pxPerSec }} />
            {(['in', 'out'] as const).map((k) => (
              <div
                key={k}
                role="slider"
                aria-label={k === 'in' ? 'In point (I)' : 'Out point (O)'}
                aria-valuenow={range[k]}
                onPointerDown={(e) => onRange && drag(e, (s) => {
                  const v = adj(s)
                  onRange(k === 'in' ? { in: Math.min(v, range.out), out: range.out } : { in: range.in, out: Math.max(v, range.in) })
                })}
                className="absolute top-0 h-[10px] w-[7px] cursor-ew-resize bg-[var(--kit-accent)]"
                style={{ left: range[k] * pxPerSec - (k === 'in' ? 0 : 7), borderRadius: k === 'in' ? '0 0 0 3px' : '0 0 3px 0' }}
              />
            ))}
          </>
        )}
      </div>
      {/* Ticks + labels: click or drag to scrub */}
      <div
        className="absolute inset-x-0 bottom-0 top-[10px] cursor-text"
        onPointerDown={(e) => drag(e, (s) => onSeek(adj(s)))}
        style={{
          backgroundImage: `linear-gradient(to right, rgb(255 255 255 / .35) 1px, transparent 1px), linear-gradient(to right, rgb(255 255 255 / .16) 1px, transparent 1px)`,
          backgroundSize: `${major * pxPerSec}px 100%, ${minor * pxPerSec}px 6px`,
          backgroundRepeat: 'repeat-x',
          backgroundPosition: '0 0, 0 100%'
        }}
      >
        {labels}
      </div>
      {/* Playhead head */}
      <div className="pointer-events-none absolute bottom-0 top-[10px] w-0" style={{ left: time * pxPerSec }}>
        <div className="absolute -left-[6px] top-0 h-[12px] w-[12px] rounded-b-[3px] bg-[var(--kit-danger)]" />
        <div className="absolute -left-px bottom-0 top-0 w-[2px] bg-[var(--kit-danger)]" />
      </div>
    </div>
  )
}
