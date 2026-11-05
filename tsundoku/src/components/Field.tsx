import type { InputHTMLAttributes, ReactNode, SelectHTMLAttributes } from "react"
import { classes, INPUT_CLASSES } from "./styles"

interface FieldProps {
  label: string
  /** A sentence under the control explaining it. */
  hint?: string
  /** Shown instead of the hint, in the danger color. */
  error?: string
  /** The control's ID, so clicking the label focuses it. */
  htmlFor?: string
  children: ReactNode
}

/** A labeled form row: the label, the control, then a hint or an error. */
export function Field({ label, hint, error, htmlFor, children }: FieldProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={htmlFor} className="font-medium">
        {label}
      </label>
      {children}
      {error ? (
        <p role="alert" className="text-xs text-danger">
          {error}
        </p>
      ) : (
        hint && <p className="text-xs text-fg-muted">{hint}</p>
      )}
    </div>
  )
}

export function TextInput({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input type="text" spellCheck={false} autoComplete="off" className={classes(INPUT_CLASSES, className)} {...props} />
}

/** A native select, which follows Windows' theme for its list. */
export function SelectInput({ className, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select className={classes(INPUT_CLASSES, "pr-8", className)} {...props} />
}

interface CheckboxProps {
  label: string
  checked: boolean
  onChange: (checked: boolean) => void
  disabled?: boolean
}

export function Checkbox({ label, checked, onChange, disabled }: CheckboxProps) {
  return (
    <label className="flex items-center gap-2 has-disabled:opacity-40">
      <input
        type="checkbox"
        className="size-4 accent-accent"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      {label}
    </label>
  )
}
