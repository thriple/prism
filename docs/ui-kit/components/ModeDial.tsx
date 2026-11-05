import { animate, motion, useMotionValue, useReducedMotion, useTransform, type MotionValue } from 'motion/react'
import { useEffect, useRef, type ReactNode } from 'react'
import { useFx, type KitEffect } from './effects'

/**
 * Camera-style mode dial: a fixed arc of ticks with an index mark at the top, and a ring of modes that
 * turns so the chosen one sits under the mark. Drag it round, scroll, click a mode, or use ← → Home End.
 *   <ModeDial label="Mode" value={i} onChange={setI} items={[{ label: 'Auto', kind: 'pill', text: 'AUTO' }, { label: 'P', kind: 'disc' }]} />
 * kind: 'disc' (white disc, dark letter) · 'pill' (outlined) · 'text' (plain) · or pass `icon`.
 */
export type DialItem = { label: string; kind?: 'disc' | 'pill' | 'text'; text?: string; icon?: ReactNode }

const STEP = 20 // degrees between modes
const EASE = [0.65, 0, 0.35, 1] as const

export function ModeDial({ items, value, onChange, label, size = 300, effect }: {
  items: DialItem[]
  value: number
  onChange: (i: number) => void
  label: string // accessible name
  size?: number // width in px; the height follows
  effect?: KitEffect // with an effect, the newly chosen mode plays it (the ring still turns)
}) {
  const still = !!useReducedMotion()
  const pos = useMotionValue(value)
  const R = size / 2 - 4 // tick radius
  const r = R * 0.64 // mode radius
  const unit = R * 0.15 // mode size
  const cx = size / 2, cy = R + 4
  const height = cy + r + unit * 0.9
  const last = items.length - 1
  const clamp = (i: number) => Math.max(0, Math.min(last, i))
  const go = (to: number) => animate(pos, to, { duration: still ? 0 : Math.min(0.45 + 0.1 * (Math.abs(to - pos.get()) - 1), 1.4), ease: EASE })

  useEffect(() => { go(value) }, [value]) // eslint-disable-line react-hooks/exhaustive-deps

  // Drag: turn by the pointer's angle around the centre, then snap.
  const ref = useRef<HTMLDivElement>(null)
  const drag = useRef<{ a0: number; p0: number; moved: boolean } | null>(null)
  const angleAt = (e: React.PointerEvent) => {
    const b = ref.current!.getBoundingClientRect()
    return (Math.atan2(e.clientY - b.top - cy, e.clientX - b.left - cx) * 180) / Math.PI
  }
  const onPointerDown = (e: React.PointerEvent) => {
    drag.current = { a0: angleAt(e), p0: pos.get(), moved: false }
  }
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current
    if (!d) return
    let da = angleAt(e) - d.a0
    da = ((da + 540) % 360) - 180
    if (!d.moved && Math.abs(da) > 3) { d.moved = true; ref.current!.setPointerCapture(e.pointerId) }
    if (d.moved) pos.set(Math.max(-0.4, Math.min(last + 0.4, d.p0 - da / STEP)))
  }
  const onPointerUp = () => {
    const d = drag.current
    drag.current = null
    if (!d?.moved) return
    const to = clamp(Math.round(pos.get()))
    if (to === value) go(value)
    else onChange(to)
  }

  // Wheel: one mode per notch.
  const wheelAt = useRef(0)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const onWheel = (e: WheelEvent) => {
      e.preventDefault()
      const now = performance.now()
      if (now - wheelAt.current < 140 || Math.abs(e.deltaY) < 4) return
      wheelAt.current = now
      const to = clamp(valueRef.current + Math.sign(e.deltaY))
      if (to !== valueRef.current) onChangeRef.current(to)
    }
    el.addEventListener('wheel', onWheel, { passive: false })
    return () => el.removeEventListener('wheel', onWheel)
  }, []) // eslint-disable-line react-hooks/exhaustive-deps
  const valueRef = useRef(value); valueRef.current = value
  const onChangeRef = useRef(onChange); onChangeRef.current = onChange

  const onKeyDown = (e: React.KeyboardEvent) => {
    const to = { ArrowRight: value + 1, ArrowUp: value + 1, ArrowLeft: value - 1, ArrowDown: value - 1, Home: 0, End: last }[e.key]
    if (to === undefined) return
    e.preventDefault()
    if (clamp(to) !== value) onChange(clamp(to))
  }

  const ticks = Array.from({ length: 41 }, (_, i) => 180 + i * 4.5)
  return (
    <div
      ref={ref}
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={last}
      aria-valuenow={value}
      aria-valuetext={items[value]?.label}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      className="relative touch-none select-none rounded-[18px] outline-none focus-visible:ring-2 focus-visible:ring-[var(--kit-accent)]/70"
      style={{ width: size, height }}
    >
      <svg width={size} height={height} className="pointer-events-none absolute inset-0" aria-hidden>
        {ticks.map((a) => {
          const mark = a === 270
          const [c, s] = [Math.cos((a * Math.PI) / 180), Math.sin((a * Math.PI) / 180)]
          const inner = mark ? R * 0.84 : R * 0.95
          return <line key={a} x1={cx + c * inner} y1={cy + s * inner} x2={cx + c * R} y2={cy + s * R} stroke={mark ? '#fff' : 'rgba(255,255,255,.5)'} strokeWidth={mark ? 1.6 : 1.2} strokeLinecap="round" />
        })}
      </svg>
      {items.map((it, i) => (
        <Mode key={i} item={it} i={i} pos={pos} cx={cx} cy={cy} r={r} unit={unit} selected={i === value} effect={effect}
          onSelect={() => { if (i !== value) onChange(i) }} />
      ))}
    </div>
  )
}

function Mode({ item, i, pos, cx, cy, r, unit, selected, effect, onSelect }: {
  item: DialItem; i: number; pos: MotionValue<number>; cx: number; cy: number; r: number; unit: number
  selected: boolean; effect?: KitEffect; onSelect: () => void
}) {
  const rotate = useTransform(pos, (p) => -90 + (i - p) * STEP)
  const ref = useRef<HTMLSpanElement>(null)
  useFx(ref, selected, { effect, enabled: selected })
  const text = item.text ?? item.label
  const face =
    item.icon ? <span className="grid place-items-center" style={{ width: unit, height: unit }}>{item.icon}</span>
    : item.kind === 'disc' ? <span className="grid place-items-center rounded-full bg-white font-bold text-black" style={{ width: unit, height: unit, fontSize: unit * 0.52 }}>{text}</span>
    : item.kind === 'pill' ? <span className="grid place-items-center rounded-full border-[1.5px] border-white px-[0.5em] font-semibold" style={{ height: unit, fontSize: unit * 0.48 }}>{text}</span>
    : <span className="font-semibold" style={{ fontSize: unit * 0.5 }}>{text}</span>
  return (
    <motion.div className="absolute size-0" style={{ left: cx, top: cy, rotate }}>
      <button
        type="button"
        tabIndex={-1}
        aria-hidden
        onClick={onSelect}
        className="absolute left-0 top-0 whitespace-nowrap text-white"
        style={{ transform: `translate(${r}px, 0) rotate(180deg) translate(-50%, -50%)`, transformOrigin: '0 0' }}
      >
        <span ref={ref} className="block">{face}</span>
      </button>
    </motion.div>
  )
}
