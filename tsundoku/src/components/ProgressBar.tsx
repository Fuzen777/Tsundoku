interface ProgressBarProps {
  /** From 0 to 1. Without it the bar slides back and forth, for work whose length isn't known. */
  value?: number
  /** What is in progress, for screen readers. */
  label: string
}

/** A thin bar for an action in progress. */
export function ProgressBar({ value, label }: ProgressBarProps) {
  const percent = value === undefined ? undefined : Math.round(Math.min(Math.max(value, 0), 1) * 100)
  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent}
      className="relative h-1 overflow-hidden rounded-full bg-line"
    >
      {percent === undefined ? (
        <div className="absolute inset-y-0 left-0 w-1/3 animate-indeterminate rounded-full bg-accent" />
      ) : (
        <div className="h-full rounded-full bg-accent transition-[width] duration-150" style={{ width: `${percent}%` }} />
      )}
    </div>
  )
}
