import { createContext, useContext, useEffect, useLayoutEffect, useState, type ReactNode, type RefObject } from 'react'

/**
 * Effects: FUI-style stepped entrances that replace a component's default "appear / change"
 * animation. 'default' keeps the kit's normal springs and blurs.
 *
 *   <EffectProvider effect="glitch">…</EffectProvider>   // whole app
 *   <SegmentedControl effect="color-flash" … />           // one component (overrides the provider)
 */
export const EFFECTS = ['default', 'blinky', 'glitch', 'trail', 'stroke-n-fill', 'shuffle', 'crash-n-blink', 'slide-n-blink', 'color-flash', 'build-on'] as const
export type KitEffect = (typeof EFFECTS)[number]
export const EFFECT_LABELS: Record<KitEffect, string> = {
  default: 'Default', blinky: 'Blinky', glitch: 'Glitch', trail: 'Trail', 'stroke-n-fill': 'Stroke-n-fill',
  shuffle: 'Shuffle', 'crash-n-blink': 'Crash-n-blink', 'slide-n-blink': 'Slide-n-blink', 'color-flash': 'Color flash', 'build-on': 'Build on'
}

const Ctx = createContext<KitEffect>('default')
export function EffectProvider({ effect, children }: { effect: KitEffect; children: ReactNode }) {
  return <Ctx.Provider value={effect}>{children}</Ctx.Provider>
}
/** The effect in force: the component's own `effect` prop, else the nearest provider, else 'default'. */
export function useKitEffect(override?: KitEffect): KitEffect {
  const ctx = useContext(Ctx)
  return override ?? ctx
}

/** Motion props for an element's exit while an effect is active: a quick cut instead of a soft fade. */
export const FX_EXIT = { opacity: 0, transition: { duration: 0.06 } } as const

// ---------------------------------------------------------------------------------------------

type Kind = 'box' | 'glyph' // box: panels, pills, dots. glyph: a single text character.
type Frame = Record<string, string | number | null> // null = the element's own value
type Spec = { step: number; el?: Frame[]; fx?: Frame[]; text?: number } // fx = the ::after overlay; text = shuffle frames

const FLASH = ['#ff0040', '#00c8ff', '#00ff30', '#8000ff']
const svg = (body: string, attrs = ''): string =>
  `url("data:image/svg+xml,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' ${attrs}>${body}</svg>`)}")`
const CROSS = svg("<rect x='.5' y='.5' width='99' height='99' fill='none' stroke='#fff' vector-effect='non-scaling-stroke'/><path d='M0 0L100 100M100 0L0 100' stroke='#fff' vector-effect='non-scaling-stroke'/>", "viewBox='0 0 100 100' preserveAspectRatio='none'")
const BANG = svg("<rect x='9' y='3' width='6' height='12' fill='#000'/><rect x='9' y='18' width='6' height='4' fill='#000'/>", "viewBox='0 0 24 24'")
const HATCH = 'repeating-linear-gradient(135deg, #fff 0 1.5px, transparent 1.5px 6px)'
const INK = 'var(--kit-ink)'

/** Random 4×4 cell mask; `solid` cells fully shown, `half` cells at 50%. */
function cells(solid: number[], half: number[]): string {
  const r = (i: number, a: number): string => `<rect x='${i % 4}' y='${Math.floor(i / 4)}' width='1.02' height='1.02' fill='white' fill-opacity='${a}'/>`
  return svg([...solid.map((i) => r(i, 1)), ...half.map((i) => r(i, 0.5))].join(''), "viewBox='0 0 4 4' preserveAspectRatio='none'")
}

