import { useRef } from 'react'
import { useFx, useTick, type KitEffect } from './effects'

/** Pill with a coloured dot; the dot pulses while `tone` is 'busy' (with an effect: replays it instead). */
const TONES = { ok: 'var(--kit-ok)', busy: 'var(--kit-busy)', warn: 'var(--kit-warn)' }

export function StatusDot({ tone, label, effect }: { tone: keyof typeof TONES; label: string; effect?: KitEffect }) {
  const color = TONES[tone]
  const dot = useRef<HTMLSpanElement>(null)
  const tick = useTick(1400, tone === 'busy')
  const fx = useFx(dot, `${tone}:${tone === 'busy' ? tick : 0}`, { effect })
  return (
    <span role="status" className="inline-flex items-center gap-2 rounded-full border border-white/12 bg-black/45 px-3 py-1.5 text-xs text-white backdrop-blur-md">
      <span className="relative h-[7px] w-[7px]">
        {tone === 'busy' && fx === 'default' && <span className="absolute inset-0 animate-ping rounded-full motion-reduce:animate-none" style={{ background: color }} />}
        <span ref={dot} className="absolute inset-0 rounded-full" style={{ background: color }} />
      </span>
      {label}
    </span>
  )
}
