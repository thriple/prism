import { animate, motion, useMotionValue, useReducedMotion } from 'motion/react'
import { useEffect, useId, useRef } from 'react'
import { useFx, type KitEffect } from '../effects'
import { GlassWell, softShadow } from './Glass'

const W = 64, H = 34, PAD = 4, KNOB = H - PAD * 2, TRAVEL = W - PAD * 2 - KNOB

/**
 * Switch whose recessed track is a glass cutout. Off, the glass is heavily frosted; on, it clears and
 * the aurora shows through. The opaque knob stretches as it travels and lights a small accent dot.
 *   <SoftToggle label="Lights" checked={on} onChange={setOn} />
 */
export function SoftToggle({ checked, onChange, label, effect, className = '' }: {
  checked: boolean
  onChange: (next: boolean) => void
  label: string
  effect?: KitEffect
  className?: string
}) {
  const id = useId()
  const ref = useRef<HTMLButtonElement>(null)
  useFx(ref, 0, { effect })
  const still = !!useReducedMotion()
  const frost = useMotionValue(checked ? 0.15 : 0.72)
  const x = useMotionValue(checked ? TRAVEL : 0)
  const sx = useMotionValue(1)
  const first = useRef(true)

  useEffect(() => {
    if (first.current) { first.current = false; return }
    const d = still ? { duration: 0 } : undefined
    const a = [
      animate(frost, checked ? 0.15 : 0.72, d ?? { duration: 0.5, ease: [0.3, 0, 0.2, 1] }),
      animate(x, checked ? TRAVEL : 0, d ?? { type: 'spring', stiffness: 520, damping: 30 }),
      // Squash and stretch: long while it travels, back to round as it lands.
      animate(sx, [1, 1.28, 0.94, 1], d ?? { duration: 0.42, times: [0, 0.35, 0.7, 1], ease: 'easeOut' })
    ]
    return () => a.forEach(c => c.stop())
  }, [checked, still, frost, x, sx])

  return (
    <div className={`inline-flex items-center gap-4 ${className}`}>
      <span id={id} className="text-[15px] text-[var(--soft-ink)]">{label}</span>
      <button
        ref={ref}
        role="switch"
        aria-checked={checked}
        aria-labelledby={id}
        onClick={() => onChange(!checked)}
        className="relative shrink-0 cursor-pointer rounded-full outline-none focus-visible:ring-2 focus-visible:ring-[var(--soft-accent)] focus-visible:ring-offset-4 focus-visible:ring-offset-[var(--soft-surface)]"
        style={{ width: W, height: H }}
      >
        <GlassWell frost={frost} className="absolute inset-0 rounded-full" />
        <motion.span
          aria-hidden
          className="absolute grid place-items-center rounded-full bg-[var(--soft-surface)]"
          style={{ left: PAD, top: PAD, width: KNOB, height: KNOB, x, scaleX: sx, boxShadow: softShadow(1, 3) }}
        >
          <motion.span
            className="block size-[6px] rounded-full bg-[var(--soft-accent)]"
            initial={false}
            animate={{ opacity: checked ? 1 : 0, scale: checked ? 1 : 0.2 }}
            transition={still ? { duration: 0 } : { type: 'spring', stiffness: 500, damping: 20, delay: checked ? 0.12 : 0 }}
          />
        </motion.span>
      </button>
    </div>
  )
}
