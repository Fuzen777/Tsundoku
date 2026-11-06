import { useState } from "react"
import { CircleStop, FileText, FolderOpen, ListVideo, Play, RefreshCw, X } from "lucide-react"
import {
  cancelDownload,
  clearFinishedDownloads,
  downloadNext,
  openDownload,
  openDownloadLog,
  resumeAllDownloads,
  resumeDownload,
  retryDownload,
  retryFailedDownloads,
  showDownloadInFolder,
  stopAllDownloads,
  stopDownload,
} from "../api"
import { arrange, folderGroup, type GroupPlace, type SortFacts } from "../arrange"
import type { DownloadProgress } from "../bindings/DownloadProgress"
import type { ListGroup } from "../bindings/ListGroup"
import type { QueueItem } from "../bindings/QueueItem"
import type { QueueState } from "../bindings/QueueState"
import { ArrangeControls, GroupedList } from "../components/ArrangedList"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { EmptyState } from "../components/EmptyState"
import { FailureFixes } from "../components/FailureFixes"
import { IconButton } from "../components/IconButton"
import { Menu } from "../components/Menu"
import { Page } from "../components/Page"
import { ProgressBar } from "../components/ProgressBar"
import { StatusMessage } from "../components/StatusMessage"
import { Thumbnail } from "../components/Thumbnail"
import { describeDownloadError, describeHistoryError, fixFor, type ErrorDescription } from "../errors"
import { formatBytes, formatContents, formatDuration, formatSpeed, joinDetails } from "../format"
import { useListArrangement, useQueue } from "../queries"
import { useDownloadProgress } from "../queueEvents"
import { strings } from "../strings"

/** The buttons for the whole queue, each shown while a download is in a state it acts on, as the row's own button. */
const WHOLE_QUEUE_ACTIONS: readonly { label: string; states: readonly QueueState[]; action: () => Promise<void> }[] = [
  { label: strings.queue.pauseAll, states: ["running", "waiting"], action: stopAllDownloads },
  { label: strings.queue.resumeAll, states: ["stopped"], action: resumeAllDownloads },
  { label: strings.queue.retryFailed, states: ["failed"], action: retryFailedDownloads },
  { label: strings.queue.clearFinished, states: ["done"], action: clearFinishedDownloads },
]

/** Every download added and not cleared yet, grouped and sorted as its toolbar says. */
export function QueueView() {
  const { data: items = [] } = useQueue()
  const [queueError, setQueueError] = useState<ErrorDescription | null>(null)
  const { arrangement, ready, change: changeArrangement, error: arrangeError } = useListArrangement("queue")
  const { group } = arrangement
  const groups = arrange(items, arrangement, queueFacts, group === "none" ? null : (item) => queueGroup(item, group))
  const actions = WHOLE_QUEUE_ACTIONS.filter(({ states }) => items.some((item) => states.includes(item.state)))

  const actOnAll = (action: () => Promise<void>) => {
    setQueueError(null)
    action().catch((error: unknown) => setQueueError(describeDownloadError(error)))
  }

  return (
    <Page
      title={strings.nav.queue}
      actions={
        actions.length > 0 &&
        actions.map(({ label, action }) => (
          <Button key={label} size="sm" onClick={() => actOnAll(action)}>
            {label}
          </Button>
        ))
      }
    >
      {queueError && <StatusMessage tone="danger" {...queueError} />}
      {items.length === 0 ? (
        <EmptyState icon={ListVideo} title={strings.queue.emptyTitle} body={strings.queue.emptyBody} />
      ) : (
        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
            <p className="text-fg-secondary">{strings.queue.count(items.length)}</p>
            <ArrangeControls
              arrangement={arrangement}
              disabled={!ready}
              onChange={(changes) => void changeArrangement(changes)}
            />
          </div>
          {arrangeError && <StatusMessage tone="danger" {...arrangeError} />}
          <GroupedList
            groups={groups}
            grouped={group !== "none"}
            keyOf={(item) => item.id}
            renderItem={(item) => <QueueRow item={item} />}
          />
        </div>
      )}
    </Page>
  )
}

/** Where downloads stand, in the order their groups come: running first, done last. */
const QUEUE_STATUSES: readonly QueueState[] = ["running", "waiting", "stopped", "failed", "done"]

function queueFacts(item: QueueItem): SortFacts {
  const { video } = item
  return { id: item.id, addedAt: item.addedAt, title: video.title, uploader: video.uploader, site: video.siteName }
}

/** The group a download falls in: its folder, its site, or where it stands. */
function queueGroup(item: QueueItem, group: Exclude<ListGroup, "none">): GroupPlace {
  switch (group) {
    case "destination":
      return folderGroup(item.folder)
    case "site":
      return { key: item.video.site, label: item.video.siteName, rank: 0 }
    case "status":
      return { key: item.state, label: strings.queue.groups[item.state], rank: QUEUE_STATUSES.indexOf(item.state) }
  }
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
          {joinDetails([
            item.preset?.name ?? formatContents(item.video),
            stage,
            ...sizeDetails(item, progress),
          ])}
        </p>
        {item.state === "running" && <ProgressBar value={progress?.fraction ?? undefined} label={stage} />}
        {item.state === "failed" && item.error && (
          <StatusMessage tone="danger" {...describeDownloadError(item.error)}>
            {fixFor(item.error) && <FailureFixes error={item.error} />}
          </StatusMessage>
        )}
        {actionError && <StatusMessage tone="danger" {...actionError} />}
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
        {(item.state === "failed" || item.state === "done") && (
          <IconButton label={strings.queue.openLog} icon={FileText} onClick={() => act(openDownloadLog)} />
        )}
        {item.state !== "done" && (
          <IconButton label={strings.queue.cancel} icon={X} onClick={() => act(cancelDownload)} />
        )}
        {item.state === "waiting" && (
          <Menu
            label={strings.queue.moreActions}
            items={[{ label: strings.queue.downloadNext, onSelect: () => act(downloadNext) }]}
          />
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
