import { createContext, useCallback, useLayoutEffect, useState, type RefObject } from "react"

/*
  The row of a picture clicked in the Downloads, Pending and History views' Masonry and Gallery
  (`components/ItemTiles.tsx`), which shows in the details panel along the bottom of the view
  (`components/DetailsDock.tsx`).
*/

/** Where the details panel goes: an element at the bottom of the content layer, under the open view (App.tsx). */
export const DetailsDockContext = createContext<HTMLElement | null>(null)

/** Inside the details panel, the row's card is the panel itself: the outermost `Card` there draws no frame of its own,
 * and the cards inside it draw theirs again. */
export const FlatCardContext = createContext(false)

/** An item's row shown in the details panel, opened as asked (on its log, say); `opened` counts each ask, so asking
 * again opens it afresh. */
export interface ShownRow<Opening> {
  id: number
  opens: Opening
  opened: number
}

/** The row shown in the details panel, and how to show or hide it. */
export interface TileRows<Opening> {
  shown: ShownRow<Opening> | null
  show: (id: number, opens: Opening) => void
  hide: () => void
}

/** How far from the edge of the list a picture brought into sight stops, in pixels: past the focus outline. */
const SIGHT_MARGIN = 8
/** How long the picture is kept in sight while the details panel rises under it (DetailsDock), with room to spare. */
const PANEL_RISING_MS = 500

/**
 * The row shown in the details panel, one for the whole view (`container` holds its pictures). Nothing in the list moves
 * as it opens, closes or shows another; as the panel rises, the picture asked about is kept in sight if the panel comes
 * over it, scrolling no more than that takes.
 */
export function useShownRow<Opening>(container: RefObject<HTMLElement | null>): TileRows<Opening> {
  const [shown, setShown] = useState<ShownRow<Opening> | null>(null)
  const show = useCallback(
    (id: number, opens: Opening) => setShown((current) => ({ id, opens, opened: (current?.opened ?? 0) + 1 })),
    [],
  )
  const hide = useCallback(() => setShown(null), [])
  useLayoutEffect(() => {
    if (!shown) return
    const picture = container.current?.querySelector(`[data-tile="${shown.id}"]`)
    if (!picture) return
    bringIntoSight(picture)
    // The list's box shrinks as the panel rises under it, frame by frame.
    const box = boxOf(picture)
    if (!box) return
    const observer = new ResizeObserver(() => bringIntoSight(picture))
    observer.observe(box)
    const timer = setTimeout(() => observer.disconnect(), PANEL_RISING_MS)
    return () => {
      clearTimeout(timer)
      observer.disconnect()
    }
  }, [shown, container])
  return { shown, show, hide }
}

/** Scrolls the box a picture scrolls in, and nothing else (`scrollIntoView` would move the window too), just enough
 * for the picture to show whole, or its top when it's taller than the box. */
function bringIntoSight(picture: Element) {
  const scroller = scrollerOf(picture)
  if (!scroller) return
  const box = picture.getBoundingClientRect()
  const area = scroller.getBoundingClientRect()
  const belowBy = box.bottom + SIGHT_MARGIN - area.bottom
  const aboveBy = area.top - (box.top - SIGHT_MARGIN)
  if (aboveBy > 0) scroller.scrollTop -= aboveBy
  else if (belowBy > 0) scroller.scrollTop += Math.min(belowBy, -aboveBy)
}

/** The box an element scrolls in: the nearest one above it that scrolls. */
function scrollerOf(element: Element): HTMLElement | null {
  for (let box = boxOf(element); box; box = boxOf(box)) if (box.scrollHeight > box.clientHeight) return box
  return null
}

/** The nearest box above an element that would scroll it, whether it has anything to scroll yet or not. */
function boxOf(element: Element | null): HTMLElement | null {
  for (let parent = element?.parentElement; parent; parent = parent.parentElement) {
    const { overflowY } = getComputedStyle(parent)
    if (overflowY === "auto" || overflowY === "scroll") return parent
  }
  return null
}
