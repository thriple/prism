import { useEffect, useRef } from 'react'

const FLOOR = -60 // dB at the bottom of the meter
const toPos = (lin: number): number => {
  const db = 20 * Math.log10(Math.max(lin, 1e-6))
  return Math.min(1, Math.max(0, (db - FLOOR) / -FLOOR))
}

/**
 * Audio level meter, one bar per channel, on a dB scale (−60 to 0). Green → amber at −12 dB → red at −3 dB,
 * with a peak marker that holds for a second then falls. Feed it linear peak levels (0–1), e.g. from an AnalyserNode.
 *   <LevelMeter levels={[l, r]} height={200} />
 * `levels` may also be a function, polled every animation frame, so fast-changing audio doesn't re-render React.
 */
export function LevelMeter({ levels, height = 160, barWidth = 8, label = 'Audio level' }: {
  levels: number[] | (() => number[])
  /** px, or any CSS length such as '100%' */
  height?: number | string
  barWidth?: number
  label?: string
}) {
  const bars = useRef<HTMLDivElement>(null)
  const src = useRef(levels)
  src.current = levels
  useEffect(() => {
    let raf = 0
    const peaks: { v: number; t: number }[] = []
    const tick = (now: number): void => {
      const lv = typeof src.current === 'function' ? src.current() : src.current
      const el = bars.current
      if (el) {
        lv.forEach((l, i) => {
          const p = toPos(l)
          const pk = peaks[i] ?? (peaks[i] = { v: 0, t: 0 })
          if (p >= pk.v) { pk.v = p; pk.t = now } else if (now - pk.t > 1000) pk.v = Math.max(p, pk.v - 0.012)
          const bar = el.children[i] as HTMLElement | undefined
          if (!bar) return
          ;(bar.children[0] as HTMLElement).style.clipPath = `inset(${(1 - p) * 100}% 0 0 0)`
          ;(bar.children[1] as HTMLElement).style.bottom = `${pk.v * 100}%`
        })
      }
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [])
  const n = (typeof levels === 'function' ? levels() : levels).length
  const marks = [0, -6, -12, -24, -48]
  return (
    <div role="meter" aria-label={label} className="flex gap-1.5" style={{ height }}>
      <div ref={bars} className="flex gap-[3px]">
        {Array.from({ length: n }, (_, i) => (
          <div key={i} className="relative h-full overflow-hidden rounded-[2px] bg-white/8" style={{ width: barWidth }}>
            <div className="absolute inset-0" style={{ clipPath: 'inset(100% 0 0 0)', background: `linear-gradient(to top, var(--kit-ok) 0%, var(--kit-ok) ${toPos(10 ** (-12 / 20)) * 100}%, var(--kit-busy) ${toPos(10 ** (-12 / 20)) * 100}%, var(--kit-busy) ${toPos(10 ** (-3 / 20)) * 100}%, var(--kit-danger) ${toPos(10 ** (-3 / 20)) * 100}%)` }} />
            <div className="absolute inset-x-0 h-[2px] bg-white" style={{ bottom: 0 }} />
          </div>
        ))}
      </div>
      <div className="relative w-5 font-mono text-[9px] text-white/40">
        {marks.map((m) => <span key={m} className="absolute left-0 -translate-y-1/2" style={{ top: `${(1 - (m - FLOOR) / -FLOOR) * 100}%` }}>{m}</span>)}
      </div>
    </div>
  )
}
