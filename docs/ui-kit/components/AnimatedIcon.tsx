import { motion, useReducedMotion, type MotionStyle } from 'motion/react'
import { createContext, useContext, useEffect, useId, useRef, useState, type FC, type SVGProps } from 'react'

/**
 * Animated line icons (1–50), numbered in the order of the reference sheet. Stroke in currentColor.
 *   <button><AnimatedIcon n={6} /></button>   plays when the button (its parent) is hovered
 *   <AnimatedIcon n={38} loop gap={1} />        loops
 *   <AnimatedIcon n={10} trigger={count} />     plays whenever `trigger` changes
 * Static under prefers-reduced-motion. Decorative unless you pass `title`.
 */

const DUR = 1.2

// ---- geometry helpers (24×24 box) ----
const r2 = (v: number): number => +v.toFixed(2)
const pt = (cx: number, cy: number, r: number, deg: number): [number, number] =>
  [r2(cx + r * Math.cos((deg * Math.PI) / 180)), r2(cy + r * Math.sin((deg * Math.PI) / 180))]
/** Clockwise arc from a0 to a1 (degrees, 0 = right, 90 = down). */
const arc = (cx: number, cy: number, r: number, a0: number, a1: number): string => {
  const [x0, y0] = pt(cx, cy, r, a0), [x1, y1] = pt(cx, cy, r, a1)
  return `M${x0} ${y0}A${r} ${r} 0 ${a1 - a0 > 180 ? 1 : 0} 1 ${x1} ${y1}`
}
/** Arrowhead with its tip at x,y pointing along `dir`. */
const head = (x: number, y: number, dir: number, len = 3.2): string => {
  const [ax, ay] = pt(x, y, len, dir + 180 - 42), [bx, by] = pt(x, y, len, dir + 180 + 42)
  return `M${ax} ${ay}L${x} ${y}L${bx} ${by}`
}
/** Clockwise arc with an arrowhead at its end. */
const arcArrow = (cx: number, cy: number, r: number, a0: number, a1: number): string =>
  arc(cx, cy, r, a0, a1) + head(...pt(cx, cy, r, a1), a1 + 90)
/** Point on the 9×4.5 orbit ellipse (icon 1). */
const ept = (deg: number): [number, number] => [r2(12 + 9 * Math.cos((deg * Math.PI) / 180)), r2(12 + 4.5 * Math.sin((deg * Math.PI) / 180))]
const ast = (x: number, y: number, s = 2.6): string => {
  const [dx, dy] = [r2(s * 0.866), r2(s * 0.5)]
  return `M${x} ${y - s}V${y + s}M${x - dx} ${y - dy}L${x + dx} ${y + dy}M${x - dx} ${y + dy}L${x + dx} ${y - dy}`
}
const star5 = (cx: number, cy: number, R: number, r: number): string =>
  'M' + Array.from({ length: 10 }, (_, i) => pt(cx, cy, i % 2 ? r : R, -90 + i * 36).join(' ')).join('L') + 'Z'
const gear = (cx: number, cy: number, R: number, r: number, n: number): string => {
  const step = 360 / n, t = step * 0.22
  return 'M' + Array.from({ length: n }, (_, i) => {
    const a = -90 + i * step
    return [pt(cx, cy, r, a - step / 2 + t * 0.3), pt(cx, cy, R, a - t), pt(cx, cy, R, a + t), pt(cx, cy, r, a + step / 2 - t * 0.3)].map((p) => p.join(' ')).join('L')
  }).join('L') + 'Z'
}

// Shared shapes
const PIN = 'M12 21.5s-7-6.2-7-11.5a7 7 0 0 1 14 0c0 5.3-7 11.5-7 11.5z'
const SHIELD = 'M12 3l7 2.8v5.4c0 4.6-3 8-7 9.8-4-1.8-7-5.2-7-9.8V5.8z'
const CORNERS = 'M3 8V6a3 3 0 0 1 3-3h2M16 3h2a3 3 0 0 1 3 3v2M21 16v2a3 3 0 0 1-3 3h-2M8 21H6a3 3 0 0 1-3-3v-2'
const REPEAT_A = 'M4 11V9a4 4 0 0 1 4-4h12M17 2l3 3-3 3'
const REPEAT_B = 'M20 13v2a4 4 0 0 1-4 4H4M7 22l-3-3 3-3'
const PAUSE = 'M8.5 4.5h0a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2h0a2 2 0 0 1-2-2v-11a2 2 0 0 1 2-2zM15.5 4.5h0a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2h0a2 2 0 0 1-2-2v-11a2 2 0 0 1 2-2z'
const PLAY = 'M7.5 5.7v12.6a1.2 1.2 0 0 0 1.8 1l10-6.3a1.2 1.2 0 0 0 0-2l-10-6.3a1.2 1.2 0 0 0-1.8 1z'

