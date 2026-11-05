/**
 * One aurora for the whole page, pinned to the viewport. It is drawn once per frame into a small
 * offscreen canvas (1/8 of the window: it is meant to be blurry, and upscaling does the blurring).
 * Every glass cutout owns a little canvas and copies the part of the aurora that sits behind it,
 * so all cutouts look through to the same layer, and still line up while they move or scale.
 * Colours come from --soft-aurora-base and --soft-aurora-1..5; theme changes ease across.
 */
type RGB = [number, number, number]

// Blob paths: centre ± amplitude, slow incommensurate periods (seconds) so it never visibly loops.
const BLOBS = [
  { cx: .20, cy: .25, ax: .20, ay: .18, px: 37, py: 53, r: .30 },
  { cx: .70, cy: .20, ax: .22, ay: .20, px: 47, py: 31, r: .28 },
  { cx: .50, cy: .70, ax: .30, ay: .18, px: 59, py: 41, r: .34 },
  { cx: .15, cy: .75, ax: .16, ay: .22, px: 43, py: 67, r: .26 },
  { cx: .85, cy: .65, ax: .14, ay: .26, px: 71, py: 49, r: .28 },
  { cx: .45, cy: .35, ax: .32, ay: .24, px: 53, py: 61, r: .22 },
  { cx: .80, cy: .90, ax: .24, ay: .14, px: 67, py: 29, r: .24 },
]

const wins = new Set<HTMLCanvasElement>()
let master: HTMLCanvasElement | null = null
let raf = 0, last = 0, t = 0
let cur: RGB[] = [], target: RGB[] = []

function parse(color: string): RGB {
  const c = document.createElement('canvas').getContext('2d')!
  c.fillStyle = '#000'; c.fillStyle = color.trim() || '#000'
  const h = c.fillStyle as string // normalised to #rrggbb (or rgba() if translucent)
  if (h[0] === '#') return [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16)]
  const m = h.match(/[\d.]+/g) ?? ['0', '0', '0']
  return [+m[0], +m[1], +m[2]]
}

function readColors() {
  const s = getComputedStyle(document.documentElement)
  target = ['base', 1, 2, 3, 4, 5].map(k => parse(s.getPropertyValue(`--soft-aurora-${k}`)))
  if (!cur.length) cur = target.map(c => [...c] as RGB)
}

function setup() {
  master = document.createElement('canvas')
  readColors()
  // Theme switches are attribute changes on <html>; re-read the colours and ease to them.
  new MutationObserver(readColors).observe(document.documentElement, { attributes: true })
}

function drawMaster() {
  const m = master!, w = Math.max(64, Math.round(innerWidth / 8)), h = Math.max(48, Math.round(innerHeight / 8))
  if (m.width !== w) m.width = w
  if (m.height !== h) m.height = h
  const g = m.getContext('2d')!
  for (let i = 0; i < cur.length; i++) for (let j = 0; j < 3; j++) cur[i][j] += (target[i][j] - cur[i][j]) * 0.06
  const [base, ...cols] = cur
  g.globalCompositeOperation = 'source-over'
  g.fillStyle = `rgb(${base.join(',')})`
  g.fillRect(0, 0, w, h)
  const size = Math.max(w, h), TAU = Math.PI * 2
  BLOBS.forEach((b, i) => {
    const x = (b.cx + b.ax * Math.sin(TAU * t / b.px + i)) * w
    const y = (b.cy + b.ay * Math.cos(TAU * t / b.py + i * 2)) * h
    const r = b.r * size * (0.85 + 0.15 * Math.sin(TAU * t / (b.px * 0.7) + i)) // breathing
    const c = cols[i % cols.length].map(Math.round).join(',')
    const grad = g.createRadialGradient(x, y, 0, x, y, r)
    grad.addColorStop(0, `rgba(${c},.95)`)
    grad.addColorStop(0.5, `rgba(${c},.45)`)
    grad.addColorStop(1, `rgba(${c},0)`)
    g.fillStyle = grad
    g.fillRect(0, 0, w, h)
  })
}

function blit(c: HTMLCanvasElement) {
  const r = c.getBoundingClientRect()
  if (r.width < 1 || r.height < 1 || r.bottom < 0 || r.top > innerHeight) return
  const w = Math.ceil(r.width / 2), h = Math.ceil(r.height / 2) // half res: it's frosted anyway
  if (c.width !== w) c.width = w
  if (c.height !== h) c.height = h
  const m = master!, sx = m.width / innerWidth, sy = m.height / innerHeight
  const g = c.getContext('2d')!
  g.imageSmoothingEnabled = true
  g.imageSmoothingQuality = 'high'
  g.drawImage(m, r.left * sx, r.top * sy, r.width * sx, r.height * sy, 0, 0, w, h)
}

function frame(now: number) {
  if (!wins.size) { raf = 0; return }
  const still = matchMedia('(prefers-reduced-motion: reduce)').matches
  if (!still) t += Math.min(now - last, 100) / 1000
  last = now
  drawMaster()
  wins.forEach(blit)
  raf = requestAnimationFrame(frame)
}

/** Register a cutout's canvas. Returns the unregister function. */
export function watchAurora(canvas: HTMLCanvasElement): () => void {
  if (!master) setup()
  wins.add(canvas)
  if (!raf) { last = performance.now(); raf = requestAnimationFrame(frame) }
  return () => { wins.delete(canvas) }
}
