import { createContext, useCallback, useLayoutEffect, useState, type RefObject } from "react"

/*
  The row of a picture clicked in the Downloads, Pending and History views' Masonry and Gallery
  (`components/ItemTiles.tsx`), which shows in the details panel along the bottom of the view
  (`components/DetailsDock.tsx`).
*/

/** Where the details panel goes: an element at the bottom of the content layer, under the open view (App.tsx). */
export const DetailsDockContext = createContext<HTMLElement | null>(null)

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

/**
 * The row shown in the details panel, one for the whole view (`container` holds its pictures). Nothing in the list moves
 * as it opens, closes or shows another; once the panel is drawn, the picture asked about is brought into sight if the
 * panel came over it, scrolling no more than that takes.
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
    if (picture) bringIntoSight(picture)
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
  for (let parent = element.parentElement; parent; parent = parent.parentElement) {
    const { overflowY } = getComputedStyle(parent)
    if ((overflowY === "auto" || overflowY === "scroll") && parent.scrollHeight > parent.clientHeight) return parent
  }
  return null
}
