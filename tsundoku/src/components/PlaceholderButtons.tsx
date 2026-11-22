import { useState, type RefObject } from "react"
import { insertPlaceholder } from "../placeholders"
import { strings } from "../strings"
import { Button } from "./Button"
import { SelectInput } from "./Field"

interface PlaceholderButtonsProps<Placeholder extends string> {
  /** The field the placeholders go in, at its cursor. */
  input: RefObject<HTMLInputElement | null>
  /** The field's text. */
  value: string
  placeholders: readonly Placeholder[]
  /** What each placeholder becomes, as its hover text. */
  hints: Readonly<Record<Placeholder, string>>
  /** The placeholders only some sites fill, by site (`SITE_PLACEHOLDERS`): a select before the buttons shows the
   * general ones alone (General, to start with), or with a site's. */
  sites?: Readonly<Record<string, readonly Placeholder[]>>
  onChange: (value: string) => void
}

/**
 * "Insert" and a button for each placeholder (`{site}`, in monospace) that puts it where the cursor is in the field,
 * over any selected text. The buttons don't take the focus, so the cursor stays in the field. With `sites`, a select
 * after "Insert" adds a site's own placeholders after the general ones.
 */
export function PlaceholderButtons<Placeholder extends string>({
  input,
  value,
  placeholders,
  hints,
  sites,
  onChange,
}: PlaceholderButtonsProps<Placeholder>) {
  // The site whose placeholders show too, or "" for the general ones alone.
  const [site, setSite] = useState("")
  const shown = [...placeholders, ...(site && sites ? (sites[site] ?? []) : [])]

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
      {sites && (
        <SelectInput
          aria-label={strings.placeholders.siteFilter}
          title={strings.placeholders.siteFilter}
          className="h-7 w-auto text-xs"
          value={site}
          onChange={(event) => setSite(event.target.value)}
        >
          <option value="">{strings.placeholders.general}</option>
          {Object.keys(sites).map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </SelectInput>
      )}
      {shown.map((placeholder) => (
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
