import { useLayoutEffect, useRef } from "react"
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

/** How far in from each side of its tab the accent line starts and ends, in pixels (the tabs' padding). */
const UNDERLINE_INSET = 12
/** The accent line's own width, in pixels, which it's stretched or squeezed from to its tab's: moved and sized by a
 * transform alone, it glides on even while the view it opens is being drawn. */
const UNDERLINE_WIDTH = 100

/**
 * The views as tabs along the top of the window, over the content layer, Settings at the right end. The open one is
 * marked by an accent line under it, which glides to the next tab opened (DESIGN.md → Motion). When they don't fit with
 * their names (a narrow window), only their icons and counts show, each name staying in its hover text and for screen
 * readers.
 */
export function ViewTabs({ current, onSelect, counts = {} }: ViewTabsProps) {
  const nav = useRef<HTMLElement>(null)
  const line = useRef<HTMLSpanElement>(null)
  // The tab the line was last placed under, and where.
  const placedFor = useRef<ViewId | null>(null)
  const placedAt = useRef("")
  // Placed under the open tab before the first paint, and again whenever a tab's width changes (a count, the names
  // leaving or coming back), at once; only a change of tab glides.
  useLayoutEffect(() => {
    const tabs = nav.current
    const mark = line.current
    if (!tabs || !mark) return
    const place = () => {
      const tab = tabs.querySelector<HTMLElement>("[aria-current=page]")
      if (!tab) return
      const transform = `translateX(${tab.offsetLeft + UNDERLINE_INSET}px) scaleX(${(tab.offsetWidth - 2 * UNDERLINE_INSET) / UNDERLINE_WIDTH})`
      if (placedAt.current === transform) return
      const glides = placedFor.current !== null && placedFor.current !== current
      placedFor.current = current
      placedAt.current = transform
      mark.style.transition = glides ? "" : "none"
      mark.style.transform = transform
      mark.style.visibility = "visible"
    }
    place()
    const observer = new ResizeObserver(place)
    for (const tab of tabs.children) observer.observe(tab)
    return () => observer.disconnect()
  }, [current])

  const tab = (item: NavItem) => (
    <ViewTab key={item.id} item={item} selected={item.id === current} count={counts[item.id]} onSelect={onSelect} />
  )
  return (
    <nav ref={nav} aria-label={strings.nav.views} className="@container relative flex min-w-0 flex-1 items-center gap-1">
      {mainNavItems.map(tab)}
      <div className="flex-1" />
      {tab(settingsNavItem)}
      <span
        ref={line}
        aria-hidden
        style={{ visibility: "hidden", width: UNDERLINE_WIDTH }}
        className="pointer-events-none absolute bottom-0 left-0 h-0.5 origin-left rounded-full bg-accent transition-transform duration-200 ease-decelerate motion-reduce:transition-none"
      />
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
        "flex h-9 shrink-0 items-center gap-2 rounded-md px-3 transition-colors focus-visible:outline-2 focus-visible:outline-accent",
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
    </button>
  )
}
