/**
 * The kit's shared motion language. Use these instead of ad-hoc values so every
 * component moves the same way: quick springs that slightly overshoot, and
 * content that fades in from a blur rather than just fading.
 */
export const SPRING = { type: 'spring', stiffness: 420, damping: 32 } as const // containers resizing (island)
export const SPRING_SNAPPY = { type: 'spring', stiffness: 520, damping: 34 } as const // menus, tooltips, pills
export const SPRING_SOFT = { type: 'spring', stiffness: 400, damping: 30 } as const // toasts, snackbars

/** Content entering: from blurred + slightly small to sharp. */
export const BLUR_IN = {
  initial: { opacity: 0, filter: 'blur(6px)', scale: 0.96 },
  animate: { opacity: 1, filter: 'blur(0px)', scale: 1 },
  exit: { opacity: 0, filter: 'blur(6px)', scale: 0.96 }
} as const

/** Panels popping up from their trigger (menus, popovers, dialogs). */
export const POP_IN = {
  initial: { opacity: 0, scale: 0.92, y: 8, filter: 'blur(8px)' },
  animate: { opacity: 1, scale: 1, y: 0, filter: 'blur(0px)' },
  exit: { opacity: 0, scale: 0.96, y: 4 }
} as const