// ---- animation engine ----
const Play = createContext({ run: 0, loop: false, delay: 0, gap: 0, still: true })
type KF = Record<string, (number | string)[]>

/** Motion drives transform-origin itself, so origins go in as originX/originY: "x y" px in the 24 box (view-box) or "x% y%" of the part (fill-box). */
const origin = (o: string) => {
  const fill = o.includes('%'), [x, y] = o.split(' ').map(parseFloat)
  return { transformBox: fill ? 'fill-box' : 'view-box', originX: fill ? x / 100 : x / 24, originY: fill ? y / 100 : y / 24 }
}
/** One SVG part. `a` = keyframes (first and last are the resting pose), `t` = times, `o` = transform origin ("12px 12px" in the 24 box, or "50% 100%" of the part itself). */
function A({ as = 'path', a, t, o, style, ...rest }: { as?: 'path' | 'g' | 'circle' | 'rect'; a?: KF; t?: number[]; o?: string } & SVGProps<SVGElement>) {
  const { run, loop, delay, gap, still } = useContext(Play)
  const M = motion[as] as typeof motion.path
  const s = { ...(o && origin(o)), ...style } as MotionStyle
  if (!a) return <M {...(rest as object)} style={s} />
  const first = Object.fromEntries(Object.entries(a).map(([k, v]) => [k, v[0]]))
  if (still || (!run && !loop)) return <M {...(rest as object)} style={s} initial={false} animate={first} />
  return (
    <M key={run} {...(rest as object)} style={s} initial={first} animate={a}
      transition={{ duration: DUR, times: t, ease: 'easeInOut', delay, repeat: loop ? Infinity : 0, repeatDelay: gap }} />
  )
}
const P = (p: SVGProps<SVGPathElement>) => <path {...p} />

