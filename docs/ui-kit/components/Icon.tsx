import { ICON_DATA } from './icon-data'

/**
 * Numbered icon set (from the user-supplied icons-motion-kit.svg): 1–48 solid, 49–79 line.
 * Filled shapes in currentColor, so they take the text colour.
 *   <Icon n={12} />   <Icon n={60} size={32} title="Next" />
 */
export const ICON_COUNT = ICON_DATA.length

/** Icon `n` (1-based). Decorative unless you pass `title`. */
export function Icon({ n, size = 24, title, className = '' }: { n: number; size?: number; title?: string; className?: string }) {
  const [viewBox, ...paths] = ICON_DATA[n - 1]
  return (
    <svg width={size} height={size} viewBox={viewBox} fill="currentColor" className={className} role={title ? 'img' : undefined} aria-hidden={title ? undefined : true}>
      {title && <title>{title}</title>}
      {paths.map((d, i) => <path key={i} d={d} />)}
    </svg>
  )
}
