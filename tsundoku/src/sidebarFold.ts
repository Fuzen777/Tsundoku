import { useState, type TransitionEvent } from "react"
import { useOpening } from "./motion"

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
  const { drawn, moving, onTransitionEnd } = useOpening(!collapsed, "width")
  return {
    collapsed,
    stripLayout: !drawn,
    moving,
    toggle: () => setCollapsed(!collapsed),
    onTransitionEnd,
  }
}
