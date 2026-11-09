import { useState } from "react"
import { openDownloadFolder } from "./api"

/** Open Folder for the download folder, beside it in Settings → Downloads and at the end of Rules: `open` shows it in
 * File Explorer, and `error` holds Windows' reason when it couldn't. */
export function useOpenDownloadFolder() {
  const [error, setError] = useState<string | null>(null)

  async function open() {
    setError(null)
    try {
      await openDownloadFolder()
    } catch (caught) {
      setError(String(caught))
    }
  }

  return { open, error }
}
