import { useRef, useState, type ButtonHTMLAttributes, type ReactNode } from 'react'
import { useFx, useKitEffect, useTick, type KitEffect } from './effects'

/** Shimmering placeholder while content loads (with an effect: replays it instead). Size it with className. */
export function Skeleton({ className = '', effect }: { className?: string; effect?: KitEffect }) {
  const ref = useRef<HTMLDivElement>(null)
  const plain = useKitEffect(effect) === 'default'
  useFx(ref, useTick(1400, !plain), { effect })
  return <div ref={ref} aria-hidden="true" className={`${plain ? 'skel' : 'bg-white/8'} rounded-[11px] ${className}`} />
}

/**
 * Image that wipes across to a second image on hover (e.g. a later video frame, a "before/after").
 * The hover is taken from the nearest `.group` ancestor, so the whole card can trigger it.
 */
export function ImageReveal({ src, hoverSrc, alt = '', className = '', effect }: { src: string; hoverSrc?: string | null; alt?: string; className?: string; effect?: KitEffect }) {
  const [hover, setHover] = useState(false)
  const img = useRef<HTMLImageElement>(null)
  const plain = useFx(img, hover, { effect, enabled: hover }) === 'default'
  return (
    <div className={`group relative overflow-hidden ${className}`} onPointerEnter={() => setHover(true)} onPointerLeave={() => setHover(false)}>
      <img src={src} alt={alt} className="absolute inset-0 h-full w-full object-cover transition-transform duration-500 group-hover:scale-110 motion-reduce:transition-none" />
      {/* Default: wipes across (CSS). With an effect: the second image plays it on hover. */}
      {hoverSrc && (plain
        ? <img src={hoverSrc} alt="" className="reveal absolute inset-0 h-full w-full object-cover" />
        : hover && <img ref={img} src={hoverSrc} alt="" className="absolute inset-0 h-full w-full object-cover" />)}
    </div>
  )
}

/** Overlapping round thumbnails that fan out on hover, with a "+N" for the rest. */
export function AvatarStack({ items, max = 5, size = 30, ring = 'var(--kit-ink)', effect }: {
  items: { key: string; src?: string; label: string }[]
  max?: number
  size?: number
  ring?: string // outline colour: match the background behind the stack
  effect?: KitEffect // with an effect, the avatars play it one after another as the stack fans out
}) {
  const [fans, setFans] = useState(0)
  const extra = items.length - max
  const style = { width: size, height: size, boxShadow: `0 0 0 2px ${ring}` }
  return (
    <div className="stack flex" onPointerEnter={() => setFans((n) => n + 1)}>
      {items.slice(0, max).map((it, i) => (
        <Avatar key={it.key} style={style} fans={fans} order={i} effect={effect} title={it.label} className="grid shrink-0 place-items-center overflow-hidden rounded-full bg-white/10 text-[11px] font-semibold">
          {it.src ? <img src={it.src} alt={it.label} className="h-full w-full object-cover" /> : it.label.slice(0, 2).toUpperCase()}
        </Avatar>
      ))}
      {extra > 0 && <Avatar style={style} fans={fans} order={max} effect={effect} className="grid shrink-0 place-items-center rounded-full bg-white/15 text-[11px] font-semibold">+{extra}</Avatar>}
    </div>
  )
}

function Avatar({ fans, order, effect, children, ...rest }: { fans: number; order: number; effect?: KitEffect; children: ReactNode; style: object; className: string; title?: string }) {
  const ref = useRef<HTMLSpanElement>(null)
  useFx(ref, fans, { effect, enabled: fans > 0, delay: order * 45 })
  return <span ref={ref} {...rest}>{children}</span>
}

/** Button with a light sweep on hover. Style it like any button. */
export function SheenButton({ className = '', children, effect, onPointerEnter, ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { children: ReactNode; effect?: KitEffect }) {
  const [hovers, setHovers] = useState(0)
  const ref = useRef<HTMLButtonElement>(null)
  // Default: a light sweep on hover. With an effect: the button plays it on hover instead.
  const plain = useFx(ref, hovers, { effect, enabled: hovers > 0 }) === 'default'
  return (
    <button
      ref={ref}
      type="button"
      {...rest}
      onPointerEnter={(e) => { setHovers((n) => n + 1); onPointerEnter?.(e) }}
      className={`${plain ? 'sheen' : ''} ${className}`}
    >
      {children}
    </button>
  )
}
