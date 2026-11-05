import { strings } from "../strings"
import { Button } from "./Button"

/** Where a video link is pasted. Fetching the link's details comes with the preview card. */
export function AddBar() {
  return (
    <form className="flex flex-1 gap-2" onSubmit={(event) => event.preventDefault()}>
      <input
        type="text"
        name="link"
        placeholder={strings.addBar.placeholder}
        spellCheck={false}
        autoComplete="off"
        className="h-8 min-w-0 flex-1 rounded-md border border-line bg-input px-3 text-sm outline-none transition-colors placeholder:text-fg-muted hover:bg-input-hover focus:border-accent"
      />
      <Button type="submit" variant="primary" disabled>
        {strings.addBar.add}
      </Button>
    </form>
  )
}
