import { useCallback } from "react"
import { copyText } from "./api"
import { strings } from "./strings"
import { useOfferToast } from "./undo"

/** Copy Link, on a row (also in the details panel) and in a picture's right-click menu: copies the link and says so
 * in the toast. The clipboard watcher leaves alone what Tsundoku itself copies, so it doesn't come back to Pending. */
export function useCopyLink(): (link: string) => void {
  const offerToast = useOfferToast()
  return useCallback(
    (link: string) => {
      copyText(link).then(
        () => offerToast({ message: strings.copyLink.copied }),
        () => offerToast({ message: strings.copyLink.failed }),
      )
    },
    [offerToast],
  )
}
