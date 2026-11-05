import { useEffect, useId, useRef } from 'react'
import { useReducedMotion } from 'motion/react'

/**
 * "Processing" indicator: a row of dot pairs joined by faint rungs. A wave runs left to right; each
 * pair snaps together into one gooey capsule, holds, then pulls apart again.
 *   <DnaThink />   <DnaThink width={200} className="text-[var(--kit-accent)]" />
 * Takes the text colour. Decorative unless you pass `label`. Reduced motion: holds one frame.
 */

const COLS = 11
const GAP = 50 // viewBox units between columns (measured in the reference)
const H = 140
const PERIOD = 1.7 // s per pair (85 frames at 50 fps)
const WAVE = 7 // columns per wavelength (each column lags 1/7 of a period)
const xs = Array.from({ length: COLS }, (_, i) => GAP / 2 + i * GAP)

/** Piecewise-linear track: `keys` are [phase, value] pairs. */
const k = (keys: [number, number][], p: number): number => {
  if (p <= keys[0][0]) return keys[0][1]
  for (let i = 1; i < keys.length; i++) {
    const [p1, v1] = keys[i]
    if (p <= p1) { const [p0, v0] = keys[i - 1]; return v0 + ((v1 - v0) * (p - p0)) / (p1 - p0) }
  }
  return keys[keys.length - 1][1]
}
// Measured per phase (averaged over all columns): dot centre distance from the middle, and dot radius.
// Closed (a capsule) → eases apart → holds → pulls slightly wider → snaps shut.
const D: [number, number][] = [
  [0, 5], [0.06, 7.5], [0.09, 10.5], [0.12, 13.5], [0.15, 20.8], [0.18, 28.5], [0.21, 33.5], [0.235, 36], [0.3, 39],
  [0.38, 41.5], [0.53, 42], [0.62, 45.5], [0.676, 43.5], [0.706, 32], [0.735, 21], [0.765, 13.5], [0.794, 9.5],
  [0.824, 7.2], [0.853, 5.5], [0.9, 4.5], [1, 5]
]
const R: [number, number][] = [
  [0, 7.5], [0.06, 7], [0.09, 6.5], [0.15, 6], [0.18, 5.5], [0.21, 5], [0.235, 4.5], [0.676, 4.5], [0.706, 5],
  [0.735, 5.5], [0.794, 6.5], [0.824, 7], [0.853, 7.5], [1, 7.5]
]

export function DnaThink({ width = 120, label, className = '' }: { width?: number; label?: string; className?: string }) {
  const svg = useRef<SVGSVGElement>(null)
  const still = !!useReducedMotion()
  const id = useId().replace(/[^a-zA-Z0-9]/g, '')

  useEffect(() => {
    const el = svg.current
    if (!el) return
    const lines = el.querySelectorAll('line')
    const dots = el.querySelectorAll('circle')
    const necks = el.querySelectorAll('rect')
    const draw = (t: number): void => {
      for (let i = 0; i < COLS; i++) {
        const p = (((t / PERIOD - i / WAVE) % 1) + 1) % 1
        const rm = k(R, p)
        const r = 0.4 * rm + 3.5 // drawn radius: the goo threshold shrinks lone dots more than merged ones
        const d = k(D, p) + rm - r + 0.5 // keep the measured outer extent
        lines[i].setAttribute('y1', String(H / 2 - d))
        lines[i].setAttribute('y2', String(H / 2 + d))
        dots[i * 2].setAttribute('cy', String(H / 2 - d))
        dots[i * 2 + 1].setAttribute('cy', String(H / 2 + d))
        dots[i * 2].setAttribute('r', String(r))
        dots[i * 2 + 1].setAttribute('r', String(r))
        // A neck between the pair: full width while they overlap (a straight-sided capsule, as in the
        // reference), thinning as they part so the goo pinches it off.
        const w = 2 * r * Math.min(1, Math.max(0, (2.8 * r - d) / (1.8 * r)))
        necks[i].setAttribute('x', String(xs[i] - w / 2))
        necks[i].setAttribute('width', String(w))
        necks[i].setAttribute('y', String(H / 2 - d))
        necks[i].setAttribute('height', String(2 * d))
      }
    }
    if (still) { draw(0.55); return }
    let raf = 0
    const t0 = performance.now()
    const tick = (now: number): void => { draw((now - t0) / 1000); raf = requestAnimationFrame(tick) }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [still])

  return (
    <svg
      ref={svg}
      width={width}
      height={(width * H) / (COLS * GAP)}
      viewBox={`0 0 ${COLS * GAP} ${H}`}
      className={className}
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <defs>
        {/* Gooey: blur, then crush alpha back to a hard edge, so nearby dots melt into one blob. */}
        <filter id={`goo${id}`} x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="3" />
          <feColorMatrix values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 20 -8" />
        </filter>
        <filter id={`glow${id}`} x="-20%" y="-50%" width="140%" height="200%">
          <feGaussianBlur stdDeviation="6" result="b" />
          <feComponentTransfer in="b" result="halo"><feFuncA type="linear" slope="0.85" /></feComponentTransfer>
          <feMerge><feMergeNode in="halo" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
      </defs>
      <g filter={`url(#glow${id})`}>
        <g stroke="currentColor" strokeWidth="1.6" opacity="0.4">
          {xs.map((x) => <line key={x} x1={x} x2={x} y1={H / 2} y2={H / 2} />)}
        </g>
        <g fill="currentColor" filter={`url(#goo${id})`}>
          {xs.map((x) => [<circle key={`${x}a`} cx={x} cy={H / 2} r="7" />, <circle key={`${x}b`} cx={x} cy={H / 2} r="7" />])}
          {xs.map((x) => <rect key={x} x={x} y={H / 2} width="0" height="0" />)}
        </g>
      </g>
    </svg>
  )
}
