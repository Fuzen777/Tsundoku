import type { ReactNode } from "react"
import { CircleAlert, CircleCheck, TriangleAlert, type LucideIcon } from "lucide-react"

export type StatusTone = "success" | "warning" | "danger" | "neutral"

const TONE_ICONS: Record<StatusTone, { icon?: LucideIcon; className: string }> = {
  success: { icon: CircleCheck, className: "text-success" },
  warning: { icon: TriangleAlert, className: "text-warning" },
  danger: { icon: CircleAlert, className: "text-danger" },
  neutral: { className: "" },
}

interface StatusMessageProps {
  tone: StatusTone
  message: string
  /** A detail shown smaller underneath and selectable for copying, such as an error's cause or a file's path. */
  detail?: string
  /** Actions about the message, such as Open File, under it. */
  children?: ReactNode
}

/** An outcome or a notice: what happened, with an icon for its tone. */
export function StatusMessage({ tone, message, detail, children }: StatusMessageProps) {
  const { icon: Icon, className } = TONE_ICONS[tone]
  return (
    <div role={tone === "danger" ? "alert" : "status"} className="flex gap-2">
      {Icon && <Icon size={16} className={`mt-0.5 shrink-0 ${className}`} />}
      <div className="flex min-w-0 flex-col gap-0.5">
        <p className={tone === "neutral" ? "text-fg-secondary" : "text-fg"}>{message}</p>
        {detail && <p className="text-xs break-words text-fg-muted select-text">{detail}</p>}
        {children && <div className="mt-1.5 flex flex-wrap gap-2">{children}</div>}
      </div>
    </div>
  )
}
