import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react"
import type { LucideIcon } from "lucide-react"
import { IconButton } from "./IconButton"
import { classes } from "./styles"

/** The space a panel that fits the window leaves under it, in pixels. */
const WINDOW_MARGIN = 16

interface PopoverProps {
  /** What the button opens, for screen readers and its tooltip. */
  label: string
  icon: LucideIcon
  /** A list of actions, or something to read. */
  role: "menu" | "dialog"
  /** The panel's size, padding and layout. */
  panelClassName: string
  /** Keeps a tall panel inside the window, scrolling within it, wherever the button is. */
  fitsWindow?: boolean
  /** The panel's contents, given a way to close it. */
  children: (close: () => void) => ReactNode
}

/** A ghost icon button that opens a panel under it, right-aligned to it, on `--color-bg` with a strong line border and a
 * shadow. Escape or a click elsewhere closes it. `Menu` and `HelpMenu` are built on it. */
export function Popover({ label, icon, role, panelClassName, fitsWindow = false, children }: PopoverProps) {
  const [open, setOpen] = useState(false)
  const container = useRef<HTMLDivElement>(null)
  const panel = useRef<HTMLDivElement>(null)

  // Measured before it's painted, and again when the window is resized, since what's above the view (such as What's
  // New) moves the button.
  useLayoutEffect(() => {
    const element = panel.current
    if (!open || !fitsWindow || !element) return
    const fit = () => {
      element.style.maxHeight = `${window.innerHeight - element.getBoundingClientRect().top - WINDOW_MARGIN}px`
    }
    fit()
    window.addEventListener("resize", fit)
    return () => window.removeEventListener("resize", fit)
  }, [open, fitsWindow])

  useEffect(() => {
    if (!open) return
    const closeOnOutsideClick = (event: MouseEvent) => {
      if (!container.current?.contains(event.target as Node)) setOpen(false)
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false)
    }
    document.addEventListener("mousedown", closeOnOutsideClick)
    document.addEventListener("keydown", closeOnEscape)
    return () => {
      document.removeEventListener("mousedown", closeOnOutsideClick)
      document.removeEventListener("keydown", closeOnEscape)
    }
  }, [open])

  return (
    <div ref={container} className="relative">
      <IconButton
        label={label}
        icon={icon}
        aria-haspopup={role}
        aria-expanded={open}
        onClick={() => setOpen((isOpen) => !isOpen)}
      />
      {open && (
        <div
          ref={panel}
          role={role}
          aria-label={label}
          className={classes(
            "absolute top-full right-0 z-10 mt-1 rounded-md border border-line-strong bg-bg shadow-lg",
            panelClassName,
          )}
        >
          {children(() => setOpen(false))}
        </div>
      )}
    </div>
  )
}
