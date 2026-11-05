import { useRef, useState, type ReactNode } from 'react'
import { FilterSelector, Icon, SegmentedControl, type FilterOption } from '../../../components'
import { detectBeats } from './beats'
import { engine } from './engine'
import {
  FX_NAMES, defaultProps, liveTransitions, maxTransition, mediaOf, newFx, propsOf, setValue, srcTime, trackOf, valueAt,
  type Anim, type Clip, type Fx, type PropName, type TextStyle
} from './model'
import { clock, edit, runtime, useProject, useTime } from './store'

const FONTS = ['Segoe UI', 'Arial', 'Helvetica', 'Verdana', 'Tahoma', 'Trebuchet MS', 'Georgia', 'Times New Roman', 'Courier New', 'Impact', 'Comic Sans MS']
const VISUAL_FX: FilterOption[] = [
  { id: 'blur', label: FX_NAMES.blur, icon: <Icon n={36} size={14} /> },
  { id: 'curves', label: FX_NAMES.curves, icon: <Icon n={39} size={14} /> },
  { id: 'hsl', label: FX_NAMES.hsl, icon: <Icon n={9} size={14} /> }
]
const AUDIO_FX: FilterOption[] = [{ id: 'beats', label: FX_NAMES.beats, icon: <Icon n={45} size={14} /> }]

/** Edit one clip in the draft project (recorded as one undo step per `key` burst). */
const editClip = (id: string, fn: (c: Clip) => void, key?: string): void =>
  edit((d) => { const c = d.clips.find((x) => x.id === id); if (c) fn(c) }, key)

/**
 * Effect Controls: transform, text, volume and effects of the selected clip(s), with keyframes.
 * A property with its stopwatch on records a key at the playhead whenever you change it.
 */
export function EffectControls({ selected }: { selected: string[] }) {
  const p = useProject()
  useTime()
  const clips = p.clips.filter((c) => selected.includes(c.id))
  const tr = liveTransitions(p).find((x) => selected.includes(x.id))
  if (tr) return <TransitionControls tr={tr} />
  if (!clips.length) return <Empty>Select a clip or transition to edit its properties.</Empty>
  if (clips.length > 2) return <Empty>{clips.length} clips selected. Select one clip (or a linked pair).</Empty>
  return (
    <div className="h-full overflow-y-auto p-2.5 thin-scroll">
      {clips.map((c) => <ClipControls key={c.id} c={c} kind={trackOf(p, c.trackId)?.kind ?? 'video'} name={c.text ? `Text: ${c.text.content.split('\n')[0]}` : mediaOf(p, c.mediaId)?.name ?? ''} />)}
    </div>
  )
}

function Empty({ children }: { children: ReactNode }) {
  return <div className="grid h-full place-items-center p-4 text-center text-[12px] text-white/40">{children}</div>
}

function ClipControls({ c, kind, name }: { c: Clip; kind: 'video' | 'audio'; name: string }) {
  const fxOptions = kind === 'audio' ? AUDIO_FX : VISUAL_FX
  const fxIds = (c.fx ?? []).map((f) => f.id)
  return (
    <div className="mb-3 rounded-[10px] border border-[var(--kit-line)] bg-white/[0.02]">
      <div className="flex items-center gap-2 border-b border-[var(--kit-line)] px-2.5 py-1.5 text-[12px] font-semibold">
        <span className={`size-2 rounded-full ${kind === 'audio' ? 'bg-[var(--kit-ok)]' : c.text ? 'bg-[#a77bea]' : 'bg-[#4f86d8]'}`} />
        <span className="truncate">{name}</span>
      </div>
      <div className="flex flex-col gap-3 p-2.5">
        {kind === 'video' ? (
          <Section title="Transform">
            <PropRow c={c} name="x" label="Position X" step={1} />
            <PropRow c={c} name="y" label="Position Y" step={1} />
            <PropRow c={c} name="scale" label={propsOf(c).lock ? 'Scale' : 'Scale Width'} step={0.5} min={0} unit="%"
              extra={
                <button type="button" aria-pressed={propsOf(c).lock} aria-label="Lock scale proportions" title="Lock proportions"
                  onClick={() => editClip(c.id, (x) => { const pr = x.props ?? (x.props = defaultProps()); pr.lock = !pr.lock; if (!pr.lock) pr.scaleY = structuredClone(pr.scale) })}
                  className={`grid size-6 place-items-center rounded-[6px] text-[12px] ${propsOf(c).lock ? 'text-[var(--kit-accent)]' : 'text-white/40'} hover:bg-white/10`}>
                  {propsOf(c).lock ? '🔒' : '🔓'}
                </button>
              } />
            {!propsOf(c).lock && <PropRow c={c} name="scaleY" label="Scale Height" step={0.5} min={0} unit="%" />}
            <PropRow c={c} name="rot" label="Rotation" step={0.5} unit="°" />
            <PropRow c={c} name="opacity" label="Opacity" step={0.5} min={0} max={100} unit="%" />
          </Section>
        ) : (
          <Section title="Audio">
            <PropRow c={c} name="volume" label="Volume" step={0.1} min={-60} max={12} unit="dB" />
          </Section>
        )}
        {c.text && <TextControls c={c} />}
        <Section title="Effects">
          <FilterSelector
            label="Effects"
            options={fxOptions}
            value={fxIds}
            onChange={(ids) => editClip(c.id, (x) => {
              const cur = x.fx ?? []
              x.fx = ids.map((id) => cur.find((f) => f.id === id) ?? newFx(id as Fx['id']))
            })}
          />
          {(c.fx ?? []).map((f) => <FxControls key={f.id} c={c} f={f} />)}
        </Section>
      </div>
    </div>
  )
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="text-[10px] font-semibold uppercase tracking-[0.12em] text-white/40">{title}</div>
      {children}
    </div>
  )
}

