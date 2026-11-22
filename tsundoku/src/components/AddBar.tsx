import { useEffect, useImperativeHandle, useRef, useState, type Ref } from "react"
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
  /** Why the last paste added nothing, said under Paste Link. */
  note: string | null
  /** The field opens: the note has nothing left to say once it's there. */
  onTypeLink: () => void
  /** The field's text: what was typed, or the link shown last, there to be edited. */
  link: string
  onLinkChange: (link: string) => void
  /** Enter or Add, with text in the field. */
  onSubmit: () => void
  ref?: Ref<AddBarHandle>
}

/**
 * At the top of the sidebar: Paste Link across it, which adds what was copied in one click, and Type a Link beside it,
 * which opens a field to type or edit a link in, in a panel under them reaching over the view, until it's added or
 * closed. Why a paste added nothing shows under Paste Link for a few seconds, over the sidebar, so nothing moves.
 */
export function AddBar({ onPaste, note, onTypeLink, link, onLinkChange, onSubmit, ref }: AddBarProps) {
  const [typing, setTyping] = useState(false)
  const bar = useRef<HTMLDivElement>(null)
  const field = useRef<HTMLInputElement>(null)
  const typeLinkButton = useRef<HTMLButtonElement>(null)
  const hasLink = link.trim() !== ""

  // The field is drawn before it's focused, so the keyboard carries on from it, then from Type a Link again.
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
    typeLinkButton.current?.focus()
  }
  useImperativeHandle(ref, () => ({ typeLink }))

  // A click anywhere else closes the field, keeping its text, as Esc does, and leaves the focus to what was clicked.
  useEffect(() => {
    if (!typing) return
    const closeOnOutsideClick = (event: MouseEvent) => {
      if (!bar.current?.contains(event.target as Node)) setTyping(false)
    }
    document.addEventListener("mousedown", closeOnOutsideClick)
    return () => document.removeEventListener("mousedown", closeOnOutsideClick)
  }, [typing])

  return (
    <div ref={bar} className="relative flex items-center gap-2">
      {/* Ctrl+V outside a text field does the same (shortcuts.ts). */}
      <Button
        variant="primary"
        className="min-w-0 flex-1"
        title={strings.shortcuts.withKeys(strings.addBar.pasteHint, strings.shortcuts.pasteAnywhere)}
        onClick={() => {
          setTyping(false)
          onPaste()
        }}
      >
        <ClipboardPaste size={16} />
        {strings.addBar.paste}
      </Button>
      <IconButton
        ref={typeLinkButton}
        label={strings.shortcuts.withKeys(strings.addBar.typeLink, strings.shortcuts.typeLink)}
        icon={TextCursorInput}
        aria-expanded={typing}
        aria-haspopup="dialog"
        onClick={typing ? stopTyping : typeLink}
      />
      {note && !typing && (
        <p
          role="status"
          className="pointer-events-none absolute top-full right-0 left-0 z-10 mt-1.5 animate-tooltip-in rounded border border-line-strong bg-bg px-2 py-1 text-xs text-fg shadow-md"
        >
          {note}
        </p>
      )}
      {typing && (
        <form
          role="dialog"
          aria-label={strings.addBar.typeLink}
          className="absolute top-full left-0 z-20 mt-1 flex w-[32rem] max-w-[calc(100vw-1rem)] gap-2 rounded-md border border-line-strong bg-bg p-2 shadow-lg"
          onKeyDown={(event) => {
            if (event.key !== "Escape") return
            event.preventDefault()
            stopTyping()
          }}
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
      )}
    </div>
  )
}
