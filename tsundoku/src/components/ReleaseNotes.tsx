import { Fragment, useMemo } from "react"
import { openWebPage } from "../api"
import { isWebLink, parseNotes, type NoteBlock, type NoteSpan } from "../notes"

/** Release notes, written in CHANGELOG.md's Markdown, as simple formatted text: headings, lists and paragraphs. */
export function ReleaseNotes({ markdown }: { markdown: string }) {
  const blocks = useMemo(() => parseNotes(markdown), [markdown])
  return (
    <div className="flex flex-col gap-2 select-text">
      {blocks.map((block, index) => (
        <Block key={index} block={block} />
      ))}
    </div>
  )
}

function Block({ block }: { block: NoteBlock }) {
  switch (block.kind) {
    case "heading":
      return (
        <p className="text-xs font-semibold text-fg-secondary">
          <Spans spans={block.spans} />
        </p>
      )
    case "paragraph":
      return (
        <p>
          <Spans spans={block.spans} />
        </p>
      )
    case "list":
      return (
        <ul className="flex list-disc flex-col gap-1 pl-5">
          {block.items.map((spans, index) => (
            <li key={index}>
              <Spans spans={spans} />
            </li>
          ))}
        </ul>
      )
  }
}

function Spans({ spans }: { spans: NoteSpan[] }) {
  return spans.map((span, index) => {
    switch (span.kind) {
      case "text":
        return <Fragment key={index}>{span.text}</Fragment>
      case "bold":
        return (
          <strong key={index} className="font-semibold">
            {span.text}
          </strong>
        )
      case "code":
        return (
          <code key={index} className="font-mono text-xs">
            {span.text}
          </code>
        )
      case "link":
        return isWebLink(span.url) ? (
          <button
            key={index}
            type="button"
            className="text-accent hover:text-accent-hover hover:underline"
            onClick={() => void openWebPage(span.url)}
          >
            {span.text}
          </button>
        ) : (
          <Fragment key={index}>{span.text}</Fragment>
        )
    }
  })
}
