import { motion, useTransform, type MotionValue } from 'motion/react'
import { useEffect, useRef, type CSSProperties, type ReactNode } from 'react'
import { watchAurora } from './aurora'

/**
 * Neumorphic shadow for a depth: 1 = raised, 0 = flat, -1 = pressed in. `s` is the offset in px at
 * depth 1 (blur is 2×, as the reference's 28 px / 50 px). Always four shadows, so it animates smoothly
 * from raised through flat to pressed.
 */
export function softShadow(d: number, s: number): string {
  const a = Math.max(d, 0) * s, b = Math.max(-d, 0) * s
  return `${a}px ${a}px ${a * 2}px var(--soft-dark), ${-a}px ${-a}px ${a * 2}px var(--soft-light), ` +
    `inset ${b}px ${b}px ${b * 2}px var(--soft-dark), inset ${-b}px ${-b}px ${b * 2}px var(--soft-light)`
}

export function useSoftShadow(depth: MotionValue<number>, s: number) {
  return useTransform(depth, d => softShadow(d, s))
}

/**
 * A cutout: a recessed window through the surface onto the aurora, frosted.
 * Layers, bottom to top: aurora canvas → frost tint (`tint`, or a flat one at `frost` opacity) →
 * grain → inner shadow (the recess) → children (opaque parts sitting in the well).
 */
export function GlassWell({ frost = 0.5, tint, well = 3, className = '', style, children }: {
  frost?: number | MotionValue<number>
  tint?: ReactNode
  well?: number // recess depth in px
  className?: string
  style?: CSSProperties
  children?: ReactNode
}) {
  const canvas = useRef<HTMLCanvasElement>(null)
  useEffect(() => watchAurora(canvas.current!), [])
  return (
    <div className={`isolate overflow-hidden ${/\b(absolute|fixed)\b/.test(className) ? '' : 'relative'} ${className}`} style={style}>
      <canvas ref={canvas} aria-hidden className="absolute inset-0 h-full w-full" />
      {tint ?? <motion.div aria-hidden className="absolute inset-0 bg-[var(--soft-frost)]" style={{ opacity: frost }} />}
      <div aria-hidden className="soft-grain pointer-events-none absolute inset-0" />
      <div aria-hidden className="pointer-events-none absolute inset-0 rounded-[inherit]" style={{ boxShadow: softShadow(-1, well) }} />
      {children}
    </div>
  )
}
