import type { ReactNode } from "react"
import type { ToolId } from "../bindings/ToolId"
import { strings } from "../strings"
import { toolIcons } from "../toolInfo"

/** A tool's icon, name and what it does, with anything in `children` on the right. */
export function ToolHeading({ id, children }: { id: ToolId; children?: ReactNode }) {
  const Icon = toolIcons[id]
  return (
    <div className="flex items-start gap-3">
      <div className="flex size-9 shrink-0 items-center justify-center rounded-md bg-input text-fg-secondary">
        <Icon size={18} />
      </div>
      <div className="min-w-0 flex-1">
        <p className="font-semibold">{strings.tools.names[id]}</p>
        <p className="text-fg-secondary">{strings.tools.descriptions[id]}</p>
      </div>
      {children}
    </div>
  )
}
