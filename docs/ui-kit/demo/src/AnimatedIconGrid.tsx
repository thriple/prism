import { useState } from 'react'
import { AnimatedIcon, ANIMATED_ICON_COUNT } from '../../components'

export function AnimatedIconGrid() {
  const [loop, setLoop] = useState(false)
  return (
    <section className="glass flex flex-col gap-4 rounded-[22px] p-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="m-0 text-base font-bold">Animated icons</h2>
          <p className="m-0 mt-1 text-[13px] text-white/60">
            {ANIMATED_ICON_COUNT} line icons that play when their parent is hovered: <code className="text-white/80">{'<AnimatedIcon n={6} />'}</code>. Add <code className="text-white/80">loop</code> to keep them going.
          </p>
        </div>
        <button type="button" onClick={() => setLoop((l) => !l)} className="h-9 shrink-0 rounded-[10px] border border-[var(--kit-line)] px-3 text-[12px] hover:bg-white/5">
          {loop ? 'Stop looping' : 'Loop all'}
        </button>
      </div>
      <div className="grid grid-cols-5 gap-1 sm:grid-cols-10">
        {Array.from({ length: ANIMATED_ICON_COUNT }, (_, i) => (
          <div key={i} className="flex flex-col items-center gap-2 rounded-[14px] py-3 hover:bg-white/5">
            <AnimatedIcon key={String(loop)} n={i + 1} size={28} loop={loop} gap={1.1} />
            <span className="font-mono text-[10px] text-white/45">{String(i + 1).padStart(2, '0')}</span>
          </div>
        ))}
      </div>
    </section>
  )
}
