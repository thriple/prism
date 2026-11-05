import { motion } from 'motion/react'
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react'
import { useFx, type KitEffect } from './effects'
import { BLUR_IN, SPRING } from './motion'

export type IslandToast = {
  glyph: ReactNode // short: '+', '3', '»', an icon
  label: string
  sub?: string
  tone?: 'accent' | 'danger' | 'plain'
}

const TONE = { accent: 'var(--kit-accent)', danger: 'var(--kit-danger)', plain: '#ffffff' }

/**
 * A black pill (usually top-centre of the window) that shows ambient status and
 * springs open to announce each action, then settles back.
 *
 *   const [toast, show] = useIslandToast()
 *   <DynamicIsland toast={toast}>{fileName} · {speed}</DynamicIsland>
 *   show({ glyph: '+1', label: 'Next frame', sub: '00:17:23:14' })
 */
export function DynamicIsland({ toast, children, className = '', idleSize = { width: 300, height: 30 }, toastSize = { width: 340, height: 56 }, effect }: {
  toast: IslandToast | null
  children: ReactNode // the resting content
  className?: string
  idleSize?: { width: number; height: number }
  toastSize?: { width: number; height: number }
  effect?: KitEffect
}) {
  const key = toast ? `t:${toast.label}:${toast.sub ?? ''}` : 'idle'
  const content = useRef<HTMLDivElement>(null)
  const fx = useFx(content, key, { effect })
  return (
    <motion.div
      role="status"
      aria-live="polite"
      className={`overflow-hidden rounded-[28px] bg-[var(--kit-ink)] text-white shadow-[0_0_0_1px_rgba(255,255,255,.08),0_18px_50px_rgba(0,0,0,.55)] ${className}`}
      initial={false}
      animate={toast ? toastSize : idleSize}
      transition={SPRING}
    >
      {/* Keyed so each new message blurs in. No AnimatePresence: rapid messages must never get stuck mid-exit. */}
      <motion.div
        key={key}
        ref={content}
        initial={fx === 'default' ? BLUR_IN.initial : false}
        animate={BLUR_IN.animate}
        transition={{ duration: 0.2 }}
        className="flex h-full items-center gap-3 px-3"
      >
        {toast ? (
          <>
            <span className="grid h-9 w-9 shrink-0 place-items-center rounded-full text-base font-bold text-[#120d0f]" style={{ background: TONE[toast.tone ?? 'accent'] }}>
              {toast.glyph}
            </span>
            <span className="flex min-w-0 flex-col">
              <span className="text-sm font-semibold">{toast.label}</span>
              {toast.sub && <span className="truncate text-xs text-white/60">{toast.sub}</span>}
            </span>
          </>
        ) : (
          <span className="flex min-w-0 flex-1 items-center gap-2.5 text-[13px]">{children}</span>
        )}
      </motion.div>
    </motion.div>
  )
}

/** Toast state for the island: `show()` replaces the current message and auto-clears it. */
export function useIslandToast(ms = 1500): [IslandToast | null, (t: IslandToast) => void] {
  const [toast, setToast] = useState<IslandToast | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  useEffect(() => () => clearTimeout(timer.current), [])
  const show = useCallback((t: IslandToast) => {
    clearTimeout(timer.current)
    setToast(t)
    timer.current = setTimeout(() => setToast(null), ms)
  }, [ms])
  return [toast, show]
}
