import { strings } from "../strings"
import { Button } from "./Button"
import { TextInput } from "./Field"

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
      <TextInput
        name="link"
        value={link}
        onChange={(event) => onLinkChange(event.target.value)}
        placeholder={strings.addBar.placeholder}
        className="flex-1"
      />
      <Button type="submit" variant="primary" disabled={!hasLink}>
        {strings.addBar.add}
      </Button>
    </form>
  )
}
