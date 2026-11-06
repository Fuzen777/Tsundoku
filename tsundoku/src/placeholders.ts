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
  return template.replace(/\{([a-z]+)\}/gi, (whole, name: string) => values[name.toLowerCase()] ?? whole)
}
