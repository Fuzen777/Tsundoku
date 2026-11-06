import { useCallback, useEffect, useRef, useState, type ReactNode } from "react"
import { X } from "lucide-react"
import type { ErrorDescription } from "../errors"
import { strings } from "../strings"
import { OfferUndoContext, type UndoOffer } from "../undo"
import { Button } from "./Button"
import { IconButton } from "./IconButton"
import { StatusMessage } from "./StatusMessage"

/** How long the toast stays while the pointer and the keyboard are elsewhere. */
const SHOWN_FOR_MS = 8000

interface ShownOffer extends UndoOffer {
  /** Tells offers apart, so a new one starts a toast of its own. */
  key: number
}

/**
 * Lets the views inside it offer to undo a removal (`useOfferUndo`), with a toast centered at the bottom of the
 * positioned element it's in. A new offer replaces the one before.
 */
export function UndoToastProvider({ children }: { children: ReactNode }) {
  const [offer, setOffer] = useState<ShownOffer | null>(null)
  const lastKey = useRef(0)
  const show = useCallback((shown: UndoOffer) => {
    lastKey.current += 1
    setOffer({ ...shown, key: lastKey.current })
  }, [])
  // An older toast's Undo may finish after a newer toast replaced it; that one stays.
  const close = useCallback((key: number) => setOffer((current) => (current?.key === key ? null : current)), [])

  return (
    <OfferUndoContext value={show}>
      {children}
      {/* Always there, so screen readers announce each toast as it appears. */}
      <div aria-live="polite" className="pointer-events-none absolute inset-x-0 bottom-4 z-10 flex justify-center px-6">
        {offer && <UndoToast key={offer.key} offer={offer} onClose={close} />}
      </div>
    </OfferUndoContext>
  )
}

interface UndoToastProps {
  offer: ShownOffer
  onClose: (key: number) => void
}

/**
 * One offer: what was done, Undo and Dismiss. It closes by itself after a few seconds, waiting while the pointer is
 * over it or it has the keyboard's focus, and Escape closes it. When Undo fails, it says why and stays until dismissed.
 */
function UndoToast({ offer, onClose }: UndoToastProps) {
  const { key, message, undo, describeError } = offer
  const [hovered, setHovered] = useState(false)
  const [focused, setFocused] = useState(false)
  const [undoing, setUndoing] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const timeLeft = useRef(SHOWN_FOR_MS)
  const waiting = hovered || focused || undoing || error !== null

  useEffect(() => {
    if (waiting) return
    const startedAt = Date.now()
    const timer = setTimeout(() => onClose(key), timeLeft.current)
    return () => {
      clearTimeout(timer)
      timeLeft.current -= Date.now() - startedAt
    }
  }, [waiting, onClose, key])

  const undoNow = () => {
    setUndoing(true)
    undo().then(
      () => onClose(key),
      (caught: unknown) => {
        setUndoing(false)
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
          <Button size="sm" loading={undoing} onClick={undoNow}>
            {strings.undoToast.undo}
          </Button>
        </>
      )}
      <IconButton label={strings.undoToast.dismiss} icon={X} onClick={() => onClose(key)} />
    </div>
  )
}
