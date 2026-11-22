import { useLayoutEffect, useRef, type Ref, type RefObject } from "react"
import { chapterHolding, type Chapter, type ShownChapters } from "../chapters"
import { mainNavItems, settingsNavItem, type NavItem, type ViewId } from "../navigation"
import { viewKeys } from "../shortcuts"
import { strings } from "../strings"
import { classes } from "./styles"

interface SidebarProps {
  current: ViewId
  /** Opens a view, or takes the open one back to its top. */
  onSelect: (view: ViewId) => void
  /** A count shown beside a view's name, such as the downloads not done yet beside Queue. */
  counts?: Partial<Record<ViewId, number>>
  /** The open view's chapters (Settings' sections, the Downloads view's groups), listed under it, the one at the top of
   * the view marked. */
  chapters: ShownChapters | null
  /** Goes to one of the open view's chapters. */
  onSelectChapter: (id: string) => void
  /** Its place in the window's grid. */
  className?: string
}

/**
 * The views, with Settings at the bottom. The open view's chapters are listed under it. While Settings is open, it moves
 * up under the views to list its sections beneath it, sliding there and back. Scrolls when they don't fit.
 */
export function Sidebar({ current, onSelect, counts = {}, chapters, onSelectChapter, className }: SidebarProps) {
  const settingsOpen = current === settingsNavItem.id
  const sidebar = useRef<HTMLElement>(null)
  const settingsGroup = useRef<HTMLDivElement>(null)
  const sectionList = useRef<HTMLUListElement>(null)
  useSettingsSlide(settingsOpen, sidebar, settingsGroup, sectionList)
  // The chapters listed under a view: the open one's, when it has some.
  const listed = (view: ViewId) => (chapters?.view === view && chapters.chapters.length > 0 ? chapters : null)
  return (
    <nav
      ref={sidebar}
      className={classes("relative flex min-h-0 flex-col gap-1 overflow-y-auto px-2 pb-3 *:shrink-0", className)}
    >
      {mainNavItems.map((item) => (
        <div key={item.id} className="flex flex-col gap-1">
          <NavButton item={item} selected={item.id === current} count={counts[item.id]} onSelect={onSelect} />
          <ChapterList label={strings.nav.chapters(item.label)} listed={listed(item.id)} onSelect={onSelectChapter} />
        </div>
      ))}
      {!settingsOpen && <div className="flex-1" />}
      <div ref={settingsGroup} className="flex flex-col gap-1">
        <NavButton item={settingsNavItem} selected={settingsOpen} onSelect={onSelect} />
        {settingsOpen && (
          <ChapterList
            listRef={sectionList}
            label={strings.nav.chapters(settingsNavItem.label)}
            listed={listed(settingsNavItem.id)}
            onSelect={onSelectChapter}
          />
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

interface ChapterListProps {
  listed: ShownChapters | null
  onSelect: (id: string) => void
  label: string
  listRef?: Ref<HTMLUListElement>
}

/** A view's chapters under it, each one's subchapters under it, indented further. The one at the top of the view is
 * marked, and so is the chapter holding it. */
function ChapterList({ listed, onSelect, label, listRef }: ChapterListProps) {
  if (!listed) return null
  const { chapters, shown } = listed
  const holding = chapterHolding(chapters, shown)
  return (
    <ul ref={listRef} aria-label={label} className="flex flex-col gap-0.5">
      {chapters.map((chapter) => (
        <li key={chapter.id} className="flex flex-col gap-0.5">
          <ChapterButton
            chapter={chapter}
            shown={chapter.id === shown}
            holdsShown={chapter.id === holding}
            onSelect={onSelect}
          />
          {chapter.subchapters && chapter.subchapters.length > 0 && (
            <ul className="flex flex-col gap-0.5">
              {chapter.subchapters.map((subchapter) => (
                <li key={subchapter.id}>
                  <ChapterButton
                    chapter={subchapter}
                    shown={subchapter.id === shown}
                    holdsShown={false}
                    nested
                    onSelect={onSelect}
                  />
                </li>
              ))}
            </ul>
          )}
        </li>
      ))}
    </ul>
  )
}

interface ChapterButtonProps {
  chapter: Chapter
  /** Whether it's the chapter at the top of the view. */
  shown: boolean
  /** Whether the one at the top of the view is one of its subchapters. */
  holdsShown: boolean
  /** A subchapter, indented further. */
  nested?: boolean
  onSelect: (id: string) => void
}

/** One of a view's chapters under it: smaller, indented to the views' names (a subchapter further), a long title cut
 * short, then how many it holds. */
function ChapterButton({ chapter, shown, holdsShown, nested = false, onSelect }: ChapterButtonProps) {
  return (
    <button
      type="button"
      aria-current={shown ? "location" : undefined}
      title={chapter.hint ?? chapter.title}
      onClick={() => onSelect(chapter.id)}
      className={classes(
        "flex h-7 w-full items-center gap-2 rounded-md pr-3 text-left text-[13px] transition-colors focus-visible:outline-2 focus-visible:outline-accent",
        nested ? "pl-14" : "pl-10",
        shown || holdsShown ? "font-medium text-fg" : "text-fg-muted hover:bg-input hover:text-fg",
      )}
    >
      <span className="min-w-0 flex-1 truncate">{chapter.title}</span>
      {chapter.count !== undefined && (
        <span className="shrink-0 text-xs font-normal text-fg-muted tabular-nums">{chapter.count}</span>
      )}
    </button>
  )
}
