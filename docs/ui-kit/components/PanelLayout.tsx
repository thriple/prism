import { useRef, useState, type PointerEvent as RPointerEvent, type ReactNode } from 'react'

export type PanelNode =
  | { type: 'split'; dir: 'row' | 'col'; sizes: number[]; children: PanelNode[] }
  | { type: 'tabs'; id: string; panels: string[]; active: number }

export type PanelDef = { title: string; content: ReactNode }
type Zone = 'center' | 'left' | 'right' | 'top' | 'bottom'
type Path = number[]

let uid = 0
const newId = (): string => `g${Date.now().toString(36)}${uid++}`

/** Tabs node helper for building a layout: `tabs('library', 'effects')`. */
export const tabs = (...panels: string[]): PanelNode => ({ type: 'tabs', id: newId(), panels, active: 0 })
/** Split helper: `split('row', [3, 7], a, b)`, sizes are relative. */
export const split = (dir: 'row' | 'col', sizes: number[], ...children: PanelNode[]): PanelNode => ({ type: 'split', dir, sizes, children })

// ── tree helpers (all return new trees) ──
const at = (n: PanelNode, p: Path): PanelNode => p.reduce((m, i) => (m as Extract<PanelNode, { type: 'split' }>).children[i], n)
const update = (n: PanelNode, p: Path, fn: (m: PanelNode) => PanelNode): PanelNode => {
  if (!p.length) return fn(n)
  if (n.type !== 'split') return n
  const children = n.children.slice()
  children[p[0]] = update(children[p[0]], p.slice(1), fn)
  return { ...n, children }
}
const mapTabs = (n: PanelNode, fn: (t: Extract<PanelNode, { type: 'tabs' }>) => PanelNode): PanelNode =>
  n.type === 'tabs' ? fn(n) : { ...n, children: n.children.map((c) => mapTabs(c, fn)) }
/** Drop empty groups, collapse one-child splits, flatten same-direction nesting. */
const normalize = (n: PanelNode): PanelNode | null => {
  if (n.type === 'tabs') return n.panels.length ? { ...n, active: Math.min(n.active, n.panels.length - 1) } : null
  const kids: PanelNode[] = [], sizes: number[] = []
  n.children.forEach((c, i) => {
    const m = normalize(c)
    if (!m) return
    if (m.type === 'split' && m.dir === n.dir) {
      const sum = m.sizes.reduce((a, b) => a + b, 0)
      m.children.forEach((cc, j) => { kids.push(cc); sizes.push((n.sizes[i] * m.sizes[j]) / sum) })
    } else { kids.push(m); sizes.push(n.sizes[i]) }
  })
  if (!kids.length) return null
  if (kids.length === 1) return kids[0]
  return { ...n, children: kids, sizes }
}

/** Move `panel` out of group `fromId` into the group at `to`, as a tab (center) or a new split beside it. */
export function movePanel(root: PanelNode, panel: string, fromId: string, to: Path, zone: Zone): PanelNode {
  const target = at(root, to)
  if (target.type !== 'tabs') return root
  if (zone === 'center' && target.id === fromId) return root
  let next = update(root, to, (m) => {
    const t = m as Extract<PanelNode, { type: 'tabs' }>
    if (zone === 'center') return { ...t, panels: [...t.panels, panel], active: t.panels.length }
    const fresh = tabs(panel)
    const dir = zone === 'left' || zone === 'right' ? 'row' : 'col'
    const first = zone === 'left' || zone === 'top'
    return split(dir, [1, 1], ...(first ? [fresh, t] : [t, fresh]))
  })
  // Remove the original tab (the group that now also holds `panel` by id is `fromId`, unless it was the target).
  let removed = false
  next = mapTabs(next, (t) => {
    if (t.id !== fromId || removed) return t
    const i = t.panels.indexOf(panel)
    if (i < 0) return t
    removed = true
    const panels = t.panels.filter((_, j) => j !== i)
    return { ...t, panels, active: Math.min(t.active, panels.length - 1) }
  })
  return normalize(next) ?? root
}

/**
 * Docked workspace like Premiere's: panels tile the space, never float. Drag a divider to resize;
 * drag a tab onto another panel to join it as a tab (centre) or split beside it (drop near an edge).
 *   const [layout, setLayout] = useState(split('col', [6, 4], split('row', [3, 7], tabs('library'), tabs('monitor')), tabs('timeline')))
 *   <PanelLayout layout={layout} onChange={setLayout} panels={{ library: { title: 'Library', content: <Library/> }, … }} />
 */
