import type { RefObject } from "react"
import { insertPlaceholder } from "../placeholders"
import { strings } from "../strings"
import { Button } from "./Button"

interface PlaceholderButtonsProps<Placeholder extends string> {
  /** The field the placeholders go in, at its cursor. */
  input: RefObject<HTMLInputElement | null>
  /** The field's text. */
  value: string
  placeholders: readonly Placeholder[]
  /** What each placeholder becomes, as its hover text. */
  hints: Readonly<Record<Placeholder, string>>
  onChange: (value: string) => void
}

/**
 * "Insert" and a button for each placeholder (`{site}`, in monospace) that puts it where the cursor is in the field,
 * over any selected text. The buttons don't take the focus, so the cursor stays in the field.
 */
export function PlaceholderButtons<Placeholder extends string>({
  input,
  value,
  placeholders,
  hints,
  onChange,
}: PlaceholderButtonsProps<Placeholder>) {
  function insert(placeholder: Placeholder) {
    const field = input.current
    const start = field?.selectionStart ?? value.length
    const end = field?.selectionEnd ?? start
    const { text, cursor } = insertPlaceholder(value, placeholder, start, end)
    onChange(text)
    // Once the field shows the new text, since setting it moves the cursor to its end.
    requestAnimationFrame(() => {
      field?.focus()
      field?.setSelectionRange(cursor, cursor)
    })
  }

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <span className="text-xs text-fg-muted">{strings.placeholders.insert}</span>
      {placeholders.map((placeholder) => (
        <Button
          key={placeholder}
          size="sm"
          className="font-mono"
          title={hints[placeholder]}
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => insert(placeholder)}
        >
          {`{${placeholder}}`}
        </Button>
      ))}
    </div>
  )
}
