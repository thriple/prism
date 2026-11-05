import { AnimatePresence, motion, MotionConfig } from 'motion/react'
import { useId, useRef, useState, type ReactNode } from 'react'
import { FX_EXIT, useFx, type KitEffect } from './effects'
import { BLUR_IN, SPRING } from './motion'

/**
 * Round trigger whose panel buds off it like a liquid drop on hover (or click / focus), stays
 * joined to it by a gooey neck, then grows into a panel above. The goo is an SVG blur+threshold
 * filter on the shapes only; the content sits unfiltered on top so text stays sharp.
 */
export function GooeyMenu({ icon, label, children, width = 220, height = 120, size = 40, gap = 10, effect }: {
  icon: ReactNode // what the round trigger shows
  label: string // accessible name of the trigger
  children: ReactNode // panel content
  width?: number
  height?: number
  size?: number // trigger diameter
  gap?: number // space between trigger and open panel
  effect?: KitEffect // with an effect, the content plays it once the drop has spread
}) {
  const [open, setOpen] = useState(false)
  const id = useId().replace(/:/g, '')
  const content = useRef<HTMLDivElement>(null)
  const fx = useFx(content, open, { effect, enabled: open, delay: 200 })
  const plain = fx === 'default'
  const panel = open
    ? { width, height, bottom: size + gap, borderRadius: 16 }
    : { width: size, height: size, bottom: 0, borderRadius: size / 2 }
  // Opening: the drop rises as a circle, then spreads. Closing: it shrinks, then drops back in.
  const later = { ...SPRING, delay: 0.08 }
  const panelTransition = open
    ? { ...SPRING, width: later, height: later, borderRadius: later }
    : { ...SPRING, bottom: later }

  return (
    <MotionConfig reducedMotion="user">
      <div
        className="relative"
        style={{ width: size, height: size }}
        onPointerEnter={() => setOpen(true)}
        onPointerLeave={() => setOpen(false)}
        onKeyDown={(e) => { if (e.key === 'Escape') setOpen(false) }}
      >
        <svg width="0" height="0" className="absolute">
          <filter id={id} x="-20%" y="-20%" width="140%" height="140%">
            <feGaussianBlur stdDeviation="6" />
            <feColorMatrix values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 22 -9" />
          </filter>
        </svg>

        {/* Shapes only, gooey-filtered. The drop-shadow draws a hairline around the merged outline. */}
        <div
          aria-hidden
          className="pointer-events-none absolute bottom-0 left-0"
          style={{ width, height: height + gap + size, filter: `url(#${id}) drop-shadow(0 0 1px rgb(255 255 255 / 0.4))` }}
        >
          <span className="absolute bottom-0 left-0 rounded-full bg-[var(--kit-ink)]" style={{ width: size, height: size }} />
          <motion.span initial={false} animate={panel} transition={panelTransition} className="absolute left-0 bg-[var(--kit-ink)]" />
        </div>

        <button
          type="button"
          aria-label={label}
          aria-expanded={open}
          onClick={() => setOpen((o) => !o)}
          onFocus={() => setOpen(true)}
          onBlur={(e) => { if (!e.currentTarget.parentElement!.contains(e.relatedTarget)) setOpen(false) }}
          className="relative grid size-full place-items-center rounded-full text-white"
        >
          {icon}
        </button>

        <AnimatePresence>
          {open && (
            // Reaches down to the trigger (padding = the gap) so moving the pointer up doesn't close it.
            <motion.div
              initial={plain ? BLUR_IN.initial : false}
              animate={BLUR_IN.animate}
              exit={plain ? { ...BLUR_IN.exit, transition: { duration: 0.1 } } : FX_EXIT}
              transition={{ duration: 0.2, delay: 0.2 }}
              className="absolute left-0"
              style={{ width, height: height + gap, bottom: size, paddingBottom: gap }}
            >
              <div ref={content} className="size-full overflow-hidden rounded-2xl text-white">{children}</div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </MotionConfig>
  )
}
