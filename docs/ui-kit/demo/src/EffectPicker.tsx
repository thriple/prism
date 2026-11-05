import { useRef } from 'react'
import { EFFECT_LABELS, EFFECTS, GooeyMenu, useFx, useTick, type KitEffect } from '../../components'

/** A white square replaying one effect on a loop, like the reference sheet. */
function Swatch({ effect }: { effect: KitEffect }) {
  const ref = useRef<HTMLSpanElement>(null)
  useFx(ref, useTick(1700, effect !== 'default'), { effect })
  return <span ref={ref} className="block size-[18px] bg-white" />
}

/** Floating effect chooser (bottom-left). One effect at a time, applied to the whole kit. */
export function EffectPicker({ value, onChange }: { value: KitEffect; onChange: (e: KitEffect) => void }) {
  const cell = (e: KitEffect): string =>
    `flex flex-col items-center justify-center gap-2 rounded-lg font-mono text-[9px] uppercase tracking-[.12em] hover:bg-white/8 ${e === value ? 'text-[var(--kit-accent)] shadow-[inset_0_0_0_1px_var(--kit-accent)]' : 'text-white/70'}`
  return (
    <div className="fixed bottom-5 left-5 z-50 flex items-center gap-3">
      <GooeyMenu label="Choose effect" width={300} height={290} icon={<span className="block size-3 bg-white" />}>
        <div className="flex h-full flex-col gap-1.5 p-2.5">
          <button type="button" aria-pressed={value === 'default'} onClick={() => onChange('default')} className={`h-9 flex-row! ${cell('default')}`}>
            Default <span className="normal-case tracking-normal text-white/40">springs + blur</span>
          </button>
          <div className="grid flex-1 grid-cols-3 gap-1.5">
            {EFFECTS.filter((e) => e !== 'default').map((e) => (
              <button key={e} type="button" aria-pressed={e === value} onClick={() => onChange(e)} className={cell(e)}>
                <Swatch effect={e} />
                {EFFECT_LABELS[e]}
              </button>
            ))}
          </div>
        </div>
      </GooeyMenu>
      <span className="rounded-full bg-black/50 px-3 py-1.5 font-mono text-[11px] uppercase tracking-[.12em] text-white/80 backdrop-blur-md">
        Effect: <span className="text-[var(--kit-accent)]">{EFFECT_LABELS[value]}</span>
      </span>
    </div>
  )
}