/** One keyframable property: stopwatch, label, value (drag or type), and previous / add-remove / next key. */
function PropRow({ c, name, label, step, min = -Infinity, max = Infinity, unit, extra }: {
  c: Clip; name: PropName; label: string; step: number; min?: number; max?: number; unit?: string; extra?: ReactNode
}) {
  const a: Anim = propsOf(c)[name]
  const st = srcTime(c, clock.get())
  const v = valueAt(a, st)
  const keys = a.k ?? []
  const onKey = keys.find((k) => Math.abs(k.t - st) < 1e-4)
  const set = (nv: number): void => editClip(c.id, (x) => {
    const pr = x.props ?? (x.props = defaultProps())
    setValue(pr[name], srcTime(x, clock.get()), Math.min(max, Math.max(min, nv)))
  }, `prop:${c.id}:${name}`)
  const seekKey = (t: number): void => engine.seek(c.start + (t - c.in))
  const prev = [...keys].reverse().find((k) => k.t < st - 1e-4)
  const next = keys.find((k) => k.t > st + 1e-4)
  return (
    <div className="flex h-7 items-center gap-1.5 text-[12px]">
      <button type="button" aria-pressed={keys.length > 0} aria-label={`${keys.length ? 'Turn off' : 'Turn on'} keyframes for ${label}`}
        title={keys.length ? 'Keyframes on (click to remove all)' : 'Animate this property'}
        onClick={() => editClip(c.id, (x) => {
          const pr = x.props ?? (x.props = defaultProps())
          const s = srcTime(x, clock.get())
          if (pr[name].k?.length) pr[name] = { v: valueAt(pr[name], s) }
          else pr[name] = { v: pr[name].v, k: [{ t: s, v: pr[name].v }] }
        })}
        className={`grid size-6 place-items-center rounded-[6px] hover:bg-white/10 ${keys.length ? 'text-[var(--kit-accent)]' : 'text-white/35'}`}>
        <svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6"><circle cx="8" cy="9" r="5.5" /><path d="M8 9V6M6.5 1.5h3" /></svg>
      </button>
      <span className="w-[84px] shrink-0 truncate text-white/65">{label}</span>
      <NumberField value={v} step={step} unit={unit} onChange={set} />
      {extra}
      <div className="ml-auto flex items-center">
        {keys.length > 0 && (
          <>
            <button type="button" aria-label="Previous keyframe" disabled={!prev} onClick={() => prev && seekKey(prev.t)} className="grid size-6 place-items-center rounded-[6px] text-white/60 enabled:hover:bg-white/10 disabled:opacity-25">‹</button>
            <button type="button" aria-label={onKey ? 'Remove keyframe' : 'Add keyframe'}
              onClick={() => editClip(c.id, (x) => {
                const pr = x.props ?? (x.props = defaultProps())
                const s = srcTime(x, clock.get())
                const k = pr[name].k!
                const i = k.findIndex((q) => Math.abs(q.t - s) < 1e-4)
                if (i >= 0) { if (k.length > 1) k.splice(i, 1) } else setValue(pr[name], s, valueAt(pr[name], s))
              })}
              className="grid size-6 place-items-center rounded-[6px] hover:bg-white/10">
              <svg width="11" height="11" viewBox="0 0 10 10"><path d="M5 .8 9.2 5 5 9.2.8 5z" fill={onKey ? 'var(--kit-accent)' : 'none'} stroke="var(--kit-accent)" strokeWidth="1.2" /></svg>
            </button>
            <button type="button" aria-label="Next keyframe" disabled={!next} onClick={() => next && seekKey(next.t)} className="grid size-6 place-items-center rounded-[6px] text-white/60 enabled:hover:bg-white/10 disabled:opacity-25">›</button>
          </>
        )}
      </div>
    </div>
  )
}

