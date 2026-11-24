import { useEffect, useState, type TransitionEvent } from "react"

/*
  What moves in the UI (DESIGN.md → Motion): parts opening and closing with a CSS transition, and Windows' wish for
  less of it.
*/

/** Whether Windows' Animation Effects are turned off (Settings → Accessibility → Visual Effects): what would move
 * changes at once, as `motion-reduce:` and `motion-safe:` do in the styles. */
export function prefersReducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches
}

/** How long opening or closing may take before it's taken as done anyway, in case its transition never ends (none ran,
 * or it was cut short). The transitions themselves take 200 ms. */
const SETTLED_AT_THE_LATEST_MS = 1000

/** A part opening or closing, as `useOpening` follows it. */
export interface Opening {
  /** Whether what it shows open is drawn: while it's open, and while it closes. */
  drawn: boolean
  /** Its transition is running: what doesn't fit it yet is clipped meanwhile. */
  moving: boolean
  /** The `onTransitionEnd` of the element whose transition it is. */
  onTransitionEnd: (event: TransitionEvent<HTMLElement>) => void
}

/**
 * A part that opens and closes with a CSS transition of `property` on one element (the sidebar's width, a fold's
 * rows): what it shows open stays drawn while it closes, until that transition ends, and is drawn at once when it opens.
 * With reduced motion, nothing transitions and it changes at once.
 */
export function useOpening(open: boolean, property: string): Opening {
  const [drawn, setDrawn] = useState(open)
  const [moving, setMoving] = useState(false)
  const [wasOpen, setWasOpen] = useState(open)
  if (open !== wasOpen) {
    setWasOpen(open)
    const animated = !prefersReducedMotion()
    setMoving(animated)
    setDrawn(open || animated)
  }

  useEffect(() => {
    if (!moving) return
    const timer = setTimeout(() => {
      setMoving(false)
      setDrawn(open)
    }, SETTLED_AT_THE_LATEST_MS)
    return () => clearTimeout(timer)
  }, [moving, open])

  return {
    drawn,
    moving,
    onTransitionEnd: (event) => {
      if (event.target !== event.currentTarget || event.propertyName !== property) return
      setMoving(false)
      setDrawn(open)
    },
  }
}
