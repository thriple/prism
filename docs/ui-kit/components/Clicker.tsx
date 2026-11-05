import { animate, motion, useMotionValue, useReducedMotion, useTransform, type MotionValue } from 'motion/react'
import { useEffect, useRef, useState } from 'react'
import { AnimatedIcon } from './AnimatedIcon'
import { useFx, type KitEffect } from './effects'

/**
 * Clicker: a big wheel whose centre sits off to the right. Labels and tick marks run round its rim, with an
 * animated icon per slot just inside. Each click turns it one slot; the slot at 9 o'clock is in focus and
 * its icon plays. It wraps round. Scroll, drag, click a slot, or use ↑ ↓ Home End.
 *   <Clicker label="Month" value={m} onChange={setM} items={[{ label: 'January', icon: 38 }, …]} />
 * `icon` is an AnimatedIcon number (1–50).
 */
export type ClickerItem = { label: string; icon: number }

const STEP = 14 // degrees between slots
const TICKS = 3 // tick marks between two labels
const SPRING = { type: 'spring', stiffness: 260, damping: 24 } as const
/** Offset in slots from the focus, wrapped into [-n/2, n/2). */
const wrap = (d: number, n: number) => ((((d + n / 2) % n) + n) % n) - n / 2

export function Clicker({ items, value, onChange, label, width = 320, height = 480, effect }: {
  items: ClickerItem[]
  value: number
  onChange: (i: number) => void
  label: string // accessible name
  width?: number
  height?: number
  effect?: KitEffect // with an effect, the focused label plays it on each click (the wheel still turns)
}) {
  const n = items.length
  const still = !!useReducedMotion()
  // `pos` is unbounded so the wheel always takes the short way round when it wraps.
  const target = useRef(value)
  const pos = useMotionValue(value)
  useEffect(() => {
    target.current += wrap(value - target.current, n)
    animate(pos, target.current, still ? { duration: 0 } : SPRING)
  }, [value]) // eslint-disable-line react-hooks/exhaustive-deps

  const g = { cx: width * 0.3 + height * 0.71, cy: height / 2, R: height * 0.71, inset: height * 0.12, unit: height * 0.085, n }
  const step = (by: number) => onChange((((value + by) % n) + n) % n)

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') step(1)
    else if (e.key === 'ArrowUp') step(-1)
    else if (e.key === 'Home') onChange(0)
    else if (e.key === 'End') onChange(n - 1)
    else return
    e.preventDefault()
  }

  // Wheel: one slot per notch. Drag: follow the pointer along the rim, then snap.
  const ref = useRef<HTMLDivElement>(null)
  const live = useRef({ value, step }); live.current = { value, step }
  useEffect(() => {
    const el = ref.current
    if (!el) return
    let at = 0
    const onWheel = (e: WheelEvent) => {
      e.preventDefault()
      const now = performance.now()
      if (now - at < 140 || Math.abs(e.deltaY) < 4) return
      at = now
      live.current.step(Math.sign(e.deltaY))
    }
    el.addEventListener('wheel', onWheel, { passive: false })
    return () => el.removeEventListener('wheel', onWheel)
  }, [])
  const drag = useRef<{ y0: number; p0: number; moved: boolean } | null>(null)
  const slotPx = (g.R * STEP * Math.PI) / 180
  const onPointerDown = (e: React.PointerEvent) => { drag.current = { y0: e.clientY, p0: pos.get(), moved: false } }
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current
    if (!d) return
    const dy = e.clientY - d.y0
    if (!d.moved && Math.abs(dy) > 4) { d.moved = true; ref.current!.setPointerCapture(e.pointerId) }
    if (d.moved) pos.set(d.p0 + dy / slotPx)
  }
  const onPointerUp = () => {
    const d = drag.current
    drag.current = null
    if (!d?.moved) return
    const by = Math.round(pos.get() - target.current)
    if (by) step(by)
    else animate(pos, target.current, SPRING)
  }

  return (
    <div
      ref={ref}
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={n - 1}
      aria-valuenow={value}
      aria-valuetext={items[value]?.label}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      className="relative touch-none select-none overflow-hidden rounded-[22px] outline-none focus-visible:ring-2 focus-visible:ring-[var(--kit-accent)]/70"
      style={{ width, height, maskImage: 'linear-gradient(transparent, #000 14%, #000 86%, transparent)' }}
    >
      {Array.from({ length: n * TICKS }, (_, k) => (
        <Tick key={k} at={Math.floor(k / TICKS) + ((k % TICKS) + 1) / (TICKS + 1)} pos={pos} g={g} />
      ))}
      {items.map((it, i) => (
        <Slot key={i} item={it} i={i} pos={pos} g={g} active={i === value} effect={effect} onSelect={() => { if (i !== value) onChange(i) }} />
      ))}
    </div>
  )
}