/** Number you can drag left/right to change (Shift = ×10), or click to type. */
function NumberField({ value, step, unit, onChange }: { value: number; step: number; unit?: string; onChange: (v: number) => void }) {
  const [draft, setDraft] = useState<string | null>(null)
  const moved = useRef(false)
  const shown = Math.abs(value) < 1e-9 ? '0' : String(Number(value.toFixed(step < 1 ? 1 : 0)))
  if (draft !== null) {
    const commit = (): void => { const n = parseFloat(draft); if (isFinite(n)) onChange(n); setDraft(null) }
    return (
      <input autoFocus value={draft} onFocus={(e) => e.currentTarget.select()} onChange={(e) => setDraft(e.target.value)} onBlur={commit}
        onKeyDown={(e) => { e.stopPropagation(); if (e.key === 'Enter') commit(); if (e.key === 'Escape') setDraft(null) }}
        className="w-[70px] rounded-[5px] border border-[var(--kit-accent)] bg-black/40 px-1 font-mono text-[12px] outline-none" />
    )
  }
  return (
    <button type="button"
      onPointerDown={(e) => {
        const x0 = e.clientX, v0 = value
        moved.current = false
        const mv = (ev: PointerEvent): void => {
          const dx = ev.clientX - x0
          if (Math.abs(dx) > 2) moved.current = true
          if (moved.current) onChange(v0 + Math.round(dx) * step * (ev.shiftKey ? 10 : 1))
        }
        const up = (): void => { window.removeEventListener('pointermove', mv); window.removeEventListener('pointerup', up) }
        window.addEventListener('pointermove', mv)
        window.addEventListener('pointerup', up)
      }}
      onClick={() => { if (!moved.current) setDraft(shown) }}
      className="min-w-[70px] cursor-ew-resize rounded-[5px] px-1 text-left font-mono text-[12px] text-[var(--kit-accent)] hover:bg-white/8">
      {shown}{unit && <span className="ml-0.5 text-white/40">{unit}</span>}
    </button>
  )
}

const ALIGNS: TextStyle['align'][] = ['left', 'center', 'right']

function TextControls({ c }: { c: Clip }) {
  const s = c.text!
  const [fonts, setFonts] = useState(FONTS)
  const set = (patch: Partial<TextStyle>, key?: string): void => editClip(c.id, (x) => { Object.assign(x.text!, patch) }, key)
  const q = (window as unknown as { queryLocalFonts?: () => Promise<{ family: string }[]> }).queryLocalFonts
  const btn = (on: boolean) => `grid size-7 place-items-center rounded-[6px] text-[13px] ${on ? 'bg-[var(--kit-accent)] text-[var(--kit-ink)]' : 'text-white/70 hover:bg-white/10'}`
  return (
    <Section title="Text">
      <textarea value={s.content} rows={2} onChange={(e) => set({ content: e.target.value }, `text:${c.id}`)} onKeyDown={(e) => e.stopPropagation()}
        className="resize-y rounded-[7px] border border-[var(--kit-line)] bg-black/30 px-2 py-1 text-[13px] outline-none focus:border-[var(--kit-accent)]" />
      <div className="flex items-center gap-1.5">
        <select value={s.font} onChange={(e) => set({ font: e.target.value })} aria-label="Font"
          className="min-w-0 flex-1 rounded-[7px] border border-[var(--kit-line)] bg-[#15171c] px-1.5 py-1 text-[12px]">
          {[...new Set([s.font, ...fonts])].map((f) => <option key={f} value={f} style={{ fontFamily: f }}>{f}</option>)}
        </select>
        {q && fonts === FONTS && (
          <button type="button" title="List every font installed on this computer" onClick={async () => {
            try { const all = await q(); setFonts([...new Set([...FONTS, ...all.map((f) => f.family)])].sort()) } catch { /* permission denied */ }
          }} className="rounded-[7px] px-2 py-1 text-[11px] text-white/60 hover:bg-white/10">All fonts…</button>
        )}
      </div>
      <div className="flex h-7 items-center gap-1.5 text-[12px]">
        <span className="w-[84px] shrink-0 pl-[30px] text-white/65">Size</span>
        <NumberField value={s.size} step={1} unit="px" onChange={(v) => set({ size: Math.max(4, v) }, `size:${c.id}`)} />
        <input type="color" value={s.color} onChange={(e) => set({ color: e.target.value }, `color:${c.id}`)} aria-label="Text colour" className="ml-auto h-6 w-8 cursor-pointer rounded bg-transparent" />
      </div>
      <div className="flex items-center gap-1">
        <SegmentedControl label="Alignment" segmentWidth={52} options={[{ label: 'Left' }, { label: 'Center' }, { label: 'Right' }]}
          value={ALIGNS.indexOf(s.align)} onChange={(i) => set({ align: ALIGNS[i] })} />
        <div className="ml-auto flex gap-0.5">
          <button type="button" aria-label="Bold" aria-pressed={s.bold} onClick={() => set({ bold: !s.bold })} className={`${btn(s.bold)} font-bold`}>B</button>
          <button type="button" aria-label="Italic" aria-pressed={s.italic} onClick={() => set({ italic: !s.italic })} className={`${btn(s.italic)} italic`}>I</button>
          <button type="button" aria-label="Underline" aria-pressed={s.underline} onClick={() => set({ underline: !s.underline })} className={`${btn(s.underline)} underline`}>U</button>
        </div>
      </div>
    </Section>
  )
}

