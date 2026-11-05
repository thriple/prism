import { motion } from 'motion/react'
import { RingThink } from './RingThink'

/**
 * Export / render progress: title, bar, detail line (frames, time left…) and Cancel.
 *   <RenderProgress progress={0.42} title="Exporting clip.mp4" detail="Frame 378 of 900 · about 0:21 left" onCancel={stop} />
 * `done` swaps the bar for a finished state; `error` shows a message in the danger colour.
 */
export function RenderProgress({ progress, title, detail, onCancel, done, error }: {
  progress: number
  title: string
  detail?: string
  onCancel?: () => void
  done?: boolean
  error?: string
}) {
  const pct = Math.round(Math.min(1, Math.max(0, progress)) * 100)
  return (
    <div role="status" className="flex w-full flex-col gap-2.5 rounded-[14px] border border-[var(--kit-line)] bg-[var(--kit-panel)] p-4 text-white">
      <div className="flex items-center gap-3">
        {!done && !error && <RingThink size={22} />}
        <span className="min-w-0 flex-1 truncate text-[13px] font-semibold">{title}</span>
        <span className="font-mono text-[12px] tabular-nums text-white/60">{pct}%</span>
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-white/10" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
        <motion.div
          className="h-full rounded-full"
          style={{ background: error ? 'var(--kit-danger)' : done ? 'var(--kit-ok)' : 'var(--kit-accent)' }}
          animate={{ width: `${pct}%` }}
          transition={{ type: 'spring', stiffness: 300, damping: 40 }}
        />
      </div>
      <div className="flex items-center justify-between gap-3 text-[12px]">
        <span className={error ? 'text-[var(--kit-danger)]' : 'text-white/55'}>{error ?? detail}</span>
        {onCancel && !done && (
          <button type="button" onClick={onCancel} className="rounded-[8px] px-2 py-1 text-white/70 hover:bg-white/10 hover:text-white">{error ? 'Close' : 'Cancel'}</button>
        )}
      </div>
    </div>
  )
}