export function PanelLayout({ layout, onChange, panels, className = '' }: {
  layout: PanelNode
  onChange: (l: PanelNode) => void
  panels: Record<string, PanelDef>
  className?: string
}) {
  const [drag, setDrag] = useState<{ panel: string; from: string; x: number; y: number } | null>(null)
  const [drop, setDrop] = useState<{ path: Path; zone: Zone; rect: DOMRect } | null>(null)

  const startTabDrag = (e: RPointerEvent, panel: string, from: string): void => {
    if (e.button !== 0) return
    const x0 = e.clientX, y0 = e.clientY
    let active = false, last: typeof drop = null
    const move = (ev: PointerEvent): void => {
      if (!active && Math.hypot(ev.clientX - x0, ev.clientY - y0) < 6) return
      active = true
      setDrag({ panel, from, x: ev.clientX, y: ev.clientY })
      const hit = (document.elementFromPoint(ev.clientX, ev.clientY) as HTMLElement | null)?.closest<HTMLElement>('[data-dock-path]')
      if (!hit) { last = null; setDrop(null); return }
      const r = hit.getBoundingClientRect()
      const fx = (ev.clientX - r.left) / r.width, fy = (ev.clientY - r.top) / r.height
      const inHeader = ev.clientY - r.top < 30
      const zone: Zone = inHeader ? 'center' : fx < 0.25 ? 'left' : fx > 0.75 ? 'right' : fy < 0.3 ? 'top' : fy > 0.7 ? 'bottom' : 'center'
      last = { path: JSON.parse(hit.dataset.dockPath!), zone, rect: r }
      setDrop(last)
    }
    const up = (): void => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      if (active && last) onChange(movePanel(layout, panel, from, last.path, last.zone))
      setDrag(null)
      setDrop(null)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
  }

  const render = (n: PanelNode, path: Path): ReactNode => {
    if (n.type === 'split') {
      return (
        <div className={`flex h-full w-full min-h-0 min-w-0 ${n.dir === 'row' ? 'flex-row' : 'flex-col'}`}>
          {n.children.map((c, i) => [
            i > 0 && <Divider key={`d${i}`} dir={n.dir} onDrag={(dpx, total) => onChange(update(layout, path, (m) => {
              const s = (m as typeof n).sizes.slice()
              const sum = s.reduce((a, b) => a + b, 0)
              const min = (sum * 80) / total // keep panels at least 80 px
              const d = Math.max(min - s[i - 1], Math.min(s[i] - min, (dpx / total) * sum))
              s[i - 1] += d; s[i] -= d
              return { ...m, sizes: s } as PanelNode
            }))} />,
            <div key={i} className="min-h-0 min-w-0" style={{ flex: `${n.sizes[i]} 1 0` }}>{render(c, [...path, i])}</div>
          ])}
        </div>
      )
    }
    const id = n.panels[n.active]
    return (
      <section data-dock-path={JSON.stringify(path)} className="flex h-full w-full min-h-0 min-w-0 flex-col overflow-hidden rounded-[10px] border border-[var(--kit-line)] bg-[var(--kit-panel)]">
        <div role="tablist" className="flex h-[30px] shrink-0 items-end gap-0.5 overflow-hidden border-b border-[var(--kit-line)] px-1.5">
          {n.panels.map((p, i) => (
            <button
              key={p}
              type="button"
              role="tab"
              aria-selected={i === n.active}
              onClick={() => onChange(update(layout, path, (m) => ({ ...m, active: i }) as PanelNode))}
              onPointerDown={(e) => startTabDrag(e, p, n.id)}
              className={`h-[26px] cursor-grab truncate rounded-t-[7px] px-3 text-[12px] active:cursor-grabbing ${i === n.active ? 'bg-white/8 text-white' : 'text-white/50 hover:text-white/80'}`}
            >
              {panels[p]?.title ?? p}
            </button>
          ))}
        </div>
        <div role="tabpanel" className="relative min-h-0 flex-1">{panels[id]?.content}</div>
      </section>
    )
  }

  return (
    <div className={`relative h-full w-full select-none ${className}`}>
      {render(layout, [])}
      {drag && drop && (
        <div
          className="pointer-events-none fixed z-[70] rounded-[10px] border-2 border-[var(--kit-accent)] bg-[var(--kit-accent)]/15 transition-all duration-100"
          style={zoneRect(drop.rect, drop.zone)}
        />
      )}
      {drag && (
        <div className="pointer-events-none fixed z-[71] rounded-[7px] bg-[var(--kit-accent)] px-2.5 py-1 text-[12px] font-semibold text-[var(--kit-ink)]" style={{ left: drag.x + 12, top: drag.y + 8 }}>
          {panels[drag.panel]?.title ?? drag.panel}
        </div>
      )}
    </div>
  )
}

const zoneRect = (r: DOMRect, z: Zone) => {
  const h = { left: r.left, top: r.top, width: r.width, height: r.height }
  if (z === 'left') return { ...h, width: r.width / 2 }
  if (z === 'right') return { ...h, left: r.left + r.width / 2, width: r.width / 2 }
  if (z === 'top') return { ...h, height: r.height / 2 }
  if (z === 'bottom') return { ...h, top: r.top + r.height / 2, height: r.height / 2 }
  return h
}

function Divider({ dir, onDrag }: { dir: 'row' | 'col'; onDrag: (dpx: number, total: number) => void }) {
  const ref = useRef<HTMLDivElement>(null)
  return (
    <div
      ref={ref}
      role="separator"
      aria-orientation={dir === 'row' ? 'vertical' : 'horizontal'}
      onPointerDown={(e) => {
        e.preventDefault()
        const parent = ref.current!.parentElement!.getBoundingClientRect()
        const total = dir === 'row' ? parent.width : parent.height
        const start = dir === 'row' ? e.clientX : e.clientY
        // Total offset since pointerdown: onDrag applies it to the layout as it was when the drag began.
        const move = (ev: PointerEvent): void => onDrag((dir === 'row' ? ev.clientX : ev.clientY) - start, total)
        const up = (): void => { window.removeEventListener('pointermove', move); window.removeEventListener('pointerup', up) }
        window.addEventListener('pointermove', move)
        window.addEventListener('pointerup', up)
      }}
      className={`group relative shrink-0 ${dir === 'row' ? 'w-[5px] cursor-col-resize' : 'h-[5px] cursor-row-resize'}`}
    >
      <div className={`absolute rounded-full bg-transparent transition-colors group-hover:bg-[var(--kit-accent)]/60 ${dir === 'row' ? 'inset-y-2 left-[2px] w-px' : 'inset-x-2 top-[2px] h-px'}`} />
    </div>
  )
}
