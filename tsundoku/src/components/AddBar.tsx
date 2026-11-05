import { strings } from "../strings"
import { Button } from "./Button"

interface AddBarProps {
  link: string
  onLinkChange: (link: string) => void
  /** Enter or Add, with a link in the field. */
  onSubmit: () => void
}

/** Where a video link is pasted, across the top of the window. */
export function AddBar({ link, onLinkChange, onSubmit }: AddBarProps) {
  const hasLink = link.trim() !== ""
  return (
    <form
      className="flex flex-1 gap-2"
      onSubmit={(event) => {
        event.preventDefault()
        if (hasLink) onSubmit()
      }}
    >
      <input
        type="text"
        name="link"
        value={link}
        onChange={(event) => onLinkChange(event.target.value)}
        placeholder={strings.addBar.placeholder}
        spellCheck={false}
        autoComplete="off"
        className="h-8 min-w-0 flex-1 rounded-md border border-line bg-input px-3 text-sm outline-none transition-colors placeholder:text-fg-muted hover:bg-input-hover focus:border-accent"
      />
      <Button type="submit" variant="primary" disabled={!hasLink}>
        {strings.addBar.add}
      </Button>
    </form>
  )
}