// Reused animated pieces
const Pin: FC<{ children?: React.ReactNode }> = ({ children }) => <><P d={PIN} />{children}</>
/** Pin that lies down flat and springs back up. */
const LayDown: FC<{ children: React.ReactNode }> = ({ children }) => (
  <A as="g" o="12px 21px" t={[0, 0.3, 0.55, 0.75, 0.88, 1]} a={{ scaleY: [1, 0.35, 0.35, 1.12, 0.96, 1], y: [0, 3, 3, -1, 0, 0] }}>{children}</A>
)
/** Shield that shrinks, pops big and settles; the glyph leaves before and returns after. */
const ShieldPop: FC<{ glyph?: React.ReactNode; glyphA?: KF; glyphT?: number[] }> = ({ glyph, glyphA, glyphT }) => (
  <>
    <A d={SHIELD} o="12px 12px" t={[0, 0.3, 0.55, 0.75, 1]} a={{ scale: [1, 0.78, 1.18, 1, 1] }} />
    {glyph && (
      <A as="g" o="12px 11.5px" t={glyphT ?? [0, 0.2, 0.7, 0.9, 1]} a={glyphA ?? { scale: [1, 0, 0, 1.2, 1], rotate: [0, 180, 180, 360, 360] }}>{glyph}</A>
    )}
  </>
)
const Brush: FC = () => (
  <g transform="rotate(35 12 12)">
    <P d="M10.5 12.5V3.5a1.5 1.5 0 0 1 3 0v9" />
    <P d="M9.5 12.5h5v1.5c0 3-1.2 5.2-2.5 7.5-1.3-2.3-2.5-4.5-2.5-7.5z" />
  </g>
)
/** The brush swings upright about its tip and back. */
const BrushSwing: FC = () => (
  <A as="g" o="6.6px 19.8px" t={[0, 0.3, 0.55, 0.75, 1]} a={{ rotate: [0, -35, -35, 6, 0] }}><Brush /></A>
)
const Dropper: FC<{ fill?: boolean }> = ({ fill }) => (
  <g transform="rotate(45 12 12)">
    <P d="M10 7V4a2 2 0 0 1 4 0v3" />
    <P d="M8 7.5h8" />
    <P d="M9.5 7.5V16l2.5 4.5 2.5-4.5V7.5" />
    {fill && (
      <A d="M9.5 11.5h5V16l-2.5 4.5L9.5 16z" fill="currentColor" o="50% 100%" t={[0, 0.2, 0.5, 0.75, 1]} a={{ scaleY: [0, 0, 1, 1, 0], opacity: [0, 1, 1, 1, 0] }} />
    )}
  </g>
)
const Sliders: FC<{ rows: { y: number; x: number[] }[]; r?: number }> = ({ rows, r = 2 }) => (
  <>
    {rows.map(({ y, x }) => (
      <g key={y}>
        <A d={`M4 ${y}H${x[0] - r}`} a={{ d: x.map((k) => `M4 ${y}H${k - r}`) }} />
        <A d={`M${x[0] + r} ${y}H20`} a={{ d: x.map((k) => `M${k + r} ${y}H20`) }} />
        <A as="circle" cx={x[0]} cy={y} r={r} a={{ cx: x }} />
      </g>
    ))}
  </>
)
/** Waves (outer first) fade out, then dot, inner, outer fade back. */
const Waves: FC<{ parts: string[]; dot: [number, number] }> = ({ parts, dot }) => (
  <>
    <A as="circle" cx={dot[0]} cy={dot[1]} r={1} fill="currentColor" t={[0, 0.3, 0.45, 0.55, 1]} a={{ opacity: [1, 1, 0, 1, 1] }} />
    {parts.map((d, i) => (
      <A key={i} d={d} t={[0, 0.25 - i * 0.1, 0.3 - i * 0.1, 0.6 + i * 0.12, 0.7 + i * 0.12, 1]} a={{ opacity: [1, 1, 0, 0, 1, 1] }} />
    ))}
  </>
)

