import { animate, motion, useMotionValue, useReducedMotion } from 'motion/react'
import { useEffect, useRef, useState, type ButtonHTMLAttributes, type KeyboardEvent, type ReactNode } from 'react'
import { useFx, type KitEffect } from '../effects'
import { useSoftShadow } from './Glass'

type Phase = 'rest' | 'hover' | 'down'
const DEPTH: Record<Phase, number> = { rest: 1, hover: 1.3, down: -0.7 }

/**
 * Raised soft button (no glass). Hover lifts it, press sinks it into the surface, release springs it
 * back up past rest and settles. The label rides the press a beat behind the surface.
 *   <SoftButton onClick={save}>Save scene</SoftButton>
 */
export function SoftButton({ children, effect, className = '', ...rest }:
  ButtonHTMLAttributes<HTMLButtonElement> & { children: ReactNode; effect?: KitEffect }) {
  const ref = useRef<HTMLButtonElement>(null)
  useFx(ref, 0, { effect })
  const still = !!useReducedMotion()
  const [phase, setPhase] = useState<Phase>('rest')
  const depth = useMotionValue(1)
  const shadow = useSoftShadow(depth, 6)

  useEffect(() => {
    const c = animate(depth, DEPTH[phase], still ? { duration: 0 }
      : phase === 'down' ? { type: 'spring', stiffness: 700, damping: 40 } // quick, firm sink
      : { type: 'spring', stiffness: 380, damping: 13 }) // bouncy return: overshoots past rest
    return () => c.stop()
  }, [phase, still, depth])

  const key = (down: boolean) => (e: KeyboardEvent) => {
    if (e.key === ' ' || e.key === 'Enter') setPhase(down ? 'down' : 'rest')
  }

  return (
    <motion.button
      ref={ref}
      {...(rest as object)}
      style={{ boxShadow: shadow }}
      onPointerEnter={() => setPhase('hover')}
      onPointerLeave={() => setPhase('rest')}
      onPointerDown={() => setPhase('down')}
      onPointerUp={() => setPhase('hover')}
      onKeyDown={key(true)}
      onKeyUp={key(false)}
      onBlur={() => setPhase('rest')}
      className={`h-12 cursor-pointer rounded-[16px] bg-[var(--soft-surface)] px-7 text-[15px] font-semibold text-[var(--soft-ink)] outline-none focus-visible:ring-2 focus-visible:ring-[var(--soft-accent)] focus-visible:ring-offset-4 focus-visible:ring-offset-[var(--soft-surface)] ${className}`}
    >
      <motion.span
        className="inline-block"
        animate={phase === 'down' ? { scale: 0.96, y: 1 } : { scale: 1, y: 0 }}
        transition={still ? { duration: 0 } : { type: 'spring', stiffness: 500, damping: 18, delay: phase === 'down' ? 0.02 : 0 }}
      >
        {children}
      </motion.span>
    </motion.button>
  )
}
