import { useEffect, useState, type ReactNode } from 'react'
import {
  AnimatedInput, AvatarStack, DnaThink, LevelMeter, PanelLayout, RenderProgress, Timecode, TimelineRuler, Transport, Waveform, split, tabs, DockPanel, RingThink, WowButton, EffectProvider, FilterSelector, Icon, type FilterOption, GooeyMenu, type KitEffect, CommandPalette, ContextMenu, DynamicIsland, HoldButton, ImageReveal, Kbd,
  SegmentedControl, SheenButton, Skeleton, StatusDot, Tooltip, TooltipToolbar, UndoSnackbar, useIslandToast,
  type PaletteItem, type ToolbarItem
} from '../../components'
import { DialDemos } from './DialDemos'
import { DigitalSparkleDemo } from './DigitalSparkleDemo'
import { EffectPicker } from './EffectPicker'
import { AnimatedIconGrid } from './AnimatedIconGrid'
import { IconGrid } from './IconGrid'
import { SparkleGrid } from './SparkleGrid'
import { ThemePanel } from './ThemePanel'
// Placeholder image for Image Reveal; the hover frame is a close-up of the same picture.
import berry from './assets/berry.jpg'
import berryDetail from './assets/berry-detail.jpg'

// Stand-in images: gradient SVGs, so the demo needs no files.
const img = (a: string, b: string, angle = 135): string =>
  'data:image/svg+xml,' + encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 90"><defs><linearGradient id="g" gradientTransform="rotate(${angle - 90} .5 .5)"><stop offset="0" stop-color="${a}"/><stop offset="1" stop-color="${b}"/></linearGradient></defs><rect width="160" height="90" fill="url(#g)"/><circle cx="110" cy="30" r="18" fill="${b}" opacity=".6"/></svg>`
  )

const FILES = [
  { name: 'harbour_dawn.mp4' },
  { name: 'reef_pass.mp4' },
  { name: 'drone_ridge.mp4' },
  { name: 'night_market.mkv' },
  { name: 'old_capture.avi' },
  { name: 'intro_loop.gif' },
  { name: 'ferry_wake.mp4' }
]

// Toolbar icons (simple strokes); `dot` adds a small unread badge.
const icon = (d: string, dot = false): ReactNode => (
  <span className="relative">
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d={d} /></svg>
    {dot && <span className="absolute -right-0.5 -top-0.5 size-1.5 rounded-full bg-sky-400" />}
  </span>
)
const I = {
  comment: 'M21 11.5a8.4 8.4 0 0 1-12.2 7.5L3 21l2-5.8A8.5 8.5 0 1 1 21 11.5z',
  inbox: 'M22 12h-6l-2 3h-4l-2-3H2M5.5 5h13L22 12v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-6z',
  toggle: 'M8 6h8a6 6 0 0 1 0 12H8A6 6 0 0 1 8 6zM8 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z',
  flag: 'M4 22V4s1.5-1 5-1 5 2 8 2 3-1 3-1v11s-1 1-3 1-5-2-8-2-5 1-5 1',
  share: 'M12 15V3M7 8l5-5 5 5M4 14v5a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-5',
  menu: 'M4 7h16M4 12h16M4 17h16',
  file: 'M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8zM14 3v5h5',
  home: 'M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z',
  bell: 'M6 8a6 6 0 0 1 12 0c0 7 3 8 3 8H3s3-1 3-8M10.3 21a1.9 1.9 0 0 0 3.4 0',
  sliders: 'M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0M14 4v4M8 10v4M16 16v4'
}

/** Demo-only switch for the dock's settings tab. */
function Switch({ label }: { label: string }) {
  const [on, setOn] = useState(true)
  return (
    <button type="button" role="switch" aria-checked={on} onClick={() => setOn(!on)} className="flex w-full items-center justify-between rounded-lg px-2 py-2 text-[13px] hover:bg-white/5">
      {label}
      <span className={`flex h-5 w-9 items-center rounded-full p-0.5 transition-colors ${on ? 'bg-[var(--kit-accent)]' : 'bg-white/15'}`}>
        <span className={`size-4 rounded-full bg-white transition-transform ${on ? 'translate-x-4' : ''}`} />
      </span>
    </button>
  )
}

