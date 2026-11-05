import { CircleAlert, CircleCheck, type LucideIcon } from "lucide-react"

export type StatusTone = "success" | "danger" | "neutral"

const TONE_ICONS: Record<StatusTone, { icon?: LucideIcon; className: string }> = {
  success: { icon: CircleCheck, className: "text-success" },
  danger: { icon: CircleAlert, className: "text-danger" },
  neutral: { className: "" },
}

interface StatusMessageProps {
  tone: StatusTone
  message: string
  /** The technical cause of an error, shown smaller underneath and selectable for copying. */
  detail?: string
}

/** The outcome of an action, under it: what happened, with an icon for its tone. */
export function StatusMessage({ tone, message, detail }: StatusMessageProps) {
  const { icon: Icon, className } = TONE_ICONS[tone]
  return (
    <div role={tone === "danger" ? "alert" : "status"} className="flex gap-2">
      {Icon && <Icon size={16} className={`mt-0.5 shrink-0 ${className}`} />}
      <div className="min-w-0">
        <p className={tone === "neutral" ? "text-fg-secondary" : "text-fg"}>{message}</p>
        {detail && <p className="text-xs break-words text-fg-muted select-text">{detail}</p>}
      </div>
    </div>
  )
}
