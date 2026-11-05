import { useState } from 'react'
import { DigitalSparkle } from '../../components'

export function DigitalSparkleDemo() {
  const [done, setDone] = useState(0)
  return (
    <section className="glass flex flex-col gap-4 rounded-[22px] p-5">
      <div>
        <h2 className="m-0 text-base font-bold">Digital sparkles</h2>
        <p className="m-0 mt-1 text-[13px] text-white/60">
          Sprite-like bursts for a finished action or a marker that needs attention: <code className="text-white/80">{'<DigitalSparkle n={1} />'}</code> plays once; add <code className="text-white/80">loop</code>, or <code className="text-white/80">tilt</code> to lay it on the ground.
        </p>
      </div>
      <div className="flex flex-wrap items-center justify-around gap-6">
        {([1, 2, 3] as const).map((n) => (
          <div key={n} data-ds={n} className="flex flex-col items-center gap-2">
            <DigitalSparkle n={n} size={110} loop gap={0.6} delay={n * 0.25} />
            <span className="font-mono text-[10px] text-white/45">0{n}</span>
          </div>
        ))}
        {/* One-shot on completion: a new key replays it. */}
        <div className="relative grid place-items-center">
          <button type="button" onClick={() => setDone((d) => d + 1)} className="h-10 rounded-[12px] border border-[var(--kit-line)] px-4 text-[13px] font-semibold hover:bg-white/5">
            {done ? 'Saved ✓' : 'Save'}
          </button>
          {done > 0 && <DigitalSparkle key={done} n={1} size={90} className="absolute -right-[45px] -top-[45px]" />}
        </div>
        {/* Terrain marker: the sparkle lies on a tilted ground plane. */}
        <div className="relative h-[130px] w-[220px] [perspective:500px]">
          <div className="absolute inset-0 rounded-[10px] border border-white/10 [transform:rotateX(62deg)]"
            style={{ backgroundImage: 'linear-gradient(rgba(255,255,255,.08) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,.08) 1px, transparent 1px)', backgroundSize: '22px 22px' }} />
          <div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2">
            <DigitalSparkle n={3} size={120} loop gap={0.4} tilt={62} />
          </div>
          <span className="absolute left-1/2 top-1/2 size-2 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white" />
        </div>
      </div>
    </section>
  )
}
