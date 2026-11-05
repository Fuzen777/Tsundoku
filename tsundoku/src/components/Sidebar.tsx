import { mainNavItems, settingsNavItem, type NavItem, type ViewId } from "../navigation"
import { classes } from "./styles"

interface SidebarProps {
  current: ViewId
  onSelect: (view: ViewId) => void
  /** A count shown beside a view's name, such as the downloads not done yet beside Queue. */
  counts?: Partial<Record<ViewId, number>>
  /** Its place in the window's grid. */
  className?: string
}

export function Sidebar({ current, onSelect, counts = {}, className }: SidebarProps) {
  return (
    <nav className={classes("flex flex-col gap-1 px-2 pb-3", className)}>
      {mainNavItems.map((item) => (
        <NavButton
          key={item.id}
          item={item}
          selected={item.id === current}
          count={counts[item.id]}
          onSelect={onSelect}
        />
      ))}
      <div className="flex-1" />
      <NavButton item={settingsNavItem} selected={current === settingsNavItem.id} onSelect={onSelect} />
    </nav>
  )
}

interface NavButtonProps {
  item: NavItem
  selected: boolean
  count?: number
  onSelect: (view: ViewId) => void
}

/** A full-width row, so it stays a plain button rather than a `<Button>` (DESIGN.md → Buttons). */
function NavButton({ item, selected, count, onSelect }: NavButtonProps) {
  const Icon = item.icon
  return (
    <button
      type="button"
      aria-current={selected ? "page" : undefined}
      onClick={() => onSelect(item.id)}
      className={classes(
        "relative flex h-9 items-center gap-3 rounded-md px-3 text-left transition-colors focus-visible:outline-2 focus-visible:outline-accent",
        selected ? "bg-input text-fg" : "text-fg-secondary hover:bg-input hover:text-fg",
      )}
    >
      <span
        className={classes(
          "absolute top-1/2 left-0 h-4 w-[3px] -translate-y-1/2 rounded-full bg-accent transition-opacity",
          selected ? "opacity-100" : "opacity-0",
        )}
      />
      <Icon size={16} />
      {item.label}
      {count !== undefined && count > 0 && (
        <span className="ml-auto min-w-5 rounded-full bg-accent px-1.5 text-center text-xs font-medium text-on-accent tabular-nums">
          {count}
        </span>
      )}
    </button>
  )
}
