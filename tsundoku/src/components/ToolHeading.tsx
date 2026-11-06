import type { ReactNode } from "react"
import type { LucideIcon } from "lucide-react"
import type { ToolId } from "../bindings/ToolId"
import { strings } from "../strings"
import { toolIcons } from "../toolInfo"

interface IconHeadingProps {
  icon: LucideIcon
  title: string
  description: string
  children?: ReactNode
}

/** An icon on a tile, a title and what it's about, with anything in `children` on the right: the top of a tool's card,
 * or of a first-run choice. */
export function IconHeading({ icon: Icon, title, description, children }: IconHeadingProps) {
  return (
    <div className="flex items-start gap-3">
      <div className="flex size-9 shrink-0 items-center justify-center rounded-md bg-input text-fg-secondary">
        <Icon size={18} />
      </div>
      <div className="min-w-0 flex-1">
        <p className="font-semibold">{title}</p>
        <p className="text-fg-secondary">{description}</p>
      </div>
      {children}
    </div>
  )
}

/** A tool's icon, name and what it does, with anything in `children` on the right. */
export function ToolHeading({ id, children }: { id: ToolId; children?: ReactNode }) {
  return (
    <IconHeading icon={toolIcons[id]} title={strings.tools.names[id]} description={strings.tools.descriptions[id]}>
      {children}
    </IconHeading>
  )
}
