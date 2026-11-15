import { useState } from "react"
import { ChevronUp, CircleStop, FileText, FolderOpen, Play, RefreshCw, X } from "lucide-react"
import {
  cancelDownload,
  downloadNext,
  enterDownloadPassword,
  openCheckWindow,
  openDownload,
  resumeDownload,
  retryDownload,
  showDownloadInFolder,
  stopDownload,
  tryDownloadNow,
} from "../api"
import type { QueueItem } from "../bindings/QueueItem"
import { describeDownloadError, fixFor, type ErrorDescription } from "../errors"
import { formatContents, joinDetails } from "../format"
import { canTryNow, describeHold, holdEnd, useTimeLeft } from "../holds"
import { pickSummary } from "../postFiles"
import { useDownloadProgress } from "../queueEvents"
import { changeFolder, moveToFolder, onAControl, sizeDetails, stageLabel, useItemActions } from "../queueItems"
import { strings } from "../strings"
import { Button } from "./Button"
import { Card } from "./Card"
import { CopyDetailsButton } from "./CopyDetailsButton"
import { FailureFixes } from "./FailureFixes"
import { IconButton } from "./IconButton"
import { LogPanel } from "./LogPanel"
import { Menu } from "./Menu"
import { PasswordForm } from "./PasswordForm"
import { ProgressBar } from "./ProgressBar"
import { StatusMessage } from "./StatusMessage"
import { Thumbnail } from "./Thumbnail"

/** What a download's row opens with, when it's shown for a picture in the Queue's Masonry or Gallery: its log, or the
 * field for its password. */
export type RowOpening = "log" | "password" | null

interface QueueRowProps {
  item: QueueItem
  opens?: RowOpening
  /** Shown for a picture: hides the row again, with a button of its own. */
  onHide?: () => void
}

/** A download in the Queue: its picture, title, what it's doing and its actions. A double-click on a finished one opens
 * its file, as Open File does. */
