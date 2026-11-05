import { animate, motion, useMotionValue, useMotionTemplate, useReducedMotion } from 'motion/react'
import { useLayoutEffect, useRef, useState, type ButtonHTMLAttributes, type ReactNode } from 'react'
import { useFx, type KitEffect } from './effects'

// Measured from the reference (scaled to a 40 px tall button). The right side is roomier: the arrow lands there.
const PAD_L = 25 // the resting dot sits here
const PAD_R = 33
const DOT = 8
const GAP = 10
const SHIFT = 14 // label slides left by this much on hover
const BORDER = 1.5

const SKIN = {
  dark: {
    btn: 'border-white/55 text-white data-[on]:border-white data-[on]:text-[var(--kit-ink)]',
    fill: 'bg-white'
  },
  light: {
    btn: 'border-[#1f1f1f] text-[#111] data-[on]:border-[#0d0d0d] data-[on]:text-white',
    fill: 'bg-[#0d0d0d]'
  }
} as const

/**
 * Outlined pill for a slightly more important action. A dot sits before the label; on hover (or keyboard
 * focus) the dot swells to fill the button, the label slides over and an arrow comes in.
 *   <WowButton onClick={go}>Continue</WowButton>   <WowButton mode="light">Button</WowButton>
 * `mode`: 'dark' (default, white on the kit's dark surfaces) or 'light' (black on white, as the reference).
 */
export function WowButton({ children, mode = 'dark', effect, className = '', onPointerEnter, onPointerLeave, onFocus, onBlur, ...rest }:
  ButtonHTMLAttributes<HTMLButtonElement> & { children: ReactNode; mode?: 'dark' | 'light'; effect?: KitEffect }) {
  const btn = useRef<HTMLButtonElement>(null)
  const fill = useRef<HTMLSpanElement>(null)
  const [on, setOn] = useState(false)
  const [hovers, setHovers] = useState(0)
  const still = !!useReducedMotion()
  const plain = useFx(fill, hovers, { effect, enabled: on }) === 'default'
  const skin = SKIN[mode]

  // The fill is clipped down to the dot at rest; each edge animates on its own track.
  const top = useMotionValue(0), right = useMotionValue(0), bottom = useMotionValue(0), left = useMotionValue(0)
  const clip = useMotionTemplate`inset(${top}px ${right}px ${bottom}px ${left}px round 999px)`

  useLayoutEffect(() => {
    const el = fill.current
    if (!el) return
    const w = el.offsetWidth, h = el.offsetHeight // the fill also covers the border
    const rest = { top: (h - DOT) / 2, right: w - BORDER - PAD_L - DOT, bottom: (h - DOT) / 2, left: BORDER + PAD_L }
    const to = on ? { top: 0, right: 0, bottom: 0, left: 0 } : rest
    const instant = still || !plain
    // Measured: in (~0.1 s), the height and left edge lead and the right edge follows;
    // out, after a beat, the right edge retreats first and the rest follows (~0.2 s).
    const ease: [number, number, number, number] = on ? [0.3, 0, 0.6, 1] : [0.25, 0.1, 0.25, 1]
    const delay = instant || on ? 0 : 0.04
    const lead = { duration: instant ? 0 : on ? 0.09 : 0.2, delay, ease }
    const trail = { duration: instant ? 0 : on ? 0.11 : 0.15, delay, ease }
    const anims = [
      animate(top, to.top, lead),
      animate(bottom, to.bottom, lead),
      animate(left, to.left, lead),
      animate(right, to.right, trail)
    ]
    return () => anims.forEach((a) => a.stop())
  }, [on]) // eslint-disable-line react-hooks/exhaustive-deps

  const enter = (): void => { setOn(true); setHovers((n) => n + 1) }
  const slide = still ? { duration: 0 } : { duration: 0.2, delay: on ? 0.04 : 0, ease: [0.2, 0.8, 0.2, 1] as const }

  return (
    <button
      ref={btn}
      type="button"
      {...rest}
      data-on={on || undefined}
      onPointerEnter={(e) => { enter(); onPointerEnter?.(e) }}
      onPointerLeave={(e) => { setOn(false); onPointerLeave?.(e) }}
      onFocus={(e) => { if (e.currentTarget.matches(':focus-visible')) enter(); onFocus?.(e) }}
      onBlur={(e) => { setOn(false); onBlur?.(e) }}
      style={{ paddingLeft: PAD_L, paddingRight: PAD_R }}
      className={`relative inline-flex h-10 items-center rounded-full border-[1.5px] text-[13px] font-semibold outline-none transition-[color,border-color] duration-200 focus-visible:ring-2 focus-visible:ring-[var(--kit-accent)] ${skin.btn} ${className}`}
    >
      <motion.span ref={fill} aria-hidden style={{ clipPath: clip }} className={`absolute -inset-[1.5px] rounded-full ${skin.fill}`} />
      <motion.span animate={{ x: on ? -SHIFT : 0 }} transition={slide} className="relative flex items-center">
        <span style={{ width: DOT + GAP }} />
        {children}
        <motion.svg
          aria-hidden
          width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"
          animate={{ opacity: on ? 1 : 0 }}
          transition={still ? { duration: 0 } : on ? { duration: 0.12, delay: 0.07 } : { duration: 0.06 }}
          className="absolute left-full"
          style={{ marginLeft: GAP }}
        >
          <path d="M2 7h10M7.5 2.5 12 7l-4.5 4.5" />
        </motion.svg>
      </motion.span>
    </button>
  )
}
