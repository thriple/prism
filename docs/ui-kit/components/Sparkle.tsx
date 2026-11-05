import { motion, useReducedMotion } from 'motion/react'
import { createContext, useContext, type CSSProperties, type ReactNode, type SVGProps } from 'react'

/**
 * Animated sparkles for accent decoration. Warm white with a glow in --kit-accent.
 *   <Sparkle n={1} />                       plays once on mount
 *   <Sparkle n={6} size={64} loop gap={1} delay={0.4} />
 * 1 long 4-point star · 2 soft diamond · 3 tilted star · 4 flare with flying tips
 * 5 plus with scattering dots · 6 burst with satellite stars.
 * Decorative (aria-hidden). Replay a one-shot by changing its `key`.
 */

const FRAMES = 35 // the reference runs 35 frames at ~33 fps
const DURATION = 1.05

type Key = { f: number } & Record<string, number | string>

/** Keyframes from sparse keys: missing values carry forward; `f` is the frame. Also returns the value at any frame. */
function tl(keys: Key[]) {
  const ks = [...keys]
  if (ks[0].f > 0) ks.unshift({ ...ks[0], f: 0 })
  if (ks[ks.length - 1].f < FRAMES) ks.push({ ...ks[ks.length - 1], f: FRAMES })
  const props = [...new Set(ks.flatMap((k) => Object.keys(k)).filter((p) => p !== 'f'))]
  const filled: Record<string, number | string>[] = []
  ks.forEach((k, i) => filled.push({ ...(i ? filled[i - 1] : {}), ...k }))
  const animate = Object.fromEntries(props.map((p) => [p, filled.map((f) => f[p] ?? filled.find((g) => g[p] !== undefined)![p])]))
  const at = (f: number) => {
    let i = 0
    while (i + 1 < ks.length && ks[i + 1].f <= f) i++
    return Object.fromEntries(props.map((p) => [p, animate[p][i]]))
  }
  return { animate, times: ks.map((k) => k.f / FRAMES), at }
}

// Shapes, centred on 0,0 in a -60…60 box.
const n2 = (v: number): string => (+v.toFixed(2)).toString()
/**
 * 4-point star: arm lengths `w`, `h`; `c` = waist (where the sides cross the diagonals: ~0 is a hairline cross);
 * `b` = side bulge (negative: concave, positive: puffy).
 */
function star(w: number, h: number, c: number, b = 0): string {
  const pts = [[0, -h], [c, -c], [w, 0], [c, c], [0, h], [-c, c], [-w, 0], [-c, -c]]
  let d = `M${n2(pts[0][0])} ${n2(pts[0][1])}`
  pts.forEach(([x0, y0], i) => {
    const [x1, y1] = pts[(i + 1) % 8]
    const dx = x1 - x0, dy = y1 - y0, len = Math.hypot(dx, dy) || 1
    d += `Q${n2((x0 + x1) / 2 + (dy / len) * b)} ${n2((y0 + y1) / 2 - (dx / len) * b)} ${n2(x1)} ${n2(y1)}`
  })
  return d + 'Z'
}
const ANG = [-84, -12, 42, 108, 196].map((d) => (d * Math.PI) / 180)
/** Tilted 5-point star with a length per point and an inner radius. */
const star5 = (len: number[], inner: number): string =>
  'M' + ANG.flatMap((a, i) => {
    const b = (a + (ANG[(i + 1) % 5] + (i === 4 ? 2 * Math.PI : 0))) / 2
    return [`${n2(len[i] * Math.cos(a))} ${n2(len[i] * Math.sin(a))}`, `${n2(inner * Math.cos(b))} ${n2(inner * Math.sin(b))}`]
  }).join('L') + 'Z'
const plus = (l: number): string => `M${n2(-l)} 0H${n2(l)}M0 ${n2(-l)}V${n2(l)}`
const dash = (a: number, b: number): string => `M${n2(a)} 0H${n2(Math.max(b, a + 0.01))}`
const NONE = star(0.01, 0.01, 0.01)

