/** `text` with `{placeholder}` put in place of the text from `start` to `end` (the selection, or the cursor when they're
 * equal), and where the cursor goes after it. */
export function insertPlaceholder(
  text: string,
  placeholder: string,
  start: number,
  end: number,
): { text: string; cursor: number } {
  const token = `{${placeholder}}`
  return { text: text.slice(0, start) + token + text.slice(end), cursor: start + token.length }
}

/** `template` with each `{placeholder}` it knows, in any case, replaced by its value. Other braces stay as they are. */
export function fillPlaceholders(template: string, values: Readonly<Record<string, string>>): string {
  const byName = new Map(Object.entries(values).map(([name, value]) => [name.toLowerCase(), value]))
  return template.replace(/\{([a-z]+)\}/gi, (whole, name: string) => byName.get(name.toLowerCase()) ?? whole)
}

/** The placeholders only some sites fill, by the site's name, which the placeholder buttons' site filter offers beside
 * the general ones (Fuzen's design): Pixiv's tags in each language and its works' details (`rules::placeholder_value`
 * and `post_names::PostNamePlaceholder` in the core). */
export const SITE_PLACEHOLDERS = {
  Pixiv: ["tagsJa", "tagsEn", "tagsBoth", "rating", "workType", "ai", "series"],
} as const satisfies Readonly<Record<string, readonly string[]>>
