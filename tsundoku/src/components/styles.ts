/** The look of every text field and select, the add bar's included. */
export const INPUT_CLASSES = [
  "h-8 min-w-0 rounded-md border border-line bg-input px-3 text-sm outline-none transition-colors",
  "placeholder:text-fg-muted hover:bg-input-hover focus:border-accent disabled:opacity-40",
].join(" ")

/** How a picked item unticked a moment ago looks while it fades before it moves away (the listing picker's Not
 * Downloading): slowly, so there's time to tick it again, which brings it back at once. Goes with `transition-opacity`. */
export const LEAVING_CLASSES = "opacity-20 duration-[1800ms] ease-linear"

/** Joins class names, skipping the empty ones. */
export function classes(...names: (string | false | null | undefined)[]): string {
  return names.filter(Boolean).join(" ")
}
