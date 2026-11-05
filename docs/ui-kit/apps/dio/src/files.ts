// Project files (.dio): JSON of the project. Media files aren't embedded; after Load they're relinked by name.
import { newProject, type Media, type Project } from './model'
import { clearRuntime, doc, importFile, resetHistory, runtime, store } from './store'

type Handle = { name: string; createWritable: () => Promise<{ write: (d: string) => Promise<void>; close: () => Promise<void> }>; getFile?: () => Promise<File> }
type Picker = {
  showSaveFilePicker?: (o: object) => Promise<Handle>
  showOpenFilePicker?: (o: object) => Promise<Handle[]>
}
const win = window as unknown as Picker
const TYPES = [{ description: 'Dio project', accept: { 'application/json': ['.dio'] } }]
let handle: Handle | null = null

export function newDocument(): void {
  clearRuntime()
  store.set(newProject())
  resetHistory()
  handle = null
  doc.set('Untitled')
}

/** Save: to the same file after the first time (File System Access API), else download a .dio file. */
export async function saveDocument(): Promise<void> {
  const data = JSON.stringify({ app: 'dio', version: 1, project: store.get() }, null, 1)
  if (win.showSaveFilePicker) {
    handle ??= await win.showSaveFilePicker({ suggestedName: `${doc.get()}.dio`, types: TYPES })
    const w = await handle.createWritable()
    await w.write(data)
    await w.close()
    doc.set(handle.name.replace(/\.dio$/i, ''))
  } else {
    download(new Blob([data], { type: 'application/json' }), `${doc.get()}.dio`)
  }
}

/** Load a .dio file; returns the media that need relinking. */
export async function loadDocument(file?: File): Promise<Media[]> {
  let f = file
  if (!f && win.showOpenFilePicker) {
    const [h] = await win.showOpenFilePicker({ types: TYPES })
    handle = h
    f = await h.getFile!()
  }
  if (!f) return []
  const json = JSON.parse(await f.text()) as { app?: string; project?: Project }
  if (json.app !== 'dio' || !json.project) throw new Error(`${f.name} isn't a Dio project`)
  const p = { ...newProject(), ...json.project }
  p.settings = { ...newProject().settings, ...p.settings }
  clearRuntime()
  store.set(p)
  resetHistory()
  doc.set(f.name.replace(/\.dio$/i, ''))
  return p.media
}

export const offlineMedia = (p: Project): Media[] => p.media.filter((m) => !runtime.has(m.id))

/** Relink: match picked files to offline media by name (preferring the same size). Returns names still missing. */
export async function relink(files: File[]): Promise<string[]> {
  const p = store.get()
  for (const m of offlineMedia(p)) {
    const f = files.find((x) => x.name === m.name && x.size === m.size) ?? files.find((x) => x.name === m.name)
    if (f) { try { await importFile(f, m.id) } catch { /* unreadable: stays offline */ } }
  }
  store.set({ ...store.get() }) // re-render everything that shows media (not an undo step)
  return offlineMedia(store.get()).map((m) => m.name)
}

export function download(blob: Blob, name: string): void {
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = name
  a.click()
  setTimeout(() => URL.revokeObjectURL(a.href), 5000)
}
