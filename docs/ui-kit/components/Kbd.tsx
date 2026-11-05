import type { ReactNode } from 'react'

/** Keyboard key hint. `<Kbd>Ctrl</Kbd><Kbd>K</Kbd>` */
export function Kbd({ children, className = '' }: { children: ReactNode; className?: string }) {
  return <kbd className={`kc font-sans ${className}`}>{children}</kbd>
}