function spec(effect: KitEffect, kind: Kind): Spec | null {
  const glyph = kind === 'glyph'
  switch (effect) {
    case 'blinky':
      return { step: 55, el: [0, 0.5, 0, 1, 0.5].map((opacity) => ({ opacity })) }
    case 'glitch':
      return {
        step: 55,
        el: [
          { translate: '-18% 0', scale: '1.5 .3', clipPath: 'inset(30% 0 40% 0)' },
          { translate: '8% -8%', scale: '1.1 .8', clipPath: 'inset(0 0 35% 0)' },
          { translate: '-4% 0', scale: null, clipPath: 'inset(15% 0 0 0)' },
          ...Array(12).fill({ translate: null, scale: null, clipPath: null }),
          { translate: '4% 0', scale: null, clipPath: 'inset(0 0 45% 0)' }, // late aftershock
          { translate: '-3% 0', scale: null, clipPath: null }
        ]
      }
    case 'trail':
      return {
        step: 55,
        el: [
          { opacity: 0.2, translate: '-55% 0', scale: '1.9 1' },
          { opacity: 0.45, translate: '-30% 0', scale: '1.6 1' },
          { opacity: 0.7, translate: null, scale: null },
          { opacity: 0.85, translate: null, scale: null }
        ]
      }
    case 'stroke-n-fill': {
      if (glyph) {
        const line = (a: number): Frame => ({ color: 'transparent', textShadow: `-1px 0 rgb(255 255 255/${a}),1px 0 rgb(255 255 255/${a}),0 -1px rgb(255 255 255/${a}),0 1px rgb(255 255 255/${a})`, opacity: 1 })
        const fill = (opacity: number): Frame => ({ color: null, textShadow: null, opacity })
        return { step: 55, el: [line(0.35), line(0.9), line(0.35), line(0.9), fill(0.55), line(0.9), fill(1), fill(0.55)] }
      }
      const line = (a: number): Frame => ({ backgroundColor: 'transparent', boxShadow: `inset 0 0 0 1px rgb(255 255 255 / ${a})`, opacity: 1 })
      const fill = (opacity: number): Frame => ({ backgroundColor: null, boxShadow: null, opacity })
      return { step: 55, el: [line(0.35), line(0.9), line(0.35), line(0.9), fill(0.55), line(0.9), fill(1), fill(0.55)] }
    }
    case 'shuffle':
      if (glyph) return { step: 55, text: 6 }
      return {
        step: 60,
        fx: [
          { opacity: 1, background: `${CROSS} center / 100% 100% no-repeat, ${INK}` },
          { opacity: 1, background: `${CROSS} center / 100% 100% no-repeat, ${INK}` },
          { opacity: 1, background: `${BANG} center / auto 70% no-repeat, #fff` },
          { opacity: 1, background: `${BANG} center / auto 70% no-repeat, #fff` },
          { opacity: 1, background: `${HATCH}, ${INK}` },
          { opacity: 1, background: `${HATCH}, ${INK}` }
        ]
      }
    case 'crash-n-blink':
      return { step: 55, el: [{ scale: '1.4', opacity: 0.5 }, { scale: null, opacity: 1 }, { scale: null, opacity: 0 }, { scale: null, opacity: 1 }, { scale: null, opacity: 0.5 }] }
    case 'slide-n-blink':
      return { step: 55, el: [[0.5, 10], [0, 8], [1, 5], [0.5, 3], [1, 1]].map(([opacity, y]) => ({ opacity, translate: `0 ${y}px` })) }
    case 'color-flash': {
      const start = Math.floor(Math.random() * 4)
      const seq = [0, 1, 2, 3].map((i) => FLASH[(start + i) % 4])
      return glyph
        ? { step: 65, el: seq.map((color) => ({ color })) }
        : { step: 65, fx: seq.map((background) => ({ opacity: 1, background })) }
    }
    case 'build-on': {
      const order = [...Array(16).keys()].sort(() => Math.random() - 0.5)
      const frames: Frame[] = []
      for (let k = 1; k <= 10; k++) {
        const n = Math.round((k / 10) * 16)
        const shown = order.slice(0, n)
        frames.push({ maskImage: cells(shown.slice(0, -2), shown.slice(-2)), maskSize: '100% 100%', maskRepeat: 'no-repeat' })
      }
      return { step: 50, el: frames }
    }
    default:
      return null
  }
}

