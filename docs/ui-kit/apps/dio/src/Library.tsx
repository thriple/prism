import { useRef, useState } from 'react'
import { RingThink, formatTC } from '../../../components'
import { offlineMedia, relink } from './files'
import { dragMedia, edit, importFile, runtime, useProject } from './store'

const KIND_LABEL = { video: 'Video', audio: 'Audio', image: 'Image' } as const

/** Project Library: import videos, images and audio (button or drop files here); drag items to the timeline. */
export function Library() {
  const p = useProject()
  const input = useRef<HTMLInputElement>(null)
  const [busy, setBusy] = useState(0)
  const [errors, setErrors] = useState<string[]>([])

  const add = async (files: FileList | File[]): Promise<void> => {
    const list = [...files]
    setBusy((n) => n + list.length)
    for (const f of list) {
      try {
        const m = await importFile(f)
        edit((d) => { d.media.push(m) })
      } catch (e) {
        setErrors((x) => [...x, (e as Error).message])
      }
      setBusy((n) => n - 1)
    }
  }

  return (
    <div
      className="flex h-full flex-col"
      onDragOver={(e) => { if (e.dataTransfer.types.includes('Files')) e.preventDefault() }}
      onDrop={(e) => { if (e.dataTransfer.files.length) { e.preventDefault(); void add(e.dataTransfer.files) } }}
    >
      <div className="flex h-10 shrink-0 items-center gap-2 border-b border-[var(--kit-line)] px-2.5">
        <button type="button" onClick={() => input.current?.click()} className="h-7 rounded-[8px] bg-white/10 px-2.5 text-[12px] font-semibold hover:bg-white/15">Import…</button>
        <input ref={input} type="file" multiple accept="video/*,audio/*,image/*" className="hidden" onChange={(e) => { if (e.target.files) void add(e.target.files); e.target.value = '' }} />
        {busy > 0 && <span className="flex items-center gap-2 text-[12px] text-white/60"><RingThink size={18} /> Reading {busy}…</span>}
        {offlineMedia(p).length > 0 && (
          <label className="h-7 cursor-pointer rounded-[8px] bg-[var(--kit-busy)]/20 px-2.5 text-[12px] font-semibold leading-7 text-[var(--kit-busy)] hover:bg-[var(--kit-busy)]/30">
            Relink {offlineMedia(p).length}…
            <input type="file" multiple accept="video/*,audio/*,image/*" className="hidden" onChange={(e) => { const fs = [...(e.target.files ?? [])]; e.target.value = ''; setBusy((n) => n + 1); void relink(fs).finally(() => setBusy((n) => n - 1)) }} />
          </label>
        )}
        <span className="ml-auto text-[11px] text-white/40">{p.media.length} items</span>
      </div>
      <div className="min-h-0 flex-1 overflow-auto p-2 thin-scroll">
        {errors.map((er, i) => (
          <div key={i} className="mb-1.5 flex items-start gap-2 rounded-[8px] bg-[var(--kit-danger)]/15 px-2 py-1.5 text-[12px] text-[var(--kit-danger)]">
            <span className="flex-1">{er}</span>
            <button type="button" aria-label="Dismiss" onClick={() => setErrors((x) => x.filter((_, j) => j !== i))}>×</button>
          </div>
        ))}
        {!p.media.length && !busy && (
          <div className="grid h-full min-h-[120px] place-items-center rounded-[10px] border border-dashed border-white/15 p-4 text-center text-[12px] text-white/45">
            Drop videos, images or audio here, or use Import.
          </div>
        )}
        <div className="grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-2">
          {p.media.map((m) => {
            const rt = runtime.get(m.id)
            return (
              <div
                key={m.id}
                draggable
                onDragStart={(e) => { dragMedia.id = m.id; e.dataTransfer.setData('text/plain', m.name); e.dataTransfer.effectAllowed = 'copy' }}
                onDragEnd={() => { dragMedia.id = null }}
                className="cursor-grab overflow-hidden rounded-[8px] border border-[var(--kit-line)] bg-white/[0.03] hover:border-white/30"
                title={m.name}
              >
                <div className="relative grid aspect-video place-items-center bg-black/50">
                  {rt?.thumb ? <img src={rt.thumb} alt="" draggable={false} className="h-full w-full object-cover" /> : <span className="text-[22px] text-[var(--kit-ok)]">♪</span>}
                  <span className="absolute bottom-1 right-1 rounded-[4px] bg-black/70 px-1 font-mono text-[9px]">{m.kind === 'image' ? `${m.width}×${m.height}` : formatTC(m.duration, p.settings.fps)}</span>
                </div>
                <div className="flex items-center gap-1.5 px-1.5 py-1 text-[11px]">
                  <span className="truncate">{m.name}</span>
                  <span className={`ml-auto shrink-0 ${rt ? 'text-white/40' : 'font-semibold text-[var(--kit-busy)]'}`}>{rt ? `${KIND_LABEL[m.kind]}${m.kind === 'video' && !m.hasAudio ? ' · silent' : ''}` : 'Offline'}</span>
                </div>
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}
