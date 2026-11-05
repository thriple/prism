import { AnimatePresence, motion } from 'motion/react'
import { useRef, useState, type ReactNode } from 'react'
import { FX_EXIT, useFx, type KitEffect } from './effects'
import { Kbd } from './Kbd'
import { SPRING_SNAPPY } from './motion'

/** Hover/focus tooltip that springs up above its child, with optional key hints. */
export function Tooltip({ label, keys, side = 'top', effect, children }: {
  label: string
  keys?: string[]
  side?: 'top' | 'bottom'
  effect?: KitEffect
  children: ReactNode
}) {
  const [open, setOpen] = useState(false)
  const dy = side === 'top' ? 4 : -4
  const ref = useRef<HTMLSpanElement>(null)
  const fx = useFx(ref, open, { effect, enabled: open })
  const plain = fx === 'default'
  return (
    <span
      className="relative inline-flex"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)}
      onBlur={() => setOpen(false)}
    >
      {children}
      <AnimatePresence>
        {open && (
          <motion.span
            ref={ref}
            role="tooltip"
            initial={plain ? { opacity: 0, y: dy, scale: 0.96 } : false}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={plain ? { opacity: 0, y: dy, scale: 0.96 } : FX_EXIT}
            transition={SPRING_SNAPPY}
            className={`pointer-events-none absolute left-1/2 z-30 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded-lg border border-[var(--kit-line)] bg-[var(--kit-ink)] px-2 py-1.5 text-xs text-white ${side === 'top' ? 'bottom-[calc(100%+10px)]' : 'top-[calc(100%+10px)]'}`}
          >
            {label}
            {keys?.map((k) => <Kbd key={k}>{k}</Kbd>)}
          </motion.span>
        )}
      </AnimatePresence>
    </span>
  )
}
