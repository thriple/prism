import { frame, liveTransitions, mediaOf, mixAt, playSpan, projectEnd, propsOf, trackOf, valueAt, type Clip, type Project } from './model'
import { renderFrame } from './render'
import { clock, runtime, store } from './store'

/**
 * Playback: a wall-clock-driven playhead, one media element per clip kept in sync with it,
 * a canvas compositor for the video tracks, and a Web Audio graph (per-clip gain → master → meters).
 */
class Engine {
  playing = false
  rate = 1
  loop = false
  canvas: HTMLCanvasElement | null = null
  private t0 = 0
  private w0 = 0
  private els = new Map<string, HTMLMediaElement>()
  private ctx: AudioContext | null = null
  private gains = new Map<string, GainNode>()
  private master: GainNode | null = null
  private meters: AnalyserNode[] = []
  private buf = new Float32Array(2048)
  private listeners = new Set<() => void>()

  constructor() {
    const tick = (now: number): void => { this.tick(now); requestAnimationFrame(tick) }
    requestAnimationFrame(tick)
  }

  /** Subscribe to play/pause/rate changes. */
  on(f: () => void): () => void { this.listeners.add(f); return () => { this.listeners.delete(f) } }
  private emit(): void { this.listeners.forEach((f) => f()) }

  play(rate = 1): void {
    this.ensureAudio()
    const p = store.get()
    const stop = this.stopAt(p)
    if (rate > 0 && clock.get() >= stop - 1e-3) clock.set(this.loop ? p.range.in : 0)
    this.rate = rate
    this.t0 = clock.get(); this.w0 = performance.now()
    this.playing = true
    this.emit()
  }
  pause(): void {
    this.playing = false
    this.rate = 1
    for (const el of this.els.values()) el.pause()
    clock.set(frame(clock.get(), store.get().settings.fps))
    this.emit()
  }
  setLoop(on: boolean): void { this.loop = on; this.emit() }
  toggle(): void { if (this.playing) this.pause(); else this.play(1) }
  /** J/K/L: each press in the same direction doubles the speed (1, 2, 4, 8×); K stops. */
  shuttle(dir: -1 | 0 | 1): void {
    if (dir === 0) return this.pause()
    const same = this.playing && Math.sign(this.rate) === dir
    this.play(same ? Math.sign(this.rate) * Math.min(8, Math.abs(this.rate) * 2) : dir)
  }
  seek(t: number): void {
    const p = store.get()
    clock.set(Math.max(0, Math.min(frame(t, p.settings.fps), Math.max(projectEnd(p), p.range.out))))
    if (this.playing) { this.t0 = clock.get(); this.w0 = performance.now() }
  }
  step(frames: number): void { if (this.playing) this.pause(); this.seek(clock.get() + frames / store.get().settings.fps) }

  /** Peak levels (L, R), 0–1, for the meter. */
  levels = (): number[] => this.meters.length
    ? this.meters.map((a) => { a.getFloatTimeDomainData(this.buf); let m = 0; for (const v of this.buf) { const x = v < 0 ? -v : v; if (x > m) m = x } return m })
    : [0, 0]

  private stopAt(p: Project): number {
    return this.loop && p.range.out > p.range.in ? p.range.out : projectEnd(p)
  }

  private ensureAudio(): void {
    if (this.ctx) { void this.ctx.resume(); return }
    const ctx = new AudioContext()
    this.ctx = ctx
    this.master = ctx.createGain()
    this.master.channelCount = 2 // mono sources play (and meter) on both sides
    this.master.channelCountMode = 'explicit'
    const split = ctx.createChannelSplitter(2)
    this.master.connect(ctx.destination)
    this.master.connect(split)
    this.meters = [0, 1].map((i) => { const a = ctx.createAnalyser(); a.fftSize = 2048; split.connect(a, i); return a })
    for (const [id, el] of this.els) if (el instanceof HTMLAudioElement) this.connect(id, el)
  }
  private connect(id: string, el: HTMLMediaElement): void {
    if (!this.ctx || this.gains.has(id)) return
    const g = this.ctx.createGain()
    this.ctx.createMediaElementSource(el).connect(g)
    g.connect(this.master!)
    this.gains.set(id, g)
  }

