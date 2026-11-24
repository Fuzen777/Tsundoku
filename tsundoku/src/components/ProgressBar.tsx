import { useState } from "react"
import { classes } from "./styles"

interface ProgressBarProps {
  /** From 0 to 1. Without it the bar slides back and forth, for work whose length isn't known. */
  value?: number
  /** What is in progress, for screen readers. */
  label: string
}

/**
 * A thin bar for an action in progress. Its fill glides to each new value over half a second, about as often as a
 * download's progress comes, so it moves steadily rather than in steps (DESIGN.md → Motion); going back, such as a
 * download's next file starting, it jumps.
 */
export function ProgressBar({ value, label }: ProgressBarProps) {
  const percent = value === undefined ? undefined : Math.round(Math.min(Math.max(value, 0), 1) * 100)
  const [last, setLast] = useState(percent)
  const [rising, setRising] = useState(true)
  if (percent !== last) {
    setLast(percent)
    setRising(percent === undefined || last === undefined || percent >= last)
  }
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
        <div
          className={classes(
            "h-full rounded-full bg-accent",
            rising && "transition-[width] duration-500 ease-linear motion-reduce:transition-none",
          )}
          style={{ width: `${percent}%` }}
        />
      )}
    </div>
  )
}
