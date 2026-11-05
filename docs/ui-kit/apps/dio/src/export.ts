// Export: H.264 video + AAC 128 kbps audio in an .mp4, rendered frame by frame from the In–Out range.
import { ArrayBufferTarget, FileSystemWritableFileStreamTarget, Muxer } from 'mp4-muxer'
import { liveTransitions, mediaOf, mixAt, playSpan, propsOf, trackOf, valueAt, type Clip, type Project } from './model'
import { renderFrame, type Source } from './render'
import { runtime } from './store'

const RATE = 48000
const AUDIO_BITRATE = 128_000
/** Developer switch (?test-codecs): VP9 + Opus, for browsers without H.264/AAC encoders (e.g. Linux Chromium). */
const TEST = new URLSearchParams(location.search).has('test-codecs')

export type ExportOptions = {
  bitrate: number
  signal: AbortSignal
  onProgress: (fraction: number, detail: string) => void
  /** Writable stream from showSaveFilePicker; without it the result comes back as a Blob. */
  writable?: FileSystemWritableFileStream
}

/** H.264 profile/level for the size and frame rate (High profile). */
function avcCodecs(w: number, h: number, fps: number): string[] {
  const mbps = Math.ceil(w / 16) * Math.ceil(h / 16) * fps // macroblocks per second
  const lvl = mbps <= 245760 ? '28' : mbps <= 522240 ? '2a' : mbps <= 983040 ? '33' : '34'
  return [`avc1.6400${lvl}`, 'avc1.640034', `avc1.4d00${lvl}`]
}

/** Which codecs this browser can encode for these settings, or an error message. */
export async function pickCodecs(p: Project, bitrate: number): Promise<{ video: string; audio: string } | string> {
  if (!('VideoEncoder' in window) || !('AudioEncoder' in window)) return 'This browser has no WebCodecs encoders. Use Chrome or Edge.'
  const { width, height, fps } = p.settings
  const videos = TEST ? ['vp09.00.40.08'] : avcCodecs(width, height, fps)
  let video = ''
  for (const c of videos) if ((await VideoEncoder.isConfigSupported({ codec: c, width, height, bitrate, framerate: fps })).supported) { video = c; break }
  const audio = TEST ? 'opus' : 'mp4a.40.2'
  const aok = (await AudioEncoder.isConfigSupported({ codec: audio, sampleRate: RATE, numberOfChannels: 2, bitrate: AUDIO_BITRATE })).supported
  if (!video) return `This browser can't encode H.264 at ${width}×${height}, ${fps} fps.`
  if (!aok) return "This browser can't encode AAC audio."
  return { video, audio }
}

const decoded = new Map<string, Promise<AudioBuffer>>()
const decode = (mediaId: string): Promise<AudioBuffer> => {
  let d = decoded.get(mediaId)
  if (!d) {
    d = runtime.get(mediaId)!.file.arrayBuffer().then((b) => new OfflineAudioContext(2, 1, RATE).decodeAudioData(b))
    decoded.set(mediaId, d)
  }
  return d
}

/** Mix the audio tracks for [a, b): per-clip volume keyframes, crossfades, track mutes. */
async function mixAudio(p: Project, a: number, b: number): Promise<AudioBuffer> {
  const ctx = new OfflineAudioContext(2, Math.max(1, Math.ceil((b - a) * RATE)), RATE)
  const live = liveTransitions(p)
  for (const c of p.clips) {
    const tr = trackOf(p, c.trackId)
    if (!tr || tr.kind !== 'audio' || tr.muted || !runtime.get(c.mediaId)) continue
    const [s0, s1] = playSpan(c, live)
    const x0 = Math.max(s0, a), x1 = Math.min(s1, b)
    if (x1 <= x0) continue
    const src = ctx.createBufferSource()
    src.buffer = await decode(c.mediaId)
    const gain = ctx.createGain()
    // Gain curve at 100 Hz: volume (dB, keyframed) × crossfade weight.
    const n = Math.max(2, Math.ceil((x1 - x0) * 100) + 1)
    const curve = new Float32Array(n)
    for (let i = 0; i < n; i++) {
      const t = x0 + ((x1 - x0) * i) / (n - 1)
      const db = valueAt(propsOf(c).volume, c.in + (t - c.start))
      const w = mixAt(p, c.trackId, Math.min(t, x1 - 1e-6), live).find((m) => m.c.id === c.id)?.w ?? 0
      curve[i] = db <= -60 ? 0 : 10 ** (db / 20) * w
    }
    gain.gain.setValueCurveAtTime(curve, x0 - a, x1 - x0)
    src.connect(gain).connect(ctx.destination)
    src.start(x0 - a, c.in + (x0 - c.start), x1 - x0)
  }
  return ctx.startRendering()
}

