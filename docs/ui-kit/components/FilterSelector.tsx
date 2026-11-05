import { animate, AnimatePresence, motion, MotionConfig } from 'motion/react'
import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode, type Ref } from 'react'
import { FX_EXIT, useFx, type KitEffect } from './effects'
import { SPRING_SNAPPY } from './motion'

export type FilterOption = { id: string; label: string; icon?: ReactNode }

// The picked row hands its position to the new chip; it covers most of the way in the first frame (as in the reference).
const LAND = { type: 'tween', duration: 0.1, ease: 'easeOut' } as const

const MENU_W = 200

// `dark` follows the kit's --kit-* surfaces; `light` is the reference's white chips on a light page.
const SKIN = {
  dark: {
    plus: 'border-[var(--kit-line)] bg-white/8 text-white/75 hover:bg-white/16 hover:text-white',
    menu: 'border-[var(--kit-line)] bg-[var(--kit-panel)] text-white shadow-[0_18px_44px_rgba(0,0,0,.55)] backdrop-blur-xl',
    row: 'hover:bg-white/10 focus-visible:bg-white/10',
    chip: 'border-[var(--kit-line)] bg-white/6 text-white/80 shadow-[0_2px_6px_rgba(0,0,0,.25)]',
    dim: 'text-white/55',
    x: 'text-white/50 hover:bg-white/10 hover:text-white'
  },
  light: {
    plus: 'border-black/5 bg-[#f1f1f2] text-[#555] shadow-[0_1px_3px_rgba(0,0,0,.08)] hover:bg-[#e2e2e4] hover:text-[#222]',
    menu: 'border-black/8 bg-white text-[#1d1d1f] shadow-[0_8px_24px_rgba(0,0,0,.1)]',
    row: 'hover:bg-[#f2f2f2] focus-visible:bg-[#f2f2f2]',
    chip: 'border-black/10 bg-white text-[#666] shadow-[0_1px_3px_rgba(0,0,0,.08)]',
    dim: 'text-[#8a8a8a]',
    x: 'text-[#777] hover:bg-black/5 hover:text-[#222]'
  }
} as const

/**
 * Applied-filter chips with a + button. The + opens a menu of the remaining options,
 * growing from where the button sits; picking one turns it into a chip and the + slides on.
 * × removes a chip and the rest reflow.
 *
 *   const [on, setOn] = useState(['designer'])
 *   <FilterSelector options={ROLES} value={on} onChange={setOn} />
 */