// ---- the icons, in reference-sheet order ----
const ICONS: FC[] = [
  // 1 orbit: the elliptical arrow turns a full circle
  () => (
    <A as="g" o="12px 12px" a={{ rotate: [0, -360], scaleY: [1, 1.4, 1] }}>
      <P d={`M${ept(-30).join(' ')}A9 4.5 0 1 0 ${ept(20).join(' ')}`} />
      <P d={head(...ept(20), -55)} />
    </A>
  ),
  // 2 scan: the line sweeps up, down, and back
  () => <><P d={CORNERS} /><A d="M2 12h20" a={{ y: [0, -7, 7, 0] }} /></>,
  // 3 pin-heart: the heart beats
  () => (
    <Pin>
      <A d="M12 13s-3-1.8-3-3.8a1.6 1.6 0 0 1 3-.8 1.6 1.6 0 0 1 3 .8c0 2-3 3.8-3 3.8z" o="12px 10.5px" t={[0, 0.25, 0.55, 0.8, 1]} a={{ scale: [1, 1.35, 0.65, 1.1, 1] }} />
    </Pin>
  ),
  // 4 image: the hills ripple like a wave and the sun rides it
  () => (
    <>
      <rect x={3} y={3} width={18} height={18} rx={5} />
      <A as="circle" cx={8.5} cy={8.5} r={1.5} a={{ cy: [8.5, 10, 7.5, 8.5], cx: [8.5, 10, 12, 8.5] }} />
      <A d="M3.5 16.5L8 12L12 15.5L15.5 11.5L20.5 16" a={{ d: ['M3.5 16.5L8 12L12 15.5L15.5 11.5L20.5 16', 'M3.5 14.5L7 16L11.5 12L15.5 15.5L20.5 12.5', 'M3.5 16L8 13.5L12 15L15.5 12L20.5 15.5', 'M3.5 16.5L8 12L12 15.5L15.5 11.5L20.5 16'] }} />
    </>
  ),
  // 5 brush + plus: the plus twinkles out, the brush stands up, the plus returns
  () => (
    <>
      <BrushSwing />
      <A d="M5 3v4M3 5h4" o="5px 5px" t={[0, 0.2, 0.7, 0.85, 1]} a={{ scale: [1, 0, 0, 1.3, 1], rotate: [0, 90, 90, 180, 180] }} />
    </>
  ),
  // 6 refresh: spins a full turn, swelling
  () => <A as="g" o="12px 12px" a={{ rotate: [0, 360], scale: [1, 1.15, 1] }}><P d={arcArrow(12, 12, 8, -60, 240)} /></A>,
  // 7 qr-scan: a bright bar scans down
  () => (
    <>
      <P d={CORNERS} />
      <P d="M7.5 7.5h3v3h-3zM13.5 7.5h3v3h-3zM7.5 13.5h3v3h-3zM14 14h.01M16.5 16.5h.01M14 16.5h.01M16.5 14h.01" />
      <A d="M3 12h18" strokeWidth={3} t={[0, 0.15, 0.4, 0.75, 1]} a={{ y: [-8, -8, 8, 0, 0], opacity: [0, 1, 1, 1, 0] }} />
    </>
  ),
  // 8 pin-alert: the mark swings like a pendulum
  () => (
    <Pin>
      <A as="g" o="12px 13px" t={[0, 0.25, 0.5, 0.75, 1]} a={{ rotate: [0, 40, -20, 8, 0] }}><P d="M12 6.5V10M12 13h.01" /></A>
    </Pin>
  ),
  // 9 eyedropper: turns upright, lets a drop fall, tips back
  () => (
    <>
      <A as="g" o="4.6px 19.4px" t={[0, 0.3, 0.6, 0.85, 1]} a={{ rotate: [0, -40, -40, 6, 0] }}><Dropper /></A>
      <A as="circle" cx={4.6} cy={21} r={0.6} fill="currentColor" t={[0, 0.4, 0.6, 1]} a={{ opacity: [0, 1, 0, 0], y: [0, 0, 2.5, 2.5] }} />
    </>
  ),
  // 10 pause → play → pause, flipping edge-on in between
  () => (
    <>
      <A d={PAUSE} o="12px 12px" t={[0, 0.12, 0.8, 0.92, 1]} a={{ scaleX: [1, 0, 0, 1, 1], opacity: [1, 1, 0, 1, 1] }} />
      <A d={PLAY} o="12px 12px" t={[0, 0.12, 0.24, 0.7, 0.8, 1]} a={{ scaleX: [0, 0, 1, 1, 0, 0], opacity: [0, 1, 1, 1, 0, 0] }} />
    </>
  ),
  // 11 undo: the tail retracts into the arrow, which nudges left, then redraws
  () => (
    <>
      <A d="M5 9h10a4.5 4.5 0 0 1 0 9h-7" t={[0, 0.35, 0.55, 0.9, 1]} a={{ pathLength: [1, 0.05, 0.05, 1, 1] }} />
      <A d="M9 5L5 9l4 4" t={[0, 0.35, 0.55, 1]} a={{ x: [0, -1.5, 0, 0] }} />
    </>
  ),
  // 12 play → pause → play
  () => (
    <>
      <A d={PLAY} o="12px 12px" t={[0, 0.12, 0.8, 0.92, 1]} a={{ scaleX: [1, 0, 0, 1, 1], opacity: [1, 1, 0, 1, 1] }} />
      <A d={PAUSE} o="12px 12px" t={[0, 0.12, 0.24, 0.7, 0.8, 1]} a={{ scaleX: [0, 0, 1, 1, 0, 0], opacity: [0, 1, 1, 1, 0, 0] }} />
    </>
  ),
  // 13 pin-off: the slash retracts, the pin lies down and stands up, the slash redraws
  () => (
    <>
      <LayDown><Pin><circle cx={12} cy={10} r={2.5} /></Pin></LayDown>
      <A d="M3 3l18 18" t={[0, 0.15, 0.85, 1]} a={{ pathLength: [1, 0, 0, 1] }} />
    </>
  ),
  // 14 eyedropper: fills with liquid, then drains
  () => <Dropper fill />,
  // 15 password: the underlines wipe away and rewrite one by one
  () => (
    <>
      {[5, 12, 19].map((x, i) => (
        <g key={x}>
          <A d={ast(x, 10)} o={`${x}px 10px`} t={[0, 0.2 + i * 0.12, 0.4 + i * 0.12, 1]} a={{ rotate: [0, 0, 60, 60] }} />
          <A d={`M${x - 2.5} 17h5`} t={[0, 0.1 + i * 0.08, 0.5 + i * 0.12, 0.65 + i * 0.12, 1]} a={{ pathLength: [1, 0, 0, 1, 1] }} />
        </g>
      ))}
    </>
  ),
  // 16 biohazard: shrinks while turning, then bursts back
  () => (
    <A as="g" o="12px 12px" t={[0, 0.45, 0.75, 1]} a={{ rotate: [0, 100, 120, 120], scale: [1, 0.7, 1.12, 1] }}>
      {[-90, 30, 150].map((a) => { const [cx, cy] = pt(12, 12, 5, a); return <P key={a} d={arc(cx, cy, 3.8, a + 50, a + 310)} /> })}
      <circle cx={12} cy={12} r={1.5} />
    </A>
  ),
  // 17 pin-star: the star spins up big, dips small, settles
  () => (
    <Pin>
      <A d={star5(12, 10, 3.2, 1.5)} o="12px 10px" t={[0, 0.35, 0.65, 1]} a={{ rotate: [0, 144, 216, 216], scale: [1, 1.35, 0.7, 1] }} />
    </Pin>
  ),
  // 18 pin: lies down flat and springs back up
  () => <LayDown><Pin><circle cx={12} cy={10} r={2.5} /></Pin></LayDown>,
  // 19 brush + trail: the painted trail retracts, the brush swings, the trail repaints
  () => (
    <>
      <BrushSwing />
      <A d="M7.5 2.5c-2.8.6-3.4 2.6-1.7 4.1 1.8 1.6 1 3.7-1.6 4.6" t={[0, 0.25, 0.7, 1]} a={{ pathLength: [1, 0, 0, 1] }} />
      <A as="circle" cx={3} cy={14} r={0.5} t={[0, 0.2, 0.9, 1]} a={{ opacity: [1, 0, 0, 1] }} />
    </>
  ),
  // 20 password field: the dots clear left to right, then type back in
  () => (
    <>
      <rect x={2} y={7} width={20} height={10} rx={5} />
      {[7, 12, 17].map((x, i) => (
        <A key={x} d={ast(x, 12, 1.8)} o={`${x}px 12px`} t={[0, 0.08 + i * 0.08, 0.5 + i * 0.12, 0.6 + i * 0.12, 0.7 + i * 0.12, 1]} a={{ scale: [1, 0, 0, 1.4, 1, 1] }} />
      ))}
    </>
  ),
  // 21 radiation: the blades fly apart while turning, then close in
  () => (
    <A as="g" o="12px 12px" a={{ rotate: [0, 120] }}>
      {[-150, -30, 90].map((a) => {
        const [x, y] = pt(0, 0, 3, a)
        const d = `M${pt(12, 12, 3.5, a - 30).join(' ')}L${pt(12, 12, 9, a - 30).join(' ')}A9 9 0 0 1 ${pt(12, 12, 9, a + 30).join(' ')}L${pt(12, 12, 3.5, a + 30).join(' ')}A3.5 3.5 0 0 0 ${pt(12, 12, 3.5, a - 30).join(' ')}z`
        return <A key={a} d={d} a={{ x: [0, x, 0], y: [0, y, 0] }} />
      })}
    </A>
  ),
  // 22 pin-question: the question mark hops and wobbles
  () => (
    <Pin>
      <A as="g" o="12px 13px" t={[0, 0.3, 0.55, 0.8, 1]} a={{ y: [0, -4, 0, -1, 0], rotate: [0, -20, 15, -5, 0] }}>
        <P d="M10 8.2a2.1 2.1 0 1 1 3 1.9c-.7.4-1 .9-1 1.6M12 13.8h.01" />
      </A>
    </Pin>
  ),
  // 23 landscape: hills and moon sink out of the frame and rise back
  function Landscape() {
    const id = useId()
    return (
      <>
        <clipPath id={id}><rect x={3} y={3} width={18} height={18} rx={5} /></clipPath>
        <rect x={3} y={3} width={18} height={18} rx={5} />
        <g clipPath={`url(#${id})`}>
          <A d="M3 16c3-4 8-4 11.5 1M9 21c2-4.5 7.5-6 12-4.5" t={[0, 0.35, 0.55, 0.9, 1]} a={{ y: [0, 8, 8, 0, 0] }} />
          <A d="M16.5 6a2.2 2.2 0 1 0 1.8 3" t={[0, 0.3, 0.6, 0.85, 1]} a={{ y: [0, 12, 12, 0, 0] }} />
        </g>
      </>
    )
  },
  // 24 brush + dash: the dash shrinks to a dot, the brush swings, the dash stretches back
  () => (
    <>
      <BrushSwing />
      <A d="M3 6h3" t={[0, 0.2, 0.75, 0.9, 1]} a={{ pathLength: [1, 0, 0, 1, 1] }} />
    </>
  ),
  // 25 badge: the star leaps out of the card with a spin and drops back
  () => (
    <>
      <A as="rect" x={5} y={3} width={14} height={18} rx={3} o="12px 21px" t={[0, 0.3, 0.6, 0.8, 1]} a={{ scaleY: [1, 1, 1.08, 0.96, 1] }} />
      <P d="M9 16.5h6" />
      <A d={star5(12, 10, 3.2, 1.5)} o="12px 10px" t={[0, 0.35, 0.6, 0.8, 1]} a={{ y: [0, -7, 0, -1, 0], rotate: [0, 72, 144, 144, 144] }} />
    </>
  ),
  // 26–27 shields
  () => <ShieldPop glyph={<P d="M9 11.5h6" />} />,
  () => <ShieldPop />,
  // 28 gear: shrinks while turning, then pops back
  () => (
    <A as="g" o="12px 12px" t={[0, 0.5, 0.75, 1]} a={{ rotate: [0, 100, 120, 120], scale: [1, 0.65, 1.15, 1] }}>
      <P d={gear(12, 12, 9.5, 7, 6)} /><circle cx={12} cy={12} r={2.8} />
    </A>
  ),
  // 29 rss: waves fade out and back in from the dot
  () => (
    <>
      <rect x={3} y={3} width={18} height={18} rx={5} />
      <Waves dot={[8, 16]} parts={[arc(8, 16, 4, -90, 0), arc(8, 16, 8, -90, 0)]} />
    </>
  ),
  // 30 circle-minus: a quick squeeze
  () => (
    <>
      <A as="circle" cx={12} cy={12} r={9} o="12px 12px" t={[0, 0.3, 0.6, 1]} a={{ scale: [1, 0.85, 1.06, 1] }} />
      <A d="M8 12h8" o="12px 12px" t={[0, 0.3, 0.6, 1]} a={{ scaleX: [1, 1.25, 0.9, 1] }} />
    </>
  ),
  // 31 shield-x
  () => <ShieldPop glyph={<P d="M9.8 9.3l4.4 4.4M14.2 9.3l-4.4 4.4" />} />,
  // 32 share: links retract, nodes shrink, then everything reconnects
  () => (
    <>
      {[[18, 5], [6, 12], [18, 19]].map(([x, y]) => (
        <A key={y} as="circle" cx={x} cy={y} r={2.5} o={`${x}px ${y}px`} t={[0, 0.3, 0.6, 0.8, 1]} a={{ scale: [1, 0.4, 0.4, 1.15, 1] }} />
      ))}
      <A d="M8.2 10.7l7.6-4.4M8.2 13.3l7.6 4.4" t={[0, 0.2, 0.6, 0.85, 1]} a={{ pathLength: [1, 0, 0, 1, 1] }} />
    </>
  ),
  // 33 database: squashes down, then stretches up
  () => (
    <A as="g" o="12px 21px" t={[0, 0.4, 0.7, 0.85, 1]} a={{ scaleY: [1, 0.75, 1.12, 0.97, 1], scaleX: [1, 1.08, 0.95, 1, 1] }}>
      <ellipse cx={12} cy={5.5} rx={7} ry={2.5} />
      <P d="M5 5.5v13c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5v-13M5 10c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5M5 14.3c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5" />
    </A>
  ),
  // 34 cast: waves fade out and back in
  () => (
    <>
      <P d="M3 8V7a3 3 0 0 1 3-3h12a3 3 0 0 1 3 3v10a3 3 0 0 1-3 3h-5" />
      <Waves dot={[3.5, 20.5]} parts={[arc(3.5, 20.5, 4, -90, 0), arc(3.5, 20.5, 8, -90, 0)]} />
    </>
  ),
  // 35 repeat: the arrows shrink as they spin half a turn, then regrow
  () => (
    <A as="g" o="12px 12px" a={{ rotate: [0, 180] }}>
      <A d={REPEAT_A} a={{ pathLength: [1, 0.3, 1] }} /><A d={REPEAT_B} a={{ pathLength: [1, 0.3, 1] }} />
    </A>
  ),
  // 36 shield-check: the check undraws and redraws after the shield pops
  () => <ShieldPop glyph={<P d="M9 11.8l2 2 4-4.3" />} glyphA={{ pathLength: [1, 0, 0, 1, 1] }} />,
  // 37 settings: the gear turns inside its frame
  () => (
    <>
      <rect x={3} y={3} width={18} height={18} rx={6} />
      <A as="g" o="12px 12px" a={{ rotate: [0, 120] }}><P d={gear(12, 12, 5.5, 4, 6)} /><circle cx={12} cy={12} r={1.5} /></A>
    </>
  ),
  // 38 send: flies off to the top right and comes back in from the bottom left
  () => (
    <A as="g" o="12px 12px" t={[0, 0.4, 0.41, 0.8, 1]} a={{ x: [0, 16, -16, 0, 0], y: [0, -16, 16, 0, 0], scale: [1, 0.6, 0.6, 1, 1], opacity: [1, 0, 0, 1, 1] }}>
      <P d="M21 3L3 10.5l7.5 3 3 7.5z" /><P d="M10.5 13.5L21 3" />
    </A>
  ),
  // 39 refresh (two arrows): half a turn with a squeeze
  () => (
    <A as="g" o="12px 12px" a={{ rotate: [0, 180], scale: [1, 0.8, 1] }}>
      <P d={arcArrow(12, 12, 8, 200, 330)} /><P d={arcArrow(12, 12, 8, 20, 150)} />
    </A>
  ),
  // 40 repeat: the two arrows split up and down, fade, and slide back together
  () => (
    <>
      <A d={REPEAT_A} t={[0, 0.35, 0.55, 0.85, 1]} a={{ y: [0, -5, -5, 0, 0], opacity: [1, 0, 0, 1, 1] }} />
      <A d={REPEAT_B} t={[0, 0.35, 0.55, 0.85, 1]} a={{ y: [0, 5, 5, 0, 0], opacity: [1, 0, 0, 1, 1] }} />
    </>
  ),
  // 41 shield-alert: the mark jumps out of the top and drops back in
  () => (
    <ShieldPop glyph={<P d="M12 8v4M12 15h.01" />} glyphT={[0, 0.35, 0.6, 0.8, 1]}
      glyphA={{ y: [0, -7, -4, 0, 0], rotate: [0, 20, 10, 0, 0] }} />
  ),
  // 42 sliders: knobs slide to new spots and back
  () => <Sliders rows={[{ y: 6, x: [15, 8, 17, 15] }, { y: 12, x: [9, 16, 7, 9] }, { y: 18, x: [13, 7, 16, 13] }]} />,
  // 43 scissors: two quick snips
  () => (
    <>
      <A as="g" o="12px 12px" a={{ rotate: [0, -14, 4, -14, 0] }}><circle cx={6} cy={6} r={3} /><P d="M8.1 8.1L20 20" /></A>
      <A as="g" o="12px 12px" a={{ rotate: [0, 14, -4, 14, 0] }}><circle cx={6} cy={18} r={3} /><P d="M8.1 15.9L20 4" /></A>
    </>
  ),
  // 44 refresh (side arrows): quarter-turns in, then back out
  () => (
    <A as="g" o="12px 12px" t={[0, 0.45, 0.8, 1]} a={{ rotate: [0, 90, 180, 180], scale: [1, 0.7, 1.08, 1] }}>
      <P d={arcArrow(12, 12, 8, 110, 245)} /><P d={arcArrow(12, 12, 8, 290, 425)} />
    </A>
  ),
  // 45 repeat (round): arcs shrink while spinning, then grow back
  () => (
    <A as="g" o="12px 12px" a={{ rotate: [0, 180], scale: [1, 0.5, 1] }}>
      <P d={arcArrow(12, 12, 8, 150, 330)} /><P d={arcArrow(12, 12, 8, -30, 150)} />
    </A>
  ),
  // 46 shield-plus
  () => <ShieldPop glyph={<P d="M12 8.5v6M9 11.5h6" />} />,
  // 47 sliders (end knobs): knobs swap ends
  () => <Sliders r={2.2} rows={[{ y: 6, x: [17, 7, 17, 17] }, { y: 12, x: [7, 17, 7, 7] }, { y: 18, x: [17, 7, 17, 17] }]} />,
  // 48 save: flips over twice
  () => (
    <A as="g" o="12px 12px" a={{ scaleX: [1, -1, 1, -1, 1], scale: [1, 0.85, 0.85, 0.85, 1] }}>
      <P d="M6 3h10l5 5v10a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3V6a3 3 0 0 1 3-3z" /><P d="M8 3v4h6V3M7.5 21v-5a1.5 1.5 0 0 1 1.5-1.5h6a1.5 1.5 0 0 1 1.5 1.5v5" />
    </A>
  ),
  // 49 map: the route and the X fade out, then plot back in
  () => (
    <>
      <P d="M3 6c3-1.5 6 1 9 0s6-1.5 9 0v12c-3-1.5-6 1-9 0s-6-1.5-9 0z" />
      {[[6.5, 9.5], [8.5, 11.5], [10.5, 10], [12.5, 11.5]].map(([x, y], i) => (
        <A key={i} d={`M${x} ${y}h.01`} t={[0, 0.1 + i * 0.05, 0.5 + i * 0.08, 1]} a={{ opacity: [1, 0, 1, 1] }} />
      ))}
      <A d="M14.5 13l2.5 2.5M17 13l-2.5 2.5" o="15.75px 14.25px" t={[0, 0.3, 0.8, 0.9, 1]} a={{ scale: [1, 0, 0, 1.3, 1] }} />
    </>
  ),
  // 50 repeat-once: the arrows spin half a turn around the 1, which pulses
  () => (
    <>
      <A as="g" o="12px 12px" a={{ rotate: [0, 180] }}>
        <A d={REPEAT_A} a={{ pathLength: [1, 0.3, 1] }} /><A d={REPEAT_B} a={{ pathLength: [1, 0.3, 1] }} />
      </A>
      <A d="M11 10.5l1.5-1v5" o="12px 12px" t={[0, 0.5, 0.75, 1]} a={{ scale: [1, 0.6, 1.2, 1] }} />
    </>
  )
]

