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
  /** In a `Menu`, the heading of the items that share it, shown over the first of them: Apply a Rule… lists the
   * manual rules, then the automatic ones. */
  group?: string
}

interface MenuProps {
  /** What the button opens, for screen readers and its tooltip. */
  label: string
  items: readonly MenuItem[]
  /** The button's icon; three dots by default. */
  icon?: LucideIcon
}

/** A ghost icon button that opens a short list of actions under it, under the headings of their groups, if any. Escape
 * or a click elsewhere closes it. */
export function Menu({ label, items, icon = MoreHorizontal }: MenuProps) {
  return (
    <Popover label={label} icon={icon} role="menu" panelClassName="flex min-w-44 flex-col p-1">
      {(close) =>
        items.flatMap((item, index) => [
          item.group !== undefined && item.group !== items[index - 1]?.group && (
            <p
              key={`group-${index}`}
              role="presentation"
              className={classes(
                "px-3 pb-1 text-xs font-medium text-fg-muted",
                index > 0 ? "mt-1 border-t border-line pt-2.5" : "pt-1.5",
              )}
            >
              {item.group}
            </p>
          ),
          <button
            // Two items can share a label (two rules with one name), so their place tells them apart.
            key={index}
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
          </button>,
        ])
      }
    </Popover>
  )
}
