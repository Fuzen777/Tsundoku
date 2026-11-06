import { useState } from "react"
import { CircleArrowUp, ExternalLink } from "lucide-react"
import { openWebPage } from "../api"
import type { ErrorDescription } from "../errors"
import { formatBytes } from "../format"
import { useAppUpdate } from "../queries"
import { strings } from "../strings"
import { Button } from "./Button"
import { Card } from "./Card"
import { ExpandToggle, LinkButton } from "./LinkButton"
import { ProgressBar } from "./ProgressBar"
import { ReleaseNotes } from "./ReleaseNotes"
import { StatusMessage } from "./StatusMessage"

const text = strings.appUpdate

/**
 * Under the add bar when a newer Tsundoku is out: Install and Restart, or Later, which hides it until next time. Under
 * them, What's New shows the release's notes, or Release Notes opens its page when it came without any.
 */
export function UpdateBanner() {
  const { data: update } = useAppUpdate()
  const [dismissedVersion, setDismissedVersion] = useState<string | null>(null)
  const [notesOpen, setNotesOpen] = useState(false)
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
      {update.notes ? (
        <div className="ml-7.5 flex flex-col gap-2">
          <ExpandToggle expanded={notesOpen} onToggle={() => setNotesOpen(!notesOpen)} label={text.whatsNew} />
          {notesOpen && (
            <div className="max-h-72 overflow-y-auto">
              <ReleaseNotes markdown={update.notes} />
            </div>
          )}
        </div>
      ) : (
        <div className="ml-7.5">
          <LinkButton icon={ExternalLink} label={text.releaseNotes} onClick={() => void openWebPage(update.releasePage)} />
        </div>
      )}
      {error && <StatusMessage tone="danger" {...error} />}
    </Card>
  )
}
