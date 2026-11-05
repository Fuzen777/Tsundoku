import { useState } from "react"
import { ListVideo } from "lucide-react"
import {
  cancelDownload,
  clearFinishedDownloads,
  openDownload,
  openDownloadLog,
  resumeDownload,
  retryDownload,
  showDownloadInFolder,
  stopDownload,
} from "../api"
import type { DownloadProgress } from "../bindings/DownloadProgress"
import type { QueueItem } from "../bindings/QueueItem"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { EmptyState } from "../components/EmptyState"
import { Page } from "../components/Page"
import { ProgressBar } from "../components/ProgressBar"
import { StatusMessage } from "../components/StatusMessage"
import { Thumbnail } from "../components/Thumbnail"
import { describeDownloadError, describeHistoryError, type ErrorDescription } from "../errors"
import { formatBytes, formatDuration, formatSpeed, joinDetails } from "../format"
import { useQueue } from "../queries"
import { useDownloadProgress } from "../queueEvents"
import { strings } from "../strings"

export function QueueView() {
  const { data: items = [] } = useQueue()
  const [clearError, setClearError] = useState<ErrorDescription | null>(null)
  const hasFinished = items.some((item) => item.state === "done")

  const clearFinished = () => {
    setClearError(null)
    clearFinishedDownloads().catch((error: unknown) => setClearError(describeDownloadError(error)))
  }

  return (
    <Page
      title={strings.nav.queue}
      actions={
        hasFinished && (
          <Button size="sm" onClick={clearFinished}>
            {strings.queue.clearFinished}
          </Button>
        )
      }
    >
      {clearError && <StatusMessage tone="danger" {...clearError} />}
      {items.length === 0 ? (
        <EmptyState icon={ListVideo} title={strings.queue.emptyTitle} body={strings.queue.emptyBody} />
      ) : (
        <div className="flex flex-col gap-2">
          {items.map((item) => (
            <QueueRow key={item.id} item={item} />
          ))}
        </div>
      )}
    </Page>
  )
}

function QueueRow({ item }: { item: QueueItem }) {
  const progress = useDownloadProgress(item)
  const [actionError, setActionError] = useState<ErrorDescription | null>(null)
  const stage = stageLabel(item, progress)

  const act = (action: (id: number) => Promise<void>, describe = describeDownloadError) => {
    setActionError(null)
    action(item.id).catch((error: unknown) => setActionError(describe(error)))
  }
  const actOnFile = (action: (downloadId: number) => Promise<void>) => {
    if (!item.result) return
    setActionError(null)
    action(item.result.downloadId).catch((error: unknown) => setActionError(describeHistoryError(error)))
  }

  return (
    <Card className="flex items-start gap-3 p-3">
      <Thumbnail key={item.video.thumbnailUrl} url={item.video.thumbnailUrl} className="w-24" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="truncate font-medium" title={item.video.title}>
          {item.video.title}
        </p>
        <p className="truncate text-xs text-fg-secondary tabular-nums">
          {joinDetails([item.preset.name, stage, ...sizeDetails(item, progress)])}
        </p>
        {item.state === "running" && <ProgressBar value={progress?.fraction ?? undefined} label={stage} />}
        {item.state === "failed" && item.error && (
          <StatusMessage tone="danger" {...describeDownloadError(item.error)} />
        )}
        {actionError && <StatusMessage tone="danger" {...actionError} />}
      </div>
      <div className="flex shrink-0 flex-wrap items-start justify-end gap-1">
        {(item.state === "running" || item.state === "waiting") && (
          <Button size="sm" onClick={() => act(stopDownload)}>
            {strings.queue.stop}
          </Button>
        )}
        {item.state === "stopped" && (
          <Button size="sm" onClick={() => act(resumeDownload)}>
            {strings.queue.resume}
          </Button>
        )}
        {item.state === "failed" && (
          <Button size="sm" onClick={() => act(retryDownload)}>
            {strings.queue.retry}
          </Button>
        )}
        {item.state === "done" && (
          <>
            <Button size="sm" onClick={() => actOnFile(openDownload)}>
              {strings.queue.openFile}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => actOnFile(showDownloadInFolder)}>
              {strings.queue.showInFolder}
            </Button>
          </>
        )}
        {(item.state === "failed" || item.state === "done") && (
          <Button size="sm" variant="ghost" onClick={() => act(openDownloadLog)}>
            {strings.queue.openLog}
          </Button>
        )}
        {item.state !== "done" && (
          <Button size="sm" variant="ghost" onClick={() => act(cancelDownload)}>
            {strings.queue.cancel}
          </Button>
        )}
      </div>
    </Card>
  )
}

/** What the item is doing: its state, or while it runs, the stage yt-dlp is at. */
function stageLabel(item: QueueItem, progress: DownloadProgress | null): string {
  const stages = strings.queue.stages
  return item.state === "running" ? stages[progress?.stage ?? "starting"] : stages[item.state]
}

/** Size, speed and time left while downloading; the file's size once done. */
function sizeDetails(item: QueueItem, progress: DownloadProgress | null): (string | null)[] {
  if (item.state === "done") {
    return [item.result?.fileSize ? formatBytes(item.result.fileSize) : null]
  }
  if (!progress || progress.totalBytes === null) return []
  return [
    strings.queue.progress(formatBytes(progress.downloadedBytes ?? 0), formatBytes(progress.totalBytes)),
    progress.speed ? formatSpeed(progress.speed) : null,
    progress.eta !== null && progress.fraction !== 1 ? strings.queue.timeLeft(formatDuration(progress.eta)) : null,
  ]
}
