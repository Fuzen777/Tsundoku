import { useEffect, useState, type TransitionEvent } from "react"

/** Windows' Animation Effects turned off (Settings → Accessibility → Visual Effects): the sidebar changes at once. */
const REDUCED_MOTION = "(prefers-reduced-motion: reduce)"
/** How long the sidebar may take to open or fold before it's taken as done anyway, in case its width's transition never
 * ends (none ran, or it was cut short). The transition itself takes 200 ms (App.tsx). */
const SETTLED_AT_THE_LATEST_MS = 1000

export interface SidebarFold {
  /** Folded to the strip, or folding: what the column's width and Expand Sidebar follow. */
  collapsed: boolean
  /** Whether its content is laid out as the strip's. Folding keeps the open layout until the strip is reached, so the
   * open sidebar slides shut rather than turning into the strip at once. */
  stripLayout: boolean
  /** Its width is easing from one to the other: the column clips what doesn't fit it yet. */
  moving: boolean
  toggle: () => void
  /** The column's `onTransitionEnd`: its width reached the strip's or the open sidebar's. */
  onTransitionEnd: (event: TransitionEvent<HTMLElement>) => void
}

/**
 * The sidebar folded to a narrow strip or open, and its width easing from one to the other, pushing the view aside as
 * it opens and letting it slide back as it folds. Its content keeps the width of the layout drawn, so nothing in it
 * reflows while it moves: opening draws the open sidebar at once, uncovered as the column widens, and folding keeps it
 * until the strip is reached.
 */
export function useSidebarFold(startsCollapsed: boolean): SidebarFold {
  const [collapsed, setCollapsed] = useState(startsCollapsed)
  const [stripLayout, setStripLayout] = useState(startsCollapsed)
  const [moving, setMoving] = useState(false)

  useEffect(() => {
    if (!moving) return
    const timer = setTimeout(() => {
      setMoving(false)
      setStripLayout(collapsed)
    }, SETTLED_AT_THE_LATEST_MS)
    return () => clearTimeout(timer)
  }, [moving, collapsed])

  return {
    collapsed,
    stripLayout,
    moving,
    toggle: () => {
      const next = !collapsed
      const animated = !window.matchMedia(REDUCED_MOTION).matches
      setCollapsed(next)
      setMoving(animated)
      if (!next || !animated) setStripLayout(next)
    },
    onTransitionEnd: (event) => {
      if (event.target !== event.currentTarget || event.propertyName !== "width") return
      setMoving(false)
      setStripLayout(collapsed)
    },
  }
}
