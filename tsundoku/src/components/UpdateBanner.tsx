import { useState } from "react"
import { CircleArrowUp } from "lucide-react"
import type { ErrorDescription } from "../errors"
import { formatBytes } from "../format"
import { useAppUpdate } from "../queries"
import { strings } from "../strings"
import { Button } from "./Button"
import { Card } from "./Card"
import { ProgressBar } from "./ProgressBar"
import { StatusMessage } from "./StatusMessage"

const text = strings.appUpdate

/** Under the add bar when a newer Tsundoku is out: Install and Restart, or Later, which hides it until next time. */
export function UpdateBanner() {
  const { data: update } = useAppUpdate()
  const [dismissedVersion, setDismissedVersion] = useState<string | null>(null)
  const [progress, setProgress] = useState<{ received: number; total: number | null } | null>(null)
  const [error, setError] = useState<ErrorDescription | null>(null)
  if (!update || dismissedVersion === update.version) return null

  async function install() {
    if (!update) return
    setError(null)
    setProgress({ received: 0, total: null })
    try {
      await update.install((received, total) => setProgress({ received, total }))
    } catch (caught) {
      setProgress(null)
      setError({ message: text.failed, detail: String(caught) })
    }
  }

  const progressText =
    progress &&
    text.downloading(formatBytes(progress.received), progress.total === null ? null : formatBytes(progress.total))

  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex items-center gap-3">
        <CircleArrowUp size={18} className="shrink-0 text-accent" />
        <p className="min-w-0 flex-1 font-medium">{text.available(update.version)}</p>
        {!progress && (
          <>
            <Button size="sm" variant="ghost" onClick={() => setDismissedVersion(update.version)}>
              {text.later}
            </Button>
            <Button size="sm" variant="primary" onClick={() => void install()}>
              {text.install}
            </Button>
          </>
        )}
      </div>
      {progress && progressText && (
        <div className="flex flex-col gap-1.5">
          <ProgressBar value={progress.total ? progress.received / progress.total : undefined} label={progressText} />
          <p className="text-xs text-fg-secondary tabular-nums">{progressText}</p>
        </div>
      )}
      {error && <StatusMessage tone="danger" {...error} />}
    </Card>
  )
}
