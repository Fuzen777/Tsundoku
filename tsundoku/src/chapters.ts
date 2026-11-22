import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useRef } from "react"
import type { ViewId } from "./navigation"
import { scrollWithin } from "./scrollWithin"

/*
  A view's chapters: the parts the sidebar lists under the view while it's open, to jump to, marking the one at the top
  as the view scrolls. Settings' sections, Rules' lists, the Downloads, Pending and History views' groups. A view lists
  them with `useChapters`, each chapter's element marked with `data-chapter` (or placed by `place`, for a list that draws
  only what's on screen), and passes `scrollRef` and `markShown` to its `Page`; the app passes them to the sidebar
  (`ChaptersContext`). Clicking the open view in the sidebar takes it back to its top the same way.
*/

/** A part of a view, as the sidebar lists it. */
export interface Chapter {
  /** Its element's `data-chapter`. */
  id: string
  title: string
  /** Its hover text, when the title is cut short or leaves something out (a folder's whole path). */
  hint?: string
  /** How many things it holds, after its title. */
  count?: number
  /** Its own parts, listed under it. */
  subchapters?: readonly Chapter[]
}

/** The open view's chapters, as the sidebar lists them. */
export interface ShownChapters {
  view: ViewId
  chapters: readonly Chapter[]
  /** The chapter at the top of the view (a subchapter when one has reached it), which the sidebar marks. */
  shown: string | null
}

/** A chapter the sidebar asked a view to go to, or its top (`id` null, the open view clicked again): a new object each
 * time, so asking again goes there again. */
export interface ChapterTarget {
  view: ViewId
  id: string | null
}

interface ChaptersLink {
  /** Tells the sidebar the open view's chapters. */
  publish: (chapters: ShownChapters) => void
  /** Takes a view's chapters back as it closes, unless the next view's have already taken their place: the next view
   * lists its own before the closing one's effects are cleaned up. */
  withdraw: (view: ViewId) => void
  /** The chapter the sidebar asked for last, if any since the view opened. */
  target: ChapterTarget | null
}

export const ChaptersContext = createContext<ChaptersLink>({ publish: () => {}, withdraw: () => {}, target: null })

/** How far below the top of the scrolling area a chapter's heading counts as having reached it, in pixels. */
const REACHED_TOP = 48

interface ChaptersOptions {
  view: ViewId
  chapters: readonly Chapter[]
  /** Makes a chapter drawn before going to it, such as unfolding the group that holds it. */
  reveal?: (id: string) => void
  /** For a list that draws only what's on screen: where a chapter starts in the scrolling area's content, in pixels,
   * drawn or not, or null while it isn't known yet (not read yet). Without it, chapters are found by `data-chapter`. */
  place?: (id: string) => number | null
}

/**
 * Lists a view's chapters in the sidebar, under the view, and goes to one when the sidebar asks: its element comes to the
 * top of the scrolling area (`scrollRef`, for `Page`'s), smoothly, or at once in a list that draws only what's on
 * screen (`place`), whose rows passed on the way would change its place as they're drawn. The one at the top is marked
 * as the view scrolls (`markShown`, for `Page`'s `onScroll`), but one gone to stays marked until the user scrolls
 * themselves, since the end of the view can keep it from reaching the top. Asked for the top (the open view clicked
 * again in the sidebar), it scrolls there smoothly. `goTo` goes to one for the view itself, such as a section another
 * view sent the user to.
 */
