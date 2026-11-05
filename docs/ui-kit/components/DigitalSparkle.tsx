import { useEffect, useRef } from 'react'
import { useReducedMotion } from 'motion/react'

/**
 * Digital sparkles: crisp, sprite-like bursts in white plus one accent colour. ~1.3 s.
 *   <DigitalSparkle n={1} />                       plays once on mount (replay by changing its `key`)
 *   <DigitalSparkle n={2} size={120} loop gap={0.6} />
 *   <DigitalSparkle n={3} loop tilt={62} />         laid flat, e.g. a marker on 3D terrain
 * 1 cross-hairs → orange X → swelling ring · 2 three waves of cyan/white rays · 3 green pinwheel that bursts and returns in white.
 * Decorative (aria-hidden). Reduced motion: holds the peak frame for the same time instead of animating.
 */

const FPS = 60
const FRAMES = 80
const COLORS = { 1: '#fe6c06', 2: '#04feed', 3: '#01fda0' } as const
const W = '#fff'

/** Piecewise-linear track: `keys` are [frame, value] pairs; clamps at both ends. */
const k = (keys: [number, number][], f: number): number => {
  if (f <= keys[0][0]) return keys[0][1]
  for (let i = 1; i < keys.length; i++) {
    const [f1, v1] = keys[i]
    if (f <= f1) { const [f0, v0] = keys[i - 1]; return v0 + ((v1 - v0) * (f - f0)) / (f1 - f0) }
  }
  return keys[keys.length - 1][1]
}
const at = (deg: number, r: number) => {
  const a = (deg * Math.PI) / 180
  return `${(r * Math.cos(a)).toFixed(2)} ${(r * Math.sin(a)).toFixed(2)}`
}
const ray = (deg: number, r0: number, r1: number, c: string, w: number, o = 1) =>
  r1 - r0 < 0.3 || o <= 0 ? '' : `<path d="M${at(deg, r0)}L${at(deg, r1)}" stroke="${c}" stroke-width="${w}" opacity="${o.toFixed(3)}"/>`
const ring = (r: number, w: number, c: string, o = 1) =>
  o <= 0 || r <= 0 ? '' : `<circle r="${r.toFixed(2)}" fill="none" stroke="${c}" stroke-width="${w.toFixed(2)}" opacity="${o.toFixed(3)}"/>`
/** Short tangential dash centred on the circle of radius r. */
const dash = (deg: number, r: number, len: number, c: string, w: number, o = 1, tilt = 0) => {
  if (o <= 0) return ''
  const a = (deg * Math.PI) / 180, t = a + Math.PI / 2 + (tilt * Math.PI) / 180 // tangent, turned by `tilt`
  const [x, y, dx, dy] = [r * Math.cos(a), r * Math.sin(a), (len / 2) * Math.cos(t), (len / 2) * Math.sin(t)]
  return `<path d="M${(x - dx).toFixed(2)} ${(y - dy).toFixed(2)}L${(x + dx).toFixed(2)} ${(y + dy).toFixed(2)}" stroke="${c}" stroke-width="${w}" opacity="${o.toFixed(3)}"/>`
}
const out = (f: number, from: number, to: number) => k([[from, 1], [to, 0]], f) // fade-out helper

/**
 * Shared ray track (measured from the reference): [frames since launch, inner radius, outer radius].
 * A ray starts as a speck, shoots out long, snaps to a short dash at the rim and lingers there.
 */
const RAY: [number, number, number][] = [
  [0, 1, 3], [2, 2, 4], [4, 2, 6], [6, 3, 8], [8, 4, 10], [10, 6, 14], [12, 9, 22], [14, 12, 66], [16, 18, 77],
  [18, 30, 82], [20, 72, 84], [22, 79, 88], [24, 83, 89], [26, 86, 90], [28, 88, 92], [30, 90, 92], [34, 92, 93], [40, 93, 94]
]
const R0 = RAY.map(([t, a]) => [t, a] as [number, number])
const R1 = RAY.map(([t, , b]) => [t, b] as [number, number])
const STROKE = 5
/** `count` rays launched at frame `start`, turned by `turn` degrees; `color(i)` picks each ray's colour. */
const burst = (f: number, start: number, turn: number, count: number, color: (i: number) => string) => {
  const t = f - start
  if (t < 0) return ''
  let s = ''
  for (let i = 0; i < count; i++) s += ray(turn + (i * 360) / count, k(R0, t), k(R1, t), color(i), STROKE, out(t, 34, 46))
  return s
}
const speck = (f: number, from: number, to: number) =>
  f < from || f > to ? '' : `<rect x="-2" y="-2" width="4" height="4" fill="${W}" opacity="${Math.min(1, k([[from, 0], [from + 3, 1]], f), out(f, to - 1, to)).toFixed(2)}"/>`