const row = 'flex w-full items-center justify-between rounded-lg px-2 py-2 text-[13px] hover:bg-white/5'
const DOCK_TABS = [
  {
    label: 'Dashboard', icon: icon(I.home),
    content: ['Profile', 'Upgrade', 'Projects', 'Documentation', 'Log out'].map((l) => <button key={l} type="button" className={row}>{l}<span className="text-white/40">›</span></button>)
  },
  {
    label: 'Notifications', icon: icon(I.bell, true),
    content: (
      <div className="thin-scroll max-h-[180px] overflow-y-auto [mask-image:linear-gradient(transparent,#000_12px,#000_calc(100%-24px),transparent)]">
        {[2, 6, 10, 14, 18, 22, 26].map((t, i) => (
          <div key={t} className={row}>Notification {i + 1}<span className="flex items-center gap-2 text-xs text-white/45">{t} s ago<span className="size-1.5 rounded-full bg-sky-400" /></span></div>
        ))}
      </div>
    )
  },
  { label: 'Settings', icon: icon(I.sliders), content: ['Dark mode', 'Notifications', 'Auto save'].map((l) => <Switch key={l} label={l} />) }
]

const ROLES: FilterOption[] = [
  { id: 'designer', label: 'Designer', icon: <Icon n={9} size={14} /> },
  { id: 'ux', label: 'UX Researcher', icon: <Icon n={14} size={14} /> },
  { id: 'creator', label: 'Creator', icon: <Icon n={2} size={14} /> },
  { id: 'engineer', label: 'Engineer', icon: <Icon n={1} size={14} /> },
  { id: 'pm', label: 'Product Manager', icon: <Icon n={44} size={14} /> },
  { id: 'founder', label: 'Founder', icon: <Icon n={13} size={14} /> }
]

// Demo meter/waveform data: a wobbling stereo level and a synthetic waveform.
const DEMO_LEVELS = (): number[] => { const t = performance.now() / 1000; return [0.35 + 0.3 * Math.abs(Math.sin(t * 2.1)), 0.3 + 0.3 * Math.abs(Math.sin(t * 1.7 + 1))] }
const DEMO_PEAKS = Float32Array.from({ length: 800 }, (_, i) => Math.abs(Math.sin(i / 9)) * (0.35 + 0.6 * Math.abs(Math.sin(i / 97))))

function Card({ title, note, children }: { title: string; note: string; children: ReactNode }) {
  return (
    <section className="glass flex flex-col gap-4 rounded-[22px] p-5">
      <div>
        <h2 className="m-0 text-base font-bold">{title}</h2>
        <p className="m-0 mt-1 text-[13px] text-white/60">{note}</p>
      </div>
      <div className="flex min-h-[72px] flex-wrap items-center gap-3">{children}</div>
    </section>
  )
}

const btn = 'h-10 rounded-xl border border-white/15 bg-white/5 px-3.5 text-[13px] hover:bg-white/15'

