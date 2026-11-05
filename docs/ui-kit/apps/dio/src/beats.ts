// Beat Highlighter: rough beat (kick/bass onset) detection for audio clips.

export const ENV_PER_SEC = 100

/** Low-frequency energy per 10 ms: a ~150 Hz low-pass, then RMS per window. Computed once at import. */
export function bassEnvelope(buf: AudioBuffer): Float32Array {
  const sr = buf.sampleRate
  const n = Math.ceil(buf.duration * ENV_PER_SEC)
  const win = sr / ENV_PER_SEC
  const out = new Float32Array(n)
  const a = 1 - Math.exp((-2 * Math.PI * 150) / sr) // one-pole low-pass coefficient
  const chans = Array.from({ length: buf.numberOfChannels }, (_, c) => buf.getChannelData(c))
  let y = 0
  for (let i = 0; i < n; i++) {
    let sum = 0
    const s0 = Math.floor(i * win), s1 = Math.min(buf.length, Math.floor((i + 1) * win))
    for (let s = s0; s < s1; s++) {
      let x = 0
      for (const ch of chans) x += ch[s]
      y += a * (x / chans.length - y)
      sum += y * y
    }
    out[i] = Math.sqrt(sum / Math.max(1, s1 - s0))
  }
  return out
}

const cache = new Map<string, number[]>()

/**
 * Beat times (seconds into the source) from a bass envelope: rises in low-end energy that stand out
 * from the surrounding second by `sensitivity`× (lower = more beats), at least `gap` seconds apart.
 */
export function detectBeats(env: Float32Array, sensitivity: number, gap: number, key: string): number[] {
  const ck = `${key}|${sensitivity}|${gap}`
  const hit = cache.get(ck)
  if (hit) return hit
  const n = env.length
  const flux = new Float32Array(n)
  for (let i = 1; i < n; i++) flux[i] = Math.max(0, env[i] - env[i - 1])
  // Running mean of the flux over ±0.5 s (prefix sums).
  const pre = new Float64Array(n + 1)
  for (let i = 0; i < n; i++) pre[i + 1] = pre[i] + flux[i]
  const R = ENV_PER_SEC / 2
  let peak = 0
  for (let i = 0; i < n; i++) if (flux[i] > peak) peak = flux[i]
  const beats: number[] = []
  let last = -Infinity
  for (let i = 1; i < n - 1; i++) {
    const a = Math.max(0, i - R), b = Math.min(n, i + R)
    const mean = (pre[b] - pre[a]) / (b - a)
    const f = flux[i]
    if (f < mean * sensitivity || f < peak * 0.05 || f < flux[i - 1] || f < flux[i + 1]) continue
    const t = i / ENV_PER_SEC
    if (t - last < gap) continue
    beats.push(t)
    last = t
  }
  cache.set(ck, beats)
  return beats
}
