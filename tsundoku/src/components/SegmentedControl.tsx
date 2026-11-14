import type { LucideIcon } from "lucide-react"
import { Tooltip } from "./Tooltip"

interface SegmentedOption<Value extends string> {
  value: Value
  label: string
  /** Shown instead of the label, which becomes the option's tooltip and its name for screen readers. */
  icon?: LucideIcon
}

interface SegmentedControlProps<Value extends string> {
  /** What is being chosen, for screen readers. */
  label: string
  options: readonly SegmentedOption<Value>[]
  value: Value
  onChange: (value: Value) => void
  disabled?: boolean
}

/** A choice between a few options shown side by side, such as an update channel, as words or icons. */
export function SegmentedControl<Value extends string>({
  label,
  options,
  value,
  onChange,
  disabled = false,
}: SegmentedControlProps<Value>) {
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex w-fit rounded-md border border-line bg-input p-0.5">
      {options.map((option) => {
        const selected = option.value === value
        const Icon = option.icon
        const button = (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={Icon ? option.label : undefined}
            disabled={disabled}
            onClick={() => {
              if (!selected) onChange(option.value)
            }}
            className={`h-6 rounded text-xs font-medium transition-colors focus-visible:outline-2 focus-visible:outline-accent disabled:opacity-40 ${
              Icon ? "inline-flex items-center px-2" : "px-3"
            } ${selected ? "bg-accent-fill text-on-accent" : "text-fg-secondary hover:text-fg"}`}
          >
            {Icon ? <Icon size={14} /> : option.label}
          </button>
        )
        return Icon ? (
          <Tooltip key={option.value} label={option.label}>
            {button}
          </Tooltip>
        ) : (
          button
        )
      })}
    </div>
  )
}
