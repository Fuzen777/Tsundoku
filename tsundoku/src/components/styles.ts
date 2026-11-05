/** The look of every text field and select, the add bar's included. */
export const INPUT_CLASSES = [
  "h-8 min-w-0 rounded-md border border-line bg-input px-3 text-sm outline-none transition-colors",
  "placeholder:text-fg-muted hover:bg-input-hover focus:border-accent disabled:opacity-40",
].join(" ")

/** Joins class names, skipping the empty ones. */
export function classes(...names: (string | false | null | undefined)[]): string {
  return names.filter(Boolean).join(" ")
}
