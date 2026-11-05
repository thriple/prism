import { useEffect, useId, useRef } from 'react'
import { useReducedMotion } from 'motion/react'

/**
 * "Processing" indicator: two sparks at 12 and 6 o'clock stretch clockwise into glowing crescents,
 * the heads race ahead and the tails catch up half a turn later, then they shrink back to sparks. ~1.6 s loop.
 *   <RingThink />   <RingThink size={64} className="text-[var(--kit-accent)]" />
 * Takes the text colour. Decorative unless you pass `label`. Reduced motion: holds one frame.
 */

const FPS = 30
const FRAMES = 47 // the reference loop, including a short blank between sweeps
const RTIP = 164 // viewBox units: ring radius at the tips

/** Piecewise-linear track: `keys` are [frame, value] pairs; clamps at both ends. */
const k = (keys: [number, number][], f: number): number => {
  if (f <= keys[0][0]) return keys[0][1]
  for (let i = 1; i < keys.length; i++) {
    const [f1, v1] = keys[i]
    if (f <= f1) { const [f0, v0] = keys[i - 1]; return v0 + ((v1 - v0) * (f - f0)) / (f1 - f0) }
  }
  return keys[keys.length - 1][1]
}
// Measured from the reference (degrees clockwise from 12 o'clock, viewBox units). Tail and head of the first crescent:
const TAIL: [number, number][] = [
  [2, -1], [3, -1.7], [5, -2], [7, 0.3], [9, 1.4], [11, 3.8], [13, 8], [15, 14.3], [17, 22.5], [19, 32], [21, 45.4],
  [23, 66.1], [25, 95.8], [27, 124], [29, 143.1], [31, 155.4], [33, 164], [35, 170], [37, 173.6], [39, 175.8], [41, 176.8], [43, 178.6]
]
const HEAD: [number, number][] = [
  [2, 1], [3, 2.4], [5, 4.6], [7, 7.4], [9, 15.3], [11, 29], [13, 53.7], [15, 94.8], [17, 128.8], [19, 147.3], [21, 158.9],
  [23, 167.4], [25, 172.5], [27, 176.4], [29, 178.2], [31, 178.9], [33, 179.3], [35, 179.7], [37, 180.4], [39, 181.8], [41, 182.2], [43, 181.4]
]
const MID_R: [number, number][] = [[3, 164], [9, 164.6], [13, 167.8], [17, 172], [19, 173], [23, 173.5], [25, 171.5], [27, 166.6], [29, 163.7], [33, 161.5], [43, 161]]
const WIDTH: [number, number][] = [[1, 0], [2, 9], [3, 11], [4, 14], [5, 19], [13, 20.6], [22, 21], [29, 20], [35, 18.6], [39, 16], [41, 15], [42, 16], [43, 9], [44, 0]]
const HAZE: [number, number][] = [[1, 0], [2, 0.02], [5, 0.04], [9, 0.055], [13, 0.086], [17, 0.12], [22, 0.12], [27, 0.094], [31, 0.067], [35, 0.047], [39, 0.035], [43, 0.02], [44, 0]]

const pt = (deg: number, r: number): string => {
  const a = (deg * Math.PI) / 180
  return `${(r * Math.sin(a)).toFixed(2)} ${(-r * Math.cos(a)).toFixed(2)}`
}

/** Crescent from `t` to `h` degrees: thickest in the middle, bulging slightly outward. Short ones round into a spark. */
function crescent(t: number, h: number, rMid: number, w: number): string {
  if (w <= 0) return ''
  const len = ((h - t) * Math.PI * rMid) / 180
  const blend = Math.min(1, Math.max(0, (len / w - 1.5) / 3.5)) // 0 = round spark, 1 = tapered crescent
  const N = 28
  const outer: string[] = [], inner: string[] = []
  for (let i = 0; i <= N; i++) {
    const u = i / N
    const s = Math.sin(Math.PI * u)
    const half = (w / 2) * ((1 - blend) * 2 * Math.sqrt(u * (1 - u)) + blend * Math.pow(s, 1.3))
    const r = RTIP + (rMid - RTIP) * Math.sqrt(s)
    const a = t + (h - t) * u
    outer.push(pt(a, r + half))
    inner.push(pt(a, r - half))
  }
  return `M${outer.join('L')}L${inner.reverse().join('L')}Z`
}

export function RingThink({ size = 40, label, className = '' }: { size?: number; label?: string; className?: string }) {
  const svg = useRef<SVGSVGElement>(null)
  const still = !!useReducedMotion()
  const id = useId().replace(/[^a-zA-Z0-9]/g, '')

  useEffect(() => {
    const el = svg.current
    if (!el) return
    const [a, b] = el.querySelectorAll('path')
    const haze = el.querySelector('circle')!
    const draw = (f: number): void => {
      const d = crescent(k(TAIL, f), k(HEAD, f), k(MID_R, f), k(WIDTH, f))
      a.setAttribute('d', d)
      b.setAttribute('d', d) // the second crescent is the same shape turned 180° (see its transform)
      haze.setAttribute('opacity', k(HAZE, f).toFixed(3))
    }
    if (still) { draw(19); return }
    let raf = 0
    const t0 = performance.now()
    const tick = (now: number): void => { draw(1 + (((now - t0) / 1000) * FPS) % FRAMES); raf = requestAnimationFrame(tick) }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [still])

  return (
    <svg
      ref={svg}
      width={size}
      height={size}
      viewBox="-200 -200 400 400"
      overflow="visible"
      className={className}
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <defs>
        <radialGradient id={`haze${id}`}>
          <stop offset="0" stopColor="currentColor" />
          <stop offset="0.6" stopColor="currentColor" stopOpacity="0.8" />
          <stop offset="1" stopColor="currentColor" stopOpacity="0" />
        </radialGradient>
        {/* A tight halo plus a wide bloom that lights the space around the crescents (measured ~+50/255 at 15 units out). */}
        <filter id={`glow${id}`} filterUnits="userSpaceOnUse" x="-260" y="-260" width="520" height="520">
          <feGaussianBlur stdDeviation="6" result="b" />
          <feComponentTransfer in="b" result="halo"><feFuncA type="linear" slope="0.8" /></feComponentTransfer>
          <feGaussianBlur in="SourceGraphic" stdDeviation="26" result="w" />
          <feComponentTransfer in="w" result="bloom"><feFuncA type="linear" slope="0.9" /></feComponentTransfer>
          <feMerge><feMergeNode in="bloom" /><feMergeNode in="halo" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
      </defs>
      <circle r="300" fill={`url(#haze${id})`} opacity="0" />
      <g fill="currentColor" filter={`url(#glow${id})`}>
        <path />
        <path transform="rotate(180)" />
      </g>
    </svg>
  )
}
