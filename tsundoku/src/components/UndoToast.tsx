import { useEffect, useRef, useState } from "react"
import { X } from "lucide-react"
import type { ErrorDescription } from "../errors"
import { strings } from "../strings"
import type { ShownToast } from "../undo"
import { Button } from "./Button"
import { IconButton } from "./IconButton"
import { StatusMessage } from "./StatusMessage"

/** How long the toast stays while the pointer and the keyboard are elsewhere. */
const SHOWN_FOR_MS = 8000

/** Where the toast shows: centered at the bottom of the positioned element it's in. Always there, so screen readers
 * announce each toast as it appears. */
export function ToastRegion({ offer, onClose }: { offer: ShownToast | null; onClose: (key: number) => void }) {
  return (
    <div aria-live="polite" className="pointer-events-none absolute inset-x-0 bottom-4 z-10 flex justify-center px-6">
      {offer && <Toast key={offer.key} offer={offer} onClose={onClose} />}
    </div>
  )
}

interface ToastProps {
  offer: ShownToast
  onClose: (key: number) => void
}

/**
 * One offer: what was done, its action (Undo, Open in Pending) and Dismiss. It closes by itself after a few seconds,
 * waiting while the pointer is over it or it has the keyboard's focus, and Escape closes it. When the action fails, it
 * says why and stays until dismissed.
 */
function Toast({ offer, onClose }: ToastProps) {
  const { key, message, action, describeError } = offer
  const [hovered, setHovered] = useState(false)
  const [focused, setFocused] = useState(false)
  const [running, setRunning] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const timeLeft = useRef(SHOWN_FOR_MS)
  const waiting = hovered || focused || running || error !== null

  useEffect(() => {
    if (waiting) return
    const startedAt = Date.now()
    const timer = setTimeout(() => onClose(key), timeLeft.current)
    return () => {
      clearTimeout(timer)
      timeLeft.current -= Date.now() - startedAt
    }
  }, [waiting, onClose, key])

  const runAction = () => {
    const result = action.run()
    if (!(result instanceof Promise)) {
      onClose(key)
      return
    }
    setRunning(true)
    result.then(
      () => onClose(key),
      (caught: unknown) => {
        if (!describeError) {
          onClose(key)
          return
        }
        setRunning(false)
        setError(describeError(caught))
      },
    )
  }

  return (
    <div
      className="pointer-events-auto flex max-w-full items-center gap-3 rounded-md border border-line-strong bg-bg py-2 pr-2 pl-4 shadow-lg"
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onFocus={() => setFocused(true)}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setFocused(false)
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") onClose(key)
      }}
    >
      {error ? (
        <div className="min-w-0">
          <StatusMessage tone="danger" {...error} />
        </div>
      ) : (
        <>
          <p className="min-w-0 truncate">{message}</p>
          <Button size="sm" loading={running} onClick={runAction}>
            {action.label}
          </Button>
        </>
      )}
      <IconButton label={strings.undoToast.dismiss} icon={X} onClick={() => onClose(key)} />
    </div>
  )
}
