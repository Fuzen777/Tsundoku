import { useState, type ClipboardEvent, type KeyboardEvent } from "react"
import { X } from "lucide-react"
import { withTags } from "../tags"
import { classes } from "./styles"

interface TagInputProps {
  id?: string
  values: readonly string[]
  onChange: (values: string[]) => void
  placeholder?: string
  /** The tag's × button, for screen readers and its tooltip. */
  removeLabel: (value: string) => string
  disabled?: boolean
}

/**
 * A field of values as tags, in order, each with × to remove it, and a text field after them: Enter adds what's typed,
 * as does leaving the field, and pasted lines become a tag each. Commas and spaces don't split, so a value can hold
 * them (a password).
 */
export function TagInput({ id, values, onChange, placeholder, removeLabel, disabled = false }: TagInputProps) {
  const [typed, setTyped] = useState("")

  function add(added: readonly string[]) {
    const next = withTags(values, added)
    if (next !== values) onChange([...next])
    setTyped("")
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "Enter") return
    event.preventDefault()
    add([typed])
  }

  function onPaste(event: ClipboardEvent<HTMLInputElement>) {
    const text = event.clipboardData.getData("text")
    if (!/[\r\n]/.test(text.trim())) return
    event.preventDefault()
    add(text.split(/\r?\n/))
  }

  return (
    <div
      className={classes(
        "flex min-h-8 flex-wrap items-center gap-1.5 rounded-md border border-line bg-input px-1.5 py-1 transition-colors",
        "focus-within:border-accent hover:bg-input-hover",
        disabled && "opacity-40",
      )}
    >
      {values.map((value) => (
        <span
          key={value}
          className="inline-flex h-6 max-w-full items-center gap-1 rounded border border-line bg-bg pr-0.5 pl-2 font-mono text-xs"
        >
          <span className="truncate whitespace-pre">{value}</span>
          <button
            type="button"
            aria-label={removeLabel(value)}
            title={removeLabel(value)}
            disabled={disabled}
            className="flex size-5 shrink-0 items-center justify-center rounded text-fg-muted transition-colors hover:bg-input-hover hover:text-fg focus-visible:outline-2 focus-visible:outline-accent"
            onClick={() => onChange(values.filter((kept) => kept !== value))}
          >
            <X size={12} />
          </button>
        </span>
      ))}
      <input
        id={id}
        type="text"
        spellCheck={false}
        autoComplete="off"
        disabled={disabled}
        placeholder={placeholder}
        className="h-6 min-w-32 flex-1 bg-transparent px-1.5 font-mono text-sm outline-none placeholder:font-sans placeholder:text-fg-muted"
        value={typed}
        onChange={(event) => setTyped(event.target.value)}
        onKeyDown={onKeyDown}
        onPaste={onPaste}
        onBlur={() => {
          if (typed.trim()) add([typed])
        }}
      />
    </div>
  )
}
