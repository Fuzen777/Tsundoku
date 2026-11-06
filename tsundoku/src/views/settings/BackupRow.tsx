import { useState } from "react"
import { chooseBackupDestination, confirmWithUser, pickBackup, restoreBackup, saveBackup } from "../../api"
import { Button } from "../../components/Button"
import { StatusMessage } from "../../components/StatusMessage"
import { describeBackupError, type ErrorDescription } from "../../errors"
import { formatFileDate } from "../../format"
import { strings } from "../../strings"

const text = strings.settings

/**
 * Settings → About's Back Up and Restore: the settings, presets, folder rules and history saved in one file, and a
 * file like it put back, which restarts Tsundoku.
 */
export function BackupRow() {
  const [busy, setBusy] = useState<"backUp" | "restore" | null>(null)
  const [savedTo, setSavedTo] = useState<string | null>(null)
  const [error, setError] = useState<ErrorDescription | null>(null)

  async function backUp() {
    const path = await chooseBackupDestination(
      text.backupDialogTitle,
      text.backupFileName(formatFileDate(new Date())),
      text.backupFilter,
    )
    if (path === null) return
    setError(null)
    setSavedTo(null)
    setBusy("backUp")
    try {
      await saveBackup(path)
      setSavedTo(path)
    } catch (caught) {
      setError(describeBackupError(caught))
    } finally {
      setBusy(null)
    }
  }

  async function restore() {
    const path = await pickBackup(text.restoreDialogTitle, text.backupFilter)
    if (path === null) return
    const confirmed = await confirmWithUser({
      title: text.restoreTitle,
      message: text.restoreMessage,
      confirmLabel: text.restoreConfirm,
      cancelLabel: strings.dialogs.cancel,
    })
    if (!confirmed) return
    setError(null)
    setSavedTo(null)
    setBusy("restore")
    try {
      // Tsundoku restarts once it's restored, so this only returns when it failed.
      await restoreBackup(path)
    } catch (caught) {
      setError(describeBackupError(caught))
      setBusy(null)
    }
  }

  return (
    <>
      <div className="flex items-center gap-3 border-t border-line pt-3">
        <div className="min-w-0 flex-1">
          <p className="font-medium">{text.backup}</p>
          <p className="text-fg-secondary">{text.backupHint}</p>
        </div>
        <Button size="sm" loading={busy === "backUp"} disabled={busy !== null} onClick={() => void backUp()}>
          {text.backUp}
        </Button>
        <Button size="sm" loading={busy === "restore"} disabled={busy !== null} onClick={() => void restore()}>
          {text.restore}
        </Button>
      </div>
      {savedTo !== null && <StatusMessage tone="success" message={text.backupSaved} detail={savedTo} />}
      {error && <StatusMessage tone="danger" {...error} />}
    </>
  )
}
