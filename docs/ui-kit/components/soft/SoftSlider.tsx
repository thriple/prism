import { AnimatePresence, animate, motion, useMotionValue, useReducedMotion, useSpring, useTransform } from 'motion/react'
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react'
import { useFx, type KitEffect } from '../effects'
import { POP_IN } from '../motion'
import { GlassWell, softShadow, useSoftShadow } from './Glass'

const RAIL = 14, HANDLE = 30

/**
 * Slider with a glass-cutout rail and an opaque handle. The filled side of the rail is clear glass,
 * the rest is frosted. The handle trails the pointer on a spring; grabbing lifts it and shows the value.
 *   <SoftSlider label="Brightness" value={v} onChange={setV} format={v => `${v}%`} />
 */
export function SoftSlider({ value, onChange, label, min = 0, max = 100, step = 1, format = String, effect, className = '' }: {
  value: number
  onChange: (v: number) => void
  label: string
  min?: number
  max?: number
  step?: number
  format?: (v: number) => string
  effect?: KitEffect
  className?: string
}) {
  const root = useRef<HTMLDivElement>(null)
  useFx(root, 0, { effect })
  const still = !!useReducedMotion()
  const [w, setW] = useState(0)
  const [drag, setDrag] = useState(false)
  const [hover, setHover] = useState(false)

  useLayoutEffect(() => {
    const ro = new ResizeObserver(([e]) => setW(e.contentRect.width))
    ro.observe(root.current!)
    return () => ro.disconnect()
  }, [])

  // Handle centre, px from the rail's left edge. Everything else is derived from it.
  const travel = Math.max(0, w - HANDLE)
  const target = useMotionValue(0)
  const sprung = useSpring(target, { stiffness: 500, damping: 38 })
  const cx = still ? target : sprung
  useEffect(() => { target.set(HANDLE / 2 + ((value - min) / (max - min)) * travel) }, [value, min, max, travel, target])
  useEffect(() => { if (w) sprung.jump(target.get()) }, [w > 0]) // eslint-disable-line react-hooks/exhaustive-deps
  const handleX = useTransform(cx, c => c - HANDLE / 2)

  const depth = useMotionValue(1)
  const shadow = useSoftShadow(depth, 4)
  useEffect(() => {
    const c = animate(depth, drag ? 1.7 : 1, still ? { duration: 0 } : { type: 'spring', stiffness: 400, damping: 20 })
    return () => c.stop()
  }, [drag, still, depth])

  const set = (v: number) => {
    const s = Math.round((Math.min(max, Math.max(min, v)) - min) / step) * step + min
    if (s !== value) onChange(+s.toFixed(10))
  }
  const fromPointer = (e: PointerEvent) => {
    const r = root.current!.getBoundingClientRect()
    set(min + ((e.clientX - r.left - HANDLE / 2) / Math.max(1, travel)) * (max - min))
  }
  const keys: Record<string, number> = { ArrowRight: step, ArrowUp: step, ArrowLeft: -step, ArrowDown: -step, PageUp: step * 10, PageDown: -step * 10 }
  const onKey = (e: KeyboardEvent) => {
    if (e.key in keys) set(value + keys[e.key])
    else if (e.key === 'Home') set(min)
    else if (e.key === 'End') set(max)
    else return
    e.preventDefault()
  }

  return (
    <div
      ref={root}
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={value}
      aria-valuetext={format(value)}
      onKeyDown={onKey}
      onPointerEnter={() => setHover(true)}
      onPointerLeave={() => setHover(false)}
      onPointerDown={e => { e.currentTarget.setPointerCapture(e.pointerId); setDrag(true); fromPointer(e) }}
      onPointerMove={e => drag && fromPointer(e)}
      onPointerUp={() => setDrag(false)}
      onPointerCancel={() => setDrag(false)}
      className={`group relative w-64 cursor-pointer touch-none rounded-full outline-none ${className}`}
      style={{ height: HANDLE }}
    >
      <GlassWell
        className="absolute inset-x-0 rounded-full"
        style={{ top: (HANDLE - RAIL) / 2, height: RAIL }}
        well={2.5}
        tint={<>
          <motion.div aria-hidden className="absolute inset-y-0 left-0 bg-[var(--soft-frost)] opacity-[.12]" style={{ width: cx }} />
          <motion.div aria-hidden className="absolute inset-y-0 right-0 bg-[var(--soft-frost)] opacity-[.75]" style={{ left: cx }} />
        </>}
      />
      <motion.div
        aria-hidden
        className="absolute top-0 left-0 grid place-items-center rounded-full bg-[var(--soft-surface)] ring-[var(--soft-accent)] ring-offset-4 ring-offset-[var(--soft-surface)] group-focus-visible:ring-2"
        style={{ width: HANDLE, height: HANDLE, x: handleX, boxShadow: shadow }}
        animate={{ scale: drag ? 1.1 : 1 }}
        transition={still ? { duration: 0 } : { type: 'spring', stiffness: 500, damping: 22 }}
      >
        {/* The dimple: a small pressed-in dot that lights up while held. */}
        <motion.span
          className="block size-[10px] rounded-full"
          style={{ boxShadow: softShadow(-1, 1.5) }}
          animate={{ backgroundColor: drag ? 'var(--soft-accent)' : 'var(--soft-surface)' }}
          transition={{ duration: still ? 0 : 0.2 }}
        />
        <AnimatePresence>
          {(drag || hover) && (
            <motion.span
              {...POP_IN}
              transition={still ? { duration: 0 } : { type: 'spring', stiffness: 520, damping: 26 }}
              className="pointer-events-none absolute bottom-[calc(100%+12px)] left-1/2 min-w-10 rounded-[10px] bg-[var(--soft-surface)] px-2 py-1 text-center text-[13px] font-semibold tabular-nums text-[var(--soft-ink)]"
              style={{ x: '-50%', boxShadow: softShadow(1, 3) }}
            >
              {format(value)}
            </motion.span>
          )}
        </AnimatePresence>
      </motion.div>
    </div>
  )
}
