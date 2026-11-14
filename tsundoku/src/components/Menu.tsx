import { MoreHorizontal, type LucideIcon } from "lucide-react"
import { Popover } from "./Popover"
import { classes } from "./styles"

export interface MenuItem {
  label: string
  onSelect: () => void
  /** A destructive action, in the danger color. */
  danger?: boolean
  /** Shown before the label in a right-click menu (ContextMenu), as the button it stands for. */
  icon?: LucideIcon
}

interface MenuProps {
  /** What the button opens, for screen readers and its tooltip. */
  label: string
  items: readonly MenuItem[]
  /** The button's icon; three dots by default. */
  icon?: LucideIcon
}

/** A ghost icon button that opens a short list of actions under it. Escape or a click elsewhere closes it. */
export function Menu({ label, items, icon = MoreHorizontal }: MenuProps) {
  return (
    <Popover label={label} icon={icon} role="menu" panelClassName="flex min-w-44 flex-col p-1">
      {(close) =>
        items.map((item) => (
          <button
            key={item.label}
            type="button"
            role="menuitem"
            onClick={() => {
              close()
              item.onSelect()
            }}
            className={classes(
              "rounded px-3 py-1.5 text-left transition-colors hover:bg-input focus-visible:outline-2 focus-visible:outline-accent",
              item.danger ? "text-danger" : "text-fg",
            )}
          >
            {item.label}
          </button>
        ))
      }
    </Popover>
  )
}
