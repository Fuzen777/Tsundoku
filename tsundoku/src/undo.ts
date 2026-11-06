import { createContext, useContext } from "react"
import type { ErrorDescription } from "./errors"

/** What a view hands the undo toast (components/UndoToast.tsx) once it has removed something. */
export interface UndoOffer {
  /** What was done, as the toast says it: "Removed 12 links". */
  message: string
  /** Puts back what was removed. */
  undo: () => Promise<void>
  /** Turns a failure of `undo` into a sentence. */
  describeError: (error: unknown) => ErrorDescription
}

/** Shows the undo toast for a removal, replacing the one before: only the latest removal can be undone. */
export const OfferUndoContext = createContext<(offer: UndoOffer) => void>(() => {})

export function useOfferUndo(): (offer: UndoOffer) => void {
  return useContext(OfferUndoContext)
}