/** Replace nulls with the element's current value, and compose translate with any it already has. */
function resolve(frames: Frame[], el: Element, pseudo?: string): Keyframe[] {
  const cs = getComputedStyle(el, pseudo)
  const own = (p: string): string => cs.getPropertyValue(p.replace(/[A-Z]/g, (c) => '-' + c.toLowerCase()))
  const base = own('translate') === 'none' ? ['0px', '0px'] : [...own('translate').split(' '), '0px'].slice(0, 2)
  const props = [...new Set(frames.flatMap((f) => Object.keys(f)))]
  const key = (f: Frame): Keyframe => {
    const k: Keyframe = { easing: 'step-end' }
    for (const p of props) {
      const v = f[p] ?? null
      if (p === 'translate') {
        const [x, y = '0px'] = v === null ? ['0px', '0px'] : String(v).split(' ')
        k.translate = `calc(${base[0]} + ${x}) calc(${base[1]} + ${y})`
      } else k[p] = v === null ? own(p) : v
    }
    return k
  }
  return [...frames.map(key), {}] // the empty last keyframe hands back to the element's own styles
}

const reduced = (): boolean => matchMedia('(prefers-reduced-motion: reduce)').matches

/** Play `effect` on `el` once. Returns a cancel function. */
export function playEffect(el: HTMLElement, effect: KitEffect, kind: Kind = 'box', delay = 0): () => void {
  const s = spec(effect, kind)
  if (!s || reduced()) return () => {}
  const anims: Animation[] = []
  const timers: ReturnType<typeof setTimeout>[] = []
  const frames = s.el ?? s.fx ?? []
  const n = s.text ?? frames.length
  const duration = n * s.step
  const opts = { duration, delay }
  // Stay hidden until the effect starts (e.g. while a panel is still opening).
  if (delay) anims.push(el.animate([{ opacity: 0 }, { opacity: 0 }], { duration: delay }))
  if (s.el) anims.push(el.animate(resolve(s.el, el), opts))
  if (s.fx) {
    el.setAttribute('data-kit-fx', '')
    if (getComputedStyle(el).position === 'static') el.style.position = 'relative'
    anims.push(el.animate(resolve(s.fx, el, '::after'), { ...opts, pseudoElement: '::after' }))
  }
  if (s.text) {
    const node = [...el.childNodes].find((c) => c.nodeType === Node.TEXT_NODE && c.nodeValue?.trim())
    if (node) {
      const real = node.nodeValue!
      const noise = '#%&@*?!01<>/'
      for (let i = 0; i < n; i++) {
        timers.push(setTimeout(() => { node.nodeValue = [...real].map(() => noise[Math.floor(Math.random() * noise.length)]).join('') }, delay + i * s.step))
      }
      timers.push(setTimeout(() => { node.nodeValue = real }, delay + duration))
      return () => { timers.forEach(clearTimeout); node.nodeValue = real; anims.forEach((a) => a.cancel()) }
    }
  }
  return () => { timers.forEach(clearTimeout); anims.forEach((a) => a.cancel()) }
}

/**
 * Plays the active effect on `ref`'s element when it mounts and whenever `trigger` changes.
 * Returns the active effect, so the component can skip its default motion when it isn't 'default'.
 */
export function useFx(ref: RefObject<HTMLElement | null>, trigger: unknown, { effect, kind = 'box', enabled = true, delay = 0 }: {
  effect?: KitEffect
  kind?: Kind
  enabled?: boolean
  delay?: number
} = {}): KitEffect {
  const fx = useKitEffect(effect)
  useLayoutEffect(() => {
    if (fx === 'default' || !enabled || !ref.current) return
    return playEffect(ref.current, fx, kind, delay)
  }, [fx, trigger, enabled]) // eslint-disable-line react-hooks/exhaustive-deps
  return fx
}

/** A counter that increments every `ms` while enabled; use it as a `useFx` trigger to replay an effect on a loop. */
export function useTick(ms: number, enabled = true): number {
  const [n, setN] = useState(0)
  useEffect(() => {
    if (!enabled) return
    const id = setInterval(() => setN((x) => x + 1), ms)
    return () => clearInterval(id)
  }, [ms, enabled])
  return n
}
