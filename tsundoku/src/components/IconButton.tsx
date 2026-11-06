import type { ButtonHTMLAttributes } from "react"
import type { LucideIcon } from "lucide-react"
import { Button, type ButtonVariant } from "./Button"

interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "title" | "aria-label"> {
  /** What the button does: its hover text and, for screen readers, its name. */
  label: string
  icon: LucideIcon
  variant?: ButtonVariant
  loading?: boolean
}

/**
 * A button that shows only an icon, with its label as hover text and aria-label. The hover text sits on a wrapper,
 * since a disabled button gets no hover of its own and would otherwise leave its icon unexplained.
 */
export function IconButton({ label, icon: Icon, variant = "ghost", ...rest }: IconButtonProps) {
  return (
    <span title={label} className="inline-flex shrink-0">
      <Button size="icon" variant={variant} aria-label={label} {...rest}>
        <Icon size={16} />
      </Button>
    </span>
  )
}
