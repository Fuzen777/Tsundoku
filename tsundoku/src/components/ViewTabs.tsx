import { mainNavItems, settingsNavItem, type NavItem, type ViewId } from "../navigation"
import { viewKeys } from "../shortcuts"
import { strings } from "../strings"
import { classes } from "./styles"

interface ViewTabsProps {
  current: ViewId
  /** Opens a view, or takes the open one back to its top. */
  onSelect: (view: ViewId) => void
  /** A count shown beside a view's name, such as the downloads not done yet beside Downloads. */
  counts?: Partial<Record<ViewId, number>>
}

/**
 * The views as tabs along the top of the window, over the content layer, Settings at the right end. The open one is
 * marked by an accent line under it. When they don't fit with their names (a narrow window), only their icons and
 * counts show, each name staying in its hover text and for screen readers.
 */
export function ViewTabs({ current, onSelect, counts = {} }: ViewTabsProps) {
  const tab = (item: NavItem) => (
    <ViewTab key={item.id} item={item} selected={item.id === current} count={counts[item.id]} onSelect={onSelect} />
  )
  return (
    <nav aria-label={strings.nav.views} className="@container flex min-w-0 flex-1 items-center gap-1">
      {mainNavItems.map(tab)}
      <div className="flex-1" />
      {tab(settingsNavItem)}
    </nav>
  )
}

interface ViewTabProps {
  item: NavItem
  selected: boolean
  count?: number
  onSelect: (view: ViewId) => void
}

/** A tab, its icon then its name: a plain button rather than a `<Button>`, like the sidebar's chapters (DESIGN.md →
 * Buttons). */
function ViewTab({ item, selected, count, onSelect }: ViewTabProps) {
  const Icon = item.icon
  return (
    <button
      type="button"
      aria-current={selected ? "page" : undefined}
      title={strings.shortcuts.withKeys(item.label, viewKeys(item.id))}
      onClick={() => onSelect(item.id)}
      className={classes(
        "relative flex h-9 shrink-0 items-center gap-2 rounded-md px-3 transition-colors focus-visible:outline-2 focus-visible:outline-accent",
        selected ? "text-fg" : "text-fg-secondary hover:bg-input hover:text-fg",
      )}
    >
      <Icon size={16} />
      {/* Every tab's name fits from about 50 rem (with two-digit counts); under 52 rem, the icons alone. */}
      <span className="@max-[52rem]:sr-only">{item.label}</span>
      {count !== undefined && count > 0 && (
        <span className="min-w-5 rounded-full bg-accent-fill px-1.5 text-center text-xs font-medium text-on-accent tabular-nums">
          {count}
        </span>
      )}
      <span
        className={classes(
          "absolute inset-x-3 bottom-0 h-0.5 rounded-full bg-accent transition-opacity",
          selected ? "opacity-100" : "opacity-0",
        )}
      />
    </button>
  )
}