export function FilterSelector({ options, value, onChange, label = 'Filters', mode = 'dark', effect }: {
  options: FilterOption[]
  value: string[]
  onChange: (ids: string[]) => void
  label?: string
  mode?: 'dark' | 'light'
  effect?: KitEffect
}) {
  const skin = SKIN[mode]
  const uid = useId()
  const [open, setOpen] = useState(false)
  const [flip, setFlip] = useState(false) // grow leftwards when there's no room on the right
  const [session, setSession] = useState(0) // bumps per opening, so rows only share a layoutId with the chip they become
  const [landed, setLanded] = useState<Record<string, number>>({})
  const [justAdded, setJustAdded] = useState(false) // after a pick the + waits ~50 ms (chip lands first), as in the reference
  const plus = useRef<HTMLButtonElement>(null)
  const menu = useRef<HTMLDivElement>(null)
  const fx = useFx(menu, session, { effect, enabled: open })
  const plain = fx === 'default'
  const byId = new Map(options.map((o) => [o.id, o]))
  const rest = options.filter((o) => !value.includes(o.id))

  const close = (refocus: boolean): void => {
    setOpen(false)
    if (refocus) plus.current?.focus()
  }
  const add = (id: string): void => {
    setLanded((l) => ({ ...l, [id]: session }))
    setOpen(false)
    setJustAdded(true)
    onChange([...value, id])
  }

  // Grow from the + corner. Imperative because the chips' AnimatePresence (initial={false}) would block
  // a declarative `initial`, and wrapping the menu in its own AnimatePresence keeps it on screen after a pick.
  useLayoutEffect(() => {
    if (!open || !plain || !menu.current || matchMedia('(prefers-reduced-motion: reduce)').matches) return
    const a = animate(menu.current, { scale: [0.35, 1], opacity: [0, 1] }, { ...SPRING_SNAPPY, opacity: { duration: 0.08 } })
    return () => a.stop()
  }, [open, session]) // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!justAdded) return
    const t = setTimeout(() => setJustAdded(false), 500)
    return () => clearTimeout(t)
  }, [justAdded])

  useEffect(() => {
    if (!open) return
    const onDown = (e: MouseEvent): void => { if (!menu.current?.contains(e.target as Node)) close(false) }
    window.addEventListener('mousedown', onDown)
    return () => window.removeEventListener('mousedown', onDown)
  }, [open])

  const onMenuKey = (e: KeyboardEvent): void => {
    if (e.key === 'Escape') { e.preventDefault(); close(true); return }
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
    e.preventDefault()
    const rows = [...(menu.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])]
    const i = rows.indexOf(document.activeElement as HTMLButtonElement)
    rows[(i + (e.key === 'ArrowDown' ? 1 : -1) + rows.length) % rows.length]?.focus()
  }

  return (
    <MotionConfig reducedMotion="user">
      <div role="group" aria-label={label} className="flex flex-wrap items-center gap-2">
        <AnimatePresence mode="popLayout" initial={false}>
          {value.map((id) => {
            const o = byId.get(id)
            if (!o) return null
            return (
              <Chip
                key={id}
                layoutId={`${uid}-${id}-${landed[id] ?? 'x'}`}
                option={o}
                fresh={id in landed}
                skin={skin}
                effect={effect}
                onRemove={() => onChange(value.filter((v) => v !== id))}
              />
            )
          })}

          {rest.length > 0 && (
            <motion.div key="+" layout="position" transition={{ layout: { ...SPRING_SNAPPY, delay: justAdded ? 0.05 : 0 } }} className="relative">
              <motion.button
                ref={plus}
                type="button"
                aria-label="Add filter"
                aria-haspopup="menu"
                aria-expanded={open}
                onClick={() => {
                  setFlip((plus.current?.getBoundingClientRect().left ?? 0) + MENU_W > window.innerWidth - 8)
                  setSession((s) => s + 1)
                  setOpen(true)
                }}
                animate={{ opacity: open ? 0 : 1 }}
                transition={{ duration: open ? 0 : 0.12, delay: !open && justAdded ? 0.05 : 0 }}
                className={`grid size-[30px] place-items-center rounded-[9px] border transition-colors ${skin.plus}`}
              >
                <PlusGlyph />
              </motion.button>

              {open && (
                <div
                  ref={menu}
                  role="menu"
                  aria-label={`Add to ${label.toLowerCase()}`}
                  onKeyDown={onMenuKey}
                  style={{ width: MENU_W, transformOrigin: flip ? 'top right' : 'top left' }}
                  className={`absolute top-0 z-50 flex ${flip ? 'right-0' : 'left-0'} flex-col gap-0.5 rounded-[14px] border p-1.5 ${skin.menu}`}
                >
                  {rest.map((o, i) => (
                    <motion.button
                      key={o.id}
                      layoutId={`${uid}-${o.id}-${session}`}
                      type="button"
                      role="menuitem"
                      autoFocus={i === 0}
                      onClick={() => add(o.id)}
                      className={`flex h-[34px] items-center gap-2.5 rounded-[9px] px-2 text-left text-[13px] outline-none transition-colors duration-100 ${skin.row}`}
                    >
                      <span className={`grid size-4 place-items-center ${skin.dim}`}>{o.icon}</span>
                      <span className="flex-1 truncate">{o.label}</span>
                      <span className={skin.dim}><PlusGlyph /></span>
                    </motion.button>
                  ))}
                </div>
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </MotionConfig>
  )
}

function Chip({ ref: outer, layoutId, option, fresh, skin, effect, onRemove }: {
  ref?: Ref<HTMLDivElement> // AnimatePresence's popLayout measures the chip through this
  layoutId: string
  option: FilterOption
  fresh: boolean // added from the menu, not there from the start
  skin: (typeof SKIN)[keyof typeof SKIN]
  effect?: KitEffect
  onRemove: () => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  const fx = useFx(ref, 0, { effect, enabled: fresh })
  const [landing, setLanding] = useState(fresh) // LAND only for the trip from the menu; later reflows use the spring
  useEffect(() => {
    if (!landing) return
    const t = setTimeout(() => setLanding(false), 150)
    return () => clearTimeout(t)
  }, [landing])
  const plain = fx === 'default'
  return (
    <motion.div
      ref={(el) => {
        ref.current = el
        if (typeof outer === 'function') outer(el)
        else if (outer) outer.current = el
      }}
      layout="position"
      layoutId={layoutId}
      initial={plain && fresh ? { opacity: 0, filter: 'blur(5px)' } : false}
      animate={{ opacity: 1, filter: 'blur(0px)' }}
      exit={plain ? { opacity: 0, filter: 'blur(4px)', transition: { duration: 0.12 } } : FX_EXIT}
      transition={{ layout: landing ? LAND : SPRING_SNAPPY, default: { duration: 0.14, ease: 'easeOut' } }}
      className={`flex h-[30px] items-center gap-1.5 rounded-[9px] border pl-2 pr-1 text-[13px] ${skin.chip}`}
    >
      <span className={`grid size-4 place-items-center ${skin.dim}`}>{option.icon}</span>
      {option.label}
      <button
        type="button"
        aria-label={`Remove ${option.label}`}
        onClick={onRemove}
        className={`grid size-[22px] place-items-center rounded-[6px] transition-colors ${skin.x}`}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"><path d="M2 2l6 6M8 2l-6 6" /></svg>
      </button>
    </motion.div>
  )
}

function PlusGlyph() {
  return <svg width="12" height="12" viewBox="0 0 12 12" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"><path d="M6 1.5v9M1.5 6h9" /></svg>
}