function Slider({ label, value, min, max, step = 1, unit, onChange }: { label: string; value: number; min: number; max: number; step?: number; unit?: string; onChange: (v: number) => void }) {
  return (
    <label className="flex h-7 items-center gap-2 text-[12px]">
      <span className="w-[84px] shrink-0 text-white/65">{label}</span>
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} className="min-w-0 flex-1 accent-[var(--kit-accent)]" />
      <span className="w-12 text-right font-mono text-[11px] text-white/60">{Number(value.toFixed(2))}{unit}</span>
    </label>
  )
}

function FxControls({ c, f }: { c: Clip; f: Fx }) {
  const set = (patch: Partial<Fx>): void => editClip(c.id, (x) => { const g = x.fx?.find((q) => q.id === f.id); if (g) Object.assign(g, patch) }, `fx:${c.id}:${f.id}`)
  return (
    <div className="rounded-[8px] bg-white/[0.03] p-2">
      <div className="mb-1 text-[11px] font-semibold text-white/75">{FX_NAMES[f.id]}</div>
      {f.id === 'blur' && <Slider label="Amount" value={f.amount} min={0} max={60} step={0.5} unit="px" onChange={(v) => set({ amount: v })} />}
      {f.id === 'hsl' && (
        <>
          <Slider label="Hue" value={f.hue} min={-180} max={180} unit="°" onChange={(v) => set({ hue: v })} />
          <Slider label="Saturation" value={f.sat} min={0} max={300} unit="%" onChange={(v) => set({ sat: v })} />
          <Slider label="Lightness" value={f.light} min={0} max={200} unit="%" onChange={(v) => set({ light: v })} />
        </>
      )}
      {f.id === 'curves' && <Curves f={f} onChange={(ch, pts) => set({ [ch]: pts } as Partial<Fx>)} />}
      {f.id === 'beats' && <BeatControls c={c} f={f} set={set} />}
    </div>
  )
}

const CH = ['rgb', 'r', 'g', 'b'] as const
const CH_COLOR = { rgb: '#ffffff', r: '#ff6b6b', g: '#5ee37b', b: '#62a8ff' }

