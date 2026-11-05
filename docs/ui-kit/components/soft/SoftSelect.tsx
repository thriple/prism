import { AnimatePresence, animate, motion, useMotionValue, useReducedMotion } from 'motion/react'
import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react'
import { useFx, type KitEffect } from '../effects'
import { POP_IN, SPRING_SNAPPY } from '../motion'
import { GlassWell, softShadow, useSoftShadow } from './Glass'

/**
 * Dropdown. The trigger is an opaque raised button that sinks while the list is open. The list's
 * background is a glass cutout; the highlighted item is an opaque pill that lifts out of the glass
 * and slides between items on a spring. ↑/↓/Home/End move, Enter picks, Esc closes.
 *   <SoftSelect label="Scene" options={['Reading', 'Cinema', 'Dinner', 'Night']} value={v} onChange={setV} />
 */
export function SoftSelect({ options, value, onChange, label, effect, className = '' }: {
  options: string[]
  value: string
  onChange: (v: string) => void
  label: string
  effect?: KitEffect
  className?: string
}) {
  const id = useId()
  const trigger = useRef<HTMLButtonElement>(null)
  const list = useRef<HTMLDivElement>(null)
  const panel = useRef<HTMLDivElement>(null)
  const still = !!useReducedMotion()
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  useFx(trigger, 0, { effect })
  useFx(panel, open, { effect, enabled: open })

  const depth = useMotionValue(1)
  const shadow = useSoftShadow(depth, 6)
  useEffect(() => {
    const c = animate(depth, open ? -0.5 : 1, still ? { duration: 0 } : { type: 'spring', stiffness: 420, damping: open ? 34 : 15 })
    return () => c.stop()
  }, [open, still, depth])

  const show = () => { setActive(Math.max(0, options.indexOf(value))); setOpen(true) }
  const close = (refocus = true) => { setOpen(false); if (refocus) trigger.current?.focus() }
  const pick = (i: number) => { onChange(options[i]); close() }

  useEffect(() => {
    if (!open) return
    list.current?.focus()
    const out = (e: PointerEvent) => {
      if (!panel.current?.contains(e.target as Node) && !trigger.current?.contains(e.target as Node)) close(false)
    }
    addEventListener('pointerdown', out)
    return () => removeEventListener('pointerdown', out)
  }, [open]) // eslint-disable-line react-hooks/exhaustive-deps

  const onTriggerKey = (e: KeyboardEvent) => {
    if (['ArrowDown', 'ArrowUp'].includes(e.key)) { e.preventDefault(); show() }
  }
  const onListKey = (e: KeyboardEvent) => {
    const n = options.length
    const moves: Record<string, number> = { ArrowDown: (active + 1) % n, ArrowUp: (active - 1 + n) % n, Home: 0, End: n - 1 }
    if (e.key in moves) setActive(moves[e.key])
    else if (e.key === 'Enter' || e.key === ' ') pick(active)
    else if (e.key === 'Escape') close()
    else if (e.key === 'Tab') setOpen(false)
    else return
    if (e.key !== 'Tab') e.preventDefault()
  }

  return (
    <div className={`relative w-60 ${className}`}>
      <motion.button
        ref={trigger}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={`${id}-list`}
        aria-label={`${label}: ${value}`}
        onClick={() => (open ? close() : show())}
        onKeyDown={onTriggerKey}
        style={{ boxShadow: shadow }}
        className="flex h-12 w-full cursor-pointer items-center justify-between rounded-[16px] bg-[var(--soft-surface)] pr-4 pl-5 text-[15px] text-[var(--soft-ink)] outline-none focus-visible:ring-2 focus-visible:ring-[var(--soft-accent)] focus-visible:ring-offset-4 focus-visible:ring-offset-[var(--soft-surface)]"
      >
        <span><span className="text-[var(--soft-muted)]">{label} </span><span className="font-semibold">{value}</span></span>
        <motion.svg aria-hidden viewBox="0 0 16 16" className="size-4" animate={{ rotate: open ? 180 : 0 }} transition={still ? { duration: 0 } : SPRING_SNAPPY}>
          <path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </motion.svg>
      </motion.button>

      <AnimatePresence>
        {open && (
          <motion.div
            ref={panel}
            {...POP_IN}
            transition={still ? { duration: 0 } : SPRING_SNAPPY}
            className="absolute top-[calc(100%+14px)] left-0 z-20 w-full origin-top rounded-[22px] bg-[var(--soft-surface)] p-[6px]"
            style={{ boxShadow: softShadow(1, 8) }}
          >
            <GlassWell frost={0.42} className="rounded-[16px]">
              <div
                ref={list}
                id={`${id}-list`}
                role="listbox"
                tabIndex={-1}
                aria-label={label}
                aria-activedescendant={`${id}-${active}`}
                onKeyDown={onListKey}
                className="relative flex flex-col gap-1 p-[6px] outline-none"
              >
                {options.map((o, i) => (
                  <motion.div
                    key={o}
                    id={`${id}-${i}`}
                    role="option"
                    aria-selected={o === value}
                    onPointerEnter={() => setActive(i)}
                    onClick={() => pick(i)}
                    initial={{ opacity: 0, filter: 'blur(6px)', y: -4 }}
                    animate={{ opacity: 1, filter: 'blur(0px)', y: 0 }}
                    transition={still ? { duration: 0 } : { ...SPRING_SNAPPY, delay: 0.04 + i * 0.03 }}
                    className="relative flex h-11 cursor-pointer items-center justify-between rounded-[12px] px-4 text-[15px] text-[var(--soft-ink)]"
                  >
                    {i === active && (
                      // The opaque pill: pops out of the glass on open, then slides between items.
                      <motion.div
                        layoutId={`${id}-hl`}
                        aria-hidden
                        className="absolute inset-0 rounded-[12px] bg-[var(--soft-surface)]"
                        style={{ boxShadow: softShadow(1, 4) }}
                        initial={{ scale: 0.8, opacity: 0 }}
                        animate={{ scale: 1.03, opacity: 1 }}
                        transition={still ? { duration: 0 } : { type: 'spring', stiffness: 520, damping: 22 }}
                      />
                    )}
                    <motion.span
                      className="relative"
                      animate={{ x: i === active ? 4 : 0, fontWeight: i === active ? 600 : 450 }}
                      transition={still ? { duration: 0 } : SPRING_SNAPPY}
                    >
                      {o}
                    </motion.span>
                    {o === value && (
                      <svg aria-hidden viewBox="0 0 16 16" className="relative size-4 text-[var(--soft-accent)]">
                        <path d="M3.5 8.5l3 3 6-7" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                      </svg>
                    )}
                  </motion.div>
                ))}
              </div>
            </GlassWell>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}
