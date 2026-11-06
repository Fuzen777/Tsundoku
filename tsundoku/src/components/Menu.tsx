import { useEffect, useRef, useState } from "react"
import { MoreHorizontal, type LucideIcon } from "lucide-react"
import { IconButton } from "./IconButton"
import { classes } from "./styles"

export interface MenuItem {
  label: string
  onSelect: () => void
  /** A destructive action, in the danger color. */
  danger?: boolean
}

interface MenuProps {
  /** What the button opens, for screen readers and its tooltip. */
  label: string
  items: readonly MenuItem[]
  /** The button's icon; three dots by default. */
  icon?: LucideIcon
}

/** A ghost icon button that opens a short list of actions under it. Escape or a click elsewhere closes it. */
export function Menu({ label, items, icon = MoreHorizontal }: MenuProps) {
  const [open, setOpen] = useState(false)
  const container = useRef<HTMLDivElement>(null)

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
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((isOpen) => !isOpen)}
      />
      {open && (
        <div
          role="menu"
          className="absolute top-full right-0 z-10 mt-1 flex min-w-44 flex-col rounded-md border border-line-strong bg-bg p-1 shadow-lg"
        >
          {items.map((item) => (
            <button
              key={item.label}
              type="button"
              role="menuitem"
              onClick={() => {
                setOpen(false)
                item.onSelect()
              }}
              className={classes(
                "rounded px-3 py-1.5 text-left transition-colors hover:bg-input focus-visible:outline-2 focus-visible:outline-accent",
                item.danger ? "text-danger" : "text-fg",
              )}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
