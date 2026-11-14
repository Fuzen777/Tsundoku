import { useEffect, useState } from "react"
import { Check, ClipboardCopy } from "lucide-react"
import { describeDownloadError, type ErrorDescription } from "../errors"
import { copyDownloadDetails } from "../queueItems"
import { strings } from "../strings"
import { IconButton } from "./IconButton"

/** How long the button says Copied after a copy. */
const COPIED_FOR_MS = 2000

interface CopyDetailsButtonProps {
  /** The failed download. */
  id: number
  /** Its error as the row shows it. */
  error: ErrorDescription
  /** Called with why copying failed, and with null when a new copy starts. */
  onError: (error: ErrorDescription | null) => void
}

/**
 * Copy Details on a failed download: puts what a bug report needs on the clipboard (Tsundoku's, the tools' and Windows'
 * versions, the link, the error and the end of the download's log, with the user's name blanked out), then says Copied
 * for a moment.
 */
export function CopyDetailsButton({ id, error, onError }: CopyDetailsButtonProps) {
  const [copying, setCopying] = useState(false)
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    if (!copied) return
    const timer = window.setTimeout(() => setCopied(false), COPIED_FOR_MS)
    return () => window.clearTimeout(timer)
  }, [copied])

  async function copy() {
    onError(null)
    setCopying(true)
    try {
      await copyDownloadDetails(id, error)
      setCopied(true)
    } catch (caught) {
      onError(describeDownloadError(caught))
    } finally {
      setCopying(false)
    }
  }

  return (
    <IconButton
      label={copied ? strings.queue.detailsCopied : strings.queue.copyDetails}
      icon={copied ? Check : ClipboardCopy}
      loading={copying}
      onClick={() => void copy()}
    />
  )
}