export function QueueRow({ item, opens = null, onHide }: QueueRowProps) {
  const progress = useDownloadProgress(item)
  const [actionError, setActionError] = useState<ErrorDescription | null>(null)
  const [showsLog, setShowsLog] = useState(opens === "log")
  const hold = item.state === "waiting" ? item.hold : null
  // The check window saving the file shows as a download, with its progress.
  const savingInWindow = hold?.kind === "windowDownload"
  const stage = savingInWindow ? strings.queue.stages.downloading : stageLabel(item, progress)
  const holdTimeLeft = useTimeLeft(hold ? holdEnd(hold) : null)
  // Enter Password shows the field; it's sent once, then the download runs with it.
  const [entersPassword, setEntersPassword] = useState(opens === "password")
  const [sendingPassword, setSendingPassword] = useState(false)
  const { act, removeWithUndo, actOnFile } = useItemActions(item, setActionError)
  const opensFile = item.state === "done" && item.result !== null

  return (
    <Card
      className="flex items-start gap-3 p-3"
      onDoubleClick={(event) => {
        if (opensFile && !onAControl(event.target)) actOnFile(openDownload)
      }}
      // Its double-click opens the file rather than selecting a word of its title.
      onMouseDown={(event) => {
        if (opensFile && event.detail > 1 && !onAControl(event.target)) event.preventDefault()
      }}
    >
      <Thumbnail key={item.video.thumbnailUrl} url={item.video.thumbnailUrl} className="w-24" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="truncate font-medium" title={item.video.title}>
          {item.video.title}
        </p>
        <p className="truncate text-xs text-fg-secondary tabular-nums">
          {joinDetails([
            item.preset?.name ?? pickSummary(item.video) ?? formatContents(item.video),
            stage,
            ...sizeDetails(item, progress),
          ])}
        </p>
        {(item.state === "running" || savingInWindow) && (
          <ProgressBar value={progress?.fraction ?? undefined} label={stage} />
        )}
        {hold && (
          <StatusMessage tone="neutral" message={describeHold(hold)} detail={holdTimeLeft ?? undefined}>
            {hold.kind === "browserCheck" && (
              <Button size="sm" onClick={() => act(openCheckWindow)}>
                {strings.queue.openCheck}
              </Button>
            )}
            {hold.kind === "needsPassword" &&
              (entersPassword ? (
                <PasswordForm
                  size="sm"
                  busy={sendingPassword}
                  onUnlock={(password) => {
                    setSendingPassword(true)
                    act((id) =>
                      enterDownloadPassword(id, password).finally(() => {
                        setSendingPassword(false)
                        setEntersPassword(false)
                      }),
                    )
                  }}
                />
              ) : (
                <Button size="sm" onClick={() => setEntersPassword(true)}>
                  {strings.queue.enterPassword}
                </Button>
              ))}
          </StatusMessage>
        )}
        {item.state === "failed" && item.error && (
          <StatusMessage tone="danger" {...describeDownloadError(item.error)}>
            {fixFor(item.error) && (
              <FailureFixes
                error={item.error}
                // An archive's password, once added, retries at once: it only unpacks.
                onRetry={item.error.kind === "unpack" ? () => act(retryDownload) : undefined}
              />
            )}
          </StatusMessage>
        )}
        {actionError && <StatusMessage tone="danger" {...actionError} />}
        {showsLog && item.state !== "waiting" && <LogPanel downloadId={item.id} live={item.state === "running"} />}
      </div>
      <div className="flex shrink-0 flex-wrap items-start justify-end gap-1">
        {(item.state === "running" || item.state === "waiting") && (
          <IconButton
            label={strings.queue.stop}
            icon={CircleStop}
            variant="secondary"
            onClick={() => act(stopDownload)}
          />
        )}
        {canTryNow(hold) && (
          <IconButton label={strings.queue.tryNow} icon={RefreshCw} onClick={() => act(tryDownloadNow)} />
        )}
        {item.state === "stopped" && (
          <IconButton
            label={strings.queue.resume}
            icon={Play}
            variant="secondary"
            onClick={() => act(resumeDownload)}
          />
        )}
        {item.state === "failed" && (
          <IconButton
            label={strings.queue.retry}
            icon={RefreshCw}
            variant="secondary"
            onClick={() => act(retryDownload)}
          />
        )}
        {item.state === "done" && (
          <>
            <IconButton
              label={strings.queue.openFile}
              icon={Play}
              variant="secondary"
              onClick={() => actOnFile(openDownload)}
            />
            <IconButton
              label={strings.queue.showInFolder}
              icon={FolderOpen}
              onClick={() => actOnFile(showDownloadInFolder)}
            />
          </>
        )}
        {item.state !== "waiting" && (
          <IconButton
            label={showsLog ? strings.queue.hideLog : strings.queue.showLog}
            icon={FileText}
            aria-expanded={showsLog}
            onClick={() => setShowsLog((shown) => !shown)}
          />
        )}
        {item.state === "failed" && item.error && (
          <CopyDetailsButton id={item.id} error={describeDownloadError(item.error)} onError={setActionError} />
        )}
        {item.state === "done" ? (
          <IconButton label={strings.queue.removeFinished} icon={X} onClick={() => act(removeWithUndo)} />
        ) : (
          <IconButton label={strings.queue.cancel} icon={X} onClick={() => act(cancelDownload)} />
        )}
        {(item.state === "waiting" || item.state === "stopped" || item.state === "failed") && (
          <Menu
            label={strings.queue.moreActions}
            items={[
              ...(item.state === "waiting" ? [{ label: strings.queue.downloadNext, onSelect: () => act(downloadNext) }] : []),
              { label: strings.queue.changeFolder, onSelect: () => act((id) => changeFolder(id, item)) },
              ...(item.folderChosen
                ? [{ label: strings.queue.usualFolder, onSelect: () => act((id) => moveToFolder(id, item, null)) }]
                : []),
            ]}
          />
        )}
        {onHide && <IconButton label={strings.queue.tiles.hideDetails} icon={ChevronUp} onClick={onHide} />}
      </div>
    </Card>
  )
}
