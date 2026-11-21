import { useImperativeHandle, useRef, useState, type Ref } from "react"
import { flushSync } from "react-dom"
import { ClipboardPaste, TextCursorInput, X } from "lucide-react"
import { strings } from "../strings"
import { Button } from "./Button"
import { TextInput } from "./Field"
import { IconButton } from "./IconButton"

/** What the app asks of the add bar. */
export interface AddBarHandle {
  /** Opens the field to type or edit a link, with its text selected (Ctrl+L). */
  typeLink: () => void
}

interface AddBarProps {
  /** Paste Link: adds the links the clipboard holds. */
  onPaste: () => void
  /** Why the last paste added nothing, said beside the button. */
  note: string | null
  /** The field opens, in the note's place: it has nothing left to say once Paste Link is back. */
  onTypeLink: () => void
  /** The field's text: what was typed, or the link shown last, there to be edited. */
  link: string
  onLinkChange: (link: string) => void
  /** Enter or Add, with text in the field. */
  onSubmit: () => void
  ref?: Ref<AddBarHandle>
}

/**
 * Across the top of the window: Paste Link, which adds what was copied in one click, and Type a Link beside it, which
 * swaps the two for a field to type or edit a link in, until it's added or closed.
 */
export function AddBar({ onPaste, note, onTypeLink, link, onLinkChange, onSubmit, ref }: AddBarProps) {
  const [typing, setTyping] = useState(false)
  const field = useRef<HTMLInputElement>(null)
  const pasteButton = useRef<HTMLButtonElement>(null)
  const hasLink = link.trim() !== ""

  // Each is drawn before it's focused, so the keyboard carries on from the field, then from the button again.
  function typeLink() {
    flushSync(() => {
      setTyping(true)
      onTypeLink()
    })
    field.current?.focus()
    field.current?.select()
  }
  function stopTyping() {
    flushSync(() => setTyping(false))
    pasteButton.current?.focus()
  }
  useImperativeHandle(ref, () => ({ typeLink }))

  if (typing) {
    return (
      <form
        className="flex min-w-0 flex-1 gap-2"
        onSubmit={(event) => {
          event.preventDefault()
          if (!hasLink) return
          onSubmit()
          stopTyping()
        }}
      >
        <TextInput
          ref={field}
          name="link"
          value={link}
          onChange={(event) => onLinkChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== "Escape") return
            event.preventDefault()
            stopTyping()
          }}
          placeholder={strings.addBar.placeholder}
          className="min-w-0 flex-1"
        />
        <Button type="submit" variant="primary" disabled={!hasLink}>
          {strings.addBar.add}
        </Button>
        <IconButton
          label={strings.shortcuts.withKeys(strings.addBar.close, strings.shortcuts.closeField)}
          icon={X}
          onClick={stopTyping}
        />
      </form>
    )
  }

  return (
    <div className="flex min-w-0 flex-1 items-center gap-2">
      {/* Ctrl+V outside a text field does the same (shortcuts.ts). */}
      <Button
        ref={pasteButton}
        variant="primary"
        title={strings.shortcuts.withKeys(strings.addBar.pasteHint, strings.shortcuts.pasteAnywhere)}
        onClick={onPaste}
      >
        <ClipboardPaste size={16} />
        {strings.addBar.paste}
      </Button>
      <IconButton
        label={strings.shortcuts.withKeys(strings.addBar.typeLink, strings.shortcuts.typeLink)}
        icon={TextCursorInput}
        onClick={typeLink}
      />
      {note && (
        <p role="status" className="min-w-0 truncate text-fg-secondary">
          {note}
        </p>
      )}
    </div>
  )
}
