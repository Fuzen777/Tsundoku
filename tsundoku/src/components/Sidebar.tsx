import { useLayoutEffect, useRef, type RefObject } from "react"
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

/**
 * The views, with Settings at the bottom. While Settings is open, it moves up under the views to list its sections
 * beneath it, sliding there and back. Scrolls when they don't fit.
 */
export function Sidebar({ current, onSelect, counts = {}, settingsSection, onSelectSection, className }: SidebarProps) {
  const settingsOpen = current === settingsNavItem.id
  const sidebar = useRef<HTMLElement>(null)
  const settingsGroup = useRef<HTMLDivElement>(null)
  const sectionList = useRef<HTMLUListElement>(null)
  useSettingsSlide(settingsOpen, sidebar, settingsGroup, sectionList)
  return (
    <nav
      ref={sidebar}
      className={classes("relative flex min-h-0 flex-col gap-1 overflow-y-auto px-2 pb-3 *:shrink-0", className)}
    >
      {mainNavItems.map((item) => (
        <NavButton
          key={item.id}
          item={item}
          selected={item.id === current}
          count={counts[item.id]}
          onSelect={onSelect}
        />
      ))}
      {!settingsOpen && <div className="flex-1" />}
      <div ref={settingsGroup} className="flex flex-col gap-1">
        <NavButton item={settingsNavItem} selected={settingsOpen} onSelect={onSelect} />
        {settingsOpen && (
          <ul ref={sectionList} aria-label={strings.settings.jumpTo} className="flex flex-col gap-0.5">
            {SETTINGS_SECTIONS.map(({ id, title }) => (
              <li key={id}>
                <SectionButton title={title} shown={id === settingsSection} onSelect={() => onSelectSection(id)} />
              </li>
            ))}
          </ul>
        )}
      </div>
    </nav>
  )
}

/** How long Settings takes to slide up or down the sidebar, in milliseconds. */
const SETTINGS_SLIDE_MS = 250

/**
 * Slides Settings (and its sections) from where it was to where it is now when it opens or closes, rather than letting
 * it jump: its place is followed while the sidebar's size changes, then, once it has moved, it's drawn back at the old
 * place and eased to the new one, while its sections fade in. Nothing moves when Windows asks for less motion.
 */
function useSettingsSlide(
  settingsOpen: boolean,
  sidebar: RefObject<HTMLElement | null>,
  settingsGroup: RefObject<HTMLDivElement | null>,
  sectionList: RefObject<HTMLUListElement | null>,
) {
  const lastTop = useRef<number | null>(null)

  // Where Settings sits as the window is resized, so a slide starts from where it really was.
  useLayoutEffect(() => {
    const nav = sidebar.current
    if (!nav) return
    const observer = new ResizeObserver(() => {
      if (settingsGroup.current) lastTop.current = settingsGroup.current.offsetTop
    })
    observer.observe(nav)
    return () => observer.disconnect()
  }, [sidebar, settingsGroup])

  useLayoutEffect(() => {
    const group = settingsGroup.current
    if (!group) return
    const top = group.offsetTop
    const from = lastTop.current
    lastTop.current = top
    if (from === null || from === top || matchMedia("(prefers-reduced-motion: reduce)").matches) return
    const easing = "cubic-bezier(0.2, 0, 0, 1)"
    group.animate([{ transform: `translateY(${from - top}px)` }, { transform: "none" }], {
      duration: SETTINGS_SLIDE_MS,
      easing,
    })
    sectionList.current?.animate([{ opacity: 0 }, { opacity: 1 }], { duration: SETTINGS_SLIDE_MS, easing: "ease-out" })
  }, [settingsOpen, settingsGroup, sectionList])
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
