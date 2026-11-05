import { useEffect, useState, type ReactNode } from 'react'
import { SoftButton, SoftSelect, SoftSlider, SoftToggle } from '../../components/soft'

function Specimen({ name, note, children }: { name: string; note: string; children: ReactNode }) {
  return (
    <section>
      <div className="flex min-h-[150px] items-center justify-center">{children}</div>
      <h2 className="mt-6 text-[17px] font-semibold">{name}</h2>
      <p className="mt-1 max-w-[38ch] text-[15px] leading-relaxed text-[var(--soft-muted)]">{note}</p>
    </section>
  )
}

export function App() {
  const [dark, setDark] = useState(() => matchMedia('(prefers-color-scheme: dark)').matches)
  const [lights, setLights] = useState(true)
  const [level, setLevel] = useState(64)
  const [scene, setScene] = useState('Reading')
  useEffect(() => { document.documentElement.dataset.softTheme = dark ? 'dark' : 'light' }, [dark])

  return (
    <main className="mx-auto max-w-[1000px] px-6 pt-10 pb-32 sm:px-12">
      <header className="flex flex-wrap items-start justify-between gap-8">
        <div>
          <h1 className="soft-extrude text-[clamp(64px,12vw,136px)] leading-[.88] font-extrabold tracking-[-0.045em]">Soft<br />glass</h1>
          <p className="mt-6 max-w-[44ch] text-[17px] leading-relaxed">
            Raised surfaces you can press, with frosted windows cut through them onto a slow aurora underneath.
          </p>
        </div>
        <SoftToggle label="Dark" checked={dark} onChange={setDark} />
      </header>

      <div className="mt-20 grid gap-x-16 gap-y-20 sm:grid-cols-2">
        <Specimen name="Button" note="Solid, no glass. Press it and it sinks into the surface, then springs back.">
          <SoftButton>Save scene</SoftButton>
        </Specimen>
        <Specimen name="Toggle" note="The track is a window. Switching on clears the frost; the knob stays solid.">
          <SoftToggle label="Lights" checked={lights} onChange={setLights} />
        </Specimen>
        <Specimen name="Slider" note="Glass rail, solid handle. The filled side of the rail is clear glass.">
          <SoftSlider label="Brightness" value={level} onChange={setLevel} format={v => `${v}%`} />
        </Specimen>
        <Specimen name="Dropdown" note="A frosted list. The highlighted item is solid and lifts out of the glass.">
          <SoftSelect label="Scene" options={['Reading', 'Cinema', 'Dinner', 'Night']} value={scene} onChange={setScene} />
        </Specimen>
      </div>
    </main>
  )
}
