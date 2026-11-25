import { createContext, useCallback, useContext, useRef, useState } from "react"
import type { ErrorDescription } from "./errors"
import { strings } from "./strings"

/** What the toast (components/UndoToast.tsx) shows: what was done, and one action about it. */
export interface ToastOffer {
  /** What was done, as the toast says it: "Removed 12 links". */
  message: string
  /** Undo, or another way on ("Open in Pending"). One that answers a promise shows it running, and closes the toast
   * once it's done; when it fails, the toast says why and stays. */
  action: { label: string; run: () => Promise<void> | void }
  /** Turns a failure of the action into a sentence. Without it, a failure closes the toast. */
  describeError?: (error: unknown) => ErrorDescription
}

/** Shows the toast, replacing the one before. App keeps it, so it stays when another view opens. */
export const OfferToastContext = createContext<(offer: ToastOffer) => void>(() => {})

export function useOfferToast(): (offer: ToastOffer) => void {
  return useContext(OfferToastContext)
}

/** A toast shown, with a key telling offers apart, so a new one starts a toast of its own. */
export interface ShownToast extends ToastOffer {
  key: number
}

/** The toast's state, which App keeps (`OfferToastContext` hands out `show`, `ToastRegion` shows it): a new offer
 * replaces the one before. */
export function useToasts() {
  const [offer, setOffer] = useState<ShownToast | null>(null)
  const lastKey = useRef(0)
  const show = useCallback((shown: ToastOffer) => {
    lastKey.current += 1
    setOffer({ ...shown, key: lastKey.current })
  }, [])
  // An older toast's action may finish after a newer toast replaced it; that one stays.
  const close = useCallback((key: number) => setOffer((current) => (current?.key === key ? null : current)), [])
  return { offer, show, close }
}

/** What a view hands the toast once it has removed something. */
export interface UndoOffer {
  /** What was done, as the toast says it: "Removed 12 links". */
  message: string
  /** Puts back what was removed. */
  undo: () => Promise<void>
  /** Turns a failure of `undo` into a sentence. */
  describeError: (error: unknown) => ErrorDescription
}

/** Shows the toast for a removal, with Undo, replacing the one before: only the latest removal can be undone. */
export function useOfferUndo(): (offer: UndoOffer) => void {
  const offerToast = useOfferToast()
  return useCallback(
    ({ message, undo, describeError }: UndoOffer) =>
      offerToast({ message, action: { label: strings.undoToast.undo, run: undo }, describeError }),
    [offerToast],
  )
}
