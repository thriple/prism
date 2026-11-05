import { useEffect, useState } from 'react'
import { AnimatedInput, HoldButton, SheenButton, Tooltip, useKitEffect, type IslandToast } from '../../components'

// The kit's theme variables (defaults live in styles/kit.css).
const VARS = [
  ['--kit-accent', 'Accent', 'highlights, active items, Undo'],
  ['--kit-danger', 'Danger', 'destructive buttons, rings'],
  ['--kit-ok', 'OK', 'StatusDot: fine'],
  ['--kit-busy', 'Busy', 'StatusDot: working'],
  ['--kit-warn', 'Warn', 'StatusDot: problem'],
  ['--kit-ink', 'Ink', 'island, tooltips, gooey menu'],
  ['--kit-panel', 'Panel', 'menus, palette, snackbar, dock'],
  ['--kit-glass', 'Glass', 'frosted cards'],
  ['--kit-line', 'Line', 'hairline borders']
] as const
type Theme = Record<string, string>

/** Hex (#rgb, #rrggbb, with or without alpha) or rgb()/rgba(). */
const valid = (v: string): boolean =>
  (/^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(v) || /^rgba?\(.+\)$/i.test(v)) && CSS.supports('color', v)

/** Resolve any valid colour to numbers via the browser. */
function rgba(v: string): { r: number; g: number; b: number; a: number } {
  const el = document.createElement('span')
  el.style.color = v
  document.body.append(el)
  const [r, g, b, a = 1] = getComputedStyle(el).color.match(/[\d.]+/g)!.map(Number)
  el.remove()
  return { r, g, b, a }
}
const hex = (n: number): string => n.toString(16).padStart(2, '0')

const readDefaults = (): Theme =>
  Object.fromEntries(VARS.map(([name]) => [name, getComputedStyle(document.documentElement).getPropertyValue(name).trim()]))

export function ThemePanel({ show }: { show: (t: IslandToast) => void }) {
  const [theme, setTheme] = useState<Theme>(readDefaults)
  const effect = useKitEffect()

  // Apply every valid value live, so the whole page recolours as you edit.
  useEffect(() => {
    for (const [name, v] of Object.entries(theme)) if (valid(v)) document.documentElement.style.setProperty(name, v)
  }, [theme])

  const set = (name: string, v: string): void => setTheme((t) => ({ ...t, [name]: v }))

  // The picker has no alpha, so keep the current alpha when picking.
  const pick = (name: string, picked: string): void => {
    const { a } = valid(theme[name]) ? rgba(theme[name]) : { a: 1 }
    const { r, g, b } = rgba(picked)
    set(name, a < 1 ? `rgb(${r} ${g} ${b} / ${+a.toFixed(2)})` : picked)
  }

  const exportTheme = (): void => {
    const lines = VARS.map(([name, , use]) => `  ${name}: ${valid(theme[name]) ? theme[name] : readDefaults()[name]}; /* ${use} */`)
    const text = `/* motion-kit theme — send this file to Claude, or paste it over :root in styles/kit.css. */\n/* effect: ${effect} — wrap the app in <EffectProvider effect="${effect}">. */\n:root {\n${lines.join('\n')}\n}\n`
    const a = document.createElement('a')
    a.href = URL.createObjectURL(new Blob([text], { type: 'text/plain' }))
    a.download = 'motion-kit-theme.txt'
    a.click()
    URL.revokeObjectURL(a.href)
    show({ glyph: '↓', label: 'Theme exported', sub: 'motion-kit-theme.txt' })
  }

  const reset = (): void => {
    for (const [name] of VARS) document.documentElement.style.removeProperty(name)
    setTheme(readDefaults())
    show({ glyph: '↺', label: 'Theme reset', tone: 'plain' })
  }

  return (
    <section className="glass flex flex-col gap-4 rounded-[22px] p-5">
      <div>
        <h2 className="m-0 text-base font-bold">Theme</h2>
        <p className="m-0 mt-1 text-[13px] text-white/60">
          Recolour the whole kit live. Click a swatch to pick, or type a hex (#f5dc4a, #0a080980) or rgb (rgb(12 9 10 / 0.94)) value. Export when you like it.
        </p>
      </div>

      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {VARS.map(([name, label, use]) => {
          const v = theme[name]
          const ok = valid(v)
          const { r, g, b } = ok ? rgba(v) : { r: 0, g: 0, b: 0 }
          return (
            <div key={name} className="flex items-center gap-3 rounded-[14px] border border-[var(--kit-line)] bg-black/20 p-2">
              <Tooltip label={`Pick ${label.toLowerCase()}`}>
                <label className="relative size-10 shrink-0 cursor-pointer overflow-hidden rounded-full border border-white/20" style={{ background: ok ? v : 'transparent' }}>
                  <input type="color" aria-label={`Pick ${label}`} value={`#${hex(r)}${hex(g)}${hex(b)}`} onChange={(e) => pick(name, e.target.value)} className="absolute inset-0 cursor-pointer opacity-0" />
                </label>
              </Tooltip>
              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-semibold">{label}</div>
                <div className="truncate text-[11px] text-white/45">{use}</div>
              </div>
              <AnimatedInput
                value={v}
                onChange={(val) => set(name, val)}
                aria-label={`${label} colour`}
                aria-invalid={!ok}
                spellCheck={false}
                maxLength={26}
                className={`w-[168px] shrink-0 rounded-lg border bg-white/5 px-2 py-1.5 font-mono text-[11px] ${ok ? 'border-[var(--kit-line)]' : 'border-[var(--kit-danger)]'}`}
              />
            </div>
          )
        })}
      </div>

      <div className="flex flex-wrap items-center gap-3">
        <SheenButton onClick={exportTheme} className="h-11 rounded-[14px] bg-[var(--kit-accent)] px-4 text-[13px] font-semibold text-[#17120a]">
          Export theme (.txt)
        </SheenButton>
        <HoldButton onComplete={reset}>Hold to reset</HoldButton>
        <span className="text-xs text-white/45">Invalid values are outlined and left out until fixed.</span>
      </div>
    </section>
  )
}
