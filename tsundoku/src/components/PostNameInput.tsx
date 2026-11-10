import { useRef, type KeyboardEvent, type ReactNode } from "react"
import type { PostNamePlaceholder } from "../bindings/PostNamePlaceholder"
import { fillPlaceholders } from "../placeholders"
import { strings } from "../strings"
import { TextInput } from "./Field"
import { PlaceholderButtons } from "./PlaceholderButtons"

const text = strings.settings

/** How posts' files are named unless Settings says otherwise (`post_names::DEFAULT_POST_FILE_NAME` in the core). */
export const DEFAULT_POST_FILE_NAME = "{account} {date} {post} {number}"

/** What a post file's name can hold, in the order the Insert buttons offer them. */
const POST_NAME_PLACEHOLDERS: readonly PostNamePlaceholder[] = ["account", "date", "post", "number", "site"]

interface PostNameInputProps {
  id: string
  value: string
  onChange: (value: string) => void
  /** The name the example shows while the field is empty: Settings' for a rule, the default for Settings. */
  emptyName: string
  disabled?: boolean
  onBlur?: () => void
  onKeyDown?: (event: KeyboardEvent<HTMLInputElement>) => void
  /** Beside the field, such as Use Default Name. */
  besideField?: ReactNode
  /** Beside the example, such as "Saved." */
  besideExample?: ReactNode
}

/**
 * A post's files' name, as Settings → Posts of Images and a rule's File Name write it: the field, a button for each
 * placeholder that puts it at the cursor, and the name a file would get, with sample values.
 */
export function PostNameInput({
  id,
  value,
  onChange,
  emptyName,
  disabled,
  onBlur,
  onKeyDown,
  besideField,
  besideExample,
}: PostNameInputProps) {
  const input = useRef<HTMLInputElement>(null)
  const example = `${fillPlaceholders(value.trim() || emptyName, text.postNameSamples)}.${text.postNameSampleExtension}`
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <TextInput
          ref={input}
          id={id}
          className="flex-1 font-mono"
          value={value}
          disabled={disabled}
          onChange={(event) => onChange(event.target.value)}
          onBlur={onBlur}
          onKeyDown={onKeyDown}
        />
        {besideField}
      </div>
      <PlaceholderButtons
        input={input}
        value={value}
        placeholders={POST_NAME_PLACEHOLDERS}
        hints={text.postNamePlaceholderHints}
        onChange={onChange}
      />
      <div className="flex items-center gap-3">
        <p className="min-w-0 flex-1 truncate text-xs text-fg-muted" title={example}>
          {text.postFileNameExample} <span className="font-mono text-fg-secondary select-text">{example}</span>
        </p>
        {besideExample}
      </div>
    </div>
  )
}
