/** A value as a tag takes it (`components/TagInput.tsx`): line breaks around it dropped, spaces kept, since they may be
 * part of it (a password). */
export function tagOf(text: string): string {
  return text.replace(/^[\r\n]+|[\r\n]+$/g, "")
}

/** `values` with `added` at the end as tags, leaving out blank ones and those already there, or `values` itself when
 * none is new. */
export function withTags(values: readonly string[], added: readonly string[]): readonly string[] {
  const next = [...values]
  for (const value of added.map(tagOf)) {
    if (value.trim() && !next.includes(value)) next.push(value)
  }
  return next.length === values.length ? values : next
}
