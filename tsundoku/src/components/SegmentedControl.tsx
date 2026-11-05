interface SegmentedOption<Value extends string> {
  value: Value
  label: string
}

interface SegmentedControlProps<Value extends string> {
  /** What is being chosen, for screen readers. */
  label: string
  options: readonly SegmentedOption<Value>[]
  value: Value
  onChange: (value: Value) => void
  disabled?: boolean
}

/** A choice between a few options shown side by side, such as an update channel. */
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
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            disabled={disabled}
            onClick={() => {
              if (!selected) onChange(option.value)
            }}
            className={`h-6 rounded px-3 text-xs font-medium transition-colors focus-visible:outline-2 focus-visible:outline-accent disabled:opacity-40 ${
              selected ? "bg-accent text-on-accent" : "text-fg-secondary hover:text-fg"
            }`}
          >
            {option.label}
          </button>
        )
      })}
    </div>
  )
}
