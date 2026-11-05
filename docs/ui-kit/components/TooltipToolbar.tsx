import { motion, useReducedMotion } from 'motion/react'
import { useRef, useState, type ReactNode } from 'react'
import { useFx, type KitEffect } from './effects'
import { Kbd } from './Kbd'
import { SPRING_SNAPPY } from './motion'

export type ToolbarItem = { label: string; keys?: string[]; icon: ReactNode; onSelect?: () => void }

/**
 * Icon toolbar with one shared tooltip that slides between buttons. All labels sit in a
 * strip inside the tooltip; the strip scrolls to the active one while the box moves (and,
 * horizontally, resizes). Hidden, the tooltip stays where it was, so the next hover slides from there.
 */
export function TooltipToolbar({ items, label, orientation = 'horizontal', effect }: {
  items: ToolbarItem[]
  label: string // accessible name of the toolbar
  orientation?: 'horizontal' | 'vertical'
  effect?: KitEffect // with an effect, the tooltip snaps to each button and plays it there
}) {
  const horizontal = orientation === 'horizontal'
  const buttons = useRef<(HTMLButtonElement | null)[]>([])
  const labels = useRef<(HTMLSpanElement | null)[]>([])
  const [active, setActive] = useState<number | null>(null)
  const [geo, setGeo] = useState({ box: {}, strip: {} })
  const reduce = useReducedMotion()
  const box = useRef<HTMLDivElement>(null)
  const fx = useFx(box, active, { effect, enabled: active !== null })
  const move = reduce || fx !== 'default' ? { duration: 0 } : SPRING_SNAPPY

  const show = (i: number): void => {
    const b = buttons.current[i]!, l = labels.current[i]!
    setActive(i)
    setGeo(horizontal
      ? { box: { x: b.offsetLeft + b.offsetWidth / 2 - l.offsetWidth / 2, width: l.offsetWidth }, strip: { x: -l.offsetLeft } }
      : { box: { y: b.offsetTop + b.offsetHeight / 2 - l.offsetHeight / 2, height: l.offsetHeight }, strip: { y: -l.offsetTop } })
  }

  return (
    <div
      role="toolbar"
      aria-label={label}
      aria-orientation={orientation}
      onPointerLeave={() => setActive(null)}
      onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget)) setActive(null) }}
      className={`relative inline-flex gap-0.5 rounded-full border border-[var(--kit-line)] bg-[var(--kit-panel)] p-1 ${horizontal ? '' : 'flex-col'}`}
    >
      {items.map((it, i) => (
        <button
          key={it.label}
          ref={(el) => { buttons.current[i] = el }}
          type="button"
          aria-label={it.label}
          aria-keyshortcuts={it.keys?.join('+')}
          onPointerEnter={() => show(i)}
          onFocus={() => show(i)}
          onClick={it.onSelect}
          className={`grid size-10 place-items-center rounded-full text-white transition-colors ${active === i ? 'bg-white/12' : ''}`}
        >
          {it.icon}
        </button>
      ))}

      <motion.div
        ref={box}
        aria-hidden
        initial={false}
        animate={{ opacity: active === null ? 0 : 1, ...geo.box }}
        transition={{ ...move, opacity: { duration: fx === 'default' ? 0.15 : 0.06 } }}
        className={`pointer-events-none absolute z-30 overflow-hidden rounded-lg border border-[var(--kit-line)] bg-[var(--kit-ink)] text-xs text-white ${horizontal ? 'bottom-[calc(100%+10px)] left-0' : 'right-[calc(100%+10px)] top-0'}`}
      >
        <motion.div initial={false} animate={geo.strip} transition={move} className={`flex ${horizontal ? 'w-max' : 'flex-col'}`}>
          {items.map((it, i) => (
            <span
              key={it.label}
              ref={(el) => { labels.current[i] = el }}
              className="flex h-8 items-center justify-center gap-1.5 whitespace-nowrap px-2.5"
            >
              {it.label}
              {it.keys?.map((k) => <Kbd key={k}>{k}</Kbd>)}
            </span>
          ))}
        </motion.div>
      </motion.div>
    </div>
  )
}
