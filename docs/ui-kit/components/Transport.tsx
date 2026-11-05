import { AnimatePresence, motion } from 'motion/react'
import { useEffect, useRef, type ReactNode } from 'react'
import { Tooltip } from './Tooltip'

const ic = (d: string, fill = false): ReactNode => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill={fill ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d={d} /></svg>
)
const I = {
  start: ic('M3.5 3v10M13 3.5 6 8l7 4.5z', true),
  back: ic('M11.5 3.5 5.5 8l6 4.5z', true),
  fwd: ic('M4.5 3.5 10.5 8l-6 4.5z', true),
  end: ic('M12.5 3v10M3 3.5 10 8l-7 4.5z', true),
  loop: ic('M3 8V6.5A2.5 2.5 0 0 1 5.5 4H12m0 0-2-2m2 2-2 2M13 8v1.5A2.5 2.5 0 0 1 10.5 12H4m0 0 2 2m-2-2 2-2')
}

/**
 * Playback controls: jump to start, step back, play/pause, step forward, jump to end, loop.
 * Pair with `useTransportKeys` for Space, J/K/L and ← →.
 *   <Transport playing={p} onToggle={toggle} onStep={step} onJump={jump} loop={l} onLoop={setL} rate={rate} />
 */
export function Transport({ playing, onToggle, onStep, onJump, loop, onLoop, rate = 1 }: {
  playing: boolean
  onToggle: () => void
  onStep: (frames: number) => void
  onJump: (to: 'start' | 'end') => void
  loop?: boolean
  onLoop?: (on: boolean) => void
  /** Current shuttle speed; shown as a badge when it isn't 1. */
  rate?: number
}) {
  const btn = 'grid size-9 place-items-center rounded-[10px] text-white/75 hover:bg-white/10 hover:text-white'
  return (
    <div role="toolbar" aria-label="Playback" className="flex items-center gap-1">
      <Tooltip label="Go to start" keys={['Home']}><button type="button" aria-label="Go to start" className={btn} onClick={() => onJump('start')}>{I.start}</button></Tooltip>
      <Tooltip label="Back one frame" keys={['←']}><button type="button" aria-label="Back one frame" className={btn} onClick={() => onStep(-1)}>{I.back}</button></Tooltip>
      <Tooltip label={playing ? 'Pause' : 'Play'} keys={['Space']}>
        <button type="button" aria-label={playing ? 'Pause' : 'Play'} onClick={onToggle} className="relative grid size-11 place-items-center rounded-full bg-white text-[var(--kit-ink)] hover:bg-[var(--kit-accent)]">
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.svg key={playing ? 'p' : 'r'} width="18" height="18" viewBox="0 0 16 16" fill="currentColor"
              initial={{ scale: 0.4, opacity: 0, rotate: -40 }} animate={{ scale: 1, opacity: 1, rotate: 0 }} exit={{ scale: 0.4, opacity: 0, rotate: 40 }}
              transition={{ type: 'spring', stiffness: 520, damping: 30 }}>
              <path d={playing ? 'M4 3h3v10H4zM9 3h3v10H9z' : 'M5 3 13 8l-8 5z'} />
            </motion.svg>
          </AnimatePresence>
          {rate !== 1 && (
            <span className="absolute -right-2 -top-1.5 rounded-full bg-[var(--kit-accent)] px-1 text-[9px] font-bold text-[var(--kit-ink)]">{rate > 0 ? '' : '−'}{Math.abs(rate)}×</span>
          )}
        </button>
      </Tooltip>
      <Tooltip label="Forward one frame" keys={['→']}><button type="button" aria-label="Forward one frame" className={btn} onClick={() => onStep(1)}>{I.fwd}</button></Tooltip>
      <Tooltip label="Go to end" keys={['End']}><button type="button" aria-label="Go to end" className={btn} onClick={() => onJump('end')}>{I.end}</button></Tooltip>
      {onLoop && (
        <Tooltip label="Loop In to Out">
          <button type="button" aria-label="Loop" aria-pressed={!!loop} className={`${btn} ${loop ? 'text-[var(--kit-accent)]' : ''}`} onClick={() => onLoop(!loop)}>{I.loop}</button>
        </Tooltip>
      )}
    </div>
  )
}

/**
 * Global playback keys: Space play/pause, K stop, J/L shuttle back/forward (the handler decides how
 * repeated presses speed up), ← → step a frame, Home/End jump. Ignored while typing in a field.
 */
export function useTransportKeys(h: {
  onToggle: () => void
  onShuttle: (dir: -1 | 0 | 1) => void
  onStep: (frames: number) => void
  onJump: (to: 'start' | 'end') => void
}, enabled = true) {
  const ref = useRef(h)
  ref.current = h
  useEffect(() => {
    if (!enabled) return
    const onKey = (e: KeyboardEvent): void => {
      const t = e.target as HTMLElement
      if (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName) || e.ctrlKey || e.metaKey || e.altKey) return
      const k = e.key.toLowerCase()
      const map: Record<string, () => void> = {
        ' ': () => ref.current.onToggle(),
        j: () => ref.current.onShuttle(-1),
        k: () => ref.current.onShuttle(0),
        l: () => ref.current.onShuttle(1),
        arrowleft: () => ref.current.onStep(e.shiftKey ? -10 : -1),
        arrowright: () => ref.current.onStep(e.shiftKey ? 10 : 1),
        home: () => ref.current.onJump('start'),
        end: () => ref.current.onJump('end')
      }
      if (map[k]) { e.preventDefault(); map[k]() }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [enabled])
}