/** Render and encode the In–Out range. Resolves with the file (Blob) unless it was written to `writable`. */
export async function exportProject(p: Project, codecs: { video: string; audio: string }, o: ExportOptions): Promise<Blob | null> {
  const { width: W, height: H, fps } = p.settings
  const a = p.range.in, b = p.range.out
  const frames = Math.max(1, Math.round((b - a) * fps))
  const target = o.writable ? new FileSystemWritableFileStreamTarget(o.writable) : new ArrayBufferTarget()
  const muxer = new Muxer({
    target,
    video: { codec: TEST ? 'vp9' : 'avc', width: W, height: H, frameRate: fps },
    audio: { codec: TEST ? 'opus' : 'aac', numberOfChannels: 2, sampleRate: RATE },
    fastStart: o.writable ? false : 'in-memory',
    firstTimestampBehavior: 'offset'
  })
  let failed: Error | null = null
  const venc = new VideoEncoder({ output: (ch, m) => muxer.addVideoChunk(ch, m), error: (e) => { failed = e } })
  venc.configure({ codec: codecs.video, width: W, height: H, bitrate: o.bitrate, framerate: fps, latencyMode: 'quality', ...(TEST ? {} : { avc: { format: 'avc' } }) })
  const aenc = new AudioEncoder({ output: (ch, m) => muxer.addAudioChunk(ch, m), error: (e) => { failed = e } })
  aenc.configure({ codec: codecs.audio, sampleRate: RATE, numberOfChannels: 2, bitrate: AUDIO_BITRATE })
  const els = new Map<string, HTMLVideoElement>()
  const check = (): void => { if (o.signal.aborted) throw new DOMException('Export cancelled', 'AbortError'); if (failed) throw failed }

  try {
    // 1. Audio: mix offline, encode in 1024-sample chunks.
    o.onProgress(0, 'Mixing audio…')
    const mix = await mixAudio(p, a, b)
    check()
    const L = mix.getChannelData(0), R = mix.getChannelData(1)
    for (let i = 0; i < mix.length; i += 1024) {
      const n = Math.min(1024, mix.length - i)
      const data = new Float32Array(n * 2)
      data.set(L.subarray(i, i + n), 0)
      data.set(R.subarray(i, i + n), n)
      const ad = new AudioData({ format: 'f32-planar', sampleRate: RATE, numberOfFrames: n, numberOfChannels: 2, timestamp: Math.round((i / RATE) * 1e6), data })
      aenc.encode(ad)
      ad.close()
    }
    await aenc.flush()
    check()

    // 2. Video: seek every needed clip to the exact frame, composite, encode.
    const cv = document.createElement('canvas')
    cv.width = W; cv.height = H
    const g = cv.getContext('2d')!
    const live = liveTransitions(p)
    const element = async (c: Clip): Promise<HTMLVideoElement> => {
      let v = els.get(c.id)
      if (!v) {
        v = document.createElement('video')
        v.muted = true; v.preload = 'auto'; v.src = runtime.get(c.mediaId)!.url
        els.set(c.id, v)
        await new Promise((res, rej) => { v!.onloadeddata = res; v!.onerror = () => rej(new Error(`Can't read ${mediaOf(p, c.mediaId)?.name}`)) })
      }
      return v
    }
    const seek = (v: HTMLVideoElement, t: number): Promise<void> => new Promise((res) => {
      const want = Math.min(Math.max(0, t), v.duration - 0.001)
      if (Math.abs(v.currentTime - want) < 1e-4 && !v.seeking) return res()
      v.addEventListener('seeked', () => res(), { once: true })
      v.currentTime = want
    })
    const t0 = performance.now()
    for (let i = 0; i < frames; i++) {
      check()
      const t = a + i / fps
      const need: Promise<void>[] = []
      for (const tr of p.video) {
        if (tr.hidden) continue
        for (const { c } of mixAt(p, tr.id, t, live)) {
          const m = mediaOf(p, c.mediaId)
          if (m?.kind === 'video' && runtime.get(c.mediaId)) need.push(element(c).then((v) => seek(v, c.in + (t - c.start))))
        }
      }
      await Promise.all(need)
      renderFrame(g, p, t, (c): Source | null => runtime.get(c.mediaId)?.img ?? els.get(c.id) ?? null)
      const frame = new VideoFrame(cv, { timestamp: Math.round((i * 1e6) / fps), duration: Math.round(1e6 / fps) })
      venc.encode(frame, { keyFrame: i % (fps * 2) === 0 })
      frame.close()
      while (venc.encodeQueueSize > 6) await new Promise((r) => venc.addEventListener('dequeue', r, { once: true }))
      if (i % 5 === 0) {
        const rate = (i + 1) / ((performance.now() - t0) / 1000)
        const left = Math.round((frames - i) / Math.max(rate, 0.1))
        o.onProgress((i + 1) / frames, `Frame ${i + 1} of ${frames} · about ${Math.floor(left / 60)}:${String(left % 60).padStart(2, '0')} left`)
      }
    }
    await venc.flush()
    check()
    muxer.finalize()
    o.onProgress(1, `${frames} frames`)
    if (o.writable) { await o.writable.close(); return null }
    return new Blob([(target as ArrayBufferTarget).buffer], { type: 'video/mp4' })
  } finally {
    for (const v of els.values()) v.removeAttribute('src')
    if (venc.state !== 'closed') venc.close()
    if (aenc.state !== 'closed') aenc.close()
  }
}
