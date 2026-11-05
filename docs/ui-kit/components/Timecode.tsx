import { useState } from 'react'

/** Seconds → "HH:MM:SS:FF" at `fps`. */
export function formatTC(sec: number, fps: number): string {
  const total = Math.max(0, Math.round(sec * fps))
  const f = total % fps, s = Math.floor(total / fps)
  const p = (n: number) => String(n).padStart(2, '0')
  return `${p(Math.floor(s / 3600))}:${p(Math.floor(s / 60) % 60)}:${p(s % 60)}:${p(f)}`
}

/**
 * Typed timecode → seconds, or null if unreadable. Accepts "01:02:03:04", partial forms read from the
 * right ("3:04" = 3 s 4 f), a bare number of frames ("120"), or seconds with a unit ("2.5s").
 */
export function parseTC(text: string, fps: number): number | null {
  const t = text.trim()
  if (/^\d+(\.\d+)?s$/.test(t)) return parseFloat(t)
  if (/^\d+$/.test(t)) return parseInt(t, 10) / fps
  if (!/^[\d:;.]+$/.test(t)) return null
  const parts = t.split(/[:;.]/).map((x) => parseInt(x || '0', 10))
  if (parts.length > 4 || parts.some(isNaN)) return null
  const [f = 0, s = 0, m = 0, h = 0] = parts.reverse()
  return h * 3600 + m * 60 + s + f / fps
}

/**
 * Timecode readout. With `onChange`, click it to type a new time (Enter to jump, Esc to cancel).
 *   <Timecode time={t} fps={30} onChange={seek} />
 */
export function Timecode({ time, fps, onChange, className = '' }: {
  time: number
  fps: number
  onChange?: (sec: number) => void
  className?: string
}) {
  const [draft, setDraft] = useState<string | null>(null)
  const base = `font-mono text-[13px] tabular-nums tracking-wide ${className}`
  if (draft === null || !onChange) {
    return (
      <button
        type="button"
        disabled={!onChange}
        onClick={() => setDraft(formatTC(time, fps))}
        className={`${base} rounded-[6px] px-1.5 py-0.5 text-[var(--kit-accent)] enabled:hover:bg-white/8`}
        aria-label={onChange ? `Timecode ${formatTC(time, fps)}, click to enter a time` : undefined}
      >
        {formatTC(time, fps)}
      </button>
    )
  }
  const commit = (): void => {
    const v = parseTC(draft, fps)
    if (v !== null) onChange(v)
    setDraft(null)
  }
  return (
    <input
      autoFocus
      value={draft}
      onFocus={(e) => e.currentTarget.select()}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        e.stopPropagation() // keep editor shortcuts out of the field
        if (e.key === 'Enter') commit()
        if (e.key === 'Escape') setDraft(null)
      }}
      aria-label="Enter timecode"
      className={`${base} w-[11ch] rounded-[6px] border border-[var(--kit-accent)] bg-black/40 px-1.5 py-0.5 text-white outline-none`}
    />
  )
}