  private element(c: Clip, kind: 'video' | 'audio'): HTMLMediaElement | null {
    let el = this.els.get(c.id)
    if (el) return el
    const rt = runtime.get(c.mediaId)
    if (!rt) return null
    el = document.createElement(kind)
    el.preload = 'auto'
    el.src = rt.url
    if (kind === 'video') el.muted = true // sound comes from the linked audio clip
    this.els.set(c.id, el)
    if (kind === 'audio') this.connect(c.id, el)
    return el
  }

  private tick(now: number): void {
    const p = store.get()
    const fps = p.settings.fps
    if (this.playing) {
      let t = this.t0 + ((now - this.w0) / 1000) * this.rate
      const stop = this.stopAt(p)
      if (t >= stop) {
        if (this.loop && p.range.out > p.range.in) { t = p.range.in; this.t0 = t; this.w0 = now } else { clock.set(stop); this.pause(); return this.draw(p) }
      }
      if (t <= 0 && this.rate < 0) { clock.set(0); this.pause(); return this.draw(p) }
      clock.set(t)
    }
    this.sync(p, clock.get(), fps)
    this.draw(p)
  }

  /** Keep every clip's element at the right source time; play the active ones while playing at 0–2×. */
  private sync(p: Project, t: number, fps: number): void {
    const ids = new Set(p.clips.map((c) => c.id))
    for (const [id, el] of this.els) if (!ids.has(id)) { el.pause(); el.removeAttribute('src'); this.els.delete(id); this.gains.get(id)?.disconnect(); this.gains.delete(id) }
    const smooth = this.playing && this.rate > 0 && this.rate <= 2
    const live = liveTransitions(p)
    // Crossfade weights of the audio clips sounding now (1 outside transitions).
    const weight = new Map<string, number>()
    for (const tr of p.audio) for (const { c, w } of mixAt(p, tr.id, t, live)) weight.set(c.id, w)
    for (const c of p.clips) {
      const m = mediaOf(p, c.mediaId), tr = trackOf(p, c.trackId)
      if (!m || !tr || m.kind === 'image') continue
      const [s0, s1] = playSpan(c, live) // includes frames borrowed by transitions
      const active = s0 <= t && t < s1
      const el = active || this.els.has(c.id) ? this.element(c, tr.kind) : null
      if (!el) continue
      const g = this.gains.get(c.id)
      if (g) {
        // Clip volume in dB (keyframable); −60 dB and below is silence.
        const db = valueAt(propsOf(c).volume, c.in + (t - c.start))
        g.gain.value = tr.muted || db <= -60 ? 0 : 10 ** (db / 20) * (weight.get(c.id) ?? 1)
      }
      if (!active) { if (!el.paused) el.pause(); continue }
      const want = c.in + (t - c.start)
      if (smooth) {
        el.playbackRate = this.rate
        if (el.paused) { el.currentTime = want; void el.play().catch(() => {}) } else if (Math.abs(el.currentTime - want) > 0.2) el.currentTime = want
      } else {
        if (!el.paused) el.pause()
        if (Math.abs(el.currentTime - want) > 0.5 / fps && !el.seeking) el.currentTime = want
      }
    }
  }

  /** Composite into the project-sized canvas (see render.ts); holds the last frame while videos seek. */
  private draw(p: Project): void {
    const cv = this.canvas
    if (!cv) return
    const { width: W, height: H } = p.settings
    if (cv.width !== W || cv.height !== H) { cv.width = W; cv.height = H }
    renderFrame(cv.getContext('2d')!, p, clock.get(), (c) => {
      const rt = runtime.get(c.mediaId)
      if (rt?.img) return rt.img
      const v = this.els.get(c.id) as HTMLVideoElement | undefined
      if (!v) return null
      return v.readyState < 2 || v.seeking ? 'wait' : v.videoWidth ? v : null
    })
  }
}

export const engine = new Engine()
