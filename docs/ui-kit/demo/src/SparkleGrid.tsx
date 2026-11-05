import { SheenButton, Sparkle, SPARKLE_COUNT } from '../../components'

export function SparkleGrid() {
  return (
    <section className="glass flex flex-col gap-4 rounded-[22px] p-5">
      <div>
        <h2 className="m-0 text-base font-bold">Sparkles</h2>
        <p className="m-0 mt-1 text-[13px] text-white/60">
          Accent decoration, by number: <code className="text-white/80">{'<Sparkle n={1} loop />'}</code>. The glow follows the accent colour.
        </p>
      </div>
      <div className="grid grid-cols-3 gap-2 sm:grid-cols-6">
        {Array.from({ length: SPARKLE_COUNT }, (_, i) => (
          <div key={i} className="flex flex-col items-center gap-2 rounded-[14px] border border-[var(--kit-line)] py-4">
            <Sparkle n={i + 1} size={96} loop gap={0.4} />
            <span className="font-mono text-[10px] text-white/45">{String(i + 1).padStart(2, '0')}</span>
          </div>
        ))}
      </div>
      <div className="flex justify-center py-6">
        <span className="relative inline-block">
          <SheenButton className="h-11 rounded-[14px] bg-[var(--kit-accent)] px-5 text-[13px] font-semibold text-[#17120a]">Upgrade to Pro</SheenButton>
          <Sparkle n={6} size={40} loop gap={1.2} className="absolute -right-5 -top-5" />
          <Sparkle n={5} size={28} loop gap={1.6} delay={0.5} className="absolute -bottom-3 -left-3" />
          <Sparkle n={1} size={22} loop gap={2} delay={0.9} className="absolute -top-3 left-6" />
        </span>
      </div>
    </section>
  )
}
