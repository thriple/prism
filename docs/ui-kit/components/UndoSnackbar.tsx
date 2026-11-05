import { AnimatePresence, motion } from 'motion/react'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { FX_EXIT, useFx, type KitEffect } from './effects'
import { Kbd } from './Kbd'
import { Ring } from './Ring'
import { SPRING_SOFT } from './motion'

/**
 * "Doing X in 8 s · Undo" — for destructive actions with a grace period.
 * You own the timer: set `until` (epoch ms) when the action starts, perform it when the time is up,
 * and clear `until` (null) on undo or when done. Position it with `className` (it's `absolute`).
 */
export function UndoSnackbar({ until, duration, message, onUndo, undoKeys = ['Ctrl', 'Z'], className = 'bottom-6 left-1/2 -translate-x-1/2', effect }: {
  until: number | null
  duration: number // ms, total grace period (for the ring)
  message: ReactNode
  onUndo: () => void
  undoKeys?: string[]
  className?: string
  effect?: KitEffect // with an effect, the bar plays it on arrival and the seconds digit on each tick
}) {
  const [now, setNow] = useState(Date.now())
  useEffect(() => {
    if (!until) return
    const id = setInterval(() => setNow(Date.now()), 100)
    return () => clearInterval(id)
  }, [until])
  const left = until ? Math.max(0, until - now) : 0
  const secs = Math.ceil(left / 1000)
  const bar = useRef<HTMLDivElement>(null)
  const digit = useRef<HTMLSpanElement>(null)
  const fx = useFx(bar, until, { effect, enabled: !!until })
  useFx(digit, secs, { effect, kind: 'glyph', enabled: !!until })
  const plain = fx === 'default'

  return (
    <AnimatePresence>
      {until && (
        <motion.div
          ref={bar}
          role="status"
          initial={plain ? { opacity: 0, y: 16, scale: 0.96 } : false}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={plain ? { opacity: 0, y: 16 } : FX_EXIT}
          transition={SPRING_SOFT}
          className={`absolute z-40 flex h-14 items-center gap-3 whitespace-nowrap rounded-[18px] border border-white/12 bg-[var(--kit-panel)] pl-2.5 pr-2 text-white shadow-[0_24px_60px_rgba(0,0,0,.55)] backdrop-blur-xl ${className}`}
        >
          <span className="relative grid place-items-center">
            <Ring progress={1 - left / duration} size={36} />
            <span ref={digit} className="absolute text-[13px] font-semibold">{secs}</span>
          </span>
          <span className="text-sm font-semibold">{message}</span>
          <button type="button" onClick={onUndo} className="ml-2 flex h-10 items-center gap-2 rounded-xl border border-white/20 bg-white/10 px-3 text-[13px] font-semibold text-[var(--kit-accent)]">
            Undo {undoKeys.map((k) => <Kbd key={k}>{k}</Kbd>)}
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
