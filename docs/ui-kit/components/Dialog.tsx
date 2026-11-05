import { AnimatePresence, motion } from 'motion/react'
import { useEffect, useRef, type ReactNode } from 'react'
import { FX_EXIT, useFx, type KitEffect } from './effects'
import { SPRING_SNAPPY } from './motion'

/**
 * Modal dialog: dims the page and springs a panel in. Esc or clicking outside closes it (unless `locked`,
 * e.g. while an export runs). Focus moves into the dialog and returns to where it was when it closes.
 *   <Dialog open={open} title="Project Settings" onClose={close} footer={<button onClick={save}>Save</button>}>…</Dialog>
 */
export function Dialog({ open, title, onClose, children, footer, width = 440, locked = false, effect }: {
  open: boolean
  title: string
  onClose: () => void
  children: ReactNode
  footer?: ReactNode
  width?: number
  locked?: boolean
  effect?: KitEffect
}) {
  const panel = useRef<HTMLDivElement>(null)
  const fx = useFx(panel, open, { effect, enabled: open })
  const plain = fx === 'default'
  useEffect(() => {
    if (!open) return
    const back = document.activeElement as HTMLElement | null
    const onKey = (e: KeyboardEvent): void => { if (e.key === 'Escape' && !locked) onClose() }
    window.addEventListener('keydown', onKey)
    requestAnimationFrame(() => panel.current?.querySelector<HTMLElement>('input, select, textarea, button')?.focus())
    return () => { window.removeEventListener('keydown', onKey); back?.focus?.() }
  }, [open, locked, onClose])
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className="fixed inset-0 z-[80] grid place-items-center bg-black/55 p-4 backdrop-blur-[2px]"
          initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.15 }}
          onMouseDown={(e) => { if (e.target === e.currentTarget && !locked) onClose() }}
        >
          <motion.div
            ref={panel}
            role="dialog"
            aria-modal="true"
            aria-label={title}
            initial={plain ? { opacity: 0, scale: 0.94, y: 8 } : false}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={plain ? { opacity: 0, scale: 0.97 } : FX_EXIT}
            transition={SPRING_SNAPPY}
            style={{ width, maxWidth: '100%' }}
            className="flex max-h-[90vh] flex-col overflow-hidden rounded-[16px] border border-[var(--kit-line)] bg-[var(--kit-panel)] text-white shadow-[0_30px_80px_rgba(0,0,0,.6)]"
            onKeyDown={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between border-b border-[var(--kit-line)] px-4 py-3">
              <h2 className="text-[14px] font-semibold">{title}</h2>
              {!locked && <button type="button" aria-label="Close" onClick={onClose} className="grid size-7 place-items-center rounded-[8px] text-white/60 hover:bg-white/10 hover:text-white">✕</button>}
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto p-4 text-[13px]">{children}</div>
            {footer && <div className="flex justify-end gap-2 border-t border-[var(--kit-line)] px-4 py-3">{footer}</div>}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
