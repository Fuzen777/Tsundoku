import { useContext, useState, type ReactNode } from "react"
import { createPortal } from "react-dom"
import { strings } from "../strings"
import { DetailsDockContext, FlatCardContext } from "../tileRows"
import { Collapsible } from "./Collapsible"

/**
 * A clicked picture's row in the Downloads, Pending and History views' Masonry and Gallery, in a panel along the bottom
 * of the content layer, across its whole width and flush with its edges, rather than under its line of pictures, so the
 * pictures don't move. The row's card is the panel itself. It rises from the bottom as it opens, the view's list ending
 * above it as it goes, and sinks back as it closes, still showing the row it held. When the row is taller than half the
 * window (a post's files, a log), the panel scrolls. Without children (nothing clicked), it's closed.
 */
export function DetailsDock({ children }: { children: ReactNode }) {
  const slot = useContext(DetailsDockContext)
  const open = children !== null && children !== undefined && children !== false
  const [kept, setKept] = useState<ReactNode>(children)
  if (open && children !== kept) setKept(children)
  if (!slot) return null
  return createPortal(
    <Collapsible open={open}>
      <section aria-label={strings.tiles.details} className="max-h-[50vh] overflow-y-auto border-t border-line bg-card px-6 py-1">
        <FlatCardContext value>{open ? children : kept}</FlatCardContext>
      </section>
    </Collapsible>,
    slot,
  )
}