export function App() {
  const [toast, show] = useIslandToast()
  const [speed, setSpeed] = useState(0)
  const [palette, setPalette] = useState(false)
  const [roles, setRoles] = useState(['designer', 'ux'])
  const [rolesLight, setRolesLight] = useState(['designer', 'ux'])
  const [tc, setTc] = useState(12.5)
  const [playing, setPlaying] = useState(false)
  const [io, setIo] = useState({ in: 4, out: 18 })
  const [panes, setPanes] = useState(() => split('row', [3, 7], tabs('library'), split('col', [6, 4], tabs('monitor'), tabs('timeline', 'notes'))))
  const [menuAt, setMenuAt] = useState<{ x: number; y: number } | null>(null)
  const [until, setUntil] = useState<number | null>(null)
  const [queued, setQueued] = useState(FILES.slice(0, 4))
  const [loading, setLoading] = useState(true)
  const [ctrlDel, setCtrlDel] = useState(false)
  const [code, setCode] = useState('')
  const [dock, setDock] = useState<number | null>(null)
  const [effect, setEffect] = useState<KitEffect>('default')

  // Undo grace period: act when it runs out.
  useEffect(() => {
    if (!until) return
    const t = setTimeout(() => { setUntil(null); setQueued([]); show({ glyph: '✓', label: 'Deleted', tone: 'danger' }) }, until - Date.now())
    return () => clearTimeout(t)
  }, [until, show])

  // Keyboard: Ctrl+K palette, hold Ctrl+Del for the hold button, Ctrl+Z undo.
  useEffect(() => {
    const down = (e: KeyboardEvent): void => {
      if (e.ctrlKey && e.key.toLowerCase() === 'k') { e.preventDefault(); setPalette((p) => !p) }
      if (e.ctrlKey && e.key === 'Delete' && !e.repeat) setCtrlDel(true)
      if (e.ctrlKey && e.key.toLowerCase() === 'z') setUntil(null)
    }
    const up = (e: KeyboardEvent): void => { if (e.key === 'Delete' || e.key === 'Control') setCtrlDel(false) }
    window.addEventListener('keydown', down)
    window.addEventListener('keyup', up)
    return () => { window.removeEventListener('keydown', down); window.removeEventListener('keyup', up) }
  }, [])

  const tool = (label: string, i: ReactNode, keys?: string[]): ToolbarItem => ({ label, keys, icon: i, onSelect: () => show({ glyph: '•', label, tone: 'plain' }) })
  const comment = tool('Comment', icon(I.comment), ['C'])
  const share = tool('Share', icon(I.share))
  const menu = tool('Menu', icon(I.menu, true), ['Ctrl', 'K'])

  const items: PaletteItem[] = FILES.map((f) => ({
    key: f.name, name: f.name, sub: 'Demo file', group: f.name.split('.').pop()!.toUpperCase(), icon: icon(I.file),
    onSelect: () => show({ glyph: '↗', label: 'Opened', sub: f.name, tone: 'plain' })
  }))

  return (
    <EffectProvider effect={effect}>
    <div className="relative mx-auto flex max-w-[1100px] flex-col gap-5 px-4 pb-24 pt-6">
      <header className="flex flex-col items-center gap-4 pb-4">
        <DynamicIsland toast={toast}>
          <span className="text-[var(--kit-accent)]">▶</span>
          <span className="flex-1 truncate">harbour_dawn.mp4</span>
          <span className="text-[11px] font-semibold text-[var(--kit-accent)]">{['100%', '50%', '25%', '12.5%'][speed]}</span>
        </DynamicIsland>
        <h1 className="m-0 text-3xl font-black tracking-tight">Motion kit</h1>
        <p className="m-0 text-sm text-white/60">Every component running. Hover, click, and try <Kbd>Ctrl</Kbd> <Kbd>K</Kbd>. Switch effects with the square, bottom left.</p>
      </header>

      <div className="grid grid-cols-1 gap-5 md:grid-cols-2">
        <Card title="Dynamic Island" note="Resting status that springs open for each action (the pill at the top).">
          <button className={btn} onClick={() => show({ glyph: '+1', label: 'Next frame', sub: '00:17:23:14' })}>Frame step</button>
          <button className={btn} onClick={() => show({ glyph: '+', label: 'Queued for trash', sub: 'reef_pass.mp4', tone: 'danger' })}>Queue</button>
          <button className={btn} onClick={() => show({ glyph: 'M', label: 'Muted', tone: 'plain' })}>Mute</button>
        </Card>

        <Card title="Segmented Control" note="The white indicator springs between options.">
          <SegmentedControl label="Speed" value={speed} onChange={(i) => { setSpeed(i); show({ glyph: String(i + 1), label: `Speed ${['100', '50', '25', '12.5'][i]}%` }) }}
            options={['100%', '50%', '25%', '12.5%'].map((l, i) => ({ label: l, hint: String(i + 1) }))} />
        </Card>

        <Card title="Tooltip" note="Springs up above its trigger on hover or keyboard focus, with key hints.">
          <Tooltip label="Back 5 s" keys={['←']}><button className={btn}>Hover me</button></Tooltip>
          <Tooltip label="Next frame" keys={['.']} side="bottom"><button className={btn}>Below</button></Tooltip>
        </Card>

        <Card title="Tooltip Toolbar" note="One tooltip slides between buttons; its labels scroll to the one you hover.">
          <div className="flex w-full items-center justify-center gap-36 pt-10">
            <TooltipToolbar label="Tools" items={[comment, tool('Inbox', icon(I.inbox, true)), tool('Draft Mode', icon(I.toggle)), tool('Feature Flags', icon(I.flag)), share, menu]} />
            <TooltipToolbar label="Tools" orientation="vertical" items={[comment, share, menu]} />
          </div>
        </Card>

        <Card title="Gooey Menu" note="Hover the button: the panel buds off it like a liquid drop, then melts back.">
          <div className="pt-32">
            <GooeyMenu label="Build info" icon={<span className="text-sm font-black">K</span>}>
              <div className="flex h-full flex-col justify-center gap-1 px-4 text-[13px]">
                <div className="flex justify-between text-white/50"><span className="font-mono">motion-kit</span><span className="font-mono">v1.2</span></div>
                <div className="flex justify-between font-semibold">Errors<span className="grid size-5 place-items-center rounded-full bg-[var(--kit-danger)]/30 text-[11px] text-[var(--kit-danger)]">3</span></div>
                <div className="flex justify-between font-semibold">Route<span className="font-normal text-white/50">Static</span></div>
              </div>
            </GooeyMenu>
          </div>
        </Card>

        <Card title="Animated Input" note="Characters spring up as you type and drop away as you delete.">
          <AnimatedInput value={code} onChange={setCode} placeholder="000" maxLength={10} aria-label="Code" className="w-full text-5xl font-bold" />
        </Card>

        <Card title="Dock Panel" note="Click an icon: the dock grows into a panel. Switch tabs; click outside or Esc to close.">
          <div className="flex h-[300px] w-full items-end justify-center">
            <DockPanel label="Workspace" tabs={DOCK_TABS} active={dock} onChange={setDock} />
          </div>
        </Card>

        <Card title="Filter Selector" note="+ grows into a menu of what's left; a pick lands as a chip and the + slides on. × removes. Dark (kit) and light modes.">
          <div className="flex w-full flex-col gap-3 self-start">
            <div className="min-h-[180px] rounded-[14px] p-3">
              <FilterSelector label="Applied filters" options={ROLES} value={roles} onChange={setRoles} />
            </div>
            <div className="min-h-[180px] rounded-[14px] bg-white p-3">
              <FilterSelector label="Applied filters" mode="light" options={ROLES} value={rolesLight} onChange={setRolesLight} />
            </div>
          </div>
        </Card>

        <Card title="Command Palette" note="Blurred scrim, spring-in box, fuzzy search, ↑ ↓ Enter Esc.">
          <button className={btn} onClick={() => setPalette(true)}>Open palette <Kbd>Ctrl</Kbd> <Kbd>K</Kbd></button>
        </Card>

        <Card title="Context Menu" note="Right-click the tile. Pops from the pointer and stays on screen.">
          <div
            onContextMenu={(e) => { e.preventDefault(); setMenuAt({ x: e.clientX, y: e.clientY }) }}
            className="grid h-[74px] w-[160px] cursor-context-menu place-items-center rounded-[11px] text-xs text-white/80"
            style={{ backgroundImage: `url("${img('#1b8a9a', '#062a33')}")`, backgroundSize: 'cover' }}
          >
            Right-click me
          </div>
        </Card>

        <Card title="Undo Snackbar" note="Destructive action with a grace period and a countdown ring.">
          <button className={btn} disabled={!!until || !queued.length} onClick={() => setUntil(Date.now() + 8000)}>Delete {queued.length} files</button>
          <button className={btn} onClick={() => setQueued(FILES.slice(0, 4))}>Reset</button>
        </Card>

        <Card title="Hold Button" note="Hold the mouse (or Ctrl+Del) for 1 s to confirm. Release early to cancel.">
          <HoldButton holding={ctrlDel} onComplete={() => show({ glyph: '✓', label: 'Confirmed by holding', tone: 'danger' })}>Hold to delete</HoldButton>
        </Card>

        <Card title="Status Dot" note="Pulses while work is running.">
          <StatusDot tone="ok" label="Direct play" />
          <StatusDot tone="busy" label="Building proxy · 41%" />
          <StatusDot tone="warn" label="Can't decode here" />
        </Card>

        <Card title="Image Reveal" note="Hover: zooms and wipes across to a second image.">
          {FILES.slice(0, 3).map((f) => (
            <ImageReveal key={f.name} src={berry} hoverSrc={berryDetail} className="h-[74px] w-[130px] rounded-[11px]" />
          ))}
        </Card>

        <Card title="Avatar Stack" note="Overlapping thumbnails fan out on hover; +N for the rest.">
          <AvatarStack items={FILES.map((f) => ({ key: f.name, label: f.name }))} max={5} />
          <AvatarStack items={[{ key: 'a', label: 'Ada' }, { key: 'b', label: 'Bo' }, { key: 'c', label: 'Cy' }]} size={36} />
        </Card>

        <Card title="Sheen Button" note="A light sweep crosses the button on hover.">
          <SheenButton className="h-11 rounded-[14px_14px_14px_4px] bg-[var(--kit-danger)] px-4 text-[13px] font-semibold text-[#1a0503]">Mark for trash <Kbd className="ml-1">X</Kbd></SheenButton>
          <SheenButton className="h-11 rounded-[14px] bg-[var(--kit-accent)] px-4 text-[13px] font-semibold text-[#17120a]">Save</SheenButton>
        </Card>

        <Card title="DNA Think" note="Processing: dot pairs melt together and pull apart in a wave.">
          <DnaThink />
          <DnaThink width={200} className="text-[var(--kit-accent)]" />
        </Card>

        <Card title="Ring Think" note="Processing: two sparks sweep into crescents and back.">
          <RingThink />
          <RingThink size={64} />
          <RingThink size={64} className="text-[var(--kit-accent)]" />
        </Card>

        <Card title="Wow Button" note="A more important button: hover and the dot swells to fill it, an arrow slides in.">
          <WowButton>Continue</WowButton>
          <div className="rounded-[14px] bg-white p-4"><WowButton mode="light">Button</WowButton></div>
        </Card>

        <Card title="Transport + Timecode" note="Playback controls with key hints; click the timecode to type a time (01:02:03:04, 3:04, 120 frames or 2.5s).">
          <Transport playing={playing} onToggle={() => setPlaying(!playing)} onStep={(n) => setTc((t) => Math.max(0, t + n / 30))} onJump={(to) => setTc(to === 'start' ? 0 : 30)} loop onLoop={() => {}} />
          <Timecode time={tc} fps={30} onChange={setTc} />
        </Card>

        <Card title="Timeline Ruler" note="Click or drag to scrub. The yellow strip on top is the render range: drag its In and Out handles.">
          <div className="w-full overflow-x-auto thin-scroll">
            <TimelineRuler duration={30} fps={30} pxPerSec={40} time={tc} onSeek={setTc} range={io} onRange={setIo} />
          </div>
        </Card>

        <Card title="Level Meter + Waveform" note="dB-scaled meter with peak hold; a waveform drawn from peak data (computePeaks turns an AudioBuffer into it).">
          <LevelMeter levels={DEMO_LEVELS} height={120} />
          <Waveform peaks={DEMO_PEAKS} className="h-[80px] flex-1 text-[var(--kit-ok)]" />
        </Card>

        <Card title="Render Progress" note="Export status: bar, detail line and Cancel.">
          <RenderProgress progress={0.42} title="Exporting Untitled.mp4" detail="Frame 378 of 900 · about 0:21 left" onCancel={() => {}} />
        </Card>

        <Card title="Panel Layout" note="Docked panels, like Premiere: drag a divider to resize; drag a tab onto another panel (centre = join, edge = split).">
          <div className="h-[300px] w-full">
            <PanelLayout layout={panes} onChange={setPanes} panels={Object.fromEntries(['library', 'monitor', 'timeline', 'notes'].map((k) => [k, { title: k[0].toUpperCase() + k.slice(1), content: <div className="grid h-full place-items-center text-[12px] text-white/35">{k}</div> }]))} />
          </div>
        </Card>

        <Card title="Skeleton" note="Shimmer placeholders while something loads.">
          {loading ? (
            <>
              <Skeleton className="h-[74px] w-[130px]" />
              <Skeleton className="h-[74px] w-[130px]" />
            </>
          ) : (
            <>
              <img src={img('#c4331c', '#3a0a14')} alt="" className="h-[74px] w-[130px] rounded-[11px] object-cover" />
              <img src={img('#2f5ad0', '#07122e')} alt="" className="h-[74px] w-[130px] rounded-[11px] object-cover" />
            </>
          )}
          <button className={btn} onClick={() => setLoading((l) => !l)}>{loading ? 'Finish loading' : 'Load again'}</button>
        </Card>
      </div>

      <ThemePanel show={show} />

      <IconGrid />

      <AnimatedIconGrid />

      <DialDemos />

      <DigitalSparkleDemo />

      <SparkleGrid />

      <UndoSnackbar
        until={until}
        duration={8000}
        message={`Deleting ${queued.length} files`}
        onUndo={() => { setUntil(null); show({ glyph: '↺', label: 'Cancelled', tone: 'plain' }) }}
        className="fixed bottom-6 left-1/2 -translate-x-1/2"
      />
      <CommandPalette open={palette} onClose={() => setPalette(false)} items={items} placeholder="Jump to a file…" />
      <ContextMenu
        at={menuAt}
        title="reef_pass.mp4"
        onClose={() => setMenuAt(null)}
        items={[
          { label: 'Play', keys: ['↵'], onSelect: () => show({ glyph: '▶', label: 'Playing', sub: 'reef_pass.mp4' }) },
          { label: 'Mark for trash', keys: ['X'], danger: true, onSelect: () => show({ glyph: '+', label: 'Queued', tone: 'danger' }) },
          'separator',
          { label: 'Copy path', onSelect: () => show({ glyph: '⧉', label: 'Path copied', tone: 'plain' }) }
        ]}
      />
    </div>
    <EffectPicker value={effect} onChange={(e) => { setEffect(e); show({ glyph: '■', label: 'Effect', sub: e, tone: 'plain' }) }} />
    </EffectProvider>
  )
}
