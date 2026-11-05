import { useEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import { ContextMenu, Dialog, RenderProgress, formatTC } from '../../../components'
import { exportProject, pickCodecs } from './export'
import { download, loadDocument, newDocument, offlineMedia, relink, saveDocument } from './files'
import { projectEnd, type Settings } from './model'
import { doc, edit, history, store, useProject } from './store'

type Open = null | 'new' | 'settings' | 'export' | 'relink' | { error: string }

const PRESETS: [string, number, number][] = [
  ['1080p (1920×1080)', 1920, 1080], ['720p (1280×720)', 1280, 720], ['4K UHD (3840×2160)', 3840, 2160],
  ['Vertical 1080×1920', 1080, 1920], ['Square 1080×1080', 1080, 1080]
]
const FPS = [24, 25, 30, 50, 60]

const btn = 'h-8 rounded-[8px] px-3 text-[13px] font-semibold'
const primary = `${btn} bg-[var(--kit-accent)] text-[var(--kit-ink)] hover:brightness-110 disabled:opacity-40`
const ghost = `${btn} text-white/75 hover:bg-white/10`
const field = 'h-8 rounded-[8px] border border-[var(--kit-line)] bg-[#15171c] px-2 text-[13px] outline-none focus:border-[var(--kit-accent)]'

/** Header File menu (New, Save, Load, Project Settings, Export) plus Ctrl+S / Ctrl+O / Ctrl+M. */
export function FileMenu() {
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null)
  const [open, setOpen] = useState<Open>(null)
  const edits = useSyncExternalStore(history.sub, history.get)
  const button = useRef<HTMLButtonElement>(null)
  const loader = useRef<HTMLInputElement>(null)

  const fail = (e: unknown): void => { if ((e as Error).name !== 'AbortError') setOpen({ error: (e as Error).message }) }
  const save = (): void => { saveDocument().catch(fail) }
  const afterLoad = (): void => { if (offlineMedia(store.get()).length) setOpen('relink') }
  const load = (): void => {
    if ((window as unknown as { showOpenFilePicker?: unknown }).showOpenFilePicker) loadDocument().then(afterLoad, fail)
    else loader.current?.click()
  }
  const actions = { new: () => (edits.startsWith('0|') ? newDocument() : setOpen('new')), save, load, settings: () => setOpen('settings'), export: () => setOpen('export') }

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (!(e.ctrlKey || e.metaKey)) return
      const k = e.key.toLowerCase()
      if (k === 's') { e.preventDefault(); actions.save() }
      if (k === 'o') { e.preventDefault(); actions.load() }
      if (k === 'm') { e.preventDefault(); actions.export() }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  return (
    <>
      <button ref={button} type="button" aria-haspopup="menu" aria-expanded={!!menu}
        onClick={() => { const r = button.current!.getBoundingClientRect(); setMenu({ x: r.left, y: r.bottom + 4 }) }}
        className="h-7 rounded-[7px] px-2.5 text-[12px] text-white/80 hover:bg-white/10">File</button>
      <input ref={loader} type="file" accept=".dio,application/json" className="hidden"
        onChange={(e) => { const f = e.target.files?.[0]; e.target.value = ''; if (f) loadDocument(f).then(afterLoad, fail) }} />
      <ContextMenu at={menu} onClose={() => setMenu(null)} width={220} items={[
        { label: 'New', onSelect: actions.new },
        { label: 'Save', keys: ['Ctrl', 'S'], onSelect: actions.save },
        { label: 'Load…', keys: ['Ctrl', 'O'], onSelect: actions.load },
        'separator',
        { label: 'Project Settings…', onSelect: actions.settings },
        { label: 'Export…', keys: ['Ctrl', 'M'], onSelect: actions.export }
      ]} />

      <Dialog open={open === 'new'} title="Start a new project?" onClose={() => setOpen(null)} width={380}
        footer={<><button type="button" className={ghost} onClick={() => setOpen(null)}>Cancel</button><button type="button" className={primary} onClick={() => { newDocument(); setOpen(null) }}>Discard and start new</button></>}>
        Unsaved changes to <b>{doc.get()}</b> will be lost.
      </Dialog>
      <SettingsDialog open={open === 'settings'} onClose={() => setOpen(null)} />
      <ExportDialog open={open === 'export'} onClose={() => setOpen(null)} />
      <RelinkDialog open={open === 'relink'} onClose={() => setOpen(null)} />
      <Dialog open={typeof open === 'object' && open !== null} title="Something went wrong" onClose={() => setOpen(null)} width={380}
        footer={<button type="button" className={primary} onClick={() => setOpen(null)}>OK</button>}>
        {typeof open === 'object' && open?.error}
      </Dialog>
    </>
  )
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return <label className="mb-3 flex items-center gap-3"><span className="w-[130px] shrink-0 text-white/60">{label}</span>{children}</label>
}

function SettingsDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const p = useProject()
  const [s, setS] = useState<Settings>(p.settings)
  useEffect(() => { if (open) setS(store.get().settings) }, [open])
  const even = (n: number): number => Math.max(2, Math.round(n / 2) * 2) // H.264 needs even sizes
  const preset = PRESETS.findIndex(([, w, h]) => w === s.width && h === s.height)
  return (
    <Dialog open={open} title="Project Settings" onClose={onClose}
      footer={<><button type="button" className={ghost} onClick={onClose}>Cancel</button><button type="button" className={primary} onClick={() => { edit((d) => { d.settings = { ...s, width: even(s.width), height: even(s.height) } }); onClose() }}>Save</button></>}>
      <Row label="Frame size">
        <select className={`${field} flex-1`} value={preset} onChange={(e) => { const [, w, h] = PRESETS[+e.target.value] ?? []; if (w) setS({ ...s, width: w, height: h }) }}>
          {PRESETS.map(([n], i) => <option key={n} value={i}>{n}</option>)}
          {preset < 0 && <option value={-1}>Custom</option>}
        </select>
      </Row>
      <Row label="Custom size">
        <input className={`${field} w-24`} type="number" min={2} step={2} value={s.width} onChange={(e) => setS({ ...s, width: +e.target.value })} aria-label="Width" />
        <span className="text-white/40">×</span>
        <input className={`${field} w-24`} type="number" min={2} step={2} value={s.height} onChange={(e) => setS({ ...s, height: +e.target.value })} aria-label="Height" />
      </Row>
      <Row label="Frame rate">
        <select className={`${field} w-28`} value={s.fps} onChange={(e) => setS({ ...s, fps: +e.target.value })}>
          {FPS.map((f) => <option key={f} value={f}>{f} fps</option>)}
        </select>
      </Row>
      <Row label="Default transition">
        <input className={`${field} w-24`} type="number" min={1} value={s.transFrames} onChange={(e) => setS({ ...s, transFrames: Math.max(1, +e.target.value) })} aria-label="Default transition length in frames" />
        <span className="text-white/45">frames ({(s.transFrames / s.fps).toFixed(2)} s)</span>
      </Row>
    </Dialog>
  )
}

const BITRATES = [4, 8, 12, 16, 25, 40]

function ExportDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const p = useProject()
  const [name, setName] = useState('')
  const [mbps, setMbps] = useState(10)
  const [state, setState] = useState<null | { progress: number; detail: string; done?: boolean; error?: string }>(null)
  const abort = useRef<AbortController | null>(null)
  const running = !!state && !state.done && !state.error
  useEffect(() => { if (open) { setName(doc.get()); setState(null); setMbps(p.settings.height >= 2000 ? 40 : p.settings.height >= 1000 ? 12 : 8) } }, [open]) // eslint-disable-line react-hooks/exhaustive-deps

  const range = p.range.out > p.range.in ? p.range : { in: 0, out: projectEnd(p) }
  const len = range.out - range.in

  const start = async (): Promise<void> => {
    const proj = { ...store.get(), range }
    const codecs = await pickCodecs(proj, mbps * 1e6)
    if (typeof codecs === 'string') { setState({ progress: 0, detail: '', error: codecs }); return }
    const file = `${name || 'Untitled'}.mp4`
    let writable: FileSystemWritableFileStream | undefined
    const picker = (window as unknown as { showSaveFilePicker?: (o: object) => Promise<FileSystemFileHandle> }).showSaveFilePicker
    try {
      if (picker) writable = await (await picker({ suggestedName: file, types: [{ description: 'MP4 video', accept: { 'video/mp4': ['.mp4'] } }] })).createWritable()
    } catch { return } // picker cancelled
    abort.current = new AbortController()
    setState({ progress: 0, detail: 'Starting…' })
    try {
      const blob = await exportProject(proj, codecs, { bitrate: mbps * 1e6, signal: abort.current.signal, writable, onProgress: (f, d) => setState({ progress: f, detail: d }) })
      if (blob) download(blob, file)
      setState({ progress: 1, detail: `Saved ${file}`, done: true })
    } catch (e) {
      await writable?.abort().catch(() => {})
      const err = e as Error
      setState({ progress: 0, detail: '', error: err.name === 'AbortError' ? 'Export cancelled.' : err.message })
    }
  }

  return (
    <Dialog open={open} title="Export" onClose={onClose} locked={running} width={480}
      footer={!state || state.error ? <><button type="button" className={ghost} onClick={onClose}>Close</button><button type="button" className={primary} disabled={len <= 0} onClick={() => void start()}>Export</button></> : state.done ? <button type="button" className={primary} onClick={onClose}>Done</button> : undefined}>
      {state && !state.error ? (
        <RenderProgress progress={state.progress} title={`Exporting ${name || 'Untitled'}.mp4`} detail={state.detail} done={state.done} onCancel={running ? () => abort.current?.abort() : undefined} />
      ) : (
        <>
          <Row label="File name">
            <input className={`${field} flex-1`} value={name} onChange={(e) => setName(e.target.value)} aria-label="File name" />
            <span className="text-white/45">.mp4</span>
          </Row>
          <Row label="Range"><span>In {formatTC(range.in, p.settings.fps)} → Out {formatTC(range.out, p.settings.fps)} ({formatTC(len, p.settings.fps)})</span></Row>
          <Row label="Video"><span>H.264 · {p.settings.width}×{p.settings.height} · {p.settings.fps} fps</span></Row>
          <Row label="Video bitrate">
            <select className={`${field} w-32`} value={mbps} onChange={(e) => setMbps(+e.target.value)}>
              {[...new Set([...BITRATES, mbps])].sort((x, y) => x - y).map((b) => <option key={b} value={b}>{b} Mbps</option>)}
            </select>
          </Row>
          <Row label="Audio"><span>AAC · 128 kbps · stereo 48 kHz</span></Row>
          {state?.error && <p className="mt-1 rounded-[8px] bg-[var(--kit-danger)]/15 px-3 py-2 text-[var(--kit-danger)]">{state.error}</p>}
          {offlineMedia(p).length > 0 && <p className="mt-1 text-[12px] text-[var(--kit-busy)]">{offlineMedia(p).length} media file(s) are offline and will be left out.</p>}
        </>
      )}
    </Dialog>
  )
}

function RelinkDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const p = useProject()
  const input = useRef<HTMLInputElement>(null)
  const [busy, setBusy] = useState(false)
  const missing = offlineMedia(p)
  useEffect(() => { if (open && !missing.length) onClose() })
  return (
    <Dialog open={open} title="Relink media" onClose={onClose} width={460}
      footer={<><button type="button" className={ghost} onClick={onClose}>Later</button><button type="button" className={primary} disabled={busy} onClick={() => input.current?.click()}>{busy ? 'Reading…' : 'Locate files…'}</button></>}>
      <p className="mb-2 text-white/65">This project uses {missing.length} file(s) that aren't loaded yet. Pick them (you can select several at once); they're matched by file name.</p>
      <ul className="max-h-48 overflow-y-auto rounded-[8px] bg-black/25 p-2 font-mono text-[12px] thin-scroll">
        {missing.map((m) => <li key={m.id} className="truncate py-0.5 text-[var(--kit-busy)]">{m.name}</li>)}
      </ul>
      <input ref={input} type="file" multiple className="hidden" accept="video/*,audio/*,image/*"
        onChange={async (e) => { const fs = [...(e.target.files ?? [])]; e.target.value = ''; setBusy(true); await relink(fs); setBusy(false) }} />
    </Dialog>
  )
}
