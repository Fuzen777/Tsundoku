import { useCallback, useLayoutEffect, useRef, useState, type RefObject } from "react"

/*
  The row shown under a line of pictures in the Downloads, Pending and History views' Masonry and Gallery
  (`components/ItemTiles.tsx`).
*/

/** An item's row shown under its line of pictures, opened as asked (on its log, say); `opened` counts each ask, so
 * asking again opens it afresh. */
export interface ShownRow<Opening> {
  id: number
  opens: Opening
  opened: number
}

/** The row shown under a line of pictures, and how to show or hide it. */
export interface TileRows<Opening> {
  shown: ShownRow<Opening> | null
  show: (id: number, opens: Opening) => void
  hide: () => void
  /** The shown row's box, brought into sight once it's drawn. */
  shownBox: RefObject<HTMLDivElement | null>
}

/**
 * The row shown under a line of pictures (`container` holds them). A picture clicked stays where it was on screen as
 * the row moves from under another line to under its own, and the row is then brought into sight, scrolling no more
 * than that takes.
 */
export function useShownRow<Opening>(container: RefObject<HTMLElement | null>): TileRows<Opening> {
  const [shown, setShown] = useState<ShownRow<Opening> | null>(null)
  const shownBox = useRef<HTMLDivElement>(null)
  // Where the picture asked about was, so it stays there.
  const held = useRef<{ id: number; top: number } | null>(null)
  const show = useCallback(
    (id: number, opens: Opening) => {
      const top = tileOf(container.current, id)?.getBoundingClientRect().top
      held.current = top === undefined ? null : { id, top }
      setShown((current) => ({ id, opens, opened: (current?.opened ?? 0) + 1 }))
    },
    [container],
  )
  const hide = useCallback(() => setShown(null), [])
  // Before it's painted: the picture asked about back where it was (a row shown higher up has closed), then the row
  // shown under it brought into sight.
  useLayoutEffect(() => {
    const before = held.current
    held.current = null
    const picture = before && tileOf(container.current, before.id)
    if (before && picture) {
      const moved = picture.getBoundingClientRect().top - before.top
      const scroller = scrollerOf(picture)
      if (moved !== 0 && scroller) scroller.scrollTop += moved
    }
    if (shown) shownBox.current?.scrollIntoView({ block: "nearest" })
  }, [shown, container])
  return { shown, show, hide, shownBox }
}

function tileOf(container: HTMLElement | null, id: number): Element | null | undefined {
  return container?.querySelector(`[data-tile="${id}"]`)
}

/** The box an element scrolls in: the nearest one above it that scrolls, or the page. */
function scrollerOf(element: Element): Element | null {
  for (let parent = element.parentElement; parent; parent = parent.parentElement) {
    const { overflowY } = getComputedStyle(parent)
    if ((overflowY === "auto" || overflowY === "scroll") && parent.scrollHeight > parent.clientHeight) return parent
  }
  return document.scrollingElement
}
