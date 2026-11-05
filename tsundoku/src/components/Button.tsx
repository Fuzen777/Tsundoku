import type { ButtonHTMLAttributes } from "react"
import { Loader2 } from "lucide-react"

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger"
export type ButtonSize = "sm" | "md" | "icon"

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant
  size?: ButtonSize
  /** An action in flight: disables the button and lays a spinner over its hidden label, so its width never changes. */
  loading?: boolean
}

const BASE_CLASSES = [
  "relative inline-flex shrink-0 items-center justify-center rounded-md font-medium transition-colors",
  "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
  "disabled:pointer-events-none disabled:opacity-40",
].join(" ")

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary: "bg-accent text-on-accent hover:bg-accent-hover",
  secondary: "border border-line bg-input text-fg hover:bg-input-hover",
  ghost: "bg-transparent text-fg-secondary hover:bg-input hover:text-fg",
  danger: "bg-danger text-on-danger hover:bg-danger-hover",
}

const SIZE_CLASSES: Record<ButtonSize, string> = {
  sm: "h-7 gap-1.5 px-2.5 text-xs",
  md: "h-8 gap-2 px-4 text-sm",
  icon: "size-8",
}

/** The app's one button. See DESIGN.md → Buttons for which variant fits which action. */
export function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  disabled,
  type = "button",
  className,
  children,
  ...rest
}: ButtonProps) {
  const classes = [BASE_CLASSES, VARIANT_CLASSES[variant], SIZE_CLASSES[size], className].filter(Boolean).join(" ")
  return (
    <button type={type} disabled={disabled || loading} className={classes} {...rest}>
      {/* `contents` keeps the children laid out as the button's own flex items, so its gap still applies. */}
      <span className={loading ? "invisible contents" : "contents"}>{children}</span>
      {loading && (
        <span className="absolute inset-0 flex items-center justify-center">
          <Loader2 size={14} className="animate-spin" />
        </span>
      )}
    </button>
  )
}