const Play = createContext({ loop: false, delay: 0, gap: 0, still: false, hold: 0 })

/** One animated SVG part following a timeline. */
function Part({ as = 'path', keys, ...rest }: { as?: 'path' | 'circle' | 'g'; keys: Key[] } & SVGProps<SVGElement>) {
  const { loop, delay, gap, still, hold } = useContext(Play)
  const t = tl(keys)
  const M = motion[as] as typeof motion.path
  const transition = { duration: DURATION, times: t.times, ease: 'linear' as const, delay, repeat: loop ? Infinity : 0, repeatDelay: gap }
  return still
    ? <M {...(rest as object)} initial={false} animate={t.at(hold)} />
    : <M {...(rest as object)} initial={t.at(0)} animate={t.animate} transition={transition} />
}

const SPARKLES: { hold: number; body: () => ReactNode }[] = [
  { // 1: pops from a dot into a long 4-point star, then thins to a hairline cross.
    hold: 10,
    body: () => <Part keys={[
      { f: 0, d: star(9, 9, 4, 1), opacity: 0 }, { f: 1, opacity: 1 }, { f: 2, d: star(3, 3, 2.2, 1.2) }, { f: 3, d: star(18, 18, 7, -1) },
      { f: 4, d: star(26, 27, 10, -2) }, { f: 5, d: star(33, 35, 12, -3) }, { f: 7, d: star(40, 42, 13, -4) }, { f: 15, d: star(40, 42, 13, -4) },
      { f: 16, d: star(36, 40, 6, -2) }, { f: 17, d: star(46, 48, 1), opacity: 0.9 }, { f: 18, d: star(44, 46, 0.5), opacity: 0.5 }, { f: 19, opacity: 0 }
    ]} />
  },
  { // 2: dot → round → big star, settles into a small diamond, puffs round, snaps to a hairline.
    hold: 12,
    body: () => <Part keys={[
      { f: 0, d: star(3, 3, 2.2, 1.2), opacity: 0 }, { f: 1, opacity: 1 }, { f: 2, d: star(22, 22, 15, 4) }, { f: 3, d: star(42, 42, 14, -3) },
      { f: 4, d: star(40, 40, 14, -3) }, { f: 6, d: star(36, 36, 13, -2.5) }, { f: 7, d: star(30, 30, 12, -1.5) }, { f: 8, d: star(22, 22, 10, -1) },
      { f: 15, d: star(23, 23, 10.5, -1) }, { f: 16, d: star(24, 24, 11, -1) }, { f: 20, d: star(23, 23, 10.5, -1) }, { f: 22, d: star(24, 24, 11, -0.5) },
      { f: 23, d: star(27, 27, 14, 1) }, { f: 24, d: star(32, 32, 22, 5) }, { f: 25, d: star(36, 36, 2, -0.5) },
      { f: 26, d: star(40, 40, 0.6), opacity: 0.6 }, { f: 27, opacity: 0 }
    ]} />
  },
  { // 3: a comet grows its points one by one into a tilted star, then thins and spins out as a streak.
    hold: 10,
    body: () => <Part keys={[
      { f: 0, d: star5([16, 16, 16, 16, 16], 1.5), rotate: 20, opacity: 0 }, { f: 1, opacity: 1 }, { f: 2, opacity: 0 },
      { f: 3, d: star5([6, 6, 26, 6, 6], 6), rotate: 0, opacity: 1 }, { f: 4, d: star5([30, 8, 44, 8, 8], 8) },
      { f: 5, d: star5([38, 26, 48, 10, 10], 10) }, { f: 6, d: star5([40, 30, 50, 38, 40], 12) }, { f: 7, d: star5([40, 30, 50, 38, 46], 12) },
      { f: 15, rotate: 0 }, { f: 18, d: star5([40, 30, 50, 38, 46], 12), rotate: -3 }, { f: 19, d: star5([38, 28, 48, 36, 44], 9), rotate: -8 },
      { f: 20, d: star5([34, 26, 44, 30, 40], 3.5), rotate: -22 }, { f: 21, d: star5([30, 22, 40, 26, 36], 1.5), rotate: -32 },
      { f: 22, d: star5([34, 6, 40, 6, 38], 1.2), rotate: -40 }, { f: 23, d: star5([4, 2, 30, 2, 4], 0.8), rotate: -50, opacity: 0.7 },
      { f: 24, opacity: 0 }
    ]} />
  },
  { // 4: flash, then a thin cross whose tips break off and fly outward while it turns and shrinks.
    hold: 9,
    body: () => (
      <Part as="g" keys={[{ f: 0, rotate: 0 }, { f: 7, rotate: 0 }, { f: 15, rotate: 4 }, { f: 16, rotate: 8 }, { f: 18, rotate: 16 }, { f: 20, rotate: 24 }, { f: 22, rotate: 30 }, { f: 27, rotate: 34 }]}>
        <Part keys={[
          { f: 0, d: star(16, 16, 6, -1), opacity: 0 }, { f: 1, opacity: 1 }, { f: 2, d: star(10, 10, 1.2) }, { f: 3, d: star(24, 24, 1.8) },
          { f: 4, d: star(34, 34, 2.4) }, { f: 5, d: star(32, 33, 2.2) }, { f: 15, d: star(32, 34, 2.2) }, { f: 16, d: star(30, 32, 2) },
          { f: 18, d: star(24, 26, 1.8) }, { f: 20, d: star(18, 19, 1.5) }, { f: 22, d: star(11, 12, 1.3) }, { f: 24, d: star(8, 8, 1.4) },
          { f: 26, d: star(6, 6, 1.2) }, { f: 27, d: star(3, 3, 0.8), opacity: 0.8 }, { f: 28, opacity: 0 }
        ]} />
        {[0, 90, 180, 270].map((deg) => (
          <g key={deg} transform={`rotate(${deg})`}>
            <Part keys={[
              { f: 0, d: star(1.5, 1.5, 1.1, 0.5), x: 26, opacity: 0 }, { f: 1, opacity: 1 }, { f: 2, d: star(3, 1.2, 0.8), x: 22 },
              { f: 3, d: star(5, 1.3, 0.9), x: 36 }, { f: 4, x: 44 }, { f: 7, x: 48 }, { f: 15, x: 50 }, { f: 16, x: 52 },
              { f: 18, x: 56, opacity: 0.9 }, { f: 20, x: 60, opacity: 0.7 }, { f: 22, x: 64, opacity: 0.4 }, { f: 23, x: 66, opacity: 0 }
            ]} />
          </g>
        ))}
      </Part>
    )
  },
  { // 5: a glowing cross bursts; its arms fly out as dashes that shrink to dots while a small plus regrows.
    hold: 11,
    body: () => (
      <g fill="none" stroke="currentColor" strokeLinecap="round">
        <Part as="circle" fill="currentColor" stroke="none" keys={[
          { f: 0, r: 0, opacity: 0 }, { f: 1, r: 3.5, opacity: 1 }, { f: 2, r: 7 }, { f: 3, r: 9 }, { f: 4, r: 9 }, { f: 5, r: 4 }, { f: 6, r: 2.5 }, { f: 8, r: 0 }
        ]} />
        <Part strokeWidth={3} keys={[
          { f: 0, d: plus(0), opacity: 0 }, { f: 2, d: plus(31), opacity: 1 }, { f: 3, d: plus(36) }, { f: 4, d: plus(40) }, { f: 5, d: plus(0) },
          { f: 6, d: plus(5) }, { f: 7, d: plus(8) }, { f: 8, d: plus(10) }, { f: 9, d: plus(11) }, { f: 11, d: plus(13) }, { f: 15, d: plus(14) },
          { f: 16, d: plus(13) }, { f: 17, d: plus(12) }, { f: 18, d: plus(11) }, { f: 19, d: plus(8) }, { f: 20, d: plus(4) },
          { f: 21, d: plus(0), opacity: 0.6 }, { f: 22, opacity: 0 }
        ]} />
        {[0, 90, 180, 270].map((deg) => (
          <g key={deg} transform={`rotate(${deg})`}>
            <Part strokeWidth={3} keys={[
              { f: 0, d: dash(28, 28), opacity: 0 }, { f: 4, opacity: 0 }, { f: 5, d: dash(28, 44), opacity: 1 }, { f: 6, d: dash(34, 44) },
              { f: 7, d: dash(40, 46) }, { f: 8, d: dash(41, 45) }, { f: 10, d: dash(43, 45) }, { f: 11, d: dash(44, 44) },
              { f: 16, opacity: 1 }, { f: 17, opacity: 0.7 }, { f: 19, opacity: 0.4 }, { f: 20, opacity: 0 }
            ]} />
          </g>
        ))}
      </g>
    )
  },
  { // 6: a long star with four satellite stars; diagonal rays grow in, satellites go out first, then it shrinks away.
    hold: 11,
    body: () => (
      <>
        <Part keys={[
          { f: 0, d: star(14, 14, 5, -1), opacity: 0 }, { f: 1, opacity: 1 }, { f: 2, d: star(30, 32, 3) }, { f: 3, d: star(38, 40, 3) },
          { f: 4, d: star(5, 5, 1.5) }, { f: 5, d: star(44, 46, 3) }, { f: 7, d: star(44, 46, 3) }, { f: 9, d: star(40, 42, 3.2) },
          { f: 13, d: star(38, 40, 3.2) }, { f: 15, d: star(34, 36, 3) }, { f: 16, d: star(28, 30, 2.6) }, { f: 17, d: star(24, 26, 2.2) },
          { f: 18, d: star(20, 22, 2) }, { f: 19, d: star(16, 18, 1.7) }, { f: 20, d: star(12, 13, 1.4), opacity: 0.7 }, { f: 21, opacity: 0 }
        ]} />
        <g transform="rotate(45)">
          <Part keys={[
            { f: 0, d: NONE }, { f: 7, d: NONE }, { f: 8, d: star(16, 16, 2) }, { f: 9, d: star(24, 24, 2.4) }, { f: 13, d: star(24, 24, 2.4) },
            { f: 15, d: star(20, 20, 2.2) }, { f: 16, d: star(15, 15, 2) }, { f: 17, d: star(10, 10, 1.6) }, { f: 18, d: star(5, 5, 1) }, { f: 19, d: NONE }
          ]} />
        </g>
        {[0, 90, 180, 270].map((deg) => (
          <g key={deg} transform={`rotate(${deg}) translate(48 0) rotate(${-deg})`}>
            <Part keys={[
              { f: 0, d: NONE }, { f: 4, d: NONE }, { f: 5, d: star(7, 7, 2) }, { f: 6, d: star(8, 8, 2.2) }, { f: 12, d: star(7, 7, 2) },
              { f: 14, d: star(4, 4, 1.4) }, { f: 15, d: star(1.5, 1.5, 0.8) }, { f: 16, d: NONE }
            ]} />
          </g>
        ))}
      </>
    )
  }
]

export const SPARKLE_COUNT = SPARKLES.length

export function Sparkle({ n, size = 48, loop = false, delay = 0, gap = 0, className = '', style }: {
  n: number // 1–6
  size?: number // px; the sparkle itself fills about two thirds of this box
  loop?: boolean
  delay?: number // seconds before it starts (stagger several)
  gap?: number // seconds between loops
  className?: string
  style?: CSSProperties // e.g. absolute positioning
}) {
  const still = !!useReducedMotion()
  const s = SPARKLES[n - 1]
  return (
    <Play.Provider value={{ loop, delay, gap, still, hold: s.hold }}>
      <svg
        aria-hidden
        width={size}
        height={size}
        viewBox="-60 -60 120 120"
        fill="#fff8ec"
        color="#fff8ec"
        className={`pointer-events-none overflow-visible ${className}`}
        style={{ filter: 'drop-shadow(0 0 2px var(--kit-accent)) drop-shadow(0 0 7px var(--kit-accent))', ...style }}
      >
        {s.body()}
      </svg>
    </Play.Provider>
  )
}
