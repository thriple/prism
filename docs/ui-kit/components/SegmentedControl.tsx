import { motion } from 'motion/react'
import { useId, useRef } from 'react'
import { useFx, useKitEffect, type KitEffect } from './effects'
import { SPRING_SNAPPY } from './motion'

/**
 * Segmented control whose white indicator springs between options.
 * `hint` is a small prefix such as the option's keyboard shortcut.
 */
export function SegmentedControl({ options, value, onChange, label, segmentWidth = 64, effect }: {
  options: { label: string; hint?: string }[]
  value: number
  onChange: (i: number) => void
  label: string // accessible name of the group
  segmentWidth?: number
  effect?: KitEffect // with an effect, the indicator jumps to the new option and plays it there
}) {
  const id = useId()
  const fx = useKitEffect(effect)
  return (
    <div role="group" aria-label={label} className="flex h-11 rounded-[14px] border border-white/10 bg-black/35 p-1">
      {options.map((o, i) => (
        <button
          key={o.label}
          type="button"
          aria-pressed={i === value}
          onClick={() => onChange(i)}
          className="relative flex items-center justify-center gap-1 text-[13px] font-semibold transition-colors"
          style={{ width: segmentWidth, color: i === value ? '#120d0f' : 'rgba(255,255,255,.78)' }}
        >
          {i === value && (fx === 'default'
            ? <motion.span layoutId={`seg-${id}`} className="absolute inset-0 rounded-[10px] bg-white/90" transition={SPRING_SNAPPY} />
            : <FxIndicator effect={fx} />)}
          {o.hint && <span className="relative text-[9px] opacity-60">{o.hint}</span>}
          <span className="relative">{o.label}</span>
        </button>
      ))}
    </div>
  )
}

/** The indicator under an effect: mounts in the new option and plays the effect there. */
function FxIndicator({ effect }: { effect: KitEffect }) {
  const ref = useRef<HTMLSpanElement>(null)
  useFx(ref, null, { effect })
  return <span ref={ref} className="absolute inset-0 rounded-[10px] bg-white/90" />
}
