import type { LucideIcon } from "lucide-react"

interface EmptyStateProps {
  icon: LucideIcon
  title: string
  body: string
}

/** What a list shows while it has nothing in it. */
export function EmptyState({ icon: Icon, title, body }: EmptyStateProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 pb-12 text-center">
      <Icon size={40} strokeWidth={1.25} className="mb-1 text-fg-muted" />
      <p className="text-base font-semibold">{title}</p>
      <p className="max-w-sm text-fg-secondary">{body}</p>
    </div>
  )
}
