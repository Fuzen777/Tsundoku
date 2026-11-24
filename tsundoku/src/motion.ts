import { useEffect, useLayoutEffect, useState, type RefObject, type TransitionEvent } from "react"

/*
  What moves in the UI (DESIGN.md → Motion): parts opening and closing with a CSS transition, and Windows' wish for
  less of it.
*/

/** Whether Windows' Animation Effects are turned off (Settings → Accessibility → Visual Effects): what would move
 * changes at once, as `motion-reduce:` and `motion-safe:` do in the styles. */
export function prefersReducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches
}

/** How long what opens, closes or changes size takes, in milliseconds (the classes' `duration-200`). */
export const MOTION_MS = 200
/** Fluent's decelerate curve (index.css's `ease-decelerate`), for what opens, comes in or changes size. */
export const DECELERATE = "cubic-bezier(0.1, 0.9, 0.2, 1)"

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

/**
 * A box whose height follows its content's (up to its own maximum, past which it scrolls), easing from one height to the
 * next over 200 ms when its content changes size, rather than jumping: the details panel showing a taller or a shorter
 * row, or a log opened in it. What's around it moves along. It doesn't ease when it first shows (its opening does that),
 * when the content's width changed (the window resized), or with reduced motion. While it eases, it clips rather than
 * scrolls.
 */
export function useEasedHeight(box: RefObject<HTMLElement | null>, content: RefObject<HTMLElement | null>): void {
  useLayoutEffect(() => {
    const outer = box.current
    const inner = content.current
    if (!outer || !inner) return
    let settled: number | null = null
    let width: number | null = null
    let easing: Animation | null = null
    const observer = new ResizeObserver(([entry]) => {
      // Where it is now, mid-way if it was easing; then where its content takes it.
      const from = easing ? outer.getBoundingClientRect().height : settled
      easing?.cancel()
      easing = null
      outer.style.overflowY = ""
      const to = outer.getBoundingClientRect().height
      const widthChanged = width !== null && entry.contentRect.width !== width
      settled = to
      width = entry.contentRect.width
      if (from === null || widthChanged || Math.abs(from - to) < 1 || prefersReducedMotion()) return
      outer.style.overflowY = "hidden"
      const eased = outer.animate([{ height: `${from}px` }, { height: `${to}px` }], { duration: MOTION_MS, easing: DECELERATE })
      easing = eased
      eased.finished.then(
        () => {
          if (easing !== eased) return
          easing = null
          outer.style.overflowY = ""
        },
        () => {},
      )
    })
    observer.observe(inner)
    return () => observer.disconnect()
  }, [box, content])
}
