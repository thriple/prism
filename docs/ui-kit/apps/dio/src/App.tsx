import { useEffect, useState } from 'react'
import { LevelMeter, PanelLayout, split, tabs, useTransportKeys, type PanelNode } from '../../../components'
import { engine } from './engine'
import { Library } from './Library'
import { end, frame, partner, trimTo } from './model'
import { Monitor } from './Monitor'
import { centerPlayhead, clock, edit, redo, store, undo, useDocName, useProject } from './store'
import { EffectControls } from './EffectControls'
import { FileMenu } from './FileMenu'
import { Timeline, addText, type Tool } from './Timeline'

const DEFAULT_LAYOUT: PanelNode = split('col', [55, 45],
  split('row', [26, 26, 48], tabs('library'), tabs('controls'), tabs('monitor')),
  split('row', [94, 6], tabs('timeline'), tabs('meter'))
)

export function App() {
  const p = useProject()
  const name = useDocName()
  const [layout, setLayout] = useState(DEFAULT_LAYOUT)
  const [tool, setTool] = useState<Tool>('select')
  const [pps, setPps] = useState(60)
  const [selected, setSelected] = useState<string[]>([])

  useTransportKeys({
    onToggle: () => engine.toggle(),
    onShuttle: (d) => engine.shuttle(d),
    onStep: (n) => engine.step(n),
    onJump: (to) => { engine.seek(to === 'start' ? 0 : Math.max(...store.get().clips.map(end), 0)); centerPlayhead() }
  })

  // Editing keys (typing in a field is left alone).
  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      const el = e.target as HTMLElement
      if (/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) || el.isContentEditable) return
      const k = e.key.toLowerCase()
      if ((e.ctrlKey || e.metaKey) && (k === 'z' || k === 'y')) {
        e.preventDefault()
        if (k === 'y' || e.shiftKey) redo(); else undo()
        return
      }
      if (e.ctrlKey || e.metaKey || e.altKey) return
      const cur = store.get()
      const t = frame(clock.get(), cur.settings.fps)
      if (k >= '1' && k <= '8') {
        if (!selected.length) return
        const sel = new Set(selected)
        edit((d) => { for (const c of d.clips) if (sel.has(c.id)) c.color = Number(k) })
      } else if (k === 't') setSelected([addText()])
      else if (k === 'v') setTool('select')
      else if (k === 'c') setTool('cut')
      else if (k === 'y') setTool('slip')
      else if (k === 'i') edit((d) => { d.range = { in: t, out: Math.max(t, d.range.out) } })
      else if (k === 'o') edit((d) => { d.range = { in: Math.min(t, d.range.in), out: t } })
      else if (k === 'q' || k === 'r') {
        // Clips under the playhead: the selected ones if any, otherwise all of them.
        const under = cur.clips.filter((c) => c.start < t && end(c) > t)
        const pick = under.filter((c) => selected.includes(c.id))
        edit((d) => trimTo(d, (pick.length ? pick : under).map((c) => c.id), t, k === 'q' ? 'front' : 'rear'))
      } else if (k === 'delete' || k === 'backspace') {
        if (!selected.length) return
        const all = new Set(selected)
        edit((d) => {
          for (const c of d.clips) if (all.has(c.id)) { const o = partner(d, c); if (o) all.add(o.id) }
          d.clips = d.clips.filter((c) => !all.has(c.id))
          d.transitions = d.transitions.filter((x) => !all.has(x.id))
        })
        setSelected([])
      } else if (k === '=' || k === '+') setPps((v) => Math.min(600, v * 1.5))
      else if (k === '-') setPps((v) => Math.max(2, v / 1.5))
      else if (k === 'escape') setSelected([])
      else return
      e.preventDefault()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [selected])

  return (
    <div className="flex h-full flex-col gap-1.5 p-1.5">
      <header className="flex h-8 shrink-0 items-center gap-3 px-2 text-[12px]">
        <span className="font-bold tracking-wide">Dio</span>
        <FileMenu />
        <span className="text-white/70">{name}</span>
        <span className="text-white/45">{p.settings.width}×{p.settings.height} · {p.settings.fps} fps</span>
      </header>
      <div className="min-h-0 flex-1">
        <PanelLayout
          layout={layout}
          onChange={setLayout}
          panels={{
            library: { title: 'Project Library', content: <Library /> },
            controls: { title: 'Effect Controls', content: <EffectControls selected={selected} /> },
            monitor: { title: 'Program', content: <Monitor /> },
            timeline: { title: 'Timeline', content: <Timeline tool={tool} setTool={setTool} pps={pps} setPps={setPps} selected={selected} setSelected={setSelected} /> },
            meter: { title: 'Audio', content: <div className="flex h-full justify-center py-3"><LevelMeter levels={engine.levels} height="100%" /></div> }
          }}
        />
      </div>
    </div>
  )
}