type Geo = { cx: number; cy: number; R: number; inset: number; unit: number; n: number }

/** x, y, rotate for a point `r` from the centre at slot offset `d` (0 = 9 o'clock, positive = above). */
function usePlace(at: number, pos: MotionValue<number>, g: Geo, r: number) {
  const d = useTransform(pos, (p) => wrap(at - p, g.n))
  const rad = (v: number) => (v * STEP * Math.PI) / 180
  return {
    d,
    x: useTransform(d, (v) => g.cx - r * Math.cos(rad(v))),
    y: useTransform(d, (v) => g.cy - r * Math.sin(rad(v))),
    rotate: useTransform(d, (v) => v * STEP),
    // fades out towards the ends of the visible arc
    fade: (v: number) => Math.max(0, 1 - Math.abs(v) / 4.5)
  }
}

function Tick({ at, pos, g }: { at: number; pos: MotionValue<number>; g: Geo }) {
  const p = usePlace(at, pos, g, g.R + g.unit * 0.2)
  const opacity = useTransform(p.d, (v) => 0.4 * p.fade(v))
  return (
    <motion.span className="absolute left-0 top-0 size-0" style={{ x: p.x, y: p.y, rotate: p.rotate, opacity }}>
      <span className="absolute -translate-x-1/2 -translate-y-1/2 rounded-full bg-white" style={{ width: g.unit * 0.42, height: 1.5 }} />
    </motion.span>
  )
}

function Slot({ item, i, pos, g, active, effect, onSelect }: {
  item: ClickerItem; i: number; pos: MotionValue<number>; g: Geo; active: boolean; effect?: KitEffect; onSelect: () => void
}) {
  const lab = usePlace(i, pos, g, g.R)
  const ico = usePlace(i, pos, g, g.R - g.inset)
  const near = (v: number) => Math.max(0, 1 - Math.abs(v)) // 1 in focus, 0 one slot away
  const labelOpacity = useTransform(lab.d, (v) => (0.4 + 0.6 * near(v)) * lab.fade(v))
  const iconOpacity = useTransform(ico.d, (v) => (0.45 + 0.55 * near(v)) * ico.fade(v))
  const scale = useTransform(ico.d, (v) => 0.85 + 0.3 * near(v))
  // Play the icon each time this slot comes into focus.
  const [plays, setPlays] = useState(0)
  useEffect(() => { if (active) setPlays((x) => x + 1) }, [active])
  const ref = useRef<HTMLSpanElement>(null)
  useFx(ref, plays, { effect, enabled: active })
  return (
    <>
      <motion.span className="absolute left-0 top-0 size-0" style={{ x: lab.x, y: lab.y, rotate: lab.rotate, opacity: labelOpacity }}>
        <span ref={ref} className="absolute -translate-x-1/2 -translate-y-1/2 whitespace-nowrap font-medium uppercase tracking-[0.06em] text-white" style={{ fontSize: g.unit * 0.3 }}>
          {item.label}
        </span>
      </motion.span>
      <motion.span className="absolute left-0 top-0 size-0" style={{ x: ico.x, y: ico.y, rotate: ico.rotate, opacity: iconOpacity, scale }}>
        <button type="button" tabIndex={-1} aria-hidden onClick={onSelect} className="absolute grid -translate-x-1/2 -translate-y-1/2 place-items-center text-white">
          <AnimatedIcon n={item.icon} size={g.unit} trigger={plays} strokeWidth={1.8} />
        </button>
      </motion.span>
    </>
  )
}
