import { useRef, useState } from 'react'
import { Icon, ICON_COUNT, useFx } from '../../components'

/** One numbered icon; hovering plays the active effect on it. */
function Cell({ n }: { n: number }) {
  const [hovers, setHovers] = useState(0)
  const ref = useRef<HTMLSpanElement>(null)
  useFx(ref, hovers, { enabled: hovers > 0 })
  return (
    <div onPointerEnter={() => setHovers((h) => h + 1)} className="flex flex-col items-center gap-2 rounded-[14px] py-3 hover:bg-white/5">
      <span ref={ref} className="grid size-9 place-items-center"><Icon n={n} size={28} /></span>
      <span className="font-mono text-[10px] text-white/45">{String(n).padStart(2, '0')}</span>
    </div>
  )
}

export function IconGrid() {
  return (
    <section className="glass flex flex-col gap-4 rounded-[22px] p-5">
      <div>
        <h2 className="m-0 text-base font-bold">Icons</h2>
        <p className="m-0 mt-1 text-[13px] text-white/60">
          {ICON_COUNT} icons, by number: <code className="text-white/80">{'<Icon n={12} />'}</code>. 01–48 solid, 49–79 line. They take the text colour.
        </p>
      </div>
      <div className="grid grid-cols-4 gap-1 sm:grid-cols-8">
        {Array.from({ length: ICON_COUNT }, (_, i) => <Cell key={i} n={i + 1} />)}
      </div>
    </section>
  )
}
