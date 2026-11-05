import { liveTransitions, mixAt, propsOf, valueAt, type Clip, type Fx, type Project } from './model'

export type Source = HTMLVideoElement | HTMLImageElement
/** For a clip: its drawable source, 'wait' if a video is still seeking, or null to skip it. */
export type SourceOf = (c: Clip) => Source | 'wait' | null

// Curves become SVG <feComponentTransfer> filters that the canvas references with ctx.filter = 'url(#…)'.
let host: SVGSVGElement | null = null
const written = new Map<string, string>()
function curvesFilter(clipId: string, f: Extract<Fx, { id: 'curves' }>): string {
  const id = `dio-curves-${clipId}`
  const key = JSON.stringify(f)
  if (written.get(id) !== key) {
    if (!host) {
      host = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
      host.setAttribute('width', '0'); host.setAttribute('height', '0')
      host.style.position = 'absolute'
      document.body.appendChild(host)
    }
    document.getElementById(id)?.remove()
    const t = (a: number[]) => a.map((v) => v.toFixed(3)).join(' ')
    host.insertAdjacentHTML('beforeend',
      `<filter id="${id}" color-interpolation-filters="sRGB">` +
      `<feComponentTransfer><feFuncR type="table" tableValues="${t(f.r)}"/><feFuncG type="table" tableValues="${t(f.g)}"/><feFuncB type="table" tableValues="${t(f.b)}"/></feComponentTransfer>` +
      `<feComponentTransfer><feFuncR type="table" tableValues="${t(f.rgb)}"/><feFuncG type="table" tableValues="${t(f.rgb)}"/><feFuncB type="table" tableValues="${t(f.rgb)}"/></feComponentTransfer>` +
      `</filter>`)
    written.set(id, key)
  }
  return `url(#${id})`
}

/** CSS filter string for a clip's visual effects ('none' if it has none). */
function filterOf(c: Clip): string {
  const parts: string[] = []
  for (const f of c.fx ?? []) {
    if (f.id === 'blur' && f.amount > 0) parts.push(`blur(${f.amount}px)`)
    if (f.id === 'hsl') parts.push(`hue-rotate(${f.hue}deg) saturate(${f.sat}%) brightness(${f.light}%)`)
    if (f.id === 'curves') parts.push(curvesFilter(c.id, f))
  }
  return parts.join(' ') || 'none'
}

export function textFont(c: Clip): string {
  const s = c.text!
  return `${s.italic ? 'italic ' : ''}${s.bold ? 'bold ' : ''}${s.size}px "${s.font}", sans-serif`
}

/** Draw a text clip's lines centred on the origin. */
function drawText(g: CanvasRenderingContext2D, c: Clip): void {
  const s = c.text!
  const lines = s.content.split('\n')
  g.font = textFont(c)
  g.fillStyle = s.color
  g.textBaseline = 'alphabetic'
  const lh = s.size * 1.2
  const widths = lines.map((l) => g.measureText(l).width)
  const box = Math.max(...widths, 1)
  const x0 = s.align === 'left' ? -box / 2 : s.align === 'right' ? box / 2 : 0
  g.textAlign = s.align
  lines.forEach((l, i) => {
    const y = -((lines.length - 1) * lh) / 2 + i * lh + s.size * 0.35
    g.fillText(l, x0, y)
    if (s.underline) {
      const w = widths[i]
      const lx = s.align === 'left' ? x0 : s.align === 'right' ? x0 - w : -w / 2
      g.fillRect(lx, y + s.size * 0.1, w, Math.max(1, s.size / 16))
    }
  })
}

/** Draw one clip at time t: fit to frame, then its position/scale/rotation/opacity (× `weight`) and effects. */
function drawLayer(g: CanvasRenderingContext2D, c: Clip, src: Source | null, t: number, W: number, H: number, weight: number): void {
  const pr = propsOf(c)
  const st = c.in + (t - c.start)
  const sx = valueAt(pr.scale, st) / 100
  const sy = pr.lock ? sx : valueAt(pr.scaleY, st) / 100
  g.save()
  g.translate(W / 2 + valueAt(pr.x, st), H / 2 + valueAt(pr.y, st))
  g.rotate((valueAt(pr.rot, st) * Math.PI) / 180)
  g.scale(sx, sy)
  g.globalAlpha *= Math.min(1, Math.max(0, valueAt(pr.opacity, st) / 100)) * weight
  g.filter = filterOf(c)
  if (src) {
    const sw = src instanceof HTMLVideoElement ? src.videoWidth : src.naturalWidth
    const sh = src instanceof HTMLVideoElement ? src.videoHeight : src.naturalHeight
    const f = Math.min(W / sw, H / sh)
    g.drawImage(src, (-sw * f) / 2, (-sh * f) / 2, sw * f, sh * f)
  } else drawText(g, c)
  g.restore()
}

let scratch: HTMLCanvasElement | null = null

/**
 * Composite the video tracks at time `t` (V1 first). A track mid-transition blends both clips additively
 * (weights 1−e and e) in a scratch canvas, so a dissolve between opaque shots never dips in brightness.
 * Returns false (drawing nothing) while a video layer is still seeking, so callers can hold the previous frame.
 */
export function renderFrame(g: CanvasRenderingContext2D, p: Project, t: number, sourceOf: SourceOf): boolean {
  const { width: W, height: H } = p.settings
  const live = liveTransitions(p)
  const tracks: { c: Clip; w: number; src: Source | null }[][] = []
  for (const tr of p.video) {
    if (tr.hidden) continue
    const layer: { c: Clip; w: number; src: Source | null }[] = []
    for (const { c, w } of mixAt(p, tr.id, t, live)) {
      if (c.text) { layer.push({ c, w, src: null }); continue }
      const src = sourceOf(c)
      if (src === 'wait') return false
      if (src) layer.push({ c, w, src })
    }
    if (layer.length) tracks.push(layer)
  }
  g.save()
  g.setTransform(1, 0, 0, 1, 0, 0)
  g.globalAlpha = 1
  g.filter = 'none'
  g.fillStyle = '#000'
  g.fillRect(0, 0, W, H)
  for (const layer of tracks) {
    if (layer.length === 1) { drawLayer(g, layer[0].c, layer[0].src, t, W, H, layer[0].w); continue }
    scratch ??= document.createElement('canvas')
    if (scratch.width !== W || scratch.height !== H) { scratch.width = W; scratch.height = H }
    const s = scratch.getContext('2d')!
    s.clearRect(0, 0, W, H)
    s.globalCompositeOperation = 'lighter'
    for (const l of layer) drawLayer(s, l.c, l.src, t, W, H, l.w)
    s.globalCompositeOperation = 'source-over'
    g.drawImage(scratch, 0, 0)
  }
  g.restore()
  return true
}