export const ANIMATED_ICON_COUNT = ICONS.length

export function AnimatedIcon({ n, size = 24, loop = false, delay = 0, gap = 0, trigger, hover = true, strokeWidth = 2, title, className = '' }: {
  n: number // 1–50
  size?: number
  loop?: boolean
  delay?: number // seconds before it starts
  gap?: number // seconds between loops
  trigger?: unknown // plays each time this changes
  hover?: boolean // play when the parent element is hovered (default true)
  strokeWidth?: number
  title?: string
  className?: string
}) {
  const still = !!useReducedMotion()
  const [run, setRun] = useState(0)
  const ref = useRef<SVGSVGElement>(null)
  const first = useRef(true)
  useEffect(() => {
    if (first.current) { first.current = false; return }
    setRun((r) => r + 1)
  }, [trigger])
  useEffect(() => {
    const host = ref.current?.parentElement
    if (!hover || !host) return
    const play = () => setRun((r) => r + 1)
    host.addEventListener('pointerenter', play)
    return () => host.removeEventListener('pointerenter', play)
  }, [hover])
  const Body = ICONS[n - 1]
  return (
    <Play.Provider value={{ run, loop, delay, gap, still }}>
      <svg ref={ref} width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={strokeWidth}
        strokeLinecap="round" strokeLinejoin="round" className={`overflow-visible ${className}`}
        role={title ? 'img' : undefined} aria-hidden={title ? undefined : true}>
        {title && <title>{title}</title>}
        <Body />
      </svg>
    </Play.Provider>
  )
}