export function useChapters({ view, chapters, reveal, place }: ChaptersOptions) {
  const { publish, withdraw, target } = useContext(ChaptersContext)
  const scrollRef = useRef<HTMLDivElement>(null)
  // The latest chapters, reveal and place, for the handlers and effects, which shouldn't run again for each render.
  const latest = useRef({ chapters, reveal, place })
  useLayoutEffect(() => {
    latest.current = { chapters, reveal, place }
  })
  const shown = useRef<string | null>(null)
  // The chapter gone to last, marked until the user scrolls.
  const asked = useRef<string | null>(null)
  // A chapter gone to that isn't drawn yet (its group unfolds first): scrolled to once it is.
  const pending = useRef<string | null>(null)

  const tell = useCallback(
    () => publish({ view, chapters: latest.current.chapters, shown: shown.current }),
    [publish, view],
  )
  const atTop = useCallback((): string | null => {
    const area = scrollRef.current
    if (asked.current !== null || !area) return asked.current
    const ids = everyId(latest.current.chapters)
    // The last one whose heading has reached the top, else the first.
    let found = ids[0] ?? null
    const placed = latest.current.place
    if (placed) {
      for (const id of ids) {
        const top = placed(id)
        if (top !== null && top <= area.scrollTop + REACHED_TOP) found = id
      }
      return found
    }
    const reached = area.getBoundingClientRect().top + REACHED_TOP
    for (const id of ids) {
      const element = chapterElement(area, id)
      if (element && element.getBoundingClientRect().top <= reached) found = id
    }
    return found
  }, [])
  const markShown = useCallback(() => {
    const now = atTop()
    if (now === shown.current) return
    shown.current = now
    tell()
  }, [atTop, tell])

  const scrollToPending = useCallback(() => {
    const area = scrollRef.current
    if (!area || pending.current === null) return
    const placed = latest.current.place
    if (placed) {
      const top = placed(pending.current)
      if (top === null) return
      pending.current = null
      area.scrollTo({ top })
      return
    }
    const element = chapterElement(area, pending.current)
    if (!element) return
    pending.current = null
    scrollWithin(element, "start", "smooth")
  }, [])
  const goTo = useCallback(
    (id: string) => {
      asked.current = id
      pending.current = id
      latest.current.reveal?.(id)
      scrollToPending()
      markShown()
    },
    [scrollToPending, markShown],
  )
  const goToTop = useCallback(() => {
    asked.current = null
    pending.current = null
    scrollRef.current?.scrollTo({ top: 0, behavior: "smooth" })
  }, [])

  // A chapter unfolded by `reveal` is drawn now.
  useLayoutEffect(scrollToPending)

  // The sidebar's chapters follow the view's, and the one at the top with them.
  const listed = JSON.stringify(chapters)
  useLayoutEffect(() => {
    shown.current = atTop()
    tell()
  }, [listed, atTop, tell])
  useEffect(() => () => withdraw(view), [withdraw, view])

  useEffect(() => {
    if (target?.view !== view) return
    if (target.id === null) goToTop()
    else goTo(target.id)
  }, [target, view, goTo, goToTop])

  // Scrolling by hand (the wheel, a drag on the scrollbar, the keyboard) goes back to marking the one at the top.
  useEffect(() => {
    const area = scrollRef.current
    if (!area) return
    const scrolledByHand = () => {
      asked.current = null
    }
    const inputs = ["wheel", "pointerdown", "keydown", "touchstart"] as const
    for (const input of inputs) area.addEventListener(input, scrolledByHand, { passive: true })
    return () => {
      for (const input of inputs) area.removeEventListener(input, scrolledByHand)
    }
  }, [])

  return { scrollRef, markShown, goTo }
}

/** Every chapter's ID in the order they come, each one's subchapters after it. */
function everyId(chapters: readonly Chapter[]): string[] {
  return chapters.flatMap((chapter) => [chapter.id, ...everyId(chapter.subchapters ?? [])])
}

function chapterElement(area: HTMLElement, id: string): HTMLElement | null {
  return area.querySelector<HTMLElement>(`[data-chapter="${CSS.escape(id)}"]`)
}

/** The chapter holding the one shown, so the sidebar marks it too, or null when the one shown is a chapter itself. */
export function chapterHolding(chapters: readonly Chapter[], id: string | null): string | null {
  if (id === null) return null
  return chapters.find((chapter) => chapter.subchapters?.some((subchapter) => subchapter.id === id))?.id ?? null
}
