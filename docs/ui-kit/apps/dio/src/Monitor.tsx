import { useEffect, useRef, useSyncExternalStore } from 'react'
import { Timecode, Transport, formatTC } from '../../../components'
import { engine } from './engine'
import { projectEnd } from './model'
import { useProject, useTime } from './store'

const usePlayback = () => useSyncExternalStore(engine.on.bind(engine), () => `${engine.playing}|${engine.rate}|${engine.loop}`)

/** Program monitor: the composited frame at the project's aspect ratio, shrunk to fit, with transport controls. */
export function Monitor() {
  const p = useProject()
  const t = useTime()
  usePlayback()
  const cv = useRef<HTMLCanvasElement>(null)
  useEffect(() => { engine.canvas = cv.current; return () => { engine.canvas = null } }, [])
  const fps = p.settings.fps
  return (
    <div className="flex h-full flex-col">
      <div className="relative min-h-0 flex-1 bg-black/60">
        <div className="absolute inset-3 flex items-center justify-center">
          <canvas ref={cv} width={p.settings.width} height={p.settings.height} className="max-h-full max-w-full bg-black shadow-[0_0_0_1px_rgba(255,255,255,.08)]" />
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-3 border-t border-[var(--kit-line)] px-3 py-2">
        <Timecode time={t} fps={fps} onChange={(s) => engine.seek(s)} />
        <div className="flex-1" />
        <Transport
          playing={engine.playing}
          rate={engine.rate}
          onToggle={() => engine.toggle()}
          onStep={(n) => engine.step(n)}
          onJump={(to) => engine.seek(to === 'start' ? 0 : projectEnd(p))}
          loop={engine.loop}
          onLoop={(on) => engine.setLoop(on)}
        />
        <div className="flex-1" />
        <div className="text-right font-mono text-[11px] leading-tight text-white/45">
          <div>In {formatTC(p.range.in, fps)}</div>
          <div>Out {formatTC(p.range.out, fps)}</div>
        </div>
      </div>
    </div>
  )
}
