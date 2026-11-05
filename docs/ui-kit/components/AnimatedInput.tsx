import { AnimatePresence, motion, MotionConfig } from 'motion/react'
import { useRef, type InputHTMLAttributes } from 'react'
import { FX_EXIT, useFx, useKitEffect, type KitEffect } from './effects'

// Bouncier than the kit presets on purpose: typed characters visibly overshoot as they land.
const LAND = { type: 'spring', stiffness: 600, damping: 18 } as const

let nextId = 0

/** Give each character a stable id so only the characters actually inserted or removed animate. */
function diffIds(prev: { value: string; ids: number[] }, value: string): number[] {
  const a = [...prev.value], b = [...value] // by code point, matching how characters are rendered
  let start = 0
  while (start < a.length && start < b.length && a[start] === b[start]) start++
  let end = 0
  while (end < a.length - start && end < b.length - start && a[a.length - 1 - end] === b[b.length - 1 - end]) end++
  const added = Array.from({ length: b.length - start - end }, () => nextId++)
  return [...prev.ids.slice(0, start), ...added, ...prev.ids.slice(prev.ids.length - end)]
}

/**
 * Text input whose characters spring up from below as they're typed and drop away when deleted.
 * A real <input> with transparent text sits on top, so caret, selection, paste and IME all work.
 * Size and font it with className; keep the text short enough to fit (e.g. via maxLength).
 */
export function AnimatedInput({ value, onChange, className = '', effect, ...rest }: Omit<InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange'> & {
  value: string
  onChange: (value: string) => void
  effect?: KitEffect // with an effect, each typed character plays it instead of springing up
}) {
  const prev = useRef({ value: '', ids: [] as number[] })
  if (prev.current.value !== value) prev.current = { value, ids: diffIds(prev.current, value) }
  const { ids } = prev.current
  const fx = useKitEffect(effect)
  const plain = fx === 'default'

  return (
    <MotionConfig reducedMotion="user">
      <span className={`relative block overflow-hidden text-center [font-kerning:none] ${className}`}>
        <span aria-hidden className="relative block whitespace-pre">
          <AnimatePresence mode="popLayout" initial={false}>
            {[...value].map((ch, i) => (
              <motion.span
                key={ids[i]}
                className="inline-block"
                initial={plain ? { y: '0.7em', opacity: 0 } : false}
                animate={{ y: 0, opacity: 1, transition: LAND }}
                exit={plain ? { y: '0.7em', opacity: 0, transition: { duration: 0.16, ease: 'easeIn' } } : FX_EXIT}
              >
                {plain ? ch : <FxGlyph ch={ch} effect={fx} />}
              </motion.span>
            ))}
          </AnimatePresence>
          {/* Keeps the line height when empty. */}
          {!value && '\u200b'}
        </span>
        <input
          {...rest}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="absolute inset-0 w-full bg-transparent text-center text-transparent caret-white outline-none placeholder:text-white/25"
        />
      </span>
    </MotionConfig>
  )
}

/** One character under an effect: plays it (glyph variant) when typed. */
function FxGlyph({ ch, effect }: { ch: string; effect: KitEffect }) {
  const ref = useRef<HTMLSpanElement>(null)
  useFx(ref, null, { effect, kind: 'glyph' })
  return <span ref={ref} className="inline-block">{ch}</span>
}
