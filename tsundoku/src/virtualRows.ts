import { useCallback, useEffect, useMemo, useState, type RefObject } from "react"

/*
  A long list that draws only the rows on screen and a few around them. The rows not drawn are stood in for by two
  spacers, above and below, so the rows drawn stay in the page's normal flow: each is measured as it's drawn, so rows of
  any height work (one expanded, say), and when a row above the screen changes height, the browser's scroll anchoring
  keeps what's on screen still.
*/

/** Where each row starts, from the list's top, and the height of them all: measured heights where known, else the
 * estimate. */
export interface RowLayout {
  starts: number[]
  total: number
}

export function layOutRows(keys: readonly number[], heights: ReadonlyMap<number, number>, estimate: number): RowLayout {
  const starts: number[] = []
  let total = 0
  for (const key of keys) {
    starts.push(total)
    total += heights.get(key) ?? estimate
  }
  return { starts, total }
}

/** The rows to draw: from `first` to `last`, both drawn (none when `last` is below `first`), and the space the rows
 * above and below them take. */
export interface RowRange {
  first: number
  last: number
  before: number
  after: number
}

/** The rows that are, even partly, between `from` and `to`, in pixels from the list's top. */
export function rowsBetween({ starts, total }: RowLayout, from: number, to: number): RowRange {
  if (starts.length === 0) return { first: 0, last: -1, before: 0, after: 0 }
  const first = Math.max(0, startingAfter(starts, from) - 1)
  const last = Math.max(first, Math.min(starts.length - 1, startingAfter(starts, to) - 1))
  const end = last + 1 < starts.length ? starts[last + 1] : total
  return { first, last, before: starts[first], after: total - end }
}

/** The first row starting below `position`, or the number of rows if none does. */
function startingAfter(starts: readonly number[], position: number): number {
  let low = 0
  let high = starts.length
  while (low < high) {
    const middle = (low + high) >> 1
    if (starts[middle] <= position) low = middle + 1
    else high = middle
  }
  return low
}

interface VirtualRowsOptions {
  /** Each row's key, in order: the ID its measured height is kept by. Keep the same array while the rows don't change. */
  keys: readonly number[]
  /** The element that scrolls. */
  scroller: RefObject<HTMLElement | null>
  /** Where the list starts in the scrolled content, in pixels. */
  offset: number
  /** A row's height until it's drawn and measured, with the space after it. */
  estimate: number
  /** How far past each edge of the screen rows are drawn, in pixels. */
  overscan: number
}

export interface VirtualRows extends RowRange {
  /** The ref of each row drawn, which also needs its key in `data-row-key`: it measures the row whenever its size
   * changes. The space between rows belongs inside it, as padding, so it's measured too. */
  measure: (element: HTMLElement | null) => (() => void) | undefined
}

export function useVirtualRows({ keys, scroller, offset, estimate, overscan }: VirtualRowsOptions): VirtualRows {
  const [heights, setHeights] = useState<ReadonlyMap<number, number>>(new Map())
  const [rowObserver] = useState(
    () =>
      new ResizeObserver((entries) => {
        setHeights((current) => {
          let changed: Map<number, number> | null = null
          for (const entry of entries) {
            const element = entry.target as HTMLElement
            const key = Number(element.dataset.rowKey)
            const height = entry.borderBoxSize[0]?.blockSize ?? element.offsetHeight
            if (current.get(key) !== height) {
              changed ??= new Map(current)
              changed.set(key, height)
            }
          }
          return changed ?? current
        })
      }),
  )
  useEffect(() => () => rowObserver.disconnect(), [rowObserver])

  // How far the scroller is scrolled, and how tall it is: read when it scrolls, and when it's resized, which also
  // happens once as soon as it's observed.
  const [view, setView] = useState({ top: 0, height: 0 })
  useEffect(() => {
    const element = scroller.current
    if (!element) return
    const update = () => setView({ top: element.scrollTop, height: element.clientHeight })
    element.addEventListener("scroll", update, { passive: true })
    const resizeObserver = new ResizeObserver(update)
    resizeObserver.observe(element)
    return () => {
      element.removeEventListener("scroll", update)
      resizeObserver.disconnect()
    }
  }, [scroller])

  const layout = useMemo(() => layOutRows(keys, heights, estimate), [keys, heights, estimate])
  const range = rowsBetween(layout, view.top - offset - overscan, view.top - offset + view.height + overscan)

  const measure = useCallback(
    (element: HTMLElement | null) => {
      if (!element) return
      rowObserver.observe(element)
      return () => rowObserver.unobserve(element)
    },
    [rowObserver],
  )
  return { ...range, measure }
}
