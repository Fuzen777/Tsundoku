import { useContext, type ReactNode } from "react"
import { createPortal } from "react-dom"
import { strings } from "../strings"
import { DetailsDockContext } from "../tileRows"

/**
 * A clicked picture's row in the Downloads, Pending and History views' Masonry and Gallery, in a panel along the bottom
 * of the content layer, across its whole width, rather than under its line of pictures, so the pictures don't move.
 * The view's list ends above it. When the row is taller than half the window (a post's files, a log), the panel
 * scrolls.
 */
export function DetailsDock({ children }: { children: ReactNode }) {
  const slot = useContext(DetailsDockContext)
  if (!slot) return null
  return createPortal(
    <section aria-label={strings.tiles.details} className="max-h-[50vh] overflow-y-auto border-t border-line px-6 py-3">
      {children}
    </section>,
    slot,
  )
}
