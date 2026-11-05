import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useFx, type KitEffect } from './effects'
import { Ring } from './Ring'

/**
 * Press-and-hold to confirm. A ring and a fill track progress; releasing early cancels.
 * Pass `holding` to drive it from a key as well (e.g. hold Ctrl+Del): true while held.
 */
export function HoldButton({ onComplete, duration = 1000, holding: external, disabled, children, className = '', effect }: {
  onComplete: () => void
  duration?: number
  holding?: boolean
  disabled?: boolean
  children: ReactNode
  className?: string
  effect?: KitEffect // with an effect, progress fills in steps and the button plays it on completion
}) {
  const [pressed, setPressed] = useState(false)
  const [progress, setProgress] = useState(0)
  const [completed, setCompleted] = useState(0)
  const button = useRef<HTMLButtonElement>(null)
  const fx = useFx(button, completed, { effect, enabled: completed > 0 })
  const shown = fx === 'default' ? progress : Math.floor(progress * 8) / 8
  const done = useRef(onComplete)
  done.current = onComplete
  const holding = !disabled && (pressed || !!external)

  useEffect(() => {
    if (!holding) {
      setProgress(0)
      return
    }
    const start = performance.now()
    let raf = 0
    const tick = (now: number): void => {
      const p = Math.min(1, (now - start) / duration)
      setProgress(p)
      if (p >= 1) {
        setPressed(false)
        setCompleted((c) => c + 1)
        done.current()
      } else raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [holding, duration])

  return (
    <button
      ref={button}
      type="button"
      disabled={disabled}
      onPointerDown={() => setPressed(true)}
      onPointerUp={() => setPressed(false)}
      onPointerLeave={() => setPressed(false)}
      onKeyDown={(e) => { if ((e.key === ' ' || e.key === 'Enter') && !e.repeat) setPressed(true) }}
      onKeyUp={() => setPressed(false)}
      className={`relative flex h-11 items-center gap-2 overflow-hidden rounded-[12px_12px_12px_4px] bg-[var(--kit-danger)] px-3 text-[13px] font-semibold text-[#1a0503] shadow-[0_10px_30px_rgba(255,94,58,.3)] disabled:opacity-40 ${className}`}
    >
      <span className="absolute inset-y-0 left-0 bg-black/20" style={{ width: `${shown * 100}%` }} />
      <span className="relative flex items-center gap-2">
        <Ring progress={shown} size={22} stroke={2.5} color="#1a0503" track="rgba(26,5,3,.25)" />
        {children}
      </span>
    </button>
  )
}
