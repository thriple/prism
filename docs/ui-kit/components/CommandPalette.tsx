import { AnimatePresence, motion } from 'motion/react'
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react'
import { FX_EXIT, useFx, type KitEffect } from './effects'
import { Kbd } from './Kbd'
import { SPRING_SNAPPY } from './motion'

export type PaletteItem = {
  key: string
  name: string
  sub?: string
  group?: string // small right-aligned tag: 'FOLDER', 'MP4', 'RECENT'…
  thumb?: string // image URL for a small preview
  icon?: ReactNode // shown when there's no thumb
  onSelect: () => void
}

/** Case-insensitive match: substring first, then subsequence. Lower = better; -1 = no match. */
export function fuzzyScore(text: string, q: string): number {
  const t = text.toLowerCase()
  const direct = t.indexOf(q)
  if (direct >= 0) return direct
  let i = 0, first = -1
  for (const ch of q) {
    i = t.indexOf(ch, i)
    if (i < 0) return -1
    if (first < 0) first = i
    i++
  }
  return 100 + first
}

/**
 * Ctrl/⌘+K style jump-to dialog: blurred scrim, spring-in box, fuzzy filter,
 * ↑/↓/Enter/Esc. You open it (e.g. on Ctrl+K) and pass the items.
 */
export function CommandPalette({ open, onClose, items, placeholder = 'Search…', limit = 50, effect }: {
  open: boolean
  onClose: () => void
  items: PaletteItem[]
  placeholder?: string
  limit?: number
  effect?: KitEffect
}) {
  const [query, setQuery] = useState('')
  const [idx, setIdx] = useState(0)
  const box = useRef<HTMLDivElement>(null)
  const fx = useFx(box, open, { effect, enabled: open })
  const plain = fx === 'default'

  useEffect(() => {
    if (open) { setQuery(''); setIdx(0) }
  }, [open])

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return items.slice(0, limit)
    return items
      .map((it) => ({ it, s: fuzzyScore(it.name, q) }))
      .filter((x) => x.s >= 0)
      .sort((a, b) => a.s - b.s)
      .slice(0, limit)
      .map((x) => x.it)
  }, [items, query, limit])

  const choose = (it?: PaletteItem): void => {
    if (!it) return
    onClose()
    it.onSelect()
  }

  const onKey = (e: KeyboardEvent): void => {
    if (e.key === 'ArrowDown') { e.preventDefault(); setIdx((i) => Math.min(shown.length - 1, i + 1)) }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setIdx((i) => Math.max(0, i - 1)) }
    else if (e.key === 'Enter') { e.preventDefault(); choose(shown[idx]) }
    else if (e.key === 'Escape' || ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k')) { e.preventDefault(); onClose() }
  }

  useEffect(() => {
    document.querySelector(`[data-kit-pal="${idx}"]`)?.scrollIntoView({ block: 'nearest' })
  }, [idx])

  return (
    <AnimatePresence>
      {open && (
        <>
          <motion.div key="scrim" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={onClose} className="fixed inset-0 z-[70] bg-[rgba(5,3,4,.55)] backdrop-blur-[4px]" />
          <motion.div
            key="box"
            ref={box}
            role="dialog"
            aria-label={placeholder}
            initial={plain ? { opacity: 0, y: -12, scale: 0.97, filter: 'blur(8px)' } : false}
            animate={{ opacity: 1, y: 0, scale: 1, filter: 'blur(0px)' }}
            exit={plain ? { opacity: 0, y: -8, scale: 0.98 } : FX_EXIT}
            transition={SPRING_SNAPPY}
            className="fixed left-1/2 top-24 z-[71] w-[min(620px,calc(100vw-32px))] -translate-x-1/2 overflow-hidden rounded-[22px] border border-[var(--kit-line)] bg-[var(--kit-panel)] text-white shadow-[0_40px_120px_rgba(0,0,0,.7)]"
          >
            <div className="flex h-[60px] items-center gap-3 border-b border-white/10 px-[18px]">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" className="text-white/70" aria-hidden="true"><circle cx="11" cy="11" r="6.5" /><path d="M20 20l-4.2-4.2" /></svg>
              <input
                autoFocus
                aria-label={placeholder}
                value={query}
                onChange={(e) => { setQuery(e.target.value); setIdx(0) }}
                onKeyDown={onKey}
                placeholder={placeholder}
                className="h-10 flex-1 bg-transparent text-lg outline-none placeholder:text-white/40"
              />
              <Kbd>Esc</Kbd>
            </div>
            <div className="thin-scroll flex max-h-[360px] flex-col gap-0.5 overflow-y-auto p-2">
              {!shown.length && <p className="m-0 px-3 py-6 italic text-white/60">Nothing matches.</p>}
              {shown.map((it, i) => (
                <button
                  key={it.key}
                  type="button"
                  data-kit-pal={i}
                  onClick={() => choose(it)}
                  onMouseMove={() => setIdx(i)}
                  className="flex h-12 items-center gap-3 rounded-xl px-2.5 text-left"
                  style={{ background: i === idx ? 'rgba(255,255,255,.09)' : 'transparent' }}
                >
                  {(it.thumb || it.icon) && (
                    <span className="grid h-7 w-11 shrink-0 place-items-center overflow-hidden rounded-[7px] bg-white/5 shadow-[inset_0_0_0_1px_rgba(255,255,255,.12)]">
                      {it.thumb ? <img src={it.thumb} alt="" className="h-full w-full object-cover" /> : it.icon}
                    </span>
                  )}
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate text-sm">{it.name}</span>
                    {it.sub && <span className="truncate text-xs text-white/55">{it.sub}</span>}
                  </span>
                  {it.group && <span className="text-[11px] tracking-[.08em] text-white/45">{it.group}</span>}
                </button>
              ))}
            </div>
            <div className="flex h-10 items-center gap-4 border-t border-white/10 px-[18px] text-xs text-white/55">
              <span className="flex items-center gap-1"><Kbd>↑</Kbd><Kbd>↓</Kbd> move</span>
              <span className="flex items-center gap-1"><Kbd>↵</Kbd> open</span>
              <span className="flex items-center gap-1"><Kbd>Esc</Kbd> close</span>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  )
}
