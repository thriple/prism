/** Circular progress ring, 0–1. Used by HoldButton and UndoSnackbar; handy on its own for countdowns. */
export function Ring({ progress, size = 36, stroke = 3, color = 'var(--kit-danger)', track = 'rgba(255,255,255,.14)' }: {
  progress: number
  size?: number
  stroke?: number
  color?: string
  track?: string
}) {
  const r = size / 2 - stroke
  const c = 2 * Math.PI * r
  const p = Math.min(1, Math.max(0, progress))
  return (
    <svg width={size} height={size} className="shrink-0 -rotate-90" aria-hidden="true">
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={track} strokeWidth={stroke} />
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={color} strokeWidth={stroke} strokeLinecap="round" strokeDasharray={c} strokeDashoffset={c * (1 - p)} />
    </svg>
  )
}
