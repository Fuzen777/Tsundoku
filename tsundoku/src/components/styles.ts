/** The look of every text field and select, the add bar's included. */
export const INPUT_CLASSES = [
  "h-8 min-w-0 rounded-md border border-line bg-input px-3 text-sm outline-none transition-colors",
  "placeholder:text-fg-muted hover:bg-input-hover focus:border-accent disabled:opacity-40",
].join(" ")

/** A picture in a grid lifting a little under the pointer (DESIGN.md → Motion): on the element holding the picture and
 * what sits on it (a `group`), and `LIFTED_SHADOW` on the picture's own rounded box. It stays put with reduced motion. */
export const LIFTS_ON_HOVER = "group transition-[translate] duration-150 ease-out motion-safe:hover:-translate-y-0.5"
/** The lifted picture's shadow (`LIFTS_ON_HOVER`). */
export const LIFTED_SHADOW = "transition-shadow duration-150 group-hover:shadow-md"

/** Joins class names, skipping the empty ones. */
export function classes(...names: (string | false | null | undefined)[]): string {
  return names.filter(Boolean).join(" ")
}