/** Curve editor: five points per channel (RGB master, R, G, B); drag a point up/down. Double-click resets the channel. */
function Curves({ f, onChange }: { f: Extract<Fx, { id: 'curves' }>; onChange: (ch: (typeof CH)[number], pts: number[]) => void }) {
  const [ch, setCh] = useState(0)
  const W = 200, H = 130
  const key = CH[ch]
  const pts = f[key]
  const svg = useRef<SVGSVGElement>(null)
  const path = (a: number[]) => a.map((v, i) => `${i ? 'L' : 'M'}${(i / 4) * W} ${(1 - v) * H}`).join('')
  return (
    <div className="flex flex-col gap-1.5">
      <SegmentedControl label="Curve channel" segmentWidth={44} options={[{ label: 'RGB' }, { label: 'R' }, { label: 'G' }, { label: 'B' }]} value={ch} onChange={setCh} />
      <svg ref={svg} viewBox={`-6 -6 ${W + 12} ${H + 12}`} className="w-full max-w-[260px] touch-none rounded-[6px] bg-black/40" onDoubleClick={() => onChange(key, [0, 0.25, 0.5, 0.75, 1])}>
        {[1, 2, 3].map((i) => <path key={i} d={`M${(i / 4) * W} 0V${H}M0 ${(i / 4) * H}H${W}`} stroke="rgba(255,255,255,.08)" />)}
        <path d={`M0 ${H}L${W} 0`} stroke="rgba(255,255,255,.15)" strokeDasharray="3 3" />
        {CH.filter((c) => c !== key).map((c) => <path key={c} d={path(f[c])} fill="none" stroke={CH_COLOR[c]} strokeOpacity={0.2} />)}
        <path d={path(pts)} fill="none" stroke={CH_COLOR[key]} strokeWidth={1.5} />
        {pts.map((v, i) => (
          <circle key={i} cx={(i / 4) * W} cy={(1 - v) * H} r={5} fill={CH_COLOR[key]} className="cursor-ns-resize"
            onPointerDown={(e) => {
              e.preventDefault()
              const r = svg.current!.getBoundingClientRect()
              const mv = (ev: PointerEvent): void => {
                const y = ((ev.clientY - r.top) / r.height) * (H + 12) - 6
                const next = [...pts]
                next[i] = Math.min(1, Math.max(0, 1 - y / H))
                onChange(key, next)
              }
              const up = (): void => { window.removeEventListener('pointermove', mv); window.removeEventListener('pointerup', up) }
              window.addEventListener('pointermove', mv)
              window.addEventListener('pointerup', up)
            }} />
        ))}
      </svg>
    </div>
  )
}

function BeatControls({ c, f, set }: { c: Clip; f: Extract<Fx, { id: 'beats' }>; set: (p: Partial<Fx>) => void }) {
  const env = runtime.get(c.mediaId)?.bass
  const beats = env ? detectBeats(env, f.sensitivity, f.gap, c.mediaId).filter((b) => b >= c.in && b < c.in + c.dur) : []
  const bpm = beats.length > 2 ? Math.round(60 / ((beats[beats.length - 1] - beats[0]) / (beats.length - 1))) : null
  return (
    <>
      <Slider label="Sensitivity" value={f.sensitivity} min={1} max={4} step={0.1} unit="×" onChange={(v) => set({ sensitivity: v })} />
      <Slider label="Min. spacing" value={f.gap} min={0.1} max={1} step={0.05} unit="s" onChange={(v) => set({ gap: v })} />
      <div className="text-[11px] text-white/50">
        {env ? `${beats.length} beats in this clip${bpm ? ` · about ${bpm} BPM` : ''}. Clips snap to them when dragged.` : 'No audio data for this clip.'}
      </div>
    </>
  )
}


/** Selected transition: its length in frames (up to what the clips' spare frames allow), and Remove. */
function TransitionControls({ tr }: { tr: ReturnType<typeof liveTransitions>[number] }) {
  const p = useProject()
  const fps = p.settings.fps
  const max = Math.floor(Math.min(maxTransition(p, tr.A, tr.B), tr.A.dur * 2, tr.B.dur * 2) * fps + 1e-6)
  return (
    <div className="p-2.5">
      <div className="mb-3 rounded-[10px] border border-[var(--kit-line)] bg-white/[0.02] p-2.5">
        <div className="mb-2 text-[12px] font-semibold">Cross Dissolve</div>
        <div className="flex h-7 items-center gap-1.5 text-[12px]">
          <span className="w-[84px] shrink-0 text-white/65">Duration</span>
          <NumberField value={Math.round(tr.dur * fps)} step={1} unit="f"
            onChange={(v) => edit((d) => { const x = d.transitions.find((q) => q.id === tr.id); if (x) x.dur = Math.min(max, Math.max(1, Math.round(v))) / fps }, `tr:${tr.id}`)} />
          <span className="ml-auto text-[11px] text-white/40">max {max} f</span>
        </div>
        <p className="mt-2 text-[11px] leading-snug text-white/45">Eased crossfade centred on the cut, using spare frames past each clip's handle. New transitions use the length set in Project Settings.</p>
        <button type="button" onClick={() => edit((d) => { d.transitions = d.transitions.filter((x) => x.id !== tr.id) })}
          className="mt-2 rounded-[8px] px-2.5 py-1 text-[12px] text-[var(--kit-danger)] hover:bg-white/10">Remove transition</button>
      </div>
    </div>
  )
}
