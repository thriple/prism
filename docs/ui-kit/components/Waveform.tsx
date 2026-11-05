import { useEffect, useRef } from 'react'

/** Max |sample| per bucket, across channels: `perSecond` buckets for each second of audio. */
export function computePeaks(buffer: AudioBuffer, perSecond = 100): Float32Array {
  const n = Math.max(1, Math.ceil(buffer.duration * perSecond))
  const out = new Float32Array(n)
  const size = buffer.length / n
  for (let c = 0; c < buffer.numberOfChannels; c++) {
    const d = buffer.getChannelData(c)
    for (let i = 0; i < n; i++) {
      let m = out[i]
      const end = Math.min(d.length, Math.floor((i + 1) * size))
      for (let j = Math.floor(i * size); j < end; j++) { const v = d[j] < 0 ? -d[j] : d[j]; if (v > m) m = v }
      out[i] = m
    }
  }
  return out
}

/**
 * Mirrored waveform drawn to a canvas that fills its box. `peaks` are 0–1 (see `computePeaks`);
 * `from`/`to` pick a slice of them (bucket indices), e.g. a trimmed clip's part of its file.
 *   <Waveform peaks={p} from={inSec * 100} to={(inSec + dur) * 100} className="h-10 w-full" />
 */
export function Waveform({ peaks, from = 0, to, gain = 1, color = 'currentColor', className = '' }: {
  peaks: ArrayLike<number>
  from?: number
  to?: number
  gain?: number
  color?: string
  className?: string
}) {
  const cv = useRef<HTMLCanvasElement>(null)
  useEffect(() => {
    const c = cv.current
    if (!c) return
    const draw = (): void => {
      const w = c.clientWidth, h = c.clientHeight, dpr = devicePixelRatio || 1
      if (!w || !h) return
      c.width = Math.round(w * dpr); c.height = Math.round(h * dpr)
      const g = c.getContext('2d')!
      g.fillStyle = color === 'currentColor' ? getComputedStyle(c).color : color
      const a = Math.max(0, from), b = Math.min(peaks.length, to ?? peaks.length)
      const per = (b - a) / c.width
      for (let x = 0; x < c.width; x++) {
        let m = 0
        for (let i = Math.floor(a + x * per), e = Math.max(i + 1, Math.floor(a + (x + 1) * per)); i < e && i < b; i++) if (peaks[i] > m) m = peaks[i]
        const v = Math.min(1, m * gain) * (c.height / 2)
        g.fillRect(x, c.height / 2 - v, 1, Math.max(1, v * 2))
      }
    }
    draw()
    const ro = new ResizeObserver(draw)
    ro.observe(c)
    return () => ro.disconnect()
  }, [peaks, from, to, gain, color])
  return <canvas ref={cv} aria-hidden className={`block ${className}`} />
}
