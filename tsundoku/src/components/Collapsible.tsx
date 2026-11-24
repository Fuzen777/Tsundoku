import type { ReactNode } from "react"
import { useOpening } from "../motion"
import { classes } from "./styles"

/** The room a parent's flex gap would leave before it, kept inside instead, so it opens and closes with the rest. */
const GAP_BEFORE = {
  2: { outer: "-mt-2", inner: "pt-2" },
  3: { outer: "-mt-3", inner: "pt-3" },
} as const

interface CollapsibleProps {
  open: boolean
  /** Drawn while it's open, and while it closes. */
  children: ReactNode
  /** In a flex column with this gap (`gap-2`, `gap-3`): it takes no room once closed, the gap included. */
  gap?: keyof typeof GAP_BEFORE
  /** Its content's layout, such as `flex flex-col gap-2`. */
  className?: string
}

/**
 * A part that folds away and back, its height easing over 200 ms (DESIGN.md → Motion) so what's under it slides along:
 * a grid whose one row goes from none of its height to all of it, its content hanging from its top. Along the bottom
 * of a view, where it grows upward, its content rises with its top edge, as a panel sliding up. What's inside is drawn
 * only while it's open or closing, and clipped only while it moves, so a menu or an outline inside isn't cut once it's
 * still. Drawn open the first time, it just shows. With reduced motion, it opens and closes at once.
 */
export function Collapsible({ open, children, gap, className }: CollapsibleProps) {
  const { drawn, moving, onTransitionEnd } = useOpening(open, "grid-template-rows")
  if (!drawn) return null
  return (
    <div
      className={classes(
        "grid transition-[grid-template-rows] duration-200 ease-decelerate motion-reduce:transition-none",
        open ? "grid-rows-[1fr]" : "grid-rows-[0fr]",
        // Drawn because it's opening: from none of its height.
        moving && "starting:grid-rows-[0fr]",
        gap && GAP_BEFORE[gap].outer,
      )}
      onTransitionEnd={onTransitionEnd}
    >
      <div className={classes("min-h-0", moving && "overflow-hidden")}>
        <div className={gap && GAP_BEFORE[gap].inner}>
          <div className={className}>{children}</div>
        </div>
      </div>
    </div>
  )
}
