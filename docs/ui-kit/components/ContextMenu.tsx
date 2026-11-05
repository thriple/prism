import { AnimatePresence, motion } from 'motion/react'
import { useEffect, useRef } from 'react'
import { FX_EXIT, useFx, type KitEffect } from './effects'
import { Kbd } from './Kbd'
import { SPRING_SNAPPY } from './motion'

/** `disabled` greys the item out (shown, not clickable); `hint` adds a short grey note under the label. */
export type MenuItem = { label: string; keys?: string[]; danger?: boolean; disabled?: boolean; hint?: string; onSelect: () => void } | 'separator'

/**
 * Right-click menu that pops open at the pointer and stays on screen.
 *
 *   const [at, setAt] = useState<{ x: number; y: number } | null>(null)
 *   <div onContextMenu={(e) => { e.preventDefault(); setAt({ x: e.clientX, y: e.clientY }) }}>…</div>
 *   <ContextMenu at={at} title="clip.mp4" items={[…]} onClose={() => setAt(null)} />
 */
export function ContextMenu({ at, title, items, onClose, width = 230, effect }: {
  at: { x: number; y: number } | null
  title?: string
  items: MenuItem[]
  onClose: () => void
  width?: number
  effect?: KitEffect
}) {
  const menu = useRef<HTMLDivElement>(null)
  const fx = useFx(menu, at, { effect, enabled: !!at })
  const plain = fx === 'default'

  useEffect(() => {
    if (!at) return
    const onDown = (e: MouseEvent): void => {
      if (!(e.target as HTMLElement).closest('[role="menu"]')) onClose()
    }
    const onKey = (e: KeyboardEvent): void => { if (e.key === 'Escape') onClose() }
    window.addEventListener('mousedown', onDown)
    window.addEventListener('keydown', onKey)
    window.addEventListener('blur', onClose)
    window.addEventListener('resize', onClose)
    return () => {
      window.removeEventListener('mousedown', onDown)
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('blur', onClose)
      window.removeEventListener('resize', onClose)
    }
  }, [at, onClose])

  const height = 16 + (title ? 30 : 0) + items.reduce((h, it) => h + (it === 'separator' ? 9 : it.hint ? 48 : 36), 0)
  const x = at ? Math.max(8, Math.min(at.x, window.innerWidth - width - 8)) : 0
  const y = at ? Math.max(8, Math.min(at.y, window.innerHeight - height - 8)) : 0
  let first = true

  return (
    <AnimatePresence>
      {at && (
        <motion.div
          ref={menu}
          role="menu"
          aria-label={title ?? 'Actions'}
          initial={plain ? { opacity: 0, scale: 0.92, filter: 'blur(6px)' } : false}
          animate={{ opacity: 1, scale: 1, filter: 'blur(0px)' }}
          exit={plain ? { opacity: 0, scale: 0.96 } : FX_EXIT}
          transition={SPRING_SNAPPY}
          style={{ left: x, top: y, width, transformOrigin: 'top left' }}
          className="fixed z-[60] flex flex-col gap-0.5 rounded-[14px] border border-[var(--kit-line)] bg-[var(--kit-panel)] p-1.5 text-white shadow-[0_24px_60px_rgba(0,0,0,.6)] backdrop-blur-xl"
        >
          {title && <div className="truncate px-2.5 pb-2 pt-1.5 text-xs text-white/55">{title}</div>}
          {items.map((it, i) => {
            if (it === 'separator') return <div key={i} className="mx-1.5 my-1 h-px bg-white/10" />
            const autoFocus = first && !it.disabled
            if (!it.disabled) first = false
            return (
              <button
                key={it.label}
                type="button"
                role="menuitem"
                autoFocus={autoFocus}
                disabled={it.disabled}
                aria-disabled={it.disabled}
                onClick={() => { onClose(); it.onSelect() }}
                className={`flex items-center justify-between rounded-[9px] px-2.5 text-left text-[13px] outline-none enabled:hover:bg-white/10 focus-visible:bg-white/10 disabled:cursor-default disabled:opacity-40 ${it.hint ? 'h-[46px]' : 'h-[34px]'}`}
                style={{ color: it.danger ? 'var(--kit-warn)' : undefined }}
              >
                <span className="flex min-w-0 flex-col">
                  <span className="truncate">{it.label}</span>
                  {it.hint && <span className="truncate text-[11px] text-white/45">{it.hint}</span>}
                </span>
                <span className="flex gap-0.5">{it.keys?.map((k) => <Kbd key={k}>{k}</Kbd>)}</span>
              </button>
            )
          })}
        </motion.div>
      )}
    </AnimatePresence>
  )
}
