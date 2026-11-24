import { useRef, type ReactNode } from "react"
import { useEnterAndLeave } from "../presence"

interface PresentItemProps {
  /** The item's key in its list (`ListKeysContext`). */
  itemKey: number
  /** A list item in a `<ul>` (pictures), else a block. */
  as?: "div" | "li"
  className?: string
  children: ReactNode
}

/** An item of a list that fades in when it's new and collapses away when it leaves (`useEnterAndLeave`). */
export function PresentItem({ itemKey, as = "div", className, children }: PresentItemProps) {
  const element = useRef<HTMLElement | null>(null)
  useEnterAndLeave(element, itemKey)
  const keep = (node: HTMLElement | null) => {
    element.current = node
  }
  return as === "li" ? (
    <li ref={keep} className={className}>
      {children}
    </li>
  ) : (
    <div ref={keep} className={className}>
      {children}
    </div>
  )
}
