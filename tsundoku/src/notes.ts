/*
  Release notes as CHANGELOG.md writes them, in a little Markdown: `###` headings, `- ` lists and paragraphs, with
  **bold**, `code` and [links](https://…). The What's New card and the update banner show them through ReleaseNotes.
*/

import type { ChangelogSection } from "./bindings/ChangelogSection"
import { formatCalendarDate } from "./format"

/** A piece of a line. */
export type NoteSpan =
  | { kind: "text"; text: string }
  | { kind: "bold"; text: string }
  | { kind: "code"; text: string }
  | { kind: "link"; text: string; url: string }

/** A heading, a paragraph or a list, each item a line of spans. */
export type NoteBlock =
  | { kind: "heading"; spans: NoteSpan[] }
  | { kind: "paragraph"; spans: NoteSpan[] }
  | { kind: "list"; items: NoteSpan[][] }

const HEADING = /^#{1,6}\s+(.*)$/
const ITEM = /^[-*]\s+(.*)$/
const INLINE = /\*\*(.+?)\*\*|`([^`]+)`|\[([^\]]+)\]\(([^)\s]+)\)/g

/**
 * The blocks of notes: a `#` heading of any level, a list of `- ` or `* ` items (an indented line under an item goes
 * on with it), and paragraphs, whose lines join with spaces. A blank line ends a paragraph or a list.
 */
export function parseNotes(markdown: string): NoteBlock[] {
  const blocks: NoteBlock[] = []
  let paragraph: string[] = []
  let items: string[] = []
  const endParagraph = () => {
    if (paragraph.length > 0) blocks.push({ kind: "paragraph", spans: parseSpans(paragraph.join(" ")) })
    paragraph = []
  }
  const endList = () => {
    if (items.length > 0) blocks.push({ kind: "list", items: items.map(parseSpans) })
    items = []
  }

  for (const rawLine of markdown.replace(/\r\n/g, "\n").split("\n")) {
    const line = rawLine.trim()
    const heading = HEADING.exec(line)
    const item = ITEM.exec(line)
    if (line === "") {
      endParagraph()
      endList()
    } else if (heading) {
      endParagraph()
      endList()
      blocks.push({ kind: "heading", spans: parseSpans(heading[1]) })
    } else if (item) {
      endParagraph()
      items.push(item[1])
    } else if (items.length > 0 && /^\s/.test(rawLine)) {
      items[items.length - 1] += ` ${line}`
    } else {
      endList()
      paragraph.push(line)
    }
  }
  endParagraph()
  endList()
  return blocks
}

/** A line's text, bold, code and links. */
export function parseSpans(text: string): NoteSpan[] {
  const spans: NoteSpan[] = []
  let end = 0
  for (const match of text.matchAll(INLINE)) {
    if (match.index > end) spans.push({ kind: "text", text: text.slice(end, match.index) })
    const [whole, bold, code, linkText, url] = match
    if (bold !== undefined) spans.push({ kind: "bold", text: bold })
    else if (code !== undefined) spans.push({ kind: "code", text: code })
    else spans.push({ kind: "link", text: linkText, url })
    end = match.index + whole.length
  }
  if (end < text.length) spans.push({ kind: "text", text: text.slice(end) })
  return spans
}

/**
 * The version whose changes the What's New card shows while it's browsed: the one picked, else the running version's,
 * else the newest there is (a build whose own version has no section). None when there are no sections at all.
 */
export function sectionToShow(
  sections: readonly ChangelogSection[],
  picked: string | null,
  running: string | undefined,
): ChangelogSection | null {
  const of = (version: string | null | undefined) => sections.find((section) => section.version === version)
  return of(picked) ?? of(running) ?? sections[0] ?? null
}

/** A section's date as the reader writes dates, or as the changelog has it when it isn't a calendar date. */
export function sectionDate(section: ChangelogSection, locale?: string): string | null {
  if (section.date === null) return null
  const isCalendarDate = !Number.isNaN(Date.parse(`${section.date}T00:00:00Z`))
  return isCalendarDate ? formatCalendarDate(section.date, locale) : section.date
}

/** Whether a link in the notes may be opened: only web pages. */
export function isWebLink(url: string): boolean {
  return /^https?:\/\//i.test(url)
}
