import { useEffect, useRef, useState, type FormEvent } from "react"
import { strings } from "../strings"
import { Button } from "./Button"
import { TextInput } from "./Field"
import { classes } from "./styles"

interface PasswordFormProps {
  /** Opens the link with the password typed. */
  onUnlock: (password: string) => void
  /** A password is being tried: the field and Unlock wait. */
  busy?: boolean
  /** `sm`, as a row's controls are (Pending, the queue), or the card's size. */
  size?: "sm" | "md"
  /** Takes the keyboard's focus when it shows: not in a list of several (Pending). */
  autoFocus?: boolean
}

/**
 * The password field and Unlock (Enter works too), for a link that needs a password: on the preview card, a pending
 * link's row and a download's. The parent gives it a new `key` after a wrong password, so the field starts empty.
 * Nothing is kept here once it's sent.
 */
export function PasswordForm({ onUnlock, busy = false, size = "md", autoFocus = true }: PasswordFormProps) {
  const [password, setPassword] = useState("")
  const field = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (autoFocus) field.current?.focus()
  }, [autoFocus])

  function submit(event: FormEvent) {
    event.preventDefault()
    if (password && !busy) onUnlock(password)
  }

  return (
    <form className="flex items-center gap-2" onSubmit={submit}>
      <TextInput
        ref={field}
        type="password"
        aria-label={strings.password.field}
        placeholder={strings.password.field}
        value={password}
        disabled={busy}
        onChange={(event) => setPassword(event.target.value)}
        className={classes("w-56", size === "sm" && "h-7 text-xs")}
      />
      <Button type="submit" size={size} variant="secondary" loading={busy} disabled={!password}>
        {strings.password.unlock}
      </Button>
    </form>
  )
}
