import { AnimatePresence, motion, MotionConfig } from 'motion/react'
import { useEffect, useId, useRef, useState, type ReactNode } from 'react'
import { FX_EXIT, useFx, type KitEffect } from './effects'
import { SPRING, SPRING_SNAPPY } from './motion'

// Default: content slides in the direction of travel. With an effect: it swaps in place.
const SLIDE = {
  enter: (d: number) => ({ x: d * 48, opacity: 0, filter: 'blur(6px)' }),
  center: { x: 0, opacity: 1, filter: 'blur(0px)' },
  exit: (d: number) => ({ x: d * -48, opacity: 0, filter: 'blur(6px)' })
}
const SNAP = { center: {}, exit: FX_EXIT }

export type DockTab = { label: string; icon: ReactNode; content: ReactNode }

/**
 * Icon dock that grows into a panel above itself. Clicking a tab opens it (the tab becomes a
 * pill with its label); switching tabs slides the content in the direction of travel. Clicking
 * the open tab again, clicking outside, or Esc closes it. `active` is the open tab or null.
 */
export function DockPanel({ tabs, active, onChange, label, width = 300, effect }: {
  tabs: DockTab[]
  active: number | null
  onChange: (i: number | null) => void
  label: string // accessible name of the dock
  width?: number // open width
  effect?: KitEffect // with an effect, content and pill swap in place and play it
}) {
  const id = useId()
  const root = useRef<HTMLDivElement>(null)
  const body = useRef<HTMLDivElement>(null)
  const [height, setHeight] = useState(0)
  const last = useRef(active)
  const dir = active !== null && last.current !== null && active < last.current ? -1 : 1
  useEffect(() => { last.current = active }, [active])
  const open = active !== null
  const content = useRef<HTMLDivElement>(null)
  const fx = useFx(content, active, { effect, enabled: open })
  const plain = fx === 'default'

  // Track the content's height so switching tabs resizes the panel smoothly.
  useEffect(() => {
    const ro = new ResizeObserver(([e]) => setHeight(e.contentRect.height))
    ro.observe(body.current!)
    return () => ro.disconnect()
  }, [])

  useEffect(() => {
    if (!open) return
    const down = (e: PointerEvent): void => { if (!root.current!.contains(e.target as Node)) onChange(null) }
    const key = (e: KeyboardEvent): void => { if (e.key === 'Escape') onChange(null) }
    document.addEventListener('pointerdown', down)
    document.addEventListener('keydown', key)
    return () => { document.removeEventListener('pointerdown', down); document.removeEventListener('keydown', key) }
  }, [open, onChange])

  return (
    <MotionConfig reducedMotion="user">
      <motion.div
        ref={root}
        role="toolbar"
        aria-label={label}
        initial={false}
        animate={{ width: open ? width : 'auto' }}
        transition={SPRING}
        className="flex flex-col overflow-hidden rounded-[22px] border border-[var(--kit-line)] bg-[var(--kit-panel)] text-white shadow-[0_18px_50px_rgba(0,0,0,.45)]"
      >
        <motion.div initial={false} animate={{ height: open ? height : 0 }} transition={SPRING} className="relative overflow-hidden">
          <div ref={body} className="relative">
            <AnimatePresence mode="popLayout" initial={false} custom={dir}>
              {open && (
                <motion.div
                  key={active}
                  id={`${id}-panel`}
                  custom={dir}
                  variants={plain ? SLIDE : SNAP}
                  initial={plain ? 'enter' : false}
                  animate="center"
                  exit="exit"
                  transition={SPRING_SNAPPY}
                  style={{ width }}
                  className="p-2"
                >
                  <div ref={content}>{tabs[active].content}</div>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </motion.div>

        <div className="flex items-center justify-between gap-1 p-1.5">
          {tabs.map((t, i) => (
            <button
              key={t.label}
              type="button"
              aria-label={t.label}
              aria-expanded={active === i}
              aria-controls={active === i ? `${id}-panel` : undefined}
              onClick={() => onChange(active === i ? null : i)}
              className="relative flex h-10 min-w-10 items-center justify-center rounded-full px-3 text-[13px] font-semibold hover:bg-white/8"
            >
              {active === i && (plain
                ? <motion.span layoutId={`dock-${id}`} className="absolute inset-0 rounded-full bg-white/12" transition={SPRING_SNAPPY} />
                : <FxPill effect={fx} />)}
              <span className="relative">{t.icon}</span>
              <motion.span
                initial={false}
                animate={{ width: active === i ? 'auto' : 0, opacity: active === i ? 1 : 0, marginLeft: active === i ? 6 : 0 }}
                transition={SPRING_SNAPPY}
                className="relative overflow-hidden whitespace-nowrap"
              >
                {t.label}
              </motion.span>
            </button>
          ))}
        </div>
      </motion.div>
    </MotionConfig>
  )
}

/** The active-tab pill under an effect: mounts on the new tab and plays the effect there. */
function FxPill({ effect }: { effect: KitEffect }) {
  const ref = useRef<HTMLSpanElement>(null)
  useFx(ref, null, { effect })
  return <span ref={ref} className="absolute inset-0 rounded-full bg-white/12" />
}
