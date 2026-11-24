import { chapterHolding, type Chapter, type ShownChapters } from "../chapters"
import { strings } from "../strings"
import { classes } from "./styles"

interface SidebarProps {
  /** The open view's chapters (Settings' sections, the Downloads view's groups), the one at the top of the view marked,
   * or null when it has none. */
  chapters: ShownChapters | null
  /** The open view's name, over its chapters. */
  viewLabel: string
  /** Goes to one of the open view's chapters. */
  onSelectChapter: (id: string) => void
  /** Its place in the window's grid. */
  className?: string
}

/**
 * The open view's chapters, under the add bar, to jump to: under the view's name, each one's subchapters under it,
 * indented. Scrolls when they don't fit. Nothing shows for a view without chapters. The views themselves are tabs
 * along the top of the window (`ViewTabs`).
 */
export function Sidebar({ chapters, viewLabel, onSelectChapter, className }: SidebarProps) {
  const listed = chapters && chapters.chapters.length > 0 ? chapters : null
  return (
    <nav
      aria-label={strings.nav.chapters(viewLabel)}
      className={classes("flex min-h-0 flex-col gap-1 overflow-y-auto px-2 pb-3 *:shrink-0", className)}
    >
      {listed && (
        <>
          <h2 className="truncate px-3 pt-1 pb-0.5 text-xs font-semibold text-fg-secondary">{viewLabel}</h2>
          <ChapterList listed={listed} onSelect={onSelectChapter} />
        </>
      )}
    </nav>
  )
}

interface ChapterListProps {
  listed: ShownChapters
  onSelect: (id: string) => void
}

/** A view's chapters, each one's subchapters under it, indented further. The one at the top of the view is marked, and
 * so is the chapter holding it. */
function ChapterList({ listed, onSelect }: ChapterListProps) {
  const { chapters, shown } = listed
  const holding = chapterHolding(chapters, shown)
  return (
    <ul className="flex flex-col gap-0.5">
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

/** One of a view's chapters: a long title cut short, then how many it holds. A plain button rather than a `<Button>`,
 * since it's a full-width row (DESIGN.md → Buttons). */
function ChapterButton({ chapter, shown, holdsShown, nested = false, onSelect }: ChapterButtonProps) {
  return (
    <button
      type="button"
      aria-current={shown ? "location" : undefined}
      title={chapter.hint ?? chapter.title}
      onClick={() => onSelect(chapter.id)}
      className={classes(
        "flex h-7 w-full items-center gap-2 rounded-md pr-3 text-left text-[13px] transition-colors focus-visible:outline-2 focus-visible:outline-accent",
        nested ? "pl-7" : "pl-3",
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
