import type { ReactNode } from "react"
import type { LucideIcon } from "lucide-react"

interface EmptyStateProps {
  icon: LucideIcon
  title: string
  body: string
  /** Under the explanation, such as the ways to add links. */
  children?: ReactNode
}

/** What a list shows while it has nothing in it. */
export function EmptyState({ icon: Icon, title, body, children }: EmptyStateProps) {
  return (
    <div className="flex min-h-full flex-col items-center justify-center gap-2 pb-12 text-center">
      <Icon size={40} strokeWidth={1.25} className="mb-1 text-fg-muted" />
      <p className="text-base font-semibold">{title}</p>
      <p className="max-w-sm text-fg-secondary">{body}</p>
      {children && <div className="mt-3 flex w-full justify-center">{children}</div>}
    </div>
  )
}
