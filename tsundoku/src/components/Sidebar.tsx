import {
  mainNavItems,
  SETTINGS_SECTIONS,
  settingsNavItem,
  type NavItem,
  type SettingsSectionId,
  type ViewId,
} from "../navigation"
import { viewKeys } from "../shortcuts"
import { strings } from "../strings"
import { classes } from "./styles"

interface SidebarProps {
  current: ViewId
  onSelect: (view: ViewId) => void
  /** A count shown beside a view's name, such as the downloads not done yet beside Queue. */
  counts?: Partial<Record<ViewId, number>>
  /** The section of Settings at the top of its view, marked among the sections listed under Settings. */
  settingsSection: SettingsSectionId | null
  /** Scrolls Settings to one of its sections. */
  onSelectSection: (section: SettingsSectionId) => void
  /** Its place in the window's grid. */
  className?: string
}

/** The views, then Settings, with Settings' sections under it while it's open. Scrolls when they don't fit. */
export function Sidebar({ current, onSelect, counts = {}, settingsSection, onSelectSection, className }: SidebarProps) {
  const settingsOpen = current === settingsNavItem.id
  return (
    <nav className={classes("flex min-h-0 flex-col gap-1 overflow-y-auto px-2 pb-3", className)}>
      {mainNavItems.map((item) => (
        <NavButton
          key={item.id}
          item={item}
          selected={item.id === current}
          count={counts[item.id]}
          onSelect={onSelect}
        />
      ))}
      <NavButton item={settingsNavItem} selected={settingsOpen} onSelect={onSelect} />
      {settingsOpen && (
        <ul aria-label={strings.settings.jumpTo} className="flex flex-col gap-0.5">
          {SETTINGS_SECTIONS.map(({ id, title }) => (
            <li key={id}>
              <SectionButton title={title} shown={id === settingsSection} onSelect={() => onSelectSection(id)} />
            </li>
          ))}
        </ul>
      )}
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
      title={strings.shortcuts.withKeys(item.label, viewKeys(item.id))}
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
        <span className="ml-auto min-w-5 rounded-full bg-accent-fill px-1.5 text-center text-xs font-medium text-on-accent tabular-nums">
          {count}
        </span>
      )}
    </button>
  )
}

interface SectionButtonProps {
  title: string
  /** Whether it's the section at the top of Settings. */
  shown: boolean
  onSelect: () => void
}

/** One of Settings' sections under Settings: smaller, indented to the views' names, a long title cut short. */
function SectionButton({ title, shown, onSelect }: SectionButtonProps) {
  return (
    <button
      type="button"
      aria-current={shown ? "location" : undefined}
      title={title}
      onClick={onSelect}
      className={classes(
        "flex h-7 w-full items-center truncate rounded-md pr-3 pl-10 text-left text-[13px] transition-colors focus-visible:outline-2 focus-visible:outline-accent",
        shown ? "font-medium text-fg" : "text-fg-muted hover:bg-input hover:text-fg",
      )}
    >
      <span className="truncate">{title}</span>
    </button>
  )
}
