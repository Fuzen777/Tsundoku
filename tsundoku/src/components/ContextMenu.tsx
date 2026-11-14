import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react"
import { createPortal } from "react-dom"
import type { MenuItem } from "./Menu"
import { classes } from "./styles"

/** The room a menu keeps from the window's edges, in pixels. */
const EDGE = 8

/** Why a menu closed: a choice or Escape gives the focus back to what opened it; a click elsewhere leaves it there. */
export type MenuClose = "chosen" | "escape" | "elsewhere"

interface ContextMenuProps {
  /** What the menu is for, for screen readers. */
  label: string
  items: readonly MenuItem[]
  /** Where it opens: the pointer, or for the keyboard, the corner of what opened it. */
  at: { x: number; y: number }
  onClose: (why: MenuClose) => void
}

/**
 * A right-click menu: the actions of what was clicked, opened where the pointer is and kept inside the window. Its
 * first action has the focus; the arrow keys, Home and End move through them, Enter picks one, and Escape, Tab, a click
 * elsewhere, scrolling or resizing the window closes it. It looks like Menu's panel.
 */
export function ContextMenu({ label, items, at, onClose }: ContextMenuProps) {
  const panel = useRef<HTMLDivElement>(null)
  const [place, setPlace] = useState(at)

  // Moved inside the window before it's painted, then its first action focused.
  useLayoutEffect(() => {
    const element = panel.current
    if (!element) return
    const { width, height } = element.getBoundingClientRect()
    setPlace({
      x: Math.max(EDGE, Math.min(at.x, window.innerWidth - width - EDGE)),
      y: Math.max(EDGE, Math.min(at.y, window.innerHeight - height - EDGE)),
    })
    element.querySelector<HTMLElement>('[role="menuitem"]')?.focus()
  }, [at])

  // Kept in a ref, so a new onClose from each render doesn't add the listeners again.
  const close = useRef(onClose)
  useLayoutEffect(() => {
    close.current = onClose
  })
  useEffect(() => {
    const closeOnOutsideClick = (event: MouseEvent) => {
      if (!panel.current?.contains(event.target as Node)) close.current("elsewhere")
    }
    const closeAway = () => close.current("elsewhere")
    const closeOnScroll = (event: Event) => {
      if (!panel.current?.contains(event.target as Node)) close.current("elsewhere")
    }
    document.addEventListener("mousedown", closeOnOutsideClick)
    document.addEventListener("scroll", closeOnScroll, true)
    window.addEventListener("resize", closeAway)
    window.addEventListener("blur", closeAway)
    return () => {
      document.removeEventListener("mousedown", closeOnOutsideClick)
      document.removeEventListener("scroll", closeOnScroll, true)
      window.removeEventListener("resize", closeAway)
      window.removeEventListener("blur", closeAway)
    }
  }, [])

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const actions = [...(panel.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])]
    const current = actions.indexOf(document.activeElement as HTMLElement)
    const focus = (index: number) => actions.at(index % actions.length)?.focus()
    switch (event.key) {
      case "Escape":
        onClose("escape")
        break
      case "Tab":
        onClose("elsewhere")
        break
      case "ArrowDown":
        focus(current + 1)
        break
      case "ArrowUp":
        focus(current < 0 ? -1 : current - 1)
        break
      case "Home":
        focus(0)
        break
      case "End":
        focus(-1)
        break
      default:
        return
    }
    event.preventDefault()
    event.stopPropagation()
  }

  return createPortal(
    <div
      ref={panel}
      role="menu"
      aria-label={label}
      onKeyDown={onKeyDown}
      // A right-click on the menu itself opens nothing more.
      onContextMenu={(event) => event.preventDefault()}
      style={{ left: place.x, top: place.y }}
      className="fixed z-50 flex max-h-[calc(100vh-16px)] w-max max-w-[calc(100vw-16px)] min-w-44 flex-col overflow-y-auto rounded-md border border-line-strong bg-bg p-1 shadow-lg"
    >
      {items.map(({ label: itemLabel, icon: Icon, danger, onSelect }) => (
        <button
          key={itemLabel}
          type="button"
          role="menuitem"
          tabIndex={-1}
          onClick={() => {
            onClose("chosen")
            onSelect()
          }}
          className={classes(
            "flex items-center gap-2 rounded px-3 py-1.5 text-left transition-colors hover:bg-input focus-visible:bg-input focus-visible:outline-2 focus-visible:outline-accent",
            danger ? "text-danger" : "text-fg",
          )}
        >
          {Icon && <Icon size={14} className="shrink-0" />}
          {itemLabel}
        </button>
      ))}
    </div>,
    document.body,
  )
}
