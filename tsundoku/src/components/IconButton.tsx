import type { ButtonHTMLAttributes } from "react"
import type { LucideIcon } from "lucide-react"
import { Button, type ButtonVariant } from "./Button"
import { Tooltip } from "./Tooltip"

interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "title" | "aria-label"> {
  /** What the button does: its tooltip and, for screen readers, its name. */
  label: string
  icon: LucideIcon
  variant?: ButtonVariant
  loading?: boolean
}

/**
 * A button that shows only an icon, with its label as its tooltip and aria-label. The tooltip listens on a wrapper,
 * so a disabled button still says what it is.
 */
export function IconButton({ label, icon: Icon, variant = "ghost", ...rest }: IconButtonProps) {
  return (
    <Tooltip label={label}>
      <Button size="icon" variant={variant} aria-label={label} {...rest}>
        <Icon size={16} />
      </Button>
    </Tooltip>
  )
}