// Each sprite draws one frame (0…FRAMES) in a -100…100 box.
const SPRITES: Record<1 | 2 | 3, (f: number, c: string) => string> = {
  // 1: white speck → white cross-hairs shoot out → orange X follows 10 frames later → a small orange ring
  //    swells very thick, then expands and thins out.
  1: (f, c) => {
    let s = speck(f, 4, 15) + burst(f, 15, 0, 4, () => W) + burst(f, 25, 45, 4, () => c)
    const r = k([[17, 0.5], [19, 1], [25, 2.5], [29, 4], [31, 5], [39, 5], [41, 7], [43, 10], [45, 13.5], [47, 21], [49, 35.5], [51, 49], [53, 58], [55, 63.5], [57, 67], [59, 69.5], [61, 72], [63, 73], [65, 74.5], [69, 75.5]], f)
    const w = k([[17, 1], [19, 2], [25, 3], [29, 4], [31, 2], [41, 2], [43, 2], [45, 3], [47, 6], [49, 19], [51, 18], [53, 12], [55, 7], [57, 6], [59, 5], [61, 4], [63, 2], [65, 1.5], [69, 1]], f)
    if (f >= 17) s += ring(r, w, c, out(f, 62, 78))
    return s
  },
  // 2: white speck with a cyan core, then three waves of 8 rays (white on the axes, cyan between; the middle
  //    wave turned 22.5°) that shoot out and leave dashes at the rim.
  2: (f, c) => {
    let s = speck(f, 3, 15)
    if (f >= 13 && f <= 43) {
      const r = k([[13, 1], [21, 1.5], [29, 2.5], [31, 4.5], [35, 2.5], [39, 4.5], [41, 5.5]], f)
      const w = k([[13, 2], [21, 3], [29, 5], [31, 1.5], [35, 3], [39, 3], [41, 1]], f)
      s += ring(r, w, c, out(f, 41, 43))
    }
    for (const [start, turn] of [[15, 0], [25, 22.5], [35, 0]] as const) s += burst(f, start, turn, 8, (i) => (i % 2 ? c : W))
    return s
  },
  // 3: green dashes gather into a small star → pinwheel of curved arms bursts out → a ring of dashes at the rim,
  //    then the same in reverse in white: back into the pinwheel, the star, and dashes drifting out.
  3: (f, c) => {
    const h = f <= 41 ? f : 82 - f // mirrored time
    const col = f <= 41 ? c : W
    let s = ''
    if (h >= 5 && h <= 31) {
      // they spin ~25° clockwise, lengthen and thicken until they join into a small star, which shrinks to the pinwheel's hub
      const r = k([[7, 28.5], [11, 27.5], [15, 26.5], [19, 25], [21, 24], [23, 22.5], [25, 20], [27, 18], [29, 9], [33, 8]], h)
      const len = k([[7, 2], [11, 3.4], [19, 5.7], [25, 10.8], [27, 15], [29, 8], [33, 7]], h)
      const w = k([[7, 1.5], [11, 2], [19, 3.5], [25, STROKE]], h)
      const spin = k([[11, 0], [19, 6], [25, 23], [27, 28]], h)
      for (let i = 0; i < 8; i++) s += dash(i * 45 + spin, r, len, col, w, k([[5, 0], [9, 1]], h), 25)
    }
    if (h >= 28) {
      const r0 = k([[28, 10], [31, 10], [33, 18], [35, 63], [37, 71], [39, 76], [41, 79]], h)
      const r1 = k([[28, 30], [29, 52], [31, 66], [33, 72], [35, 78], [37, 81], [39, 83], [41, 85]], h)
      for (let i = 0; i < 8; i++) s += arm(i * 45, r0, r1, col)
    }
    // sprinkles at the rim that drift in and swell to small squares at the turnaround
    if (h >= 25) {
      const o = k([[25, 0], [31, 0.8]], h), r = k([[25, 91], [33, 90], [41, 81]], h), sz = k([[25, 2], [33, 2.5], [41, 4]], h)
      for (let i = 0; i < 16; i++) s += dash(11.25 + i * 22.5, r, sz, i % 2 ? c : W, sz, o)
    }
    return s
  }
}

/** Arm bend (measured): [radius, degrees clockwise from the arm's base angle]. Sweeps fast near the centre, then straightens. */
const BEND: [number, number][] = [
  [10, 0], [14, 6], [17, 13], [20, 22], [24, 35], [28, 44], [32, 48], [36, 52], [40, 55], [44, 57], [48, 59], [52, 60], [60, 60], [68, 58], [76, 56], [80, 55], [86, 54]
]
/** One pinwheel arm: the part of the curve between radius r0 and r1. */
function arm(deg: number, r0: number, r1: number, c: string) {
  if (r1 - r0 < 0.5) return ''
  const pts: string[] = []
  for (let j = 0; j <= 14; j++) {
    const r = r0 + ((r1 - r0) * j) / 14
    pts.push(at(deg + k(BEND, r), r))
  }
  return `<path d="M${pts.join('L')}" fill="none" stroke="${c}" stroke-width="${STROKE}" stroke-linejoin="round"/>`
}

const PEAK = { 1: 41, 2: 31, 3: 31 } as const

export function DigitalSparkle({ n, size = 96, loop = false, delay = 0, gap = 0, color, tilt = 0, className = '' }: {
  n: 1 | 2 | 3
  size?: number
  loop?: boolean
  delay?: number // seconds before it starts
  gap?: number // seconds between loops
  color?: string // accent (defaults: 1 orange, 2 cyan, 3 green); white parts stay white
  tilt?: number // degrees to lay it back (e.g. 60 for a marker on the ground)
  className?: string
}) {
  const still = !!useReducedMotion()
  const g = useRef<SVGGElement>(null)
  useEffect(() => {
    const el = g.current
    if (!el) return
    const c = color ?? COLORS[n]
    const period = FRAMES / FPS + gap
    let raf = 0
    const t0 = performance.now() + delay * 1000
    const tick = (now: number) => {
      const t = (now - t0) / 1000
      const done = !loop && t > FRAMES / FPS
      const f = t < 0 || done ? -1 : (loop ? t % period : t) * FPS
      el.innerHTML = f < 0 || f > FRAMES ? '' : SPRITES[n](still ? PEAK[n] : f, c)
      if (!done) raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [n, loop, delay, gap, color, still])
  return (
    <svg width={size} height={size} viewBox="-100 -100 200 200" aria-hidden className={`pointer-events-none overflow-visible ${className}`}
      style={tilt ? { transform: `perspective(${size * 4}px) rotateX(${tilt}deg)` } : undefined}>
      <g ref={g} strokeLinecap="butt" />
    </svg>
  )
}
