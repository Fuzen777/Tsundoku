import type { Ref } from "react"
import { strings } from "../strings"
import { Button } from "./Button"
import { TextInput } from "./Field"

interface AddBarProps {
  link: string
  onLinkChange: (link: string) => void
  /** Enter or Add, with a link in the field. */
  onSubmit: () => void
  /** The field, for Ctrl+L to focus. */
  inputRef?: Ref<HTMLInputElement>
}

/** Where a video link is pasted, across the top of the window. */
export function AddBar({ link, onLinkChange, onSubmit, inputRef }: AddBarProps) {
  const hasLink = link.trim() !== ""
  return (
    <form
      className="flex flex-1 gap-2"
      onSubmit={(event) => {
        event.preventDefault()
        if (hasLink) onSubmit()
      }}
    >
      <TextInput
        ref={inputRef}
        name="link"
        value={link}
        onChange={(event) => onLinkChange(event.target.value)}
        placeholder={strings.addBar.placeholder}
        title={strings.shortcuts.withKeys(strings.addBar.placeholder, strings.shortcuts.focusAddBar)}
        className="flex-1"
      />
      {/* Ctrl+V outside a text field pastes and adds in one go (shortcuts.ts). */}
      <span title={strings.shortcuts.withKeys(strings.addBar.add, strings.shortcuts.pasteAnywhere)} className="inline-flex">
        <Button type="submit" variant="primary" disabled={!hasLink}>
          {strings.addBar.add}
        </Button>
      </span>
    </form>
  )
}
