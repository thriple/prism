import { useState } from 'react'
import { Clicker, Kbd, ModeDial, type ClickerItem, type DialItem } from '../../components'

// Scene-mode glyphs for the dial (demo only).
const glyph = (d: string) => (
  <svg viewBox="0 0 24 24" width="100%" height="100%" fill="currentColor"><path d={d} /></svg>
)
const DIAL: DialItem[] = [
  { label: 'Macro', icon: glyph('M12 21c-3.5 0-6-2.3-6.5-5.5 2.6 0 4.8 1.3 5.8 3.2V13c-2.6-.3-4.3-2.5-4.3-5.2L9 9l3-4.5L15 9l2-1.2c0 2.7-1.7 4.9-4.3 5.2v5.7c1-1.9 3.2-3.2 5.8-3.2C18 18.7 15.5 21 12 21z') },
  { label: 'Landscape', icon: glyph('M3 19l6.5-11 4 6.5 2-3L21 19zM17.5 7.5a2 2 0 1 1 0-4 2 2 0 0 1 0 4z') },
  { label: 'Auto', kind: 'pill', text: 'AUTO' },
  { label: 'Program', kind: 'disc', text: 'P' },
  { label: 'Shutter priority', kind: 'disc', text: 'S' },
  { label: 'Aperture priority', kind: 'disc', text: 'A' },
  { label: 'Manual', kind: 'disc', text: 'M' },
  { label: 'Custom 1', kind: 'text', text: 'C1' },
  { label: 'Custom 2', kind: 'text', text: 'C2' },
  { label: 'Custom 3', kind: 'text', text: 'C3' }
]
const MONTHS: ClickerItem[] = [
  ['January', 38], ['February', 3], ['March', 5], ['April', 21], ['May', 25], ['June', 29],
  ['July', 32], ['August', 43], ['September', 28], ['October', 48], ['November', 49], ['December', 6]
].map(([label, icon]) => ({ label: label as string, icon: icon as number }))

export function DialDemos() {
  const [mode, setMode] = useState(2)
  const [month, setMonth] = useState(1)
  return (
    <section className="glass flex flex-col gap-4 rounded-[22px] p-5">
      <div>
        <h2 className="m-0 text-base font-bold">Dials</h2>
        <p className="m-0 mt-1 text-[13px] text-white/60">
          <code className="text-white/80">ModeDial</code> turns its ring of modes under the index mark; <code className="text-white/80">Clicker</code> clicks a wheel of labelled slots round and plays the focused slot's animated icon. Drag, scroll, click or use the arrow keys.
        </p>
      </div>
      <div className="flex flex-wrap items-start justify-center gap-8">
        <div className="flex flex-col items-center gap-3">
          <ModeDial label="Shooting mode" items={DIAL} value={mode} onChange={setMode} size={300} />
          <span className="text-[13px] text-white/70">{DIAL[mode].label} <span className="ml-2 inline-flex gap-1"><Kbd>←</Kbd><Kbd>→</Kbd></span></span>
        </div>
        <div className="flex flex-col items-center gap-3">
          <Clicker label="Month" items={MONTHS} value={month} onChange={setMonth} width={300} height={420} />
          <span className="text-[13px] text-white/70">{MONTHS[month].label} <span className="ml-2 inline-flex gap-1"><Kbd>↑</Kbd><Kbd>↓</Kbd></span></span>
        </div>
      </div>
    </section>
  )
}
