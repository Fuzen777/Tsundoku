import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react"
import { createPortal } from "react-dom"

/** How long the pointer rests on a control, or the keyboard's focus stays on it, before its tooltip shows. */
const SHOW_AFTER_MS = 500
/** Moving to another control this soon after a tooltip closed shows the next one at once. */
const STAYS_WARM_FOR_MS = 300
/** Space between the control and its tooltip, and between a tooltip and the window's edges. */
const GAP_PX = 6
const EDGE_PX = 8

/** When the last tooltip closed, so a pointer moving along a row of buttons doesn't wait at each one. */
let lastHiddenAt = 0

interface TooltipProps {
  /** What the tooltip says. The control should carry it as its accessible name too (an `aria-label`). */
  label: string
  children: ReactNode
}

/**
 * Shows `label` in a small box under its children (over them when there's no room below) once the pointer rests on
 * them, or the keyboard's focus reaches them, for half a second. A press, Escape, scrolling or leaving closes it. It
 * listens on a wrapper, so a disabled button inside still shows it, and it's drawn over everything in a portal, so a
 * card's clipping never cuts it.
 */
export function Tooltip({ label, children }: TooltipProps) {
  const anchor = useRef<HTMLSpanElement>(null)
  const box = useRef<HTMLDivElement>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const [shown, setShown] = useState(false)
  // After a press, the tooltip stays away until the pointer leaves, so it doesn't cover what the press opened.
  const pressed = useRef(false)

  const showSoon = () => {
    if (pressed.current) return
    clearTimeout(timer.current)
    const delay = Date.now() - lastHiddenAt < STAYS_WARM_FOR_MS ? 0 : SHOW_AFTER_MS
    timer.current = setTimeout(() => setShown(true), delay)
  }

  const hide = useCallback(() => {
    clearTimeout(timer.current)
    setShown((wasShown) => {
      if (wasShown) lastHiddenAt = Date.now()
      return false
    })
  }, [])

  useEffect(() => () => clearTimeout(timer.current), [])

  useEffect(() => {
    if (!shown) return
    const hideOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") hide()
    }
    document.addEventListener("keydown", hideOnEscape)
    window.addEventListener("scroll", hide, true)
    window.addEventListener("resize", hide)
    return () => {
      document.removeEventListener("keydown", hideOnEscape)
      window.removeEventListener("scroll", hide, true)
      window.removeEventListener("resize", hide)
    }
  }, [shown, hide])

  // Placed before the browser paints, so the box never shows in the wrong place. It's drawn hidden at the top left,
  // measured, then moved.
  useLayoutEffect(() => {
    const shownBox = box.current
    if (!shown || !anchor.current || !shownBox) return
    const around = anchor.current.getBoundingClientRect()
    const { width, height } = shownBox.getBoundingClientRect()
    const below = around.bottom + GAP_PX
    const top = below + height + EDGE_PX <= window.innerHeight ? below : around.top - GAP_PX - height
    const centered = around.left + around.width / 2 - width / 2
    const left = Math.min(Math.max(centered, EDGE_PX), window.innerWidth - width - EDGE_PX)
    shownBox.style.left = `${left}px`
    shownBox.style.top = `${Math.max(top, EDGE_PX)}px`
    shownBox.style.visibility = "visible"
  }, [shown, label])

  return (
    <span
      ref={anchor}
      className="inline-flex shrink-0"
      onPointerEnter={showSoon}
      onPointerLeave={() => {
        pressed.current = false
        hide()
      }}
      onPointerDown={() => {
        pressed.current = true
        hide()
      }}
      onFocus={(event) => {
        if (event.target.matches(":focus-visible")) showSoon()
      }}
      onBlur={hide}
    >
      {children}
      {shown &&
        createPortal(
          <div
            ref={box}
            aria-hidden
            style={{ left: 0, top: 0, visibility: "hidden" }}
            className="pointer-events-none fixed z-50 max-w-72 animate-tooltip-in rounded border border-line-strong bg-bg px-2 py-1 text-xs text-fg shadow-md"
          >
            {label}
          </div>,
          document.body,
        )}
    </span>
  )
}
